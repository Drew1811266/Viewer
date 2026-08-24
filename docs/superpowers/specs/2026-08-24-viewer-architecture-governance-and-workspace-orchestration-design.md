# Viewer 架构治理与工作区编排优化设计

> Status: Active
>
> 日期：2026-08-24
>
> 类型：纯架构优化
>
> 约束：不新增产品功能，不改变任何用户可见行为

## 1. 背景

Viewer 当前是一个面向 macOS 13+ Apple Silicon 的本地优先桌面应用，使用 Tauri 2、Rust、React/TypeScript、SQLite FTS5、macOS 原生图片能力以及 libmpv/FFmpeg 媒体运行时。Rust 已形成 Domain、Application、Infrastructure、Platform、Desktop 的模块化单体分层，前端也已经具备 `ViewerState`、reducer、controller 和可注入的 `ViewerBridge`。

当前问题不是产品能力缺失，而是工程治理与编排边界发生漂移：

1. `architecture:health` 同时承担依赖合法性和规模趋势统计，但正式 `pnpm verify` 没有执行该命令。
2. 视频运行时加入后，`viewer-infrastructure -> viewer-video-mpv` 与 `viewer-platform-macos -> viewer-video-mpv` 被旧依赖白名单误判为非法，说明治理模型没有随已确认架构演进。
3. `ui/src/App.tsx` 中的 `ViewerWorkspace` 仍集中承担项目组合、预览、文件组织、工作区外壳、异步反馈和生命周期清理等多类职责。
4. 当前规模和复杂度基线积累了大量趋势告警。它们能提示维护风险，但不能代替对职责、依赖和测试边界的架构判断。

本轮采用“治理优先、渐进迁移”的方式修正这些问题，不进行全栈重写。

## 2. 已确认决策

本设计选择以下路线：

1. 先把不可违反的依赖和外部契约变成正式阻断门禁。
2. 再以 `ViewerWorkspace` 为前端编排试点，按职责建立四个协调器。
3. 保留现有入口、状态权威和兼容 facade，逐段迁移而非一次性替换。
4. 每个阶段都必须能独立合并、完整验证和安全回退。
5. 本轮不因行数、复杂度或告警数量机械拆包。

未选择的路线：

- **全栈集中重构**：回归面过大，无法满足行为冻结和阶段独立合并要求。
- **仅修治理脚本**：可以消除错误告警，但不能解决 `ViewerWorkspace` 的职责集中。

## 3. 目标

- 让 Rust workspace 的合法依赖方向和关键前端边界可被自动、确定性地验证。
- 将结构性阻断规则与维护性趋势报告分开。
- 明确 `viewer-video-mpv` 的低层媒体驱动定位。
- 把 `ViewerWorkspace` 收窄为组合上下文、调用协调器和路由类型化意图的根编排组件。
- 保持 `ViewerState` 为后端项目业务状态的唯一全局前端投影。
- 保留现有异步作用域、错误呈现、项目生命周期和文件事务语义。
- 让每个迁移阶段都具备独立的行为回归证据。

## 4. 非目标与冻结契约

### 4.1 非目标

本轮不包含：

- 新产品功能、视觉调整或交互改版；
- Rust 业务规则、文件操作语义或媒体播放策略调整；
- SQLite、FTS5、`.viewer` 元数据或缓存格式迁移；
- 新状态管理框架、全局事件总线或第二套全局 Context；
- 为通过检查而拆分 `viewer-video-mpv`；
- 对所有超大 Rust 模块进行顺带重构；
- 自动重试、降级、超时或错误文案重新设计；
- 签名、公证、发布和新格式兼容性工作。

### 4.2 冻结契约

以下用户或跨层契约在本轮保持不变：

- Tauri command 名称、参数与返回 DTO；
- Tauri event 名称及 payload；
- capabilities、CSP 和本地资源访问边界；
- SQLite schema、`.viewer` 便携数据 schema 与文件事务日志；
- 快捷键、焦点顺序、ARIA/可访问性语义；
- 用户可见文案、错误位置、反馈时机和操作结果；
- 项目打开、关闭、切换、扫描、浏览、预览、对比和文件组织行为。

内部 TypeScript hook、组件 props、Rust 私有接口和模块位置可以演进，但只有在上述契约保持不变且测试证明没有外部消费者受影响时，才允许收窄或移动。现有 facade 在迁移期间保留。

## 5. Rust 目标依赖模型

生产依赖方向如下：

```text
viewer-domain
    ↑
viewer-application
    ↑
viewer-infrastructure / viewer-platform-macos
    ↑
viewer-desktop（唯一 composition root）

viewer-video-mpv
    └── 低层媒体驱动；只依赖外部库
        可被 infrastructure、platform-macos、desktop 显式使用
```

各模块职责：

- `viewer-domain`：纯类型、值对象、不变量和状态机；不引用平台、存储、Tauri 或媒体实现。
- `viewer-application`：用例、端口和业务编排；只依赖 Domain。
- `viewer-infrastructure`：SQLite、索引、扫描、通用文件操作、缓存和媒体派生适配器。
- `viewer-platform-macos`：Quick Look、Image I/O、FSEvents、Trash、Finder 和 macOS 原生呈现适配器。
- `viewer-video-mpv`：libmpv 进程、渲染与运行时驱动；不承载 Viewer 产品用例。
- `viewer-desktop`：唯一知道具体实现并负责 Tauri、生命周期和对象装配的 composition root。

测试专用依赖不定义生产方向。例如 `viewer-platform-macos` 测试可以通过 dev-dependency 使用 `viewer-infrastructure` 的夹具或测试实现，但不得因此产生同方向的生产依赖。

第一阶段只正式登记媒体驱动层及合法依赖，不拆分 `viewer-video-mpv`。只有未来出现独立发布、独立替换、平台复用或不同生命周期等明确证据时，才重新评估拆包。

## 6. 架构治理设计

现有 `architecture:health` 拆为两个责任不同的入口。

### 6.1 `architecture:boundaries`

这是确定性阻断门禁，并纳入 `pnpm verify`。至少验证：

- Rust workspace 生产依赖符合第 5 节的允许图；
- 生产依赖不存在循环；
- Domain/Application 不引用平台、Tauri、SQLite 或媒体具体实现；
- `viewer-desktop` 仍是唯一具体实现装配点；
- React 组件不直接调用 Tauri，实际调用集中在 API adapter；
- command、event、DTO、capability 和相关安全清单严格匹配本轮冻结基线。

边界检查失败必须返回非零状态，不允许通过刷新趋势基线绕过。若实施发现必须改变任一冻结契约，应停止本轮并为契约变化另行设计和审批，而不是修改冻结基线放行。

### 6.2 `architecture:trends`

这是非阻断趋势报告，继续统计：

- 生产与测试代码比例；
- 超大文件和长函数；
- 决策复杂度；
- 其他能提示职责漂移但不能单独证明架构错误的指标。

趋势报告不得把任意行数阈值当作架构结论。新增异常项必须进入显式分类记录：

- `accepted`：有合理边界、责任人、理由和重新评估触发条件；
- `governance-target`：属于当前或后续治理任务，有明确 owner；
- `test-exception`：只存在于测试或夹具，说明为何不影响生产边界。

禁止无审查地整体刷新基线，禁止通过降低覆盖率要求或扩大忽略范围消除告警。

## 7. 前端目标编排模型

`App` 继续作为 `ViewerBridge` 注入入口，`useViewerController` 继续持有 `ViewerState` 及后端命令。这里的协调器是现有 React 组合内的 TypeScript hook/module，不是新的进程、服务单例、Context 或状态框架。`ViewerWorkspace` 只负责：

1. 建立当前项目上下文；
2. 调用四个协调器；
3. 将协调器输出传给现有视图组件；
4. 路由少量类型化用户意图；
5. 组合最终界面。

### 7.1 Viewing Coordinator

负责图片、文本、视频和对比预览会话，包括预览导航、原图请求、取消、失效、修复和临时视图状态。它只依赖窄 `PreviewPort`，不接收完整 `ViewerBridge`。

### 7.2 Organization Coordinator

负责选择映射、径向菜单、内部拖放、重命名、复制、移动、废纸篓对话框及操作结果。它通过 `useViewerController` 暴露的命令工作，不直接访问 Tauri；文件语义仍属于 Rust/Application。

### 7.3 Workspace Shell Coordinator

负责侧边栏、响应式布局、工具栏、设置、信息面板、偏好和弹层开关。它只拥有界面外壳状态，不承载项目业务状态。

### 7.4 Feedback Coordinator

把扫描、索引、缩略图、文本读取和文件操作状态映射为任务栏、局部错误与全局通知。它只观察和呈现，不启动、重试或取消业务任务。

协调器之间禁止互相直接调用。跨域交互通过根组件路由类型化意图，例如：

- `OpenPreview`
- `EnterCompare`
- `StartRename`
- `ShowOperationResults`
- `OpenSettings`
- `CloseProject`

意图名称描述用户意图，不携带 Tauri command 名称或具体 adapter 类型。

## 8. 数据流与状态所有权

标准数据流为：

```text
用户事件
  → 组件产生类型化意图
  → 对应协调器
  → useViewerController 或窄媒体端口
  → Rust/Tauri
  → ViewerState 或 Viewing 的局部媒体会话投影
  → 纯 selector / view model
  → 组件渲染
```

约束如下：

- `ViewerState` 是后端项目、扫描、搜索和文件操作状态的唯一前端投影；现有媒体事件投影保留在 Viewing Coordinator 的最小会话作用域，不复制为新的全局状态。
- 协调器只保存最小范围的临时交互状态，例如当前对话框、预览请求和临时选择。
- 不复制项目树、查询结果、任务状态或文件操作状态到新的全局 store。
- 展示派生通过纯 selector/view model 计算，避免协调器之间共享可变状态。
- 视图组件只接收渲染数据、窄端口或类型化回调，不导入 Tauri 调用实现。

## 9. 异步、错误与生命周期边界

### 9.1 统一异步作用域

所有异步任务继续受现有 `projectSessionId`、`generation` 和 `requestRevision` 约束。协调器消费统一作用域，不得自行发明平行版本号或生命周期协议。

项目切换、目录切换、预览对象变化或请求被替代后，旧结果即使完成也不得更新当前界面。取消用于节约资源；结果提交前的作用域校验负责正确性。

### 9.2 错误归属

- Rust/Application 产生业务和基础设施错误。
- 发起调用的协调器决定该错误属于局部操作、任务反馈还是全局通知。
- Feedback Coordinator 只展示和聚合已经产生的错误。
- 本轮保留现有错误文案、位置、严重程度和触发时机。

文件操作不采用前端乐观提交。重命名、复制、移动和删除仍以 Rust 事务、恢复日志和最终投影为权威。

### 9.3 项目生命周期

关闭或切换项目遵循：

```text
停止接受新意图
  → 使旧作用域失效
  → 取消可取消请求
  → 释放预览与监听资源
  → 清除临时 UI 状态
```

这只集中清理责任，不改变 watcher、扫描器、数据库或媒体运行时的外部行为。本轮不新增重试、降级、超时或错误吞噬策略。

## 10. 分阶段迁移

### 阶段 A：行为冻结与边界特征测试

- 固定关键 IPC、事件、DTO、安全和可访问性契约。
- 为项目切换、过期异步结果、预览和文件操作补齐必要特征测试。
- 记录当前用户可见行为，不先移动实现。

### 阶段 B：治理拆分

- 建立 `architecture:boundaries` 和 `architecture:trends`。
- 登记 `viewer-video-mpv` 的合法驱动依赖。
- 将边界检查加入正式 `verify`，趋势报告保留在质量报告入口。
- 对现有趋势异常逐项分类，不自动刷新基线。

### 阶段 C：协调器基础

- 引入类型化意图、窄端口、纯 selector 和 view model。
- 保留 `App`、`ViewerWorkspace`、`ViewerBridge` 和现有 facade 入口。
- 此阶段只建立可测试边界，不迁移高风险异步行为。

### 阶段 D：按风险迁移协调器

迁移顺序固定为：

```text
Workspace Shell → Feedback → Organization → Viewing
```

每次只迁移一个职责域。迁移完成并通过验证后，立即删除 `ViewerWorkspace` 中对应的重复职责，不长期维护两套实现。Viewing 最后迁移，因为它承担取消、过期结果、媒体资源和项目生命周期风险。

### 阶段 E：收窄根组件和兼容面

- `ViewerWorkspace` 只保留组合、协调器调用和意图路由。
- 仅在测试和静态引用检查证明没有消费者后，收窄过宽的内部 facade。
- 不以目标行数作为完成标准。

各阶段必须独立提交、独立通过完整验证。任一阶段失败时，可以回退该阶段而不撤销之前已验证阶段。

## 11. 测试与验证策略

### 11.1 自动化层次

- **纯函数测试**：意图、selector、view model 和作用域判定。
- **协调器契约测试**：验证命令调用、窄端口、取消、过期结果和清理责任。
- **根组件集成测试**：验证 `ViewerWorkspace` 的组合、路由及现有视图行为。
- **Bridge 契约测试**：冻结 command、event、DTO 和 adapter 行为。
- **Rust 回归测试**：现有 Domain、Application、Infrastructure、Platform、Desktop 测试全部保持通过。
- **治理测试**：依赖合法图、循环检测、UI 导入边界、基线格式和命令退出状态。

每个阶段至少运行相关定向测试、UI/Rust 覆盖率门禁和 `pnpm verify:clean`。不得增加被跳过或忽略的测试，不得降低现有 UI/Rust 覆盖率基线。

### 11.2 行为回归矩阵

自动化和现有验收工具应覆盖：

- 项目打开、关闭、切换及关闭阻塞；
- 目录浏览、搜索、扫描、索引与渐进加载；
- 图片、文本、视频预览和对比；
- 快捷键、选择、径向菜单和内部拖放；
- 重命名、复制、移动、废纸篓、撤销与操作结果；
- 设置、偏好、响应式侧边栏和弹层；
- 缩略图、文本、媒体和文件操作错误反馈；
- 项目切换后的旧异步结果失效和资源清理。

测试断言应以行为和边界为中心，不把组件内部结构或行数写成脆弱快照。

## 12. 验收标准

本轮完成必须同时满足：

1. 用户可见行为和持久化格式没有变化。
2. Tauri IPC、事件、DTO、capability、快捷键和可访问性契约没有变化。
3. Rust 生产依赖符合目标图，架构边界检查进入正式质量门并通过。
4. `viewer-video-mpv` 被明确归类为低层驱动，没有为消除告警而机械拆分。
5. `ViewerWorkspace` 只负责根组合和类型化意图路由。
6. 四个协调器职责互斥、依赖明确，可以独立测试。
7. `ViewerState` 仍是后端项目业务状态的唯一全局投影，媒体状态只存在于 Viewing 的局部会话投影；没有新增全局 store 或事件总线。
8. React 视图组件不直接调用 Tauri。
9. 不增加忽略测试，不降低覆盖率，不通过刷新基线掩盖问题。
10. `pnpm verify:clean` 在干净工作树验证条件下通过。

## 13. 停止与回退条件

出现以下任一情况时，停止当前阶段并重新评审，不继续扩大修改面：

- 为完成拆分必须改变冻结契约；
- 无法证明旧异步结果在项目或预览切换后失效；
- 同一业务状态在 `ViewerState` 和新协调器中出现两个可写副本；
- 需要新增全局状态框架或跨协调器直接调用才能继续；
- 相关测试通过但完整验证失败；
- 只能通过放宽规则、降低覆盖率或扩大忽略清单才能合并；
- 当前阶段无法单独回退。

回退以阶段提交为单位。兼容 facade 使旧调用面在迁移过程中持续可用，但不得成为永久的第二套编排实现。

## 14. 实施计划边界

后续实施计划必须把本设计拆成小型、顺序明确的任务，并为每个任务标明：

- 要固定或新增的测试；
- 要移动的单一职责；
- 允许修改的文件范围；
- 相关定向验证和完整验证命令；
- 该任务的回退点。

实施计划不得加入本设计非目标中的产品或发布工作，也不得顺带重构未进入当前迁移路径的 Rust 大模块。
