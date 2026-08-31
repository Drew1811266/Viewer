# Task 20：历史查看、精确恢复与来源确认实施记录

> 日期：2026-08-31  
> 分支：`codex/continuous-review-domain`  
> 隔离工作区：`.worktrees/continuous-review-domain`  
> 范围：实施计划 Task 20；Task 21–23 未开始；连续评审生产入口仍未启用。

## 结果

Task 20 完成了内部 opt-in 的历史查看、选择恢复和素材适用性确认，并保持以下语义边界：

- 历史原文只作为背景；用户明确选择并审阅后端恢复计划后才可恢复，不自动生成反向意见。
- 恢复计划冻结精确 decisions 与后端 plan；范围或冲突选择变化会使旧预览失效，不能提交用户未看过的计划。
- 面板明确显示将恢复的目标、存档覆盖变化及需要重新核验的素材，不声称 Agent 已读、已执行或问题已修复。
- 完整 snapshot 的每个可见目标生成独立单键引用；空、多键和 legacy 引用不会被静默截断。
- legacy 原文、目标和素材仍可只读查看；旧协议没有证据是能力限制，已声明的必需证据损坏仍是完整性错误。
- 换新素材必须明确选择候选并再次确认位置；同路径、歧义候选和独立新图都不会自动绑定。
- 当前素材选择变化会撤销尚未提交的旧来源确认；已提交恢复／继续写事务则保持保护直到协调器完成。
- 历史和来源确认使用单一 modal；未保存输入不会被存档或恢复流程覆盖。

生产 `App` 仍未传入 continuous coordinator 或 history selector，legacy 用户可见入口没有切换。Task 21 才负责可选声明、迁移入口和完整流程启用。

## 架构

历史显示范围与恢复 decision 计算位于纯 `reviewHistoryModel`；异步 session／selector／entity token、证据读取、恢复预览和来源准备集中在专用 `useContinuousHistoryReview`，没有继续扩大 `useImageReviewWorkbench`。

状态机区分可失效的 presentation operation 与已经提交给 coordinator 的 transaction operation。前者可以随面板、选择或会话变化丢弃旧结果；后者在 promise settle 前持续阻止重复提交和误导性关闭。同一 session 的最终成功或失败仍可见，真正替换的 session 不接收旧结果。

组件只使用 review-workspace typed DTO；最终全量策略门禁发现并移除了 `ReviewSourceConfirmation` 对通用 transport `api/types` 的越层依赖。历史图片 URL 只采用 `getEvidence` 返回值，不读取或拼接 `.viewer` 路径。没有新增第三方依赖、Rust／协议变更或原素材写入。

## 审查与修正

首次独立审查发现恢复提交未绑定已展示计划、完整 snapshot 引用缺失、多键静默截断、legacy 内容不显示、busy 可卡死及双 modal 等问题；主审另确认首次冲突死锁和读取错误不可见。`25b619a` 以冻结 preview、单键引用、legacy renderer 和单 modal 状态机修正。

后续复审发现展示生命周期会过早解除已提交写事务保护、普通存档提示可能遮住完整性错误，以及候选 props 的瞬时确认窗口；`19e8b98` 将 presentation／transaction 状态分离并修正错误优先级和同步候选校验。第三轮发现已打开来源确认在当前素材变化后仍保留旧候选；`7f9daa3` 撤销未提交来源状态。最终独立复审 clean。

主流程完整门禁随后发现一项依赖边界失败：来源确认组件直接导入通用 transport Anchor 类型。新实现者以 `38a6a8f` 改从允许的 review-workspace decision DTO 派生等价类型，运行时行为不变；最终窄复审 clean。

## 验证证据

最终 HEAD 上由主流程重新执行：

- Task 20 聚焦：5 个文件，56 项通过。
- UI 全仓：145 个文件，1290 项通过，1 项既有跳过；UI check 与 build 通过。
- `pnpm test:policy`：33 项通过，48 条 scope requirement 精确映射。
- review protocol：54 项通过；manual review loop：7 项通过；video packaging：30 项通过。
- `pnpm verify:clean`：exit 0；Rust workspace、fmt、Clippy、Tauri security、Cargo/npm/video license 全部通过，且验证未改变工作区。
- `pnpm architecture:trends`：exit 0；47 项既有非阻断告警，无 Task 20 新增告警，未改基线。
- `git diff --check`：exit 0。

现有 Biome 配置弃用信息、JSDOM canvas 提示、chunk-size 建议和 Cargo 重复依赖 warning 均为既有非阻断信息。

## 提交

- `e567900` — `feat(review): restore history with explicit source confirmation`
- `25b619a` — `fix(review): bind history decisions to reviewed previews`
- `19e8b98` — `fix(review): preserve submitted history transactions`
- `7f9daa3` — `fix(review): revoke stale source confirmations`
- `38a6a8f` — `fix(review): derive source confirmation anchor type`

任务过程报告提交：`5991cb0`、`0ab7834`、`ecca594`、`ec5bd5e`。

## 未进入本任务的范围

- 未启用生产连续评审入口，未迁移真实工程，未操作用户原素材。
- 未开始声明／迁移入口和完整流程切换（Task 21）。
- 未开始 Rust→Node 多轮真实验收（Task 22）或 Current 产品文档同步（Task 23）。
- 软件仍处于开发初期。代码签名、Apple 公证、正式安装包、上架、公开发布和发售不属于当前任务、阻塞项或默认后续待办。
