# Viewer 工程优化与持续治理设计

> Status: Active

> 状态：已批准
>
> 日期：2026-07-23
>
> 范围：仅覆盖 2026-07-23 架构评审发现的工程问题
>
> 决策：取消独立 M4 里程碑；Viewer 0.1 仅表示持续开发中的小型阶段性基线

## 1. 背景

Viewer 当前采用 Rust、Tauri、React、TypeScript、SQLite 和 macOS 原生适配器。仓库已经具备清晰的 Domain/Application/Infrastructure/Platform/Desktop 分层、较完整的自动化测试、安全边界、依赖锁定和架构决策记录。

架构评审确认当前主干健康，但发现以下工程风险：

1. 多个 Rust、Tauri 和 React 文件已经超过 1,000～2,700 行，职责开始集中。
2. TypeScript 尚未启用严格模式，前端没有统一 lint、format 和根级编辑器规范。
3. 质量、安全、依赖和网络型审计分散在不同脚本中，CI 责任边界不够清晰。
4. 仓库缺少顶层开发入口文档，并会遗留嵌套 `.DS_Store` 和测试生成的 `.viewer` 数据。
5. 部分仓库策略通过完整文本匹配冻结 CI，实现过于脆弱。
6. 活跃文档、路线图、历史 review 和需求矩阵对 M4 的定位不一致。
7. 依赖策略通过但存在重复版本、未使用许可证白名单和长期例外。
8. 测试数量充足，但缺少覆盖率、复杂度、文件规模和依赖健康度的趋势基线。

本设计采用渐进式工程治理，不进行集中式架构重写。

## 2. 目标

本方案必须实现以下目标：

- 统一开发入口、代码规范和持续验证命令。
- 在不改变用户行为的前提下拆分超大模块。
- 将质量责任变成日常持续门禁，而不是集中到版本末尾。
- 让文档清楚区分活跃规范、已取代决策和历史证据。
- 为覆盖率、复杂度、依赖例外和文件规模建立可追踪基线。
- 保持主干在每个阶段都可构建、可测试、可继续开发。

## 3. 非目标

本方案不包含：

- 新产品功能或现有产品行为调整。
- UI 视觉重设计。
- Windows 实现或验收。
- 公共发布、商店、自动更新或云服务。
- `.app`/DMG 交付、Developer ID、签名或公证。
- 真实用户素材的最终验收。
- 两小时稳定性测试或任何等价的集中式发布验收。
- 为减少行数而进行无边界价值的机械拆分。
- 为消除传递依赖告警而强制覆盖 Tauri 依赖版本。

## 4. Viewer 0.1 与 M4 决策

### 4.1 Viewer 0.1 的新含义

Viewer 0.1 是持续开发中的小型阶段性技术基线，不代表：

- 功能完成；
- 内部发布；
- 正式交付；
- 架构永久冻结；
- 进入签名、公证或安装验证；
- 已完成所有真实素材、人工交互或长时间运行验证。

0.1 之后允许继续开发大量新功能和工程优化。已有安全与数据一致性约束仍然有效，但不再由版本交付语义承载。

### 4.2 取消独立 M4

活跃路线图中删除 M4 里程碑、M4 计划入口、M4 owner、M4 前置条件和“最终验收”表述。

原 M4 内容按性质重新归类：

| 原内容 | 新归属 |
| --- | --- |
| 自动化安全边界 | 每次变更的确定性 CI |
| Rust/npm 许可证与依赖策略 | CI 和定时依赖治理 |
| 合成性能与内存回归 | 对应模块的持续回归检查 |
| 可访问性自动检查 | 前端质量门禁或对应功能完成定义 |
| 真实图片素材体验 | 可选专项验证，不阻塞阶段完成 |
| Finder 跨应用拖放 | 可选人工专项验证 |
| VoiceOver 完整旅程 | 可选人工专项验证 |
| 长时间运行和 RSS 观察 | 可选稳定性专项验证 |
| `.app`/DMG、签名、公证、干净机器安装 | 当前开发阶段不适用 |

不得创建一个仅改名但职责等同 M4 的“最终优化验收”阶段。

### 4.3 历史文档处理

G1～G4、M1～M3 的 review、旧计划和已有证据属于历史事实，不重写其原始结论。涉及 M4 的历史文档统一增加 `Historical/Superseded` 提示，并链接新的治理 ADR。

活跃规范必须使用当前术语，不得继续把 M4 作为 owner、完成条件或下一阶段。

## 5. 方案选择

### 5.1 已选择：渐进式工程治理

顺序为：

1. 阶段 A：工程基线统一；
2. 阶段 B：模块解耦；
3. 阶段 C：持续质量治理。

该方案优先建立保护网，再拆分模块，最后引入趋势治理。每个阶段均可独立验证和回滚。

### 5.2 未选择：集中式架构重构

一次性拆分全部 Rust、Tauri 和 React 模块会扩大回归面，并阻塞后续功能开发，不适合当前持续开发状态。

### 5.3 未选择：只补门禁

只增加规范和 CI 无法缓解已经出现的模块膨胀，不能解决主要维护性风险。

## 6. 阶段 A：工程基线统一

### 6.1 开发入口

新增顶层 `README.md`，至少包含：

- 项目当前定位和 0.1 的阶段性含义；
- 技术栈与支持平台；
- 目录结构和分层导航；
- Node、pnpm、Rust、macOS 工具链要求；
- 安装、开发、测试、构建和验证命令；
- CI 与本地门禁矩阵；
- 活跃架构文档入口。

新增 `CONTRIBUTING.md`，至少包含：

- 分支与提交约定；
- 测试与验证要求；
- 依赖变更流程；
- ADR 和设计文档更新条件；
- 大模块新增职责前的边界检查；
- 不提交生成物和项目私有元数据的要求。

新增根级 `.editorconfig`，统一换行、编码、尾部空格和文件结尾。

### 6.2 仓库卫生

- 根 `.gitignore` 使用递归规则忽略任意层级 `.DS_Store`。
- 不全局忽略 `.viewer`，因为它可能是合法测试夹具或产品元数据。
- 会写入 `.viewer` 的自动化测试必须使用临时项目目录。
- 生成固定图片夹具的脚本只允许更新已声明的图片和 manifest。
- 验证命令结束后不得新增未跟踪生成物。

### 6.3 TypeScript 基线

采用分批严格化：

1. 启用 lint 和 format，但先保持行为不变；
2. 修复当前源码中的类型问题；
3. 启用 `strict: true`；
4. 评估并启用 `noUncheckedIndexedAccess`；
5. 将 strict、lint 和 format 纳入根级验证命令。

工具选择必须满足：

- 支持 React 19 和 TypeScript 6；
- 可在本地和 CI 使用同一命令；
- 配置数量最少；
- 不与 TypeScript 编译职责重叠。

ESLint + Prettier 与 Biome 二选一，不同时引入两套格式化器。实施计划根据现有代码兼容性选择具体工具。

### 6.4 CI 职责

确定性 CI 分为两个逻辑组：

#### Quality

- 仓库策略；
- TypeScript strict；
- lint 和 format；
- UI 测试；
- UI 生产构建；
- Rust fmt；
- Rust Clippy；
- Rust workspace 测试；
- 工作树清洁检查。

#### Security

- Tauri capability 与 CSP；
- 路径、IPC、Markdown 和图片协议安全测试；
- cargo-deny advisories、licenses、sources；
- npm 许可证清单。

网络型 npm 漏洞审计改为定时任务或显式依赖检查任务。外部网络不可用不得造成普通功能提交随机失败，但定时审计失败必须可见并被跟踪。

### 6.5 仓库策略重构

`repository-policy.test.mjs` 不再复制完整 CI YAML 或完整长命令字符串。策略检查改为验证关键行为，例如：

- CI 在 push 和 pull request 上运行；
- 权限保持只读；
- runner、Node、pnpm 和 Rust 版本明确；
- lockfile 使用 frozen/locked 模式；
- quality 和 security 必需步骤存在；
- Action 固定到不可变提交；
- Tauri capability 不扩大；
- 直接依赖清单与 notices 一致。

策略测试验证“不变量”，而不是验证无关空格、步骤名称或完整文本。

### 6.6 文档迁移

阶段 A 同步更新：

- `docs/PRODUCT_SPEC.md`；
- `docs/TECHNICAL_FOUNDATIONS.md`；
- `docs/superpowers/plans/2026-07-16-viewer-0.1-roadmap.md`；
- `docs/milestones/viewer-0.1-scope-matrix.md`；
- `docs/architecture/viewer-0.1-api-baseline.md`；
- 受 M4 影响的活跃 specs 和 plans；
- `scripts/check-scope-coverage.mjs`；
- 新的持续治理 ADR。

文档迁移要求：

- 活跃路线图不存在 M4；
- 0.1 不再绑定发布或交付；
- 合成基准只描述已测事实，不宣称真实素材最终结论；
- 仍适用的质量要求归属 `Continuous`；
- 未开发产品功能不纳入本优化方案；
- 纯发布要求标记为当前阶段不适用；
- 历史文档有清晰状态提示。

## 7. 阶段 B：模块解耦

### 7.1 通用拆分规则

每个拆分单元遵循：

1. 先确认或补充特征测试；
2. 只移动一个职责域；
3. 保持外部接口兼容；
4. 运行相关测试；
5. 运行完整验证；
6. 单独提交；
7. 不在同一提交中修改业务语义。

不以绝对行数作为唯一验收标准。模块必须能回答：

- 它负责什么；
- 调用者如何使用；
- 它依赖哪些抽象；
- 内部实现能否在不影响调用者的情况下替换。

### 7.2 Rust Infrastructure

#### `operation/copy.rs`

按职责提取：

- 文件引用和父目录身份绑定；
- 临时文件租约及清理义务；
- 可取消复制流；
- 内容哈希和稳定证据；
- no-replace 原子落位；
- 平台差异与错误映射。

`LocalFileMutation` 保留为面向 application port 的组合适配器，不继续承载所有底层实现。

#### `operation/service.rs`

按职责提取：

- 命令预检；
- 批次注册和生命周期；
- 目标与冲突解析；
- 单项执行路由；
- 结果代码分类；
- journal 与 projection commit 协调。

`LocalFileCommandAdapter` 保留为 application-facing facade。

#### `search/index.rs`

按 schema、写入事务、查询、projection 更新和迁移边界拆分。FTS 查询与普通节点存储不得继续共享一个不透明实现块。

### 7.3 Tauri Desktop

#### `state.rs`

`DesktopRuntime` 降级为组合根和会话协调器。提取：

- project session lifecycle；
- scan/index orchestration；
- preview/image service；
- marker service；
- organization/operation service；
- derived indexing publication。

提取后不得把 Tauri 类型泄露到 Domain/Application。

#### `dto.rs`

按 IPC 领域拆分：

- project/lifecycle；
- browse；
- search；
- preview；
- marker；
- operation。

共享安全转换和错误脱敏保留在小型公共模块。

#### `operation_runtime.rs`

按运行记录、进度发布、提交适配和撤销适配拆分。运行状态机必须保持单一所有者，避免多个模块分别决定终态。

### 7.4 React

#### `useViewerController`

拆分为：

- `useProjectSessionController`；
- `useSearchController`；
- `useSelectionMarkerController`；
- `useOperationController`；
- `useLifecycleSubscriptions`。

根 controller 只组合子控制器并返回稳定 facade。现有组件不需要一次性迁移到新接口。

#### `App.tsx`

提取：

- application shell；
- workspace routing；
- preview/compare session；
- operation dialog coordinator；
- radial-menu session；
- task feedback projection。

`App` 最终只处理顶层组合、页面路由和跨域协调。

#### `viewerReducer`

按 project、workspace、search 和 operation 拆分 reducer。跨域 action 由根 reducer 按固定顺序协调，不允许子 reducer 直接修改其他域内部状态。

#### `ContentBrowser`

提取选择模型、虚拟内容布局、拖动交互和 cell presentation。虚拟化坐标与选择语义保持独立，以继续支持跨屏框选。

### 7.5 公共契约约束

阶段 B 默认不得修改：

- IPC command 名称；
- IPC event 名称；
- DTO JSON 字段与 casing；
- Domain/Application 公共类型语义；
- operation journal 状态和迁移；
- `.viewer` schema 与迁移；
- 错误代码和用户安全消息的既有契约。

若拆分确实需要不兼容修改，必须单独提交 ADR 和迁移计划，不得隐含在重构提交中。

## 8. 阶段 C：持续质量治理

### 8.1 覆盖率

- Vitest 生成前端覆盖率报告。
- Rust 使用 `cargo llvm-cov` 生成 workspace 覆盖率报告。
- 首次只记录基线，不立即设置全仓阻断阈值。
- 对路径校验、文件事务、恢复、权限边界和 reducer 等核心模块设置局部门槛。
- 阈值只允许基于实际基线提高，不以追求百分比替代有效测试。

### 8.2 复杂度和规模

生成以下趋势：

- 生产文件行数；
- 函数长度；
- 圈复杂度或等价复杂度指标；
- 模块依赖方向；
- 测试与生产代码比例；
- 超大文件和超大函数新增情况。

初期只告警。连续两个开发阶段有稳定基线后，才决定哪些指标阻断 CI。

### 8.3 依赖治理

每个 cargo-deny 例外记录：

- advisory；
- 原因；
- 上游依赖链；
- 负责人；
- 创建日期；
- 下次复查日期；
- 可移除条件。

重复版本先分类为：

- 可由直接依赖升级消除；
- Tauri 或平台链固定；
- 仅构建期；
- 对产物大小或安全有实际影响。

只有有明确收益且不会破坏上游兼容时才执行去重。未遇到的 `ISC` 白名单应删除或记录明确保留理由。

### 8.4 文档治理

新增文档索引，并要求规范文件具有以下状态之一：

- `Active`：当前事实来源；
- `Superseded`：已被新决策取代；
- `Historical`：保留实施或验收证据，不再指导当前工作。

每个 Superseded 文档必须链接替代文档。每个 Active 领域只能有一个明确入口。

## 9. 验证策略

### 9.1 每次合并必须通过

- repository policy；
- TypeScript strict、lint、format；
- UI tests 和 production build；
- Rust fmt、Clippy、workspace tests；
- Tauri/security boundary；
- cargo-deny；
- npm license inventory；
- IPC fixture/shape checks；
- operation recovery 和关键安全测试；
- 工作树清洁检查。

### 9.2 合成回归

现有确定性图片、扫描、搜索和文件事务夹具继续用于回归。合成回归只回答“是否相对已知基线明显退化”，不被描述为真实素材验收或版本发布证据。

### 9.3 可选专项验证

以下验证可以按功能需要执行并记录，但不阻塞工程优化阶段：

- 真实图片文件夹体验；
- Finder 实际拖放；
- VoiceOver；
- 长时间运行；
- 内存压力；
- 签名、公证或安装。

## 10. 错误处理与回滚

- strict 迁移按目录或模块推进，避免一次性全仓静默断言。
- lint/format 与逻辑重构分开提交，减少 review 噪音。
- 模块提取出现行为差异时回滚该提取，不回滚已经稳定的阶段 A 基线。
- 网络型审计失败不阻断普通提交，但必须创建可见的治理记录。
- 覆盖率工具不可用时不伪造结果；报告任务失败并保留已有确定性门禁。
- 文档迁移必须先更新 source of truth，再更新派生矩阵和校验脚本。
- 任何安全或数据一致性测试回归都阻断合并。

## 11. 完成标准

### 11.1 阶段 A

- 新开发者能仅通过 README 完成环境准备和完整验证。
- TypeScript strict、lint 和 format 通过。
- quality/security CI 职责清晰。
- 仓库策略不再逐字冻结完整 CI 文本。
- 活跃路线图不存在 M4 或 Viewer 0.1 发布交付目标。
- 自动化验证后 `git status` 不新增生成物。

### 11.2 阶段 B

- 已识别超大模块按本设计职责拆分。
- IPC、Domain/Application 和持久化契约保持兼容。
- 所有现有自动化测试通过。
- 合成性能基线没有无法解释的明显回退。
- 新模块可独立测试和替换，不要求理解整个运行时。

### 11.3 阶段 C

- 覆盖率、复杂度、规模和依赖健康度具有可追踪基线。
- 安全例外均有原因、负责人和复查日期。
- 文档均具有明确状态和唯一活跃入口。
- 不存在名为 M4 或职责等价的集中式最终验收阶段。

## 12. 实施顺序

严格按以下顺序执行：

1. 文档语义和 M4 决策落地；
2. README、CONTRIBUTING、EditorConfig 和仓库卫生；
3. TypeScript lint/format/strict；
4. CI 与 repository policy 重构；
5. React controller 和 App 拆分；
6. Tauri state、DTO 和 operation runtime 拆分；
7. Infrastructure operation 和 search 拆分；
8. 覆盖率、复杂度和依赖治理；
9. 文档状态索引与最终一致性检查。

每一步都必须先通过相关测试，再运行完整验证。不得把多个高风险模块拆分合并为一个不可独立审查的提交。
