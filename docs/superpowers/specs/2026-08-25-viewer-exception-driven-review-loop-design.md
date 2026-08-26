# Viewer 例外驱动基础评审闭环设计

> Status: Implemented — 2026-08-26 已完成实现与验证
>
> Date: 2026-08-25
>
> Product phase: AI 素材评审路线阶段 2

## 1. 背景

Viewer 当前已经具备批量图片、视频浏览和预览能力。阶段 1 建立了评审领域对象、
Review Stream、Draft/Completed 状态机、项目内开放协议、单写入者 Repository、不可变完成
历史以及 Agent 无关参考读取器，但没有把这些能力接入当前项目会话和用户界面。

本阶段在不推翻现有浏览、预览、搜索、对比、标记和文件整理架构的前提下，实现第一条
真正可用的例外驱动评审闭环：用户冻结一批图片和视频，只给需要返工的素材留下自然语言
意见，主动完成本轮后，其余可评审素材才得到 `pass`。

本设计继承并收窄以下文档的边界：

- [AI 素材评审闭环产品方向与架构设计](2026-08-25-viewer-ai-material-review-workflow-design.md)
- [Viewer 开放评审协议](../../protocol/README.md)
- [Viewer 技术框架与开源来源](../../TECHNICAL_FOUNDATIONS.md)

实现结果：阶段 2 已接入当前项目会话和现有工作区。用户可以从选择或当前浏览范围固定
素材、持久化单目标／多目标自然语言 Feedback、关闭重开后明确恢复 Draft、在冲突核验后
发布不可变 Completed Round，并由既有 Agent 无关读取器读取精确手动 Stream。浏览、预览、
搜索、对比、日常标记、收藏和文件整理继续沿用原有架构与语义。

## 2. 已确认的设计决策

1. 同一 Project 同时只允许一个未完成 Draft。
2. 重新打开项目时发现唯一 Draft，Viewer 提示用户继续；不会自动创建新一轮。
3. 有选中素材时只冻结选中的图片和视频；没有选中素材时冻结当前浏览范围内的全部图片
   和视频，并严格跟随“显示后代文件”状态。
4. 搜索结果不提供“把全部搜索结果加入本轮”；用户仍可选择明确的搜索结果后创建本轮。
5. 现有“保留／待定／淘汰”和收藏属于独立的日常整理语义，不参与 Review Outcome 计算。
6. `unreviewable` 只表示 Viewer 已确认的技术失败，不要求用户选择结构化原因。
7. 一次显式提交产生一条 Feedback；一个 Feedback 可以绑定一个或多个素材，同一素材也可
   拥有多条 Feedback。
8. 每个 Project 在本阶段至多有一个无 Production Scope 的手动 Review Stream；后续手动
   Round 追加到该 Stream。
9. 完成本轮前必须显示 `revise`、`unreviewable` 和将默认 `pass` 的数量及意见汇总；不显示
   或推断“浏览数量”。
10. 素材被替换、移动、删除或内容身份发生变化时阻止完成本轮；本阶段不自动重绑。
11. 放弃 Draft 需要二次确认；持久删除成功后才退出评审，不生成任何 Completed 历史。
12. 评审界面采用现有工作区上的上下文式右侧面板，不建立第二套浏览、预览或视频系统。
13. 核心编排由 `viewer-application` 的 `ReviewSessionService` 负责，Tauri 和 React 不复制领域
    规则或直接写协议文件。

## 3. 目标与非目标

### 3.1 目标

- 从当前选择或当前浏览范围创建固定素材名单。
- 支持图片、视频和不可预览图片候选进入本轮。
- 支持单目标、多目标自然语言 Feedback 的添加、编辑和删除。
- 把每一次已提交变更安全保存到项目内 Draft，并在重启后恢复。
- 自动识别稳定的技术失败并产生 `unreviewable`。
- 在明确确认后原子发布不可变 Completed Round。
- 让现有 Agent 无关读取器直接读取本阶段产生的结果。
- 保持现有项目浏览、预览、搜索、对比、标记、收藏和文件整理行为不变。
- 为未来手动入口、CLI、MCP、本地 API 和 Agent 插件保留同一个应用用例边界。

### 3.2 非目标

- 读取或执行 `viewer-production.json`；
- 建立返工版本、父版本或 Revision Relation；
- 原图与返工图的对照复审；
- 图片区域、视频时间点和视频时间段标注；
- “全部搜索结果”或隐式动态查询作为固定名单来源；
- CLI、URL Scheme、MCP、本地 API 或具体 Agent 插件；
- 自动启动 Agent、模型、生成任务或返工任务；
- 账户、云同步、在线协作或服务器；
- 代码签名、Apple 公证、正式安装包、公开发布、发售或销售任务。

Viewer 仍处于开发初期。最后一项不是本阶段的完成条件，也不得作为普通后续功能阶段的默认
收尾事项；只有项目负责人明确进入对应阶段并单独授权后才讨论。

## 4. 用户工作流

### 4.1 创建本轮

工作区工具栏直接提供“开始评审”，不把未来核心任务隐藏在“更多”菜单中。

```text
用户选择素材，或保持无选择并停留在当前浏览范围
  → 点击“开始评审”
  → Viewer 在后端重新解析明确范围
  → 显示图片数、视频数和被排除的其他文件数
  → 用户确认固定名单
  → Viewer 取得写入租约并在租约内重新检查唯一 Draft
  → Viewer 准备内容证据并原子保存 Draft
  → 保存成功后进入评审状态
```

范围规则如下：

- 选择非空时，候选集合只来自明确选中的 Entity ID；文本、目录和其他文件被排除并计数。
- 选择为空时，候选集合来自当前 Folder Projection；是否包含后代与当前聚合开关一致。
- JPEG、PNG、`UnsupportedImage` 和 Video 属于评审候选；Markdown、TXT 和 Other 仅作上下文。
- 候选为空时不创建 Draft，并返回可操作错误。
- 前端只提交来源描述和 Entity ID，不提交自己构造的路径、媒体边界或文件证据。

### 4.2 评审中

评审状态是当前工作区上的一层上下文：

- 顶部 Context Bar 始终显示固定素材总数、Feedback 数和 `unreviewable` 数；
- 右侧 Review Inspector 按需展开，用于编写和查看意见；
- 现有批量网格仍是正式评审入口；图片预览、视频播放和多图对比按需复用；
- 用户可以浏览项目内其他上下文，但非本轮素材不能成为该轮 Feedback 目标；
- Context Bar 提供“返回本轮素材”；
- 本轮素材上的意见数量使用文字、图标和可访问名称表达，不只依赖颜色；
- 浏览曝光、滚动位置、停留时间和大图打开次数都不记录为评审完成证据。

### 4.3 Feedback

用户先选择一个或多个本轮素材，再输入自然语言：

```text
选择目标
  → 输入自然语言
  → 点击“添加意见”或按 ⌘ Enter
  → Application 校验并持久化
  → 成功后界面显示新的已保存快照
```

- 每次提交创建一条独立 Feedback ID。
- 提交时冻结该条 Feedback 的目标列表；之后改变工作区选择不会修改旧目标。
- 已提交 Feedback 可以编辑或删除，每次操作都必须先保存成功再替换应用快照。
- 空白输入不进入协议，也不产生 `revise`。
- 阶段 2 的 Anchor 只使用 `asset`；区域和时间 Anchor 继续保留在 Domain/协议中但不暴露编辑
  入口。
- React 中尚未提交的输入不是协议 Draft；关闭或切换上下文前需要阻止意外丢失非空输入。

### 4.4 完成本轮

“完成本轮”先打开只读汇总：

- 固定素材总数；
- 有 Feedback、将成为 `revise` 的素材数；
- 已确认技术失败、将成为 `unreviewable` 的素材数；
- 没有 Feedback 且可评审、将在确认时成为 `pass` 的素材数；
- 全部自然语言意见及其目标数量；
- 已发现的版本冲突。

存在版本冲突时禁止确认完成。没有冲突时，用户明确确认后才调用 Domain 的 `complete` 并发布
Completed Round。发布成功后本轮进入只读状态；不提供 Completed → Draft 的回退。

### 4.5 恢复与放弃

- 打开项目时只做无副作用探测；没有 Draft 时保持普通浏览。
- 恰好一个 Draft 时提示继续，取得写入租约并完成验证后进入评审状态。
- 取得写入租约后必须重新读取 Draft 和 Stream 状态，避免探测与写入之间的竞态。
- 多个 Draft 违反本阶段不变量，进入 `RecoveryRequired`，不得猜测选择最新文件。
- 放弃需要二次确认，删除失败时仍保持 Draft 活跃。
- 完成结果可以只读查看；更正通过新的 Round 表达，不覆盖旧文件。

## 5. 架构

### 5.1 依赖方向

```text
React Review UI
  → typed ViewerBridge review commands
    → Tauri review adapter
      → ReviewSessionService (viewer-application)
        → Review Domain
        → ReviewAssetCatalogPort
        → ReviewRepositoryPort / repository lifecycle port
        → ClockPort

Infrastructure implements application ports:
  ProjectReviewRepository
  IndexedReviewAssetCatalog
```

依赖方向继续遵守当前生产架构：Domain 不依赖上层，Application 只依赖 Domain，Infrastructure
和 Tauri 作为外层 Adapter。`viewer-desktop` 仍是唯一生产组合根。

### 5.2 ReviewSessionService

`ReviewSessionService` 是一个 Project Session 内唯一的评审写入协调者，负责：

- 探测并恢复唯一 Draft；
- 解析手动 Stream；
- 根据明确范围创建固定 Asset Version；
- 添加、编辑和删除 Feedback；
- 构造完成前汇总；
- 完成前重新验证全部素材；
- 调用 Domain 计算 Outcome；
- 发布 Completed Round；
- 处理不确定发布后的 Repository 重开与线性恢复；
- 删除被明确放弃的 Draft；
- 在项目关闭时确定性释放写入租约。

它不负责文件系统 JSON、Tauri DTO、React 布局、图片解码或视频播放。

Service 只在 `Preparing`、`Active` 和 `Completing` 需要持有写入租约。创建被取消、Draft 被
成功放弃或 Completed head 被核对成功后立即释放租约；只读查看 Completed 历史不占用写入
租约。

### 5.3 应用状态

应用层使用明确状态而不是散落布尔值：

```text
Idle
Preparing
Active
Completing
CompletedReadOnly
WriteUnavailable
RecoveryRequired
```

`Preparing` 和 `Completing` 可以报告进度并接受限定取消；进入 `Active` 或
`CompletedReadOnly` 必须有已经验证的持久事实。`WriteUnavailable` 不影响普通项目浏览。

每个 Active Session 公开一个仅在当前应用会话内有效的单调 `revision`。所有变更命令携带
预期 Round ID 与 revision；Service 在互斥区中串行执行命令，拒绝过期 revision。Repository
文件格式不需要增加这个易失字段。

### 5.4 ReviewSessionSnapshot

UI 只消费应用投影，不直接消费协议 DTO。最小投影包含：

- session state、Round ID、Stream ID 和 revision；
- 固定成员及其 Asset Version ID、Entity ID 映射和安全展示信息；
- 已保存 Feedback；
- 自动 `unreviewable` 状态；
- 版本冲突；
- 汇总计数；
- 保存、准备、完成和恢复状态；
- 对用户安全的错误码和受影响相对路径。

绝对路径、锁文件细节、原始 I/O 错误和协议内部 DTO 不进入前端。

## 6. 应用端口

### 6.1 ReviewAssetCatalogPort

该端口从当前项目会话的已验证索引读取素材事实，至少提供：

- 根据明确 Entity ID 集合解析候选；
- 根据当前 Folder Projection 解析候选；
- 读取相对路径、FileKind、大小、修改时间和稳定文件身份；
- 读取图片显示尺寸或视频时长／显示尺寸；
- 读取已确认的图片、视频技术失败；
- 为恢复和完成重新验证 Asset Version；
- 报告缺失、身份改变、路径改变和内容证据不匹配。

Adapter 必须重新读取索引和文件证据，不能信任创建弹窗中的旧 BrowserFile DTO。

“已确认技术失败”必须映射为协议支持的稳定类别。缩略图或视频封面失败本身不足以把素材
判定为 `unreviewable`；批量网格能够正常呈现的图片仍可评审，未实际打开的视频也不能因此
被判定失败。完成时遇到仍为 Pending／Unknown 的媒体状态，Application 必须先触发有界验证；
验证仍无法得出稳定结果时阻止完成，不能默认 `pass`。

### 6.2 Review Repository 扩展

阶段 1 的 `ReviewRepositoryPort` 增加阶段 2 所需、仍由 Infrastructure 实现的能力：

- 无副作用探测 Review Repository；
- 发现零个或一个 Active Draft；
- 多 Draft 时返回 `RecoveryRequired`；
- 删除明确放弃的 Draft；
- 在需要写入时取得并持有单写入者租约；
- 不确定发布后重开并执行既有线性孤儿恢复；
- 显式释放写入会话。

探测和写入生命周期可以由一个独立的 Application-owned provider port 表达，避免把项目根路径
或 Infrastructure 类型泄漏给 `ReviewSessionService`。

### 6.3 ClockPort

复用现有 `ClockPort` 产生 Feedback、Round 创建和完成时间。Review、Round、Stream、Asset
Version 和 Feedback ID 使用 Domain 的类型化 UUID 构造器；测试不依赖具体随机值。

## 7. 手动 Review Stream

阶段 2 对手动工作流使用以下不变量：

- 一个 Project 至多存在一个 `production: None` 的手动 Stream；
- 第一次创建手动 Round 时生成 Stream ID；
- 后续手动 Round 使用同一 Stream ID；
- 新 Draft 的 `previousCompletedRoundId` 等于该 Stream 当前 head；
- 第一个 Draft 被放弃且尚无 Completed 历史时，不产生 Stream index 条目；
- 多个无 Production Scope 的既有 Stream 属于歧义数据，阶段 2 不猜测合并；
- 有 Production Scope 的 Stream 属于后续阶段，不能被误认成手动 Stream。

这不是项目级隐式 latest；所有读取和完成历史仍属于明确 Stream ID。

## 8. 素材身份与内容证据

### 8.1 创建准备

手动素材没有 Producer 提供的内容版本身份，因此阶段 2 在创建 Draft 前，以有界并发流式计算
BLAKE3，并捕获：

- 当前 Entity ID／平台稳定文件身份；
- 项目相对路径；
- `sizeBytes`；
- `modifiedNs`；
- BLAKE3（内容可读时）；
- 图片或视频媒体边界；
- 已确认技术失败。

准备过程显示完成数量，可以取消，不能无限并发读取大型视频。全部候选完成解析后一次构造并
保存 Draft；中途失败或取消不会留下半名单 Draft。

写入租约在准备开始前取得，并在租约内重新确认没有另一个 Active Draft。准备被取消或失败时
立即释放租约，因此两个 Viewer 实例不能利用探测与保存之间的时间窗口各自创建 Draft。

无法读取内容但仍能安全确认文件身份的候选可以使用 `blake3: null` 并进入
`unreviewable`。无法建立安全身份的候选不能进入固定名单，创建操作必须失败并明确报告。

### 8.2 恢复与完成验证

Active Session 保持 Asset Version ID 到 Entity ID 的易失绑定。恢复 Draft 时，对每个保存了
BLAKE3 的素材重新流式计算并精确比较摘要，再根据相对路径和文件身份建立绑定；不能仅按
文件名恢复。`blake3: null` 的技术失败素材使用原文件身份、路径、大小和修改时间进行保守
验证，任一证据变化都产生冲突。

完成前逐项检查：

- Entity ID／平台文件身份和相对路径；
- 大小和修改时间；
- Watcher 是否报告成员变化；
- 媒体边界和稳定失败状态。

完成验证采用确定性规则：路径、平台文件身份或大小变化时直接冲突；相同文件身份下修改时间
变化，或 Watcher 报告过该成员变化时，重新计算 BLAKE3，摘要一致才允许继续，摘要不同则
冲突。没有成员事件且身份、路径、大小和修改时间全部一致时，可以复用创建时摘要，避免每次
完成无条件重复读取全部大型视频。

替换、移动、删除或内容身份变化产生 Version Conflict 并阻止完成。阶段 2 不自动重新绑定、
排除或把新文件加入本轮。

## 9. Feedback 与 Outcome

Feedback 的权威内容仍是用户原始自然语言。Application 只提供目标绑定、时间和 ID，不分析、
重写、总结或分类文本。

完成顺序保持阶段 1 的 Domain 语义：

1. 目标存在有效 Feedback → `revise`；
2. 没有 Feedback 且存在稳定技术失败 → `unreviewable`；
3. 没有 Feedback、没有技术失败 → `pass`。

因此，用户即使只根据批量网格留下意见，该素材仍是 `revise`；大图或视频入口不是 Feedback
成立的前置条件。

## 10. 持久化与事务边界

### 10.1 创建和编辑

Service 对每个变更采用 clone-validate-save-swap：

```text
clone 当前 Draft
  → 在副本调用 Domain 行为
  → Repository 原子保存副本
  → 保存成功后替换内存 Draft 并递增 revision
```

保存失败时，已保存应用快照不变。React 可以保留尚未成功提交的编辑文本，但不得显示为已保存
Feedback。

### 10.2 完成发布

```text
锁定评审变更
  → 全量重验固定素材
  → 更新并持久化稳定 unreviewable 状态
  → Domain complete
  → Repository publish Completed 文件
  → 原子更新 Stream index
  → 核对 head
  → UI 进入 CompletedReadOnly
```

如果 Completed 文件已经持久化但 index 更新失败，Service 不报告成功。它重新建立 Repository
写入会话，调用阶段 1 的线性孤儿恢复并核对 exact Round/Stream head：

- 恢复并核对成功 → 可以报告完成；
- 无法唯一恢复 → `RecoveryRequired`，保持只读并要求显式处理。

### 10.3 放弃

放弃只删除未完成 Draft。删除操作受同一写入租约和路径所有权校验保护。删除成功前不清空
Application Session；Completed 文件和 index 不受此入口影响。

## 11. Tauri 与 React 边界

### 11.1 Tauri

新增 review command/DTO 模块，但保持 Adapter 轻量：

- 每个命令校验 `sessionId`、`generation`、Round ID 和 revision；
- DTO 只包含类型化、长度受限、对用户安全的字段；
- Tauri 项目状态组合并持有一个 `ReviewSessionService`；
- 项目关闭、切换或窗口关闭时停止准备任务并释放 Repository 租约；
- Tauri 不构造 Outcome、不序列化协议文件、不持有另一份 Draft 规则。

### 11.2 React

新增窄的评审 UI 单元：

- `ReviewSessionCoordinator`：加载命令、串行变更和状态快照；
- `ReviewContextBar`：范围、计数、返回本轮素材和完成入口；
- `ReviewInspector`：多目标意见编辑和已保存意见列表；
- `ReviewStartDialog`：创建前范围确认；
- `ReviewCompletionDialog`：完成前只读汇总；
- `ReviewRecoveryNotice`：Draft 恢复、Busy、只读和 RecoveryRequired；
- `ReviewConflictList`：受影响路径和安全恢复动作。

评审协调器通过 Workspace Port 组合现有选择、浏览、预览和对比协调器，不复制
`ViewerWorkspace`。生产组件仍不能直接导入 Tauri API。

## 12. 错误与恢复

| 条件 | 评审行为 | 其他 Viewer 行为 |
| --- | --- | --- |
| Project 只读 | 可查看 Completed 和已有 Draft；禁止创建、修改或完成 Draft | 正常只读浏览 |
| Writer Busy | 不抢锁、不超时删除；提供重试 | 正常浏览和预览 |
| Draft 保存失败 | 保留最后已保存快照和用户输入，显示未保存 | 不受影响 |
| Stale revision | 重新加载快照，保留编辑器文本，要求用户重试 | 不受影响 |
| Version Conflict | 允许查看/编辑意见，阻止完成 | 不受影响 |
| UnsupportedVersion | Review 只读，不重写或降级 | 项目继续打开 |
| InvalidData | Review fail closed，报告安全错误 | 项目继续打开 |
| 多 Draft／含糊孤儿 | `RecoveryRequired`，不猜测 latest | 项目继续打开 |
| 不确定 publish | 重开、线性恢复并核对；不能核对则只读 | 不显示虚假成功 |
| 放弃删除失败 | 保持 Draft Active | 不受影响 |

内部路径、进程信息、锁内容、原始解析错误和绝对路径不得直接显示给前端或外部 Agent。

## 13. 安全、性能与可访问性

### 13.1 安全

- 所有路径继续使用 Project Root、`RelativePath`、no-follow 和符号链接拒绝边界。
- Review UI 不接受绝对路径或任意协议文件路径。
- 文本、目标数量、Round 素材数量和文档大小继续遵守协议上限。
- 外部 Agent 仍只能把 index 收录的 Completed Round 当作指令。
- Draft、准备进度和 React 状态永远不是 Agent 指令。

### 13.2 性能

- BLAKE3 使用有界后台队列和流式读取，不把完整文件载入内存。
- 准备和恢复报告进度并支持取消。
- 固定名单上限继续使用 `MAX_ASSETS_PER_ROUND`。
- UI 投影按 ID 和 revision 增量更新，不能在每个按键上复制全部二进制或媒体元数据。
- 大型 Round 的完成验证在后台执行，主线程保持可响应。

### 13.3 可访问性

- 核心入口、Feedback 提交、Inspector 和完成对话框支持完整键盘操作。
- 保存状态、冲突、意见和技术失败同时使用文字、图标和 ARIA 状态，不只依赖颜色。
- 对话框管理焦点进入、退出和 Escape；非空未提交意见不能因无提示关闭而丢失。

## 14. 测试策略

### 14.1 Domain

- 固定名单拒绝重复 ID／路径和未知 Feedback 目标；
- Draft 中不存在 `pass`；
- Feedback 目标在完成时得到 `revise`；
- 无 Feedback 的技术失败得到 `unreviewable`；
- 其余素材只在完成时得到 `pass`；
- Completed 不可回退或覆盖。

### 14.2 Application

使用 Fake Asset Catalog、Repository 和 Clock 验证：

- 一个 Project 只允许一个 Active Draft；
- 明确选择与 Folder Projection 的范围规则；
- 手动 Stream 的创建、复用和 previous head；
- clone-validate-save-swap；
- revision 拒绝过期命令；
- 单目标和多目标 Feedback；
- 自动 `unreviewable`；
- Version Conflict 阻止完成；
- publish 后 head 核对；
- 不确定 publish 的重开与线性恢复；
- 放弃失败不清空 Session。

### 14.3 Infrastructure

- 无副作用探测不创建 `.viewer/reviews/`；
- 零／一／多 Draft 发现；
- Draft 删除只作用于 owned path；
- Writer lease 生命周期和 Busy；
- BLAKE3 流式计算、取消、文件变化和符号链接；
- 真实文件替换、移动、删除和权限失败；
- 原子 Draft 保存和阶段 1 全部 publish 故障注入继续通过。

### 14.4 Tauri 合约

- Session/Generation/Round/revision 校验；
- Review DTO 的序列化形状和输入上限；
- 项目关闭释放任务与写入租约；
- 安全错误码不泄漏绝对路径；
- 架构门继续保证只有 `ui/src/api/viewer.ts` 直接导入 Tauri 包。

### 14.5 React

- 工具栏入口与只读禁用；
- 创建范围确认和排除计数；
- 本轮投影与返回入口；
- 多目标 Feedback、`⌘ Enter`、编辑和删除；
- 保存中、未保存、Stale revision 和恢复；
- Completion 汇总不出现“浏览进度”；
- Version Conflict 阻止确认；
- Completed 只读；
- 键盘、焦点和 ARIA 状态。

### 14.6 端到端与回归

真实临时 Project 覆盖：

```text
打开项目
  → 从选择创建 Round
  → 添加单目标和多目标意见
  → 关闭并重新打开 Viewer
  → 恢复 Draft
  → 完成本轮
  → Node 参考读取器读取 exact manual Stream head
```

另覆盖只读项目、Writer Busy、损坏图片、视频探测失败、文件替换和 publish fault recovery。
最终执行完整 `pnpm verify:clean`，并保留现有环境依赖型原生测试的显式条件边界，不把跳过项
冒充通过。

2026-08-26 的完成验证包括：协议测试 13 项、评审闭环端到端场景 6 项、仓库策略测试
33 项、UI 121 个测试文件中的 1057 项通过（1 项既有跳过）、完整 Rust workspace 测试、
架构边界、TypeScript／Biome、UI 构建、依赖与许可证检查，以及 `pnpm verify` 和
`pnpm verify:clean`。需要捆绑视频运行时或特定原生验收环境的 Rust 项继续保持显式 ignored，
没有被记作通过。

真实一次性项目还完成了界面验收：批量图片和视频浏览、图片／视频预览、从选择创建本轮、
单目标和多目标意见、Draft 重开恢复、无浏览数量的完成汇总、素材替换后的完成阻断、证据
恢复、Completed 只读重开、精确 Stream 读取，以及搜索、图片对比、标记、收藏、文件重命名
和撤销回归。一次性项目不使用用户生产素材。

## 15. 架构验收条件

阶段 2 已于 2026-08-26 满足以下验收条件：

1. 用户可以从当前选择或当前浏览范围创建固定图片／视频名单。
2. 普通文件、现有标记和收藏不会被误纳入 Outcome。
3. 同一 Project 不可能通过应用用例创建两个未完成 Draft。
4. 每条已提交 Feedback 在界面确认成功前已经存在于项目 Draft。
5. 批量网格中的判断与大图／视频判断具有同等评审资格，不要求逐项打开。
6. 完成前不存在 `pass`；完成后 Outcome 覆盖且只覆盖固定名单。
7. 文件替换、移动、删除或内容身份变化不会被静默判为通过。
8. Draft 可以在 Viewer 重启后恢复，但永远不会被参考读取器当作指令。
9. Completed 发布后，现有 Agent 无关读取器可以读取手动 Stream 的 exact head。
10. UI、Tauri、Application、Domain 和 Infrastructure 的职责符合现有架构门。
11. 评审故障不破坏普通项目浏览、预览、搜索、标记或文件整理。
12. 完整验证通过，产品文档明确区分当前已实现行为和未来阶段。

## 16. 后续阶段边界

阶段 2 稳定后，阶段 3 才设计 Production Manifest、Revision Relation、返工版本和多轮对照
复审。阶段 4 才实现图片区域和视频时间 Anchor 的交互。阶段 5 才实现 CLI、MCP、本地 API
和具体 Agent 适配器。

未来适配器必须调用本设计的 `ReviewSessionService` 用例，不得直接写 `.viewer/reviews/` 或在
某个 Agent 插件中复制 Review Domain。
