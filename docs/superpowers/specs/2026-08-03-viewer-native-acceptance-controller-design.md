# Viewer PID-Bound Native Acceptance Controller Design

> 状态：方案 A 已批准，等待书面规格复核
> 日期：2026-08-03
> 所属计划：`docs/superpowers/plans/2026-08-02-viewer-atlas-to-product-complete-migration.md` Task 15
> 迁移台账：`docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md`

## 1. 目的

Task 15 需要为 89 个正式产品状态分别生成 `1024 × 720` 与 `1440 × 900` 的原生联合视觉证据。当前通用 Computer Use 运行时只能通过应用名称、应用包路径或 bundle identifier 选择 macOS 应用；由 `pnpm start:viewer` 启动的权威开发进程是裸可执行文件，运行时没有 bundle identifier，因而不能被该选择器可靠连接。选择已打包的 `Viewer.app` 会绕过权威裸开发进程，不能作为迁移完成证据。

本设计新增一个只用于本地验收的 PID 绑定原生控制器。它驱动真实 Viewer 开发进程进入台账状态，但不改变产品业务状态模型、不向产品注入测试状态，也不进入发布物。

## 2. 已批准决策

产品负责人批准方案 A：

1. 使用非发布型 macOS 原生控制器连接当前工作树的唯一裸 Viewer 开发进程。
2. 控制器通过真实键盘、指针、右键和拖拽操作进入状态，不通过 React 状态注入、测试路由或隐藏产品命令伪造状态。
3. 文件操作只允许发生在一次性验收夹具副本中。
4. 先以测试驱动方式实现安全门禁和命令协议，再运行 88 个剩余状态的双视口原生验收。

## 3. 范围与非目标

### 3.1 范围

- 精确发现并绑定当前工作树的唯一 `target/debug/viewer-desktop` 进程；
- 验证 PID、可执行路径、进程唯一性、窗口所有权和窗口几何；
- 通过 macOS Accessibility 与 CoreGraphics 发送真实交互；
- 从真实窗口读取可访问性树、控件边界、焦点和窗口状态；
- 在一次性夹具中执行打开、选择、筛选、预览、对比、重命名、整理、废纸篓、冲突和任务等验收动作；
- 捕获只包含权威 Viewer 窗口的原生截图；
- 为台账状态生成可复现的动作记录、原始截图、联合对照图和结果元数据；
- 在失败时安全停止并保留诊断信息。

### 3.2 非目标

- 不修改 `ui/`、`src-tauri/` 或任何发布产品行为来方便验收；
- 不添加测试专用路由、Tauri 命令、localStorage 开关或 React 状态注入；
- 不控制已打包的 `Viewer.app`，也不接受其它工作树或主仓库进程；
- 不把原生控制器、测试夹具或验收依赖包含进应用包；
- 不替代自动化组件测试、可访问性测试或人工联合视觉裁决；
- 不在本阶段开发 Windows 控制器。

## 4. 文件与模块边界

新增文件限定在非发布脚本和规格目录：

```text
scripts/
  viewer-native-acceptance.mjs
  viewer-native-acceptance.test.mjs
  viewer-native-acceptance.swift

docs/superpowers/specs/
  2026-08-03-viewer-native-acceptance-controller-design.md

docs/superpowers/plans/
  2026-08-03-viewer-native-acceptance-controller-plan.md
```

生成物保留在被 Git 忽略的目录：

```text
target/atlas-product-migration-fixture/ViewerAcceptance/   只读基准夹具
$HOME/ViewerAcceptanceRuns/<run-id>/                       一次性可写副本
target/atlas-product-migration-reference/<atlas-sha256>/<viewport>/<ID>/reference.png
target/atlas-product-migration-acceptance/<commit>/<viewport>/<ID>/
```

`package.json` 只允许增加本地验收命令，不允许把脚本接入 Tauri 构建、应用资源或发布依赖。

## 5. 架构

### 5.1 Node 编排器

`viewer-native-acceptance.mjs` 是唯一公开入口，负责：

- 推导当前工作树、权威可执行路径、commit 与输出根目录；
- 读取进程表并执行所有安全前置检查；
- 创建或重置一次性夹具副本；
- 编译并启动 Swift 辅助进程；
- 以逐行 JSON 协议发送最小动作；
- 校验每个响应、超时和窗口边界；
- 记录每个台账 ID 的动作日志与证据清单；
- 任一步失败时立即停止当前状态，不继续执行破坏性动作。

编排器不自行猜测坐标。坐标动作必须来自当前窗口坐标系中的已验证控件边界，或来自对应台账配方中经过窗口边界校验的相对坐标。

### 5.2 Swift PID 辅助进程

`viewer-native-acceptance.swift` 仅提供 macOS 原生能力：

- 用 PID 创建 `AXUIElement`；
- 返回窗口、控件角色、标题、值、标识、边界和焦点；
- 执行可访问性动作；
- 用 CoreGraphics 发送键盘、指针、右键和拖拽事件；
- 按窗口编号捕获当前 Viewer 窗口；
- 对每条命令返回结构化成功或错误响应。

辅助进程不遍历文件系统、不决定夹具路径、不启动或终止 Viewer、不修改系统设置。所有路径和动作授权由 Node 编排器完成。

### 5.3 状态配方

每个台账 ID 使用声明式配方：

```js
{
  id: 'FIL-01',
  viewport: { width: 1024, height: 720 },
  fixtureVariant: 'baseline',
  steps: [
    { action: 'openProject', fixture: 'ViewerAcceptance' },
    { action: 'activate', target: { role: 'button', name: '筛选' } },
    { action: 'waitFor', target: { role: 'dialog', name: '筛选' } },
  ],
  assertion: { visible: [{ role: 'dialog', name: '筛选' }] },
}
```

配方只描述用户可执行动作和可观察结果。它不能调用产品内部函数、写产品状态文件来跳过交互，或把预期视觉值作为运行时样式覆盖。

## 6. 安全门禁

所有门禁必须在第一次 UI 动作前通过，并在每个可能改变文件状态的动作前重新检查相关条件。

### 6.1 进程门禁

- 进程表中必须恰好有一个 Viewer 候选进程；
- 该进程的命令首字段必须精确等于当前工作树的 `target/debug/viewer-desktop`；
- PID 必须存活，且不能等于控制器自身或其辅助进程；
- 进程可执行路径经 `realpath` 后仍必须位于当前工作树；
- 出现打包版、其它工作树 Viewer 或第二个候选进程时立即拒绝运行；
- 启动后 PID 或可执行路径变化时当前验收运行失效，必须重新建立运行。

### 6.2 窗口门禁

- 目标窗口的所有者 PID 必须等于已批准 PID；
- 必须恰好选择一个主 Viewer 窗口；
- 窗口尺寸必须精确等于当前配方的 `1024 × 720` 或 `1440 × 900`；
- 所有相对坐标必须位于目标窗口内容边界；
- 截图前再次验证 PID、窗口编号、窗口尺寸和前台归属；
- 屏幕、Dock 或系统辅助功能设置发生临时变化时，必须记录原值并在运行尾部恢复。

### 6.3 文件门禁

- 允许写入和破坏性文件操作的根目录必须同时满足：精确位于 `$HOME/ViewerAcceptanceRuns/<run-id>/`、包含本次随机 `run-id`、不是符号链接、不是用户主目录、仓库根目录或兄弟运行目录；
- 每次运行从 `target/atlas-product-migration-fixture/ViewerAcceptance/` 只读基准夹具复制到新的浅层运行目录 `<run-id>/<variant>/测试图/`，以保持图谱批准的项目显示名称；状态之间按配方重置，验收结束后只删除精确匹配的本次 `run-id`；基准夹具自身永远不是可写操作目标；
- 源、目标、废纸篓模拟和冲突文件都必须位于该运行目录内；
- 拒绝空路径、`/`、用户主目录、工作树根、父级逃逸、未解析变量、glob 和符号链接逃逸；
- 永不调用系统废纸篓清空或操作用户真实项目。

### 6.4 命令门禁

- 允许命令采用固定白名单，不支持任意 AppleScript、shell、JavaScript 或 Swift 代码执行；
- 每条命令带递增序号、目标 PID、窗口编号和超时；
- 未知命令、额外字段、越界坐标、过长文本和不支持的按键组合必须拒绝；
- 破坏性动作必须同时带台账 ID、夹具根和显式 `destructive: true`，并再次通过文件门禁；
- 任一响应超时、顺序错误或结构无效时终止辅助进程并停止运行。

## 7. JSON 行协议

协议版本固定为 `1`。每行一个 UTF-8 JSON 对象。

请求公共字段：

```json
{"version":1,"sequence":7,"pid":4859,"windowId":194,"command":"inspect","timeoutMs":3000,"payload":{}}
```

响应公共字段：

```json
{"version":1,"sequence":7,"ok":true,"result":{}}
```

错误响应：

```json
{"version":1,"sequence":7,"ok":false,"error":{"code":"WINDOW_MISMATCH","message":"Target window changed"}}
```

白名单命令：

- `inspect`：返回应用与主窗口摘要；
- `query`：按角色、名称、标识和祖先范围查找可访问元素；
- `activate`：对唯一匹配元素执行 AXPress 或等价动作；
- `focus`：聚焦唯一匹配元素；
- `setValue`：只向允许的文本字段设置有限长度文本；
- `key`：发送白名单按键及修饰键；
- `pointer`：点击、双击或右键窗口内坐标；
- `drag`：在窗口内执行有时限的指针拖拽；
- `capture`：捕获已验证窗口到编排器预先批准的证据路径；
- `shutdown`：正常结束辅助进程。

文件拖入 Viewer 的 Finder 交互不通过任意路径型 `drag` 命令完成。编排器只可把批准夹具文件暴露给受控 Finder 窗口，再使用经过验证的屏幕坐标执行真实拖拽；执行前后均校验夹具路径和 Viewer 窗口。

## 8. 数据流与运行生命周期

1. 编排器验证工作树、commit、干净状态和脚本版本。
2. 编排器发现唯一权威 Viewer PID，并验证窗口和规定视口。
3. 编排器创建随机 `run-id` 的一次性夹具副本。
4. 编排器编译 Swift 辅助进程到 `target/native-acceptance-tools/<source-hash>/`。
5. 编排器启动辅助进程并完成协议握手、Accessibility 授权与窗口所有权校验。
6. 对一个台账 ID 重置需要的夹具变体，执行真实状态配方并记录动作日志。
7. 状态断言通过后再次验证窗口，捕获原生产品图并生成元数据。
8. 参考图与原生产品图进入同一个联合对照输入；视觉裁决仍由逐项检查完成，不由像素相似度单独决定。
9. 参考图必须从当前 `viewer-complete-ui-visual-atlas.html` 导出，并按该文件完整 SHA-256、精确视口与状态 ID 寻址；历史产品提交中的参考截图不能替代当前图谱基准。
9. 当前 ID 通过后才进入下一个 ID；失败则保留现场并停止该波次。
10. 运行结束恢复系统设置，关闭辅助进程，保留证据与诊断，删除或保留一次性夹具由非破坏性清理策略决定。

## 9. 错误处理

错误分为四类：

- `PRECONDITION_*`：工作树、commit、进程、窗口、权限或视口不满足；未执行任何 UI 动作；
- `SAFETY_*`：路径、坐标、命令或破坏性动作越界；立即终止运行；
- `STATE_*`：目标控件不存在、不唯一、状态断言失败或等待超时；保存 AX 树摘要和窗口截图后停止当前状态；
- `CAPTURE_*`：窗口身份、尺寸、前台或写入路径在截图前发生变化；当前证据无效且不得写入台账。

失败不会被自动重试掩盖。只允许对明确的可观察条件做有限、基于条件的等待；禁止使用长时间固定 sleep 作为状态正确性的证据。

## 10. 测试设计

实现遵循红—绿—重构。第一批测试必须在实现文件不存在或导出缺失时按预期失败。

### 10.1 Node 单元与集成测试

- 正确解析进程表并只接受一个精确工作树 Viewer；
- 拒绝零个、两个、打包版、其它工作树、错误可执行路径和失效 PID；
- 验证窗口所有者、窗口数量与精确几何；
- 接受安全运行夹具路径，拒绝主目录、仓库根、父级逃逸、符号链接和其它 run-id；
- 拒绝窗口外坐标、未知命令、额外字段、乱序响应和超时；
- 只允许批准截图路径位于当前 commit、viewport 和 ID 目录；
- 辅助进程握手、请求/响应关联、失败停止和清理均可在不操作真实 Viewer 的测试运行时中验证；
- 台账配方 ID 与 89 状态清单一一对应，不允许遗漏、重复或额外 ID。

### 10.2 Swift 协议测试

- 源码可由系统 Swift 编译器以严格警告设置编译；
- 无 Accessibility 权限、错误 PID、错误窗口、无唯一匹配和无效命令返回稳定错误码；
- 测试模式只验证协议解析和坐标边界，不发送真实事件；
- 真实事件冒烟测试只针对当前权威 Viewer 与一次性夹具运行。

### 10.3 原生验收测试

- 每个配方执行后必须有可观察状态断言；
- 每个截图必须有同一运行的 PID、commit、viewport、windowId、动作日志和时间戳；
- `1024 × 720` 与 `1440 × 900` 使用同一业务状态和夹具变体；
- 联合对照检查排版、间距、对齐、圆角、边框、裁切、按钮层级、焦点和启禁用状态；
- 任何 P0、P1 或 P2 差异都先回到正式产品代码修复并重新捕获，不能只在证据或脚本中遮盖。

## 11. 可访问性与平台状态

- 键盘验收使用真实 Tab、Shift+Tab、方向键、Enter、Space 和 Escape；
- 焦点恢复通过 AX 焦点元素与可见状态共同验证；
- Reduce Motion、Increase Contrast、Reduce Transparency 和系统外观由系统级控制开启，记录原值并恢复；
- `forced-colors` 继续由自动化 CSS 合约证明，macOS 原生高对比截图只证明当前平台表面；
- 200% 有效缩放和最小窗口状态必须保持关键控件可达；
- Windows 原生控制与 Windows forced-colors 证据仍属于未来 Windows 阶段。

## 12. 完成条件

控制器本身完成需要：

1. 所有安全门禁和协议测试通过；
2. Swift 辅助进程可编译并通过协议测试；
3. 对当前唯一裸开发进程完成至少一个非破坏性真实状态的原生冒烟运行；
4. 脚本未被 Tauri 构建或应用包包含；
5. 文档明确记录使用方式、权限、恢复和失败语义。

迁移目标完成仍需要：

1. 89 个台账状态全部有当前产品 commit 的自动化证据；
2. 89 个状态全部有 `1024 × 720` 与 `1440 × 900` 原生联合对照路径；
3. 每个状态的 P0/P1/P2 均为零；
4. 可访问性和平台状态完成规定的原生复核；
5. 完整检查、测试、构建、策略、安全与启动器门禁重新运行并通过；
6. 台账与验证文档只把最终当前 commit 作为完成证据；
7. 当前工作树保持唯一最新开发版可运行。

## 13. 自审结论

- 没有 `TBD`、`TODO` 或未定义占位项；
- 控制器与发布产品、业务状态和用户文件边界明确分离；
- 进程、窗口、路径、命令和证据均有拒绝式安全门禁；
- 测试与原生证据不能互相替代；
- 规格不缩减 Task 15 的 89 状态、双视口、逐项视觉裁决和最终门禁范围。
