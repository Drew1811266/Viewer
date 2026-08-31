# Viewer Continuous Review and Agent Handoff Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在不依赖特定 Agent 的前提下，实现自由编辑、成功保存即可读、精确手动存档和安全复审，避免旧意见被当作新任务。

**Architecture:** 以不可变完整状态快照为事实源，当前索引原子切换；存档只移出用户确认的精确目标版本，历史证据共享且不可变。Domain 计算状态，Application 编排，Infrastructure 负责协议／事务，UI 保持几何与评审生命周期分离。新旧协议显式分派，不把 Completed 重命名后继续沿用原执行含义。

**Tech Stack:** 现有 Rust workspace、Tauri 2、macOS ImageIO／CoreGraphics、React／TypeScript、Vitest、Node.js 内置模块、JSON Schema 与 BLAKE3；不新增第三方运行时依赖。

**Spec:** [已批准设计](../specs/2026-08-27-viewer-continuous-review-and-agent-handoff-design.md)，完整设计与技术细化于 2026-08-27 获用户确认。

> Status: Active
>
> 执行状态：阶段 A–D（任务 1–16）及阶段 E 的 Task 17–19 已完成，累计 19 / 23。
> 阶段 C 最终实现 `21edbf6` 已通过完整 `pnpm verify:clean` 和只读独立复验，检查点 C 完成。
> 2026-08-28 获批的“Node 入口 + Rust 只读核心”已实现：`7863439`，并发修正 `76726a1`；完整门禁及只读独立复核通过。
> Task 16 桥接 `db456ef`、审阅修正 `384d4cc` 已通过完整门禁与只读独立复验；阶段 D 完成。
> 2026-08-28 Task 17 实现 `da1dcff`、修正 `d69a36a` 已通过完整门禁与独立复验，见[协调器记录](../../reviews/2026-08-28-continuous-review-coordinator-task17.md)。
> 2026-08-30 Task 18 实现 `c6d5d11` 已通过完整门禁与独立复验，见[工作台记录](../../reviews/2026-08-30-continuous-review-workbench-task18.md)。Task 19 实现 `d869049`、安全修正 `9a722dc`、架构拆分 `a3305bd` 及精确空选择修正 `e96831d`、`4537cba` 已通过完整门禁与独立复验，见[存档 UI 记录](../../reviews/2026-08-30-continuous-review-archive-ui-task19.md)。下一步 Task 20；Task 20–23 未开始，新 UI 仍未启用。
> 阶段 D 既有状态见[Task 16 契约检查点](../../progress/2026-08-28-continuous-review-task16-contract-checkpoint.md)及[阶段 D 桥接记录](../../reviews/2026-08-28-continuous-review-bridge-phase-d.md)；读取器结果见[阶段 D 读取器记录](../../reviews/2026-08-28-continuous-review-reader-phase-d.md)。
> 应用用例、声明、迁移、Agent 读取器和新桥接仅在隔离工程验证；新桌面接口已组装，旧 UI 未切换，真实工程未迁移。
>
> 计划基线：`a539f8df5f3e0f3239110df44515ba7cb583e308`；实施基线：`d7abb9961f377ae257c3283ef7893218ec0c53f9`。
> 阶段 A 在用户授权的 `.worktrees/continuous-review-domain`、`codex/continuous-review-domain` 内实施；原分支未改动。
> 阶段 A 证据见[阶段 A 记录](../../reviews/2026-08-27-continuous-review-domain-phase-a.md)；
> 暂停事实见[阶段 B 暂停检查点](../../progress/2026-08-27-continuous-review-phase-b-paused-checkpoint.md)；
> 阶段 B 结果见[阶段 B 记录](../../reviews/2026-08-27-continuous-review-persistence-phase-b.md)；
> 阶段 C 已通过状态与限制见[阶段 C 记录](../../reviews/2026-08-27-continuous-review-application-phase-c.md)；最新状态见上述 Task 17 协调器记录。

## Global Constraints

- 原始自然语言是修改意图的权威；Anchor 和预览辅助定位，不能替代原文。
- 当前无意见只表示没有当前返工要求，不生成项目级“全部通过”，也不自动回退历史。
- 每次读取固定一个已提交快照；不把两个快照的文字、标记、编号或预览拼接。
- 文件变化、读取行为、回传声明和存档动作都不能自动推导“问题已解决”。
- 新协议为 `viewer.review/3`；外部使用依据声明为 `viewer.review.usage/1`；v1/v2 历史原位置、原字节保留。
- Viewer 独占 `.viewer/reviews/`；读取器不写项目、不联网、不启动 Viewer／Agent。
- 人工 Review Stream 按项目唯一；Production Stream 不被人工入口改写；不增加浏览遥测。
- 索引最多 16 MiB；状态／存档／声明 JSON 和单个 PNG 最多 64 MiB；单 PNG 最多 16,777,216 像素；单快照引用 PNG 总量最多 4 GiB。
- 当前素材最多 50,000；意见最多 10,000；每条意见目标最多 10,000；文本最多 65,536 字节；画笔单条最多 2,048 点，单快照合计最多 200,000 点。
- 历史链单次追溯最多 10,000 个节点；超限不猜结果、不复用旧结果、不自动清理历史。
- 不修改原始素材；测试工程使用 `TempDir`／`mkdtemp` 或用户“下载/测试图”的受控副本。
- 不把新能力提前写入 Current 产品文档；不修改版本号，不增加视频时间标注 UI、AI 合并或自动 Agent 执行。
- 代码签名、Apple 公证、正式安装包、上架、公开发布和发售均不是本轮任务、阻塞或验收项。
- 依赖方向保持 Domain ← Application ← Infrastructure／平台适配；本轮不重构无关浏览、搜索、文件事务或视频播放。

## 1. 执行方式、检查点与切换顺序

这是同一条评审状态链上的耦合变更，不拆为彼此独立上线的产品子项目。按六个可验证阶段推进：

| 阶段 | 任务 | 检查点与可证明结果 |
| --- | --- | --- |
| A：纯规则 | 1–5 | 稳定身份、部分存档、后补保护、恢复和增量均通过 Domain 测试 |
| B：可持久化协议 | 6–10 | Rust v3 编解码、同一租约、原子提交、早期证据和素材校验通过；不切换 UI |
| C：应用用例 | 11–13 | 持续保存、声明与迁移在隔离工程中可调用；旧生产入口不自动迁移 |
| D：对外边界 | 14–16 | Node 独立读取与 Rust 输出互通，桌面命令和证据 token 受会话约束 |
| E：用户流程 | 17–21 | 编辑、存档、历史、换版、迁移和可选声明入口完整接通 |
| F：闭环验收 | 22–23 | 实际多轮流程与旧功能回归通过，产品／协议文档按已实现事实同步 |

每个任务至少完成一个 RED → GREEN → 回归 → 提交周期。复杂任务中的负例逐个加入，不先写完
全部生产代码再补测试。步骤以一次小修改为单位；表中的验收集不能用单个示例测试替代。

进入实施时先用 `superpowers:using-git-worktrees` 核对隔离环境；规划阶段不新建 worktree。
从当前工作状态开始的分支／worktree选择遵守用户授权，不自行切换或丢弃已有改动。
执行前读 `CONTRIBUTING.md`、设计和本计划，记录基线 `git status --short`，运行最近的聚焦测试。
未明确选择并行执行前不自行派发子代理；计划自检由当前 agent 完成。

阶段 E 完整前，新 UI 只在组件测试、验收载体及显式创建的 v3 测试项目中启用。不得在应用启动
时自动迁移用户工程，也不得让半成品界面操作真实旧索引。旧写入口与新写入口共用同一租约。

## 2. 文件结构与职责

下列路径均相对仓库根。`Create` 路径为计划新增，不能将不存在当成当前仓库损坏。

| 所在位置 | 计划文件／修改入口 | 职责 |
| --- | --- | --- |
| Domain | `crates/viewer-domain/src/review/continuous/{mod,model,mutation,archive,restore,delta,source,validation}.rs` | 纯规则与跨快照身份核验；不序列化文件布局，不做 IO |
| Domain exports | `crates/viewer-domain/src/lib.rs`、`review/mod.rs` | 新强类型 ID 与窄公共导出；不复用文件整理的 ReviewState 名称 |
| Application | `crates/viewer-application/src/review_workspace/{mod,ports,service,editing,archiving,history,usage,migration,projection}.rs` | 用例、版本守卫、恢复和只读投影 |
| Evidence port | `crates/viewer-application/src/review_evidence.rs` | 已验证图像表示与编号预览；保留 legacy ArtifactPort |
| Infrastructure | `crates/viewer-infrastructure/src/review/continuous/{mod,repository,commit,history,recovery,evidence,usage,migration,command}.rs` | v3 文件事务与有界历史、命令摘要；复用现有 atomic／lease |
| Protocol | `crates/viewer-infrastructure/src/review/protocol/v3/{mod,records,validate}.rs` | 新 Schema 对应的 DTO 和闭合校验 |
| macOS | `crates/viewer-platform-macos/src/image/review_evidence.rs`，现有 `review_annotation.rs` | 捕获底图、从不可变底图渲染；保留 marker 图形状态隔离 |
| External reader | `crates/viewer-infrastructure/src/bin/viewer-review-reader.rs`、`review/continuous/reader/`；`scripts/review-protocol/{native-reader,read-current,read-history,read-latest}.mjs` | Rust 复用有界 IO／协议／Domain；Node 仅受限进程调用与 CLI |
| Desktop | `src-tauri/src/{commands,dto,state}/review_workspace.rs`、`state/session.rs` | 新桥接和组装；不让 UI 传入任意文件路径 |
| UI port / DTO | `ui/src/api/reviewWorkspaceTypes.ts`、`api/viewer.ts`、`app/workspace/ports.ts` | 闭合命令及响应，原始协议文件不暴露给组件 |
| UI coordination | `ui/src/app/review/{useContinuousReviewCoordinator,continuousReviewModel}.ts` | 稳定快照、操作去重、编辑输入与状态 |
| UI presentation | `ui/src/components/review/{ReviewArchiveDialog,ReviewHistoryPanel,ReviewMigrationDialog,ReviewSourceConfirmation,ReviewUsageImport}.tsx` | 各自独立的用户决策面板 |
| Existing workbench | `useImageReviewWorkbench.ts`、`WorkspaceImageReviewPreview.tsx`、`ImageReviewWorkspace.tsx`、`ReviewWorkspaceLayer.tsx` | 接入持续评审，不修改通用 ImagePreview 的数据所有权 |

## 3. 公共命名与接口约定

执行者不得在不同任务中另起同义类型。以下内容是接口词汇表，详细字段与构造约束由对应任务提供。

| 名称 | 定义任务 | 用途 |
| --- | --- | --- |
| `ReviewSnapshotId`, `ReviewTargetId`, `ReviewTextRevisionId`, `ReviewTargetRevisionId`, `ReviewArchiveId`, `ReviewCommandId`, `ReviewUsageId` | 1 | 沿现有 `id_type!` 风格新增；禁止用 UI 编号替代 |
| `ContinuousReviewState`, `VersionedFeedback`, `VersionedTarget`, `TargetVersionKey`, `SnapshotRef`, `HistoryRef` | 1 | 当前内容、精确目标版本和历史来源 |
| `ReviewAvailability`, `ReviewPendingReason`, `ReviewChange`, `ReviewChangeKind`, `ContinuousReviewError` | 1 | 可执行性、状态变化与纯规则错误 |
| `ArchiveSelection`, `ArchiveBasis`, `ArchivePlan`, `ArchiveCheckpoint`, `ArchiveDisposition` | 3 | 选择范围与精确移出结果 |
| `RestorePlan`, `RestoreDecision`, `RestoreDisposition` | 4 | 非破坏性恢复与冲突选择 |
| `ReviewDelta`, `TargetDelta`, `SourceCheck`, `SourceCheckStatus`, `CurrentReviewProjection` | 5 | 增量与独立读时检查 |
| `ReviewStateRecord`, `ReviewIndexV3`, `ReviewArchiveRecord`, `ReviewUsageRecord`, `ReviewReadResult` | 6 | JSON wire 类型；Domain 不依赖它们 |
| `ContinuousReviewRepositoryPort`, `ContinuousReviewRepositoryProviderPort`, `ReviewCommitRequest`, `ReviewCommitReceipt`, `ReviewCommitError` | 7 | CAS 提交与持久去重 |
| `ReviewEvidencePort`, `BoundReviewImage`, `ReviewEvidenceRequest`, `ReviewEvidenceResult` | 9 | 已绑定图像；不可从 UI 构造可信句柄 |
| `ContinuousReviewService`, `ReviewWorkspaceView`, `ReviewWorkspaceCommand`, `ReviewCommandEnvelope`, `ReviewWorkspacePreview` | 11 | 应用公共边界 |
| `ReviewWorkspacePort`, `ContinuousReviewCoordinator` | 16–17 | TypeScript 端口与工作台接口 |

所有 SnapshotRef 都含 `snapshotId` 与 `blake3`，由已提交入口或带摘要引用解析；Domain 不持有
仓库绝对路径。目标版本键固定为四元组：feedback、textRevision、target、targetRevision。
每个状态 ID 只对应一份不可变内容，新增操作需新的状态 ID；幂等重试复用原操作 ID。

`ContinuousReviewError` 的固定类别：`InvalidData`、`LimitExceeded`、`MissingReference`、
`DuplicateIdentity`、`StaleSnapshot`、`SelectionConflict`、`NeedsConfirmation`。
IO／不确定提交错误不得塞入 Domain，见任务 7–8 的 `ReviewCommitError`。

### 阶段 A 实施细化（后续任务继续沿用）

- `ArchiveCheckpoint` 保存 `project_id`、`stream_id` 和 `before: SnapshotRef`，不只保存裸
  `before_snapshot_id`。未知 usage 仍需用真实摘要引用 C 的历史内容；读取时访问 `before.snapshot_id`。
- 增加 `apply_archive`／`apply_restore` 纯函数：重新校验计划、生成新状态，不信任被外部改写的移出数组。
- 恢复 decisions 是明确选择范围；空选择不复活任何目标。即时撤销由应用层核实后选择
  `archive.removed`，不能用全部历史目标替代。共同文字冲突不得覆盖未选目标。
- `ContinueAsNew` 增加 `created_at_ms`、`confirmed_anchor: Option<FeedbackAnchor>`；没有
  确认位置时保持待确认。继续提出不撤销旧意见的存档覆盖；同身份恢复才撤销对应覆盖。
- 恢复保留旧版本的保存事实，另返回 `requires_source_check`；执行条件必须再经过读时源检查。
  历史 Ready 不是新读取授权；无检查或源变化都不能进入 actionable。
- 纯状态变更清空未绑定的 parent，由应用／仓储提供真实前驱摘要；无内容变化的操作不制造版本。
- 复用旧素材／Anchor 校验只扩大 `round.rs` 内部可见性；公共接口变更按治理要求提前登记
  [ADR 0006](../../adr/0006-continuous-review-snapshot-and-archive-protocol.md)，Task 6 再补线协议决策。
- 新增私有 `validation.rs` 与测试 fixture 模块以复用跨快照约束；不增加依赖或改变旧评审行为。
- 恢复计划按意见版本共享 `RestoreFeedbackContent`／原文和素材元数据；每项只持有自己的目标，
  应用时按索引合并，避免共同意见 10,000 个目标造成全文复制放大。

---

### Task 1: 强类型身份、当前状态与验证

**Files:** Create `crates/viewer-domain/src/review/continuous/mod.rs`、`crates/viewer-domain/src/review/continuous/model.rs`、`crates/viewer-domain/tests/continuous_review_state.rs`；Modify `crates/viewer-domain/src/lib.rs`、`crates/viewer-domain/src/review/mod.rs`。

**Interfaces:** Produces 词汇表任务 1 类型；沿用 `AssetVersion`、`FeedbackAnchor`、`ProjectId`、`ReviewStreamId`、`FeedbackId`。公开方法：

```rust
impl ContinuousReviewState {
    pub fn empty(project_id: ProjectId, stream_id: ReviewStreamId, snapshot_id: ReviewSnapshotId) -> Self;
    pub fn validate(&self) -> Result<(), ContinuousReviewError>;
    pub fn target_key(&self, target_id: ReviewTargetId) -> Option<TargetVersionKey>;
}
impl VersionedTarget {
    pub fn asset(id: ReviewTargetId, revision: ReviewTargetRevisionId, asset: AssetVersionId) -> Self;
}
```

- [x] **Step 1 — 写空状态和身份约束 RED 测试。**

```rust
#[test]
fn an_empty_current_state_has_identity_but_no_pass_outcomes() {
    let state = ContinuousReviewState::empty(
        ProjectId::from_u128(1), ReviewStreamId::from_u128(2),
        ReviewSnapshotId::from_u128(3),
    );
    assert_eq!(state.snapshot_id, ReviewSnapshotId::from_u128(3));
    assert!(state.feedback.is_empty());
    assert!(state.validate().is_ok());
}
```

- [x] **Step 2 — 运行 RED。** `cargo test --locked -p viewer-domain --test continuous_review_state`；预期新类型缺失或约束断言失败，不能接受路径／测试未发现作为 RED。
- [x] **Step 3 — 建立最小模型。** State 字段为 `project_id`、`stream_id`、`snapshot_id`、`parent: Option<SnapshotRef>`、`assets: Vec<AssetVersion>`、`feedback: Vec<VersionedFeedback>`；不含 `pass`。Feedback 字段为 `id`、`text_revision_id`、`text`、`created_at_ms`、`targets`、`history_ref`。Target 字段为 `id`、`revision_id`、`asset_version_id`、`anchor`、`availability`；`asset()` 建立 `FeedbackAnchor::Asset` 和 Ready。

```rust
pub enum ReviewAvailability { Ready, NeedsConfirmation(Vec<ReviewPendingReason>) }
pub enum ReviewPendingReason {
    SourceChanged, SourceMissing, SourceUnreadable, SourceUnverified,
    LegacyUsageUnknown, LegacyEvidenceAbsent, ApplicabilityUnconfirmed,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct TargetVersionKey {
    pub feedback_id: FeedbackId,
    pub text_revision_id: ReviewTextRevisionId,
    pub target_id: ReviewTargetId,
    pub target_revision_id: ReviewTargetRevisionId,
}
```

- [x] **Step 4 — 逐条加负例并通过。** 外键缺失、重复 ID、空文字／空目标、非有限 Anchor、错误媒体、空待确认原因、资源超限。模型复用现有 Anchor 验证；状态不接受跨上下文引用。`SnapshotRef { snapshot_id: ReviewSnapshotId, blake3: [u8;32] }`；`HistoryRef { project_id, stream_id, source: HistorySource }`。HistorySource 为 Snapshot { snapshot, keys: Vec<TargetVersionKey> } 或 Legacy { round_id, record_blake3, targets: Vec<LegacyTargetRef> }；`LegacyTargetRef { round_id, feedback_id, target_index: u32 }` 的序号只用于定位不可变旧记录，不冒充 v3 稳定 Target ID。带 Anchor 的类型只派生 PartialEq，不错误派生 Eq；现有 UUID ID 没有 Ord，索引用 HashMap 并在序列化边界显式规范排序。
- [x] **Step 5 — GREEN 与旧 Domain 回归。** `cargo test --locked -p viewer-domain`；空状态合法，有意见却无对应素材非法。
- [x] **Step 6 — 提交。** 只暂存本任务 Files，`git commit -m "feat(review): model continuous review identities and state"`。

### Task 2: 新增、文字编辑、重绘与目标级撤回

**Files:** Create `crates/viewer-domain/src/review/continuous/mutation.rs`、`crates/viewer-domain/tests/continuous_review_mutation.rs`；Modify `crates/viewer-domain/src/review/continuous/mod.rs`、`crates/viewer-domain/src/review/continuous/model.rs`。

**Interfaces:** Consumes Task 1。Produces `update_feedback_text(&VersionedFeedback, ReviewTextRevisionId, &str) -> Result<VersionedFeedback, ContinuousReviewError>`、`replace_target(&VersionedFeedback, ReviewTargetId, ReviewTargetRevisionId, FeedbackAnchor) -> Result<VersionedFeedback, ContinuousReviewError>`、`withdraw_targets(&ContinuousReviewState, &[ReviewTargetId], ReviewSnapshotId) -> Result<ContinuousReviewState, ContinuousReviewError>`。`ReviewChangeKind` 至少区分 Added、Edited、Withdrawn、Archived、Restored、Rebound、AvailabilityChanged。

`ReviewChange { target_id, before: Option<TargetVersionKey>, after: Option<TargetVersionKey>, kind,
archive_id: Option<ReviewArchiveId>, historical_key: Option<TargetVersionKey> }`；最后两字段仅在
归档／恢复覆盖事件中使用，校验组合合法。新增接口 `add_feedback(&ContinuousReviewState,
VersionedFeedback, ReviewSnapshotId) -> Result<ContinuousReviewState, ContinuousReviewError>`。
保存状态的前驱引用由 Application／Repository 对照真实已提交摘要设置，纯函数不自造摘要。

- [x] **Step 1 — 写“修改不是追加”的 RED 测试。**

```rust
let feedback = VersionedFeedback {
    id: FeedbackId::from_u128(10), text_revision_id: ReviewTextRevisionId::from_u128(11),
    text: "袖口收紧".to_owned(), created_at_ms: 1,
    targets: vec![VersionedTarget::asset(ReviewTargetId::from_u128(12), ReviewTargetRevisionId::from_u128(13), AssetVersionId::from_u128(14))],
    history_ref: None,
};
let revised = update_feedback_text(&feedback, ReviewTextRevisionId::from_u128(15), "袖口收紧，保留褶皱").unwrap();
assert_eq!(revised.id, feedback.id);
assert_eq!(revised.targets[0].id, feedback.targets[0].id);
assert_ne!(revised.text_revision_id, feedback.text_revision_id);
assert_eq!(feedback.text, "袖口收紧");
assert_eq!(revised.text, "袖口收紧，保留褶皱");
```

- [x] **Step 2 — RED。** `cargo test --locked -p viewer-domain --test continuous_review_mutation`。
- [x] **Step 3 — 实现复制后更新，不改历史对象。**

```rust
let mut next = feedback.clone();
next.text = text.to_owned();
next.text_revision_id = revision_id;
next.validate()?;
Ok(next)
```

为 VersionedFeedback 增加 `validate() -> Result<(), ContinuousReviewError>` 并复用 Task 1 约束；新增意见需新 Feedback ID；重绘只改指定目标 revision／anchor；撤回只移除选定目标，最后一目标撤回后移除空 Feedback，但保留完整空 State。
- [x] **Step 4 — 加入共同意见两目标、删除不存在目标、重复 ID、新素材追加及不保留空标记的测试。** 撤回记录原因 Withdrawn；不产生反向修改文字。
- [x] **Step 5 — GREEN。** `cargo test --locked -p viewer-domain --test continuous_review_state --test continuous_review_mutation`。
- [x] **Step 6 — 提交。** `git commit -m "feat(review): preserve feedback identity across current edits"`，只暂存 Files。

### Task 3: 精确部分存档与后补意见保护

**Files:** Create `crates/viewer-domain/src/review/continuous/archive.rs`、`crates/viewer-domain/tests/continuous_review_archive.rs`；Modify `crates/viewer-domain/src/review/continuous/mod.rs`。

**Interfaces:** `ArchiveBasis = Known { snapshot: SnapshotRef, source: AgentDeclared | UserSelected } | Unknown`；ArchiveSelection 为基于 C 的目标键与依据组。ArchivePlan 包含 expectedSnapshotId、归档证据、removed、retained、alreadyCovered；ArchiveCheckpoint 固化该结果与 archiveId／时刻。Produces：

```rust
pub enum ArchiveDisposition { RemoveCurrent, RetainLaterEdit, AlreadyAbsent }
pub fn classify_archive(current: Option<TargetVersionKey>, basis: TargetVersionKey) -> ArchiveDisposition;
pub fn plan_archive(current: &ContinuousReviewState, bases: &[ContinuousReviewState], selection: &ArchiveSelection, coverage: &[ArchiveCoverage]) -> Result<ArchivePlan, ContinuousReviewError>;
```

`ArchiveSelection { expected_snapshot_id, groups: Vec<ArchiveGroup> }`；每组含 basis 及选定
`Vec<TargetVersionKey>`。Known 组的键来自依据 B，可包含 C 中已删除的目标；Unknown 组的键
必须仍与 C 精确相等。`ArchiveBasisSource::AgentDeclared { usage_id }`／`UserSelected` 是正式
枚举定义。`ArchiveCoverage { archive_id, key, active }` 由已核验存档及恢复链提供，不能查目录猜测。

- [x] **Step 1 — RED：B 读后 C 改动不能一起消失。**

```rust
let b = TargetVersionKey {
    feedback_id: FeedbackId::from_u128(1), text_revision_id: ReviewTextRevisionId::from_u128(2),
    target_id: ReviewTargetId::from_u128(3), target_revision_id: ReviewTargetRevisionId::from_u128(4),
};
let c = TargetVersionKey { text_revision_id: ReviewTextRevisionId::from_u128(5), ..b };
assert_eq!(classify_archive(Some(c), b), ArchiveDisposition::RetainLaterEdit);
assert_eq!(classify_archive(Some(b), b), ArchiveDisposition::RemoveCurrent);
assert_eq!(classify_archive(None, b), ArchiveDisposition::AlreadyAbsent);
```

- [x] **Step 2 — RED。** `cargo test --locked -p viewer-domain --test continuous_review_archive`。
- [x] **Step 3 — 实现纯比较。**

```rust
match current {
    None => ArchiveDisposition::AlreadyAbsent,
    Some(key) if key == basis => ArchiveDisposition::RemoveCurrent,
    Some(_) => ArchiveDisposition::RetainLaterEdit,
}
```

聚合算法先验证 C 和所有依据上下文，再按 Target ID 建索引；已知依据归档 B 原文，未知依据归档用户确认的 C 且 usageBasis=null。一个目标多个依据报 SelectionConflict，不用顺序覆盖。

正式字段为 `ArchivePlan { expected_snapshot_id, groups, removed, retained, already_covered }`；
removed／already_covered 为精确键数组，retained 元素同时记录依据键、可选当前键和 disposition。
`ArchivePlan::is_noop() -> bool` 判断没有新历史事实也没有当前移出。`ArchiveCheckpoint` 增加
archive_id、created_at_ms、project_id、stream_id、before: SnapshotRef，并固定计划中的分组与结果。
未知依据组的历史内容取 before 对应的 C；usageBasis=null 不等于历史内容没有可核验来源。
- [x] **Step 4 — 增加共同意见只存图 1、图 2 保留，删除后不复活，新增不移出，重绘保留，空选择不建档、有效覆盖去重及已撤销覆盖可再次存档的测试。**
- [x] **Step 5 — GREEN。** `cargo test --locked -p viewer-domain --test continuous_review_archive`。
- [x] **Step 6 — 提交。** `git commit -m "feat(review): archive exact target revisions without losing later edits"`。

### Task 4: 撤销存档、选择恢复与继续提出

**Files:** Create `crates/viewer-domain/src/review/continuous/restore.rs`、`crates/viewer-domain/tests/continuous_review_restore.rs`；Modify `crates/viewer-domain/src/review/continuous/mod.rs`。

**Interfaces:** `RestoreDisposition = Restore | ConfirmConflict | AlreadyCurrent`。`RestoreDecision { historical_key: TargetVersionKey, choice: RestoreChoice }`；`RestoreChoice` 为 PreserveCurrent／UseHistorical／ContinueAsNew，后者携带新 Feedback／文字／Target／目标版本 ID、目标 AssetVersionId 和确认后的 Anchor。RestorePlan 含恢复目标、冲突目标和按键记录的存档覆盖撤销事件。Produces `classify_restore(Option<TargetVersionKey>, TargetVersionKey) -> RestoreDisposition`、`plan_restore(&ContinuousReviewState, &ArchiveCheckpoint, &[ContinuousReviewState], &[RestoreDecision]) -> Result<RestorePlan, ContinuousReviewError>`。第三参数为经过验证的历史依据内容，不能仅凭历史键恢复猜测的原文。一个历史键最多一个 decision，未知键或重复选择报错。

- [x] **Step 1 — RED：新内容不能被整体回滚。** 构造 old 与 newer 两个同 Target ID、不同文字版本的键。

```rust
let old = TargetVersionKey {
    feedback_id: FeedbackId::from_u128(1), text_revision_id: ReviewTextRevisionId::from_u128(2),
    target_id: ReviewTargetId::from_u128(3), target_revision_id: ReviewTargetRevisionId::from_u128(4),
};
let newer = TargetVersionKey { text_revision_id: ReviewTextRevisionId::from_u128(5), ..old };
assert_eq!(classify_restore(Some(newer), old), RestoreDisposition::ConfirmConflict);
assert_eq!(classify_restore(Some(old), old), RestoreDisposition::AlreadyCurrent);
assert_eq!(classify_restore(None, old), RestoreDisposition::Restore);
```

- [x] **Step 2 — RED。** `cargo test --locked -p viewer-domain --test continuous_review_restore`。
- [x] **Step 3 — 实现恢复判定与新来源。**

```rust
match current {
    None => RestoreDisposition::Restore,
    Some(key) if key == historical => RestoreDisposition::AlreadyCurrent,
    Some(_) => RestoreDisposition::ConfirmConflict,
}
```

无后续变更的撤销追加恢复事件；有后续变更按明确选择处理。ContinueAsNew 使用新 Feedback／Target 身份，HistoryRef 指向旧键；尚未确认新图适用性时标 ApplicabilityUnconfirmed。不得复用旧坐标并直接 Ready。
- [x] **Step 4 — 加测试：源改变时撤销仍待确认；已删除目标与明确恢复区分；重复恢复去重；旧档案字节和新意见都保持。**
- [x] **Step 5 — GREEN。** `cargo test --locked -p viewer-domain --test continuous_review_restore --test continuous_review_archive`。
- [x] **Step 6 — 提交。** `git commit -m "feat(review): restore archived feedback without overwriting current work"`。

### Task 5: 完整当前投影与精确净增量

**Files:** Create `crates/viewer-domain/src/review/continuous/delta.rs`、`crates/viewer-domain/src/review/continuous/source.rs`、`crates/viewer-domain/tests/continuous_review_delta.rs`；Modify `crates/viewer-domain/src/review/continuous/mod.rs`。

**Interfaces:** SourceCheck 含 assetVersionId、checkedAtMs、`SourceCheckStatus = Match | Changed | Missing | Unreadable | Unverified`。Produces `project_current(&ContinuousReviewState, &[SourceCheck]) -> Result<CurrentReviewProjection, ContinuousReviewError>`、`diff_review(&ContinuousReviewState, &ContinuousReviewState, &[ReviewChange]) -> Result<ReviewDelta, ContinuousReviewError>`。Projection 含 actionable／needsConfirmation；Delta 每目标可同时含文字、几何、绑定变化和明确移出原因，不含旧全文。

Rust 字段为 `asset_version_id`、`checked_at_ms`、`status`；序列化／DTO 才用 camelCase。
`CurrentReviewProjection { actionable: Vec<ReviewTargetId>, needs_confirmation: Vec<ReviewTargetId> }`；
`ReviewDelta { since_snapshot_id, current_snapshot_id, targets: Vec<TargetDelta> }`；TargetDelta 含
target_id、before／after 版本键、text_changed／anchor_changed／binding_changed／availability_changed
以及 `removal_reason: Option<ReviewChangeKind>`。投影返回身份，读取结果另带完整原文，不能只有 ID。

- [x] **Step 1 — RED：读时检查不能改变保存快照。** 用 Task 1 字段构造一条 Ready 目标的 state，checks 对它返回 Changed。

```rust
let before = state.clone();
let projected = project_current(&state, &checks).unwrap();
assert!(projected.actionable.is_empty());
assert_eq!(projected.needs_confirmation.len(), 1);
assert_eq!(state, before);
assert!(diff_review(&before, &state, &[]).unwrap().targets.is_empty());
```

- [x] **Step 2 — RED。** `cargo test --locked -p viewer-domain --test continuous_review_delta`。
- [x] **Step 3 — 实现按身份对齐和原文／几何净比较。** 版本键用于识别，不以“版本号变了”强制输出无内容差异的修改；取 union(Target IDs)，逐项比较存活状态及字段，移出原因由 ReviewChange 验证。Projection 不升级用户待确认项。

```rust
let can_execute = matches!(target.availability, ReviewAvailability::Ready)
    && matches!(source_check.status, SourceCheckStatus::Match);
if can_execute { actionable.push(target.id); }
else { needs_confirmation.push(target.id); }
```

- [x] **Step 4 — 加测试：新增后删除、改后改回、只重绘、共同意见部分存档、撤回不是逆向指令、缺少 source check 不能 Ready、同目标不能落入两类。**
- [x] **Step 5 — GREEN／检查点 A。** `cargo test --locked -p viewer-domain`；审阅纯规则后才进入存储。
- [x] **Step 6 — 提交。** `git commit -m "feat(review): separate current projection from historical deltas"`。

### Task 6: v3 Schema、Rust 编解码与契约夹具

**Files:** Create `docs/protocol/viewer-review-index-v3.schema.json`、`docs/protocol/viewer-review-state-v3.schema.json`、`docs/protocol/viewer-review-archive-v3.schema.json`、`docs/protocol/viewer-review-read-result-v3.schema.json`、`docs/protocol/viewer-review-usage-v1.schema.json`；`crates/viewer-infrastructure/src/review/protocol/v3/mod.rs`、`crates/viewer-infrastructure/src/review/protocol/v3/records.rs`、`crates/viewer-infrastructure/src/review/protocol/v3/validate.rs`；`tests/fixtures/review-protocol/review-index-v3.valid.json`、`tests/fixtures/review-protocol/review-state-v3.valid.json`、`tests/fixtures/review-protocol/review-archive-v3.valid.json`、`tests/fixtures/review-protocol/review-read-result-v3.valid.json`、`tests/fixtures/review-protocol/review-usage-v1.valid.json`、`tests/fixtures/review-protocol/continuous-review-cases.json`；`scripts/review-protocol/v3-fixtures.mjs`；`tests/review_protocol_v3_contract.rs`。Modify `docs/adr/0006-continuous-review-snapshot-and-archive-protocol.md`、`crates/viewer-infrastructure/src/review/protocol/mod.rs`、`crates/viewer-infrastructure/src/review/mod.rs`、`crates/viewer-infrastructure/Cargo.toml`、`scripts/review-protocol/schema-contract.test.mjs`、`docs/README.md`。

**Interfaces:** `encode_state_v3(&ReviewStateRecord) -> Result<Vec<u8>, ReviewProtocolError>`、`decode_state_v3(&[u8]) -> Result<ReviewStateRecord, ReviewProtocolError>`；`encode_index_v3`／`decode_index_v3` 对应 ReviewIndexV3，`encode_archive_v3`／`decode_archive_v3` 对应 ReviewArchiveRecord，`encode_usage_v1`／`decode_usage_v1` 对应 ReviewUsageRecord，参数及返回规则为 typed record→bytes 与 bytes→typed record。StateRecord 含 domain state、操作 ID／载荷摘要、转移原因、证据清单；IndexV3 按 Stream 保存 currentRef、archiveRefs、legacyRefs；ArchiveRecord 包含 checkpoint、beforeRef、resultSnapshotId，禁止循环摘要引用。Wire DTO 独立于 Domain；Task 7 增加的应用存储模型由 adapter 映射，不能反向让 Application import wire DTO。

`createProjectV3Case(name) -> Promise<{projectRoot, streamId, snapshotIds, archiveId, cleanup()}>`
在 v3-fixtures 定义，读取这些黄金 JSON／case 数据，在独占 mkdtemp 工程内组装合法引用图和摘要。
name 闭合集为 current_nonempty、current_empty、partial_archive、later_edit、pending_source、legacy_mixed。
源 PNG 复制 `tests/fixtures/images/alpha.png`；旧证据复制既有 project-v2-mixed 的 PNG，不改其字节。
content-addressed 文件名由实际摘要生成，只有临时工程被写入；这些路径不是新增静态 fixture 目录。

- [x] **Step 1 — RED：闭合格式和空当前状态不是 Completed。**

```js
test('v3 current state is closed and has no completed outcome contract', async () => {
  const state = await schema('viewer-review-state-v3.schema.json')
  assert.equal(state.additionalProperties, false)
  assert.equal(state.properties.protocolVersion.const, 'viewer.review/3')
  assert.equal(state.properties.kind.const, 'state')
  assert.equal(state.properties.feedback.minItems, 0)
  assert.equal(Object.hasOwn(state.properties, 'outcomes'), false)
})
```

- [x] **Step 2 — RED。** `node --test scripts/review-protocol/schema-contract.test.mjs`；随后 `cargo test --locked -p viewer-infrastructure --test review_protocol_v3_contract`。
- [x] **Step 3 — 写入闭合 Schema 和 DTO。** 所有嵌套对象 `additionalProperties:false`；字符串、数组、字节和数字分别有界。记录依次使用 `kind: state/index/archive`；Usage 固定独立版本；ReadResult 明确 `role: current/history`、status、snapshotRef、sourceChecks、actionable、needsConfirmation、historyRefs、delta。

```json
{"protocolVersion":{"const":"viewer.review/3"},"kind":{"const":"state"},"feedback":{"type":"array","minItems":0,"maxItems":10000}}
```

该片段是 properties 子树。State 根的完整必填字段为 protocolVersion、kind、projectId、reviewStreamId、snapshotId、parent、commandId、payloadDigest、assets、feedback、changes、evidence；Index 为 protocolVersion、kind、projectId、streams；Archive 为 protocolVersion、kind、projectId、reviewStreamId、archiveId、createdAtMs、beforeRef、resultSnapshotId、groups、removed、retained；Usage 为 protocolVersion、declarationId、projectId、reviewStreamId、basis、targets、outputs。ReadResult 按 Task 15 的判别联合闭合，不容许把 history 当 current。Rust records 使用 `#[serde(deny_unknown_fields)]`，再经过 Domain 验证，不只靠反序列化成功。
- [x] **Step 4 — 加 Rust↔JSON 夹具往返及负例。** 重复目标、跨 Stream 引用、摘要错误、错误角色、循环父链、未知 major、图片标记重复编号、缺失必需证据声明；legacy 能力缺失必须显式标记。夹具至少含非空当前、空当前、部分存档、后补保留、待确认和 legacy 混合场景；测试修改均先复制到临时工程。
- [x] **Step 5 — GREEN，扩展 ADR。** `cargo test --locked -p viewer-infrastructure --test review_protocol_v3_contract && pnpm test:review-protocol && pnpm test:policy`。在阶段 A 已登记的 ADR 0006 中补充 wire 契约与旧接口失败方式，Current 协议说明暂不宣称已启用 v3。
- [x] **Step 6 — 提交。** `git commit -m "feat(review): define versioned continuous review wire contracts"`。

### 阶段 B 的 wire 细化（Task 6）

- SnapshotRef 不接受任意文件路径；状态位置固定由 ID 推导，索引中的其他记录位置按类型与 ID 校验。
- 历史读取采用 `selector + entries`：每个依据快照独立携带内容与选中键，防止多个 B 的版本
  被拼成虚构快照。legacy entry 保留旧轮次与 Feedback／目标位置，不补造 v3 身份。
- 所有 nullable 字段要求显式存在；无载荷标签也拒绝多余字段。UUID 要求连字符形式；
  除十进制字符串 modifiedNs 外，整数使用 JavaScript 精确范围；项目路径上限 4096 UTF-8 字节。
- 聚合目标数组上限由既有 10,000 条意见 × 每条 10,000 目标推导；64 MiB 文档上限通常更早命中。
  v3 编码器在写出过程中限制缓冲增长，不先构造无限大 JSON 再检查大小。
- 新增窄 wire 子模块、read/history-result 模块与 bounded 编码模块；Node Schema 夹具验证器只用于测试。
  未增加依赖，也未让 Domain 或 Application 引用 Infrastructure wire DTO。
- Task 7–8 继续负责真实摘要、父链循环／可达性和 CAS；Task 9 继续负责真实 PNG 核验与捕获。
  Task 6 的编解码通过不等于这些仓储检查已经实现。

### Task 7: 不可变仓储端口、共享租约与原子提交

> 2026-08-27 已完成，20 项新仓储测试、49 项旧仓储测试、Domain 和架构边界门禁通过。
> 实施新增窄 prepare／mapping／references／archives／owned_io 模块；提前建立 evidence／usage
> 的仓储安装与核验部分，不代表 Task 9 原生捕获或 Task 12 外部导入已经完成。

**接口细化：** `ReviewCommitRequest` 增加 `production: Option<ProductionScope>`，
`StoredContinuousSnapshot` 也返回同一固定索引中的作用域。首次提交按显式作用域登记 Stream；
后续提交必须完全一致。None 对应唯一人工流，不允许猜测／改写已有任务和批次归属。

**Files:** Create `crates/viewer-application/src/review_workspace/mod.rs`、`crates/viewer-application/src/review_workspace/ports.rs`；`crates/viewer-infrastructure/src/review/continuous/mod.rs`、`crates/viewer-infrastructure/src/review/continuous/repository.rs`、`crates/viewer-infrastructure/src/review/continuous/commit.rs`、`crates/viewer-infrastructure/src/review/continuous/history.rs`；`crates/viewer-infrastructure/tests/continuous_review_repository.rs`。Modify `crates/viewer-application/src/lib.rs`、`crates/viewer-infrastructure/src/review/mod.rs`、`crates/viewer-infrastructure/src/review/atomic.rs`、`crates/viewer-infrastructure/src/review/provider.rs`。

**Interfaces:** Consumes Tasks 1–6。Application 定义 `PreparedContinuousSnapshot { state, command_id, payload_digest: [u8;32], changes, evidence }`；`StoredContinuousSnapshot` 在这些只读字段之外包含 `reference: SnapshotRef`，来自读取时固定的索引引用，不写回状态文件造成自摘要循环。`ReviewCommitRequest { expected: Option<SnapshotRef>, next: PreparedContinuousSnapshot, archives: Vec<ArchiveCheckpoint>, adopted_usage: Vec<ReviewUsageDeclaration>, staged_evidence: Vec<PreparedEvidenceFile> }`；`ReviewCommitReceipt { command_id, payload_digest, snapshot: SnapshotRef }`。这些都是应用模型，不引用 Infrastructure wire DTO。load_current 一次返回内容与其固定引用，不能分别读两次索引来拼接。

`PreparedEvidenceFile` 含应用拥有的临时路径、摘要、PNG 尺寸／字节数；`ReviewEvidenceBinding` 按 AssetVersionId 绑定 base／annotated 的 `EvidenceRef { blake3, size_bytes, width, height }` 和编号到 TargetVersionKey 的映射。旧缺失能力用 `EvidenceCapability::LegacyAbsent`，不是伪造空 PNG。`ReviewUsageDeclaration` 在本任务定义字段：id、project_id、stream_id、basis、targets、outputs；outputs 元素为 `UsageOutput { relative_path: RelativePath, blake3, previous_asset_version_id }`。Task 12 才实现其输入核验。

`ContinuousReviewRepositoryPort: Send + Sync` 同步方法：`load_current(stream_id) -> Result<Option<StoredContinuousSnapshot>, ReviewCommitError>`、`load_snapshot(stream_id, &SnapshotRef) -> Result<StoredContinuousSnapshot, ReviewCommitError>`、`load_archive(stream_id, archive_id) -> Result<ArchiveCheckpoint, ReviewCommitError>`、`find_command(stream_id, command_id) -> Result<CommandLookup, ReviewCommitError>`、`commit(ReviewCommitRequest) -> Result<ReviewCommitReceipt, ReviewCommitError>`。`CommandLookup = Found(ReviewCommitReceipt) | Absent | Unavailable`；Absent 仅在完整查明后返回。Provider 的 `open_reader()`／`open_writer()` 返回 `Arc<dyn ContinuousReviewRepositoryPort>`，writer 生命周期持有现有 `write.lock` 租约；reader 不创建目录。`ReviewCommitError` 区分 StaleSnapshot、CommandConflict、ReadOnly、LeaseBusy、Integrity、LimitExceeded、LookupUnavailable、Io、OutcomeUnknown。

- [x] **Step 1 — RED：CAS 失败不改变当前。** 在新集成测试中建立 `TempDir`，使用 `ProjectReviewRepositoryProvider` 新增的 `continuous_reader()`／`continuous_writer()`，提交固定 ID 的空初始状态，再用另一个 expected 提交。

```rust
assert!(matches!(writer.commit(stale_request), Err(ReviewCommitError::StaleSnapshot)));
let reread = reader.load_current(stream_id).unwrap().unwrap();
assert_eq!(reread.state.snapshot_id, first.snapshot.snapshot_id);
```

测试中的 first 来自第一次 `commit` 返回值；stale_request 完整构造为同项目／Stream、新 snapshot ID、错误 expected、空 archives／usage／evidence。新集成测试文件直接拥有工程，不读写真实 `.viewer`。
- [x] **Step 2 — RED。** `cargo test --locked -p viewer-infrastructure --test continuous_review_repository`。
- [x] **Step 3 — 按设计的提交顺序实现。** 验证期望与输入 → 安装证据／状态／存档不可变文件 → 验证全部引用 → CAS 重查 → 原子替换并同步索引。证据按内容寻址复用；同名已有文件仍核验完整字节。对象引用图不得循环。

```rust
if observed_current != request.expected {
    return Err(ReviewCommitError::StaleSnapshot);
}
```

这项比较必须在最后的索引发布前再次执行；IO 放入桌面既有阻塞任务边界。复用 atomic_create_once／atomic_replace，只在 review 内部扩大可见性，不新开第二把锁。
- [x] **Step 4 — 加入原子存档、多个 Stream 不互相覆盖、只读无目录副作用、旧／新 writer 互斥、非法相对位置／符号链接和同名不同摘要测试。** Repository 不接受任意外部 snapshot 路径，只接受从已提交索引可达的身份和引用。
- [x] **Step 5 — GREEN。** `cargo test --locked -p viewer-infrastructure --test continuous_review_repository --test review_repository && pnpm architecture:boundaries`。
- [x] **Step 6 — 提交。** `git commit -m "feat(review): atomically commit continuous review snapshots"`。

### Task 8: 持久幂等、故障恢复与资源边界

**Files:** Create `crates/viewer-infrastructure/src/review/continuous/recovery.rs`、`crates/viewer-infrastructure/tests/continuous_review_recovery.rs`。Modify `crates/viewer-application/src/review_workspace/ports.rs`、`crates/viewer-infrastructure/src/review/continuous/mod.rs`、`crates/viewer-infrastructure/src/review/continuous/commit.rs`、`crates/viewer-infrastructure/src/review/continuous/history.rs`。

**Interfaces:** Consumes Task 7。Adds `RecoveryDraft { stream_id, command_id, expected_snapshot_id, payload_digest, editor_input: RecoveryEditorInput, failure: ReviewRecoveryFailure }`；`RecoveryEditorInput { text: String, feedback_id: Option<FeedbackId>, targets: Vec<VersionedTarget>, history_ref: Option<HistoryRef> }`，不是可执行脚本。ReviewRecoveryFailure 为 RenderFailed／SourceChanged／WriteFailed／CommitUnknown／Cancelled／StaleSnapshot。Repository 增加 `save_recovery(&RecoveryDraft) -> Result<(), ReviewCommitError>`、`load_recovery() -> Result<Vec<RecoveryDraft>, ReviewCommitError>`、`resolve_recovery(ReviewCommandId) -> Result<CommandLookup, ReviewCommitError>`。`ReviewCommitFaultPoint` 为 AfterRecovery、AfterEvidence、AfterState、AfterArchive、BeforeIndex、AfterIndex；测试注入器复用现有 FaultInjector 风格，无 UI 开关。

- [x] **Step 1 — RED：提交成功但回执丢失只能产生一次状态。** 构造 Task 7 的有效 request，注入 AfterIndex 错误并重新打开 writer。

```rust
assert!(matches!(writer.commit(request.clone()), Err(ReviewCommitError::OutcomeUnknown)));
let retry = reopened.commit(request).unwrap();
assert_eq!(retry.snapshot, reader.load_current_ref(stream_id).unwrap().unwrap());
```

为端口增加 `load_current_ref(stream_id) -> Result<Option<SnapshotRef>, ReviewCommitError>`；实现读取已固定的索引，不重新编码 State 计算替代摘要。
- [x] **Step 2 — RED。** `cargo test --locked -p viewer-infrastructure --test continuous_review_recovery`。
- [x] **Step 3 — 先查持久命令，再判断新写入。**

```rust
match repository.find_command(stream_id, command_id)? {
    CommandLookup::Found(receipt) if receipt.payload_digest == payload_digest => return Ok(receipt),
    CommandLookup::Found(_) => return Err(ReviewCommitError::CommandConflict),
    CommandLookup::Unavailable => return Err(ReviewCommitError::LookupUnavailable),
    CommandLookup::Absent => {}
}
```

只读取可达且核验过的链，10,000 节点上限同时约束 CPU／IO；循环、缺链或超限不得返回 Absent。已成功命令即使当前已前进，也返回原 receipt；不要求客户端重建曾生成的 snapshot／evidence ID。
- [x] **Step 4 — 遍历全部故障点；测试磁盘写失败、索引同步失败、损坏引用、同 command ID 不同 payload、并发、限制边界与已撤销覆盖再次存档。** 索引前失败保持旧入口；结果不确定则先核验，不新建命令重试。Recovery 不得出现在当前／历史读取结果；本轮不删除任何已提交历史或共享证据。
- [x] **Step 5 — GREEN。** `cargo test --locked -p viewer-infrastructure --test continuous_review_recovery --test continuous_review_repository`。
- [x] **Step 6 — 提交。** `git commit -m "feat(review): recover uncertain commits without duplicate feedback"`。

Task 8 接口细化：RecoveryDraft 显式包含 stream_id，使首次保存（expected_snapshot_id 为空）也能精确查命令。恢复记录采用 Viewer 内部闭合协议 viewer.review.recovery/1，不是 Agent 入口；列表最多 10,000 记录／合计 64 MiB，写前在同一守卫下计算替换后的数量／字节。最多 64 个暂存残留另行有界忽略，不当作草稿，也不自动清理。AfterRecovery 在显式保存输入后注入，提交不伪造编辑器输入。AfterIndex 位于索引 rename 后、目录 fsync 前；失败不回滚，重试核验原 receipt。恢复记录保留，不删除历史或证据。

### Task 9: 首次保存前捕获底图与不可变编号证据

**Files:** Create `crates/viewer-application/src/review_evidence.rs`、`crates/viewer-platform-macos/src/image/review_evidence.rs`、`crates/viewer-infrastructure/src/review/continuous/evidence.rs`。Modify `crates/viewer-application/src/lib.rs`、`crates/viewer-application/src/review_workspace/ports.rs`、`crates/viewer-platform-macos/src/image/mod.rs`、`crates/viewer-platform-macos/src/image/review_annotation.rs`、`crates/viewer-infrastructure/src/review/continuous/mod.rs`。Tests 写在新 macOS 模块的 `#[cfg(test)]` 与 `crates/viewer-infrastructure/tests/continuous_review_repository.rs`。

**Interfaces:** `ReviewEvidencePort` 两个 async 方法：`capture_base(PreparedReviewAsset, ReviewTaskCancellation) -> Result<BoundReviewImage, ReviewArtifactError>`；`render(ReviewEvidenceRequest) -> Result<ReviewEvidenceResult, ReviewArtifactError>`。`BoundReviewImage` 包含不可变 PNG、完整 AssetVersion 绑定、方向／像素映射、摘要；只能由可信平台／仓储适配器验证后提供，不能从 IPC 路径反序列化。`ReviewEvidenceRequest { base: BoundReviewImage, annotations: Vec<NumberedTargetAnnotation>, cancellation }`；annotation 含 ordinal、TargetVersionKey、Anchor。Result 含 Task 7 的文件／引用／编号绑定。历史底图通过仓储核验加载为同一种 BoundReviewImage，不重新读取源图。

Repository 增加 `load_evidence(stream_id, &HistorySelector, AssetVersionId, EvidenceRole)
-> Result<BoundReviewImage, ReviewCommitError>`。`EvidenceRole = Base | Annotated`；HistorySelector
在本任务定义为 Snapshot(SnapshotRef)／Archive(ReviewArchiveId)／Legacy(ReviewRoundId)，Task 11
直接复用。只接受已提交可达记录中该角色的证据，不将源文件路径授权为历史证据。

- [x] **Step 1 — RED：覆写源文件后还能从旧底图重绘。** 用现有 `viewer_test_support::image_fixtures::image_fixture("alpha.png")` 建立临时源文件，capture 后替换其内容，再 render。

```rust
let before_hash = base.blake3();
let result = renderer.render(ReviewEvidenceRequest { base, annotations, cancellation }).await.unwrap();
assert_eq!(result.base_ref.blake3, before_hash);
assert_eq!(result.annotations.len(), 4);
```

`BoundReviewImage::blake3() -> [u8;32]` 是只读访问器；Result 的 base_ref 与 annotations 对应前述 EvidenceRef 和编号绑定。测试使用两笔画、两个矩形，绘制数据不依赖真实图片私密内容。
- [x] **Step 2 — RED。** `cargo test --locked -p viewer-platform-macos review_evidence`。
- [x] **Step 3 — 最小提取已有渲染原语。** 分离安全源解码、不可变底图、标记绘制、PNG 编码；保留每个 marker 的 CGContext 状态隔离。整图意见也在首次成功保存前捕获底图，局部意见额外要求编号预览。

```rust
let base = evidence.capture_base(prepared_asset, cancellation.clone()).await?;
let rendered = evidence.render(ReviewEvidenceRequest { base, annotations, cancellation }).await?;
```

本段调用由 Task 11 用例接入；捕获与保存必须核验 source 指纹、尺寸和方向一致，PreparedReviewAsset 必须是工作台正在评审的版本，不能在保存时重新捕获新版本冒充用户已看到的旧图。只改文字时复用底图；几何／编号改变重新渲染，所有映射固定于同一快照。
- [x] **Step 4 — 加入同 Feedback 多 Target 编号、EXIF 方向、原生四轮廓颜色、source 捕获中变化、取消、PNG／总量超限、legacy 原本缺失与已声明 PNG 损坏的区分测试。** 不将 4 GiB 全部读入内存；逐文件验证。普通缓存淘汰不得删除 repository evidence。
- [x] **Step 5 — GREEN。** `cargo test --locked -p viewer-platform-macos review_ && cargo test --locked -p viewer-infrastructure --test continuous_review_repository`。
- [x] **Step 6 — 提交。** `git commit -m "feat(review): preserve image evidence before source replacement"`。

### Task 10: 动态素材加入、读时核验和显式换版

**Files:** Modify `crates/viewer-application/src/review_assets.rs`、`crates/viewer-infrastructure/src/review/assets.rs`、`crates/viewer-infrastructure/src/review/change_ledger.rs`、`crates/viewer-domain/src/review/continuous/source.rs`、`crates/viewer-domain/src/review/continuous/model.rs`。Create `crates/viewer-infrastructure/tests/continuous_review_assets.rs`。

**Interfaces:** Adds `ContinuousReviewAssetPort`，由 `IndexedReviewAssetCatalog` 实现；async `prepare_additions(&[EntityId], cancellation) -> Result<Vec<PreparedReviewAsset>, ReviewAssetError>`、`check_sources(&[AssetVersion], cancellation) -> Result<Vec<SourceCheck>, ReviewAssetError>`。既有 `ReviewAssetCatalogPort` 的 legacy 方法保持行为。`SourceBindingDecision { target_key, new_asset_version_id, anchor, confirmation }`；confirmation 明确 `UserConfirmed` 或已核验 Producer 关系加用户位置确认，不能由文件名生成。

阶段 A 审阅约束：保存的 `AssetVersion`（包括捕获时路径、实体与 mtime）是不可变事实。
Task 10 须在实时素材目录／独立定位映射中表达已确认的改名和移动，先明确该映射的归属及
校验接口，再实现源读取；不能修改旧捕获记录、只因改名生成新内容版本，或放松内容身份核验。
此定位映射尚未在阶段 A 实现，属于本任务的前置接口细化，不得忽略设计第 7.1 节。

Task 10 实施接口细化：定位映射归 IndexedReviewAssetCatalog 会话所有，按 AssetVersionId
保存显式确认的 entity_id／relative_path；不写入旧 AssetVersion。端口提供 confirm_relocation，
输入包含旧版本、明确候选位置、前一定位（CAS）与 UserConfirmed；再次核验候选实体及完整
BLAKE3 相等后才登记。不搜索同名／同哈希文件自动归并。未确认的移动返回 Unverified。
映射不是当前意见或可执行事实，也不是跨进程持久协议；后续 Application／独立 reader
没有这项已核验定位时必须按历史路径安全核验或返回待确认，不能猜测或假定内存映射存在。
prepare_additions 增量保留跟踪；同实体、同内容及已确认定位复用旧内容版本。显式换版
由独立 SourceBindingDecision 产生新 target revision，不因文件变动自动绑定。

- [x] **Step 1 — RED：评审图 2 不能丢失图 1 的变化跟踪。** 在新测试中准备两个临时素材，分别调用 prepare_additions 后更改第一张，检查两张版本。

```rust
let checks = catalog.check_sources(&[first.asset, second.asset], cancellation).await.unwrap();
assert_eq!(checks.len(), 2);
assert_eq!(checks[0].status, SourceCheckStatus::Changed);
assert_eq!(checks[1].status, SourceCheckStatus::Match);
```

- [x] **Step 2 — RED。** `cargo test --locked -p viewer-infrastructure --test continuous_review_assets`。
- [x] **Step 3 — 实现有界流式核验与增量登记。** 复用安全打开／文件身份前后检查和已有 BLAKE3；新加入合并成员而非 replace_members 清空旧集合。校验不能只依赖 size／mtime；读时结果不写入保存状态。

```rust
let disposition = if observed_hash == captured_hash {
    SourceCheckStatus::Match
} else {
    SourceCheckStatus::Changed
};
```

移动位置与素材业务版本分开：已有可靠实体／事务映射可给出候选定位，重复内容和多候选不得静默确认；用户明确绑定才产生新的目标 revision。未知新文件可独立提出意见。
- [x] **Step 4 — 测试同大小同 mtime 覆写、改名、跨目录移动、两个相同内容文件、多候选、缺失、不可读、检查中再次变化及取消。** 不能自动存档、转移几何或判定解决；源异常保留旧原文与证据。
- [x] **Step 5 — GREEN／检查点 B。** `cargo test --locked -p viewer-infrastructure --test continuous_review_assets --test continuous_review_repository --test continuous_review_recovery && cargo test --locked -p viewer-platform-macos review_`。检查点复审补充：continuous 图片／视频均绕过索引媒体缓存，hash＋probe 跨区间身份核验；图片 raw probe 按 EXIF 归一到证据正向尺寸。1–8／非法方向、真实 catalog→native capture 及视频旧缓存负例均须通过，legacy 契约不变。
- [x] **Step 6 — 提交。** `git commit -m "feat(review): validate source versions without guessing asset lineage"`。

### Task 11: 持续评审 Application 用例

阶段 B 复审后续项：接入用例时量测多存档／长历史的保存与读取成本。目前每次提交会核验全部
已索引 archive／usage，并重复追溯父链；单次 10,000 节点边界不等于已证明交互延迟达标。
若量测需要优化，复用单次操作内的有界验证上下文，不能省略摘要、祖先关系或证据验证，
也不能跨当前索引复用未经核验的缓存。阶段 C 已完成隔离量测和单次操作复用，见阶段记录；
它不是大规模实际 UI 延迟的性能保证。

**Files:** Create `crates/viewer-application/src/review_workspace/service.rs`、`crates/viewer-application/src/review_workspace/editing.rs`、`crates/viewer-application/src/review_workspace/archiving.rs`、`crates/viewer-application/src/review_workspace/history.rs`、`crates/viewer-application/src/review_workspace/projection.rs`、`crates/viewer-application/tests/continuous_review_service.rs`、`crates/viewer-application/tests/support/continuous_review.rs`；`crates/viewer-infrastructure/src/review/continuous/command.rs`。Modify `crates/viewer-application/src/review_workspace/mod.rs`、`crates/viewer-application/src/review_workspace/ports.rs`、`crates/viewer-infrastructure/src/review/continuous/mod.rs`。

**Interfaces:** `ContinuousReviewService::new(repository_provider, asset_port, evidence_port, command_codec, clock)`；依赖分别为 `Arc<dyn ContinuousReviewRepositoryProviderPort>`、`Arc<dyn ContinuousReviewAssetPort>`、`Arc<dyn ReviewEvidencePort>`、`Arc<dyn ReviewCommandCodecPort>`、`Arc<dyn ClockPort>`。async `view(ReviewStreamId) -> Result<ReviewWorkspaceView, ReviewWorkspaceError>`、`prepare(ReviewCommandId, Option<ReviewSnapshotId>, ReviewWorkspaceCommand) -> Result<ReviewCommandEnvelope, ReviewWorkspaceError>`、`apply(ReviewCommandEnvelope) -> Result<ReviewApplyResult, ReviewWorkspaceError>`、`preview_archive(ArchiveSelection) -> Result<ArchivePlan, ReviewWorkspaceError>`、`preview_restore(ReviewArchiveId, Vec<RestoreDecision>) -> Result<RestorePlan, ReviewWorkspaceError>`、`history(HistorySelector) -> Result<HistoryView, ReviewWorkspaceError>`。prepare 纯计算，不保存意见。`ReviewWorkspaceError` 包含 Domain／repository／asset／evidence 错误及 Cancelled、NoChanges、CapabilityUnavailable，不把不确定提交降为普通失败。

`ReviewWorkspaceCommand` 是闭合 enum：SaveFeedback（新增或已有 Feedback ID、文字、目标编辑、生成 ID 集）、Withdraw（目标键）、Archive（已预览 selection）、Restore（archive ID 与 decisions）、ContinueHistorical（HistoryRef、新身份和 SourceBindingDecision）、ConfirmSource（binding）、AdoptUsage（核验过的 declaration ID）、Migrate（Task 13 的选择）。`ReviewCommandEnvelope { command_id, expected_snapshot_id: Option<ReviewSnapshotId>, payload_digest: [u8;32], command }`。新增 ID 在 prepare 中一次生成后包含于 envelope，重试不重新生成。`ReviewCommandCodecPort::digest(&ReviewCommandEnvelope) -> Result<[u8;32], ReviewWorkspaceError>` 由 Infrastructure 实现，对不含 digest 本身的规范化字段计算 BLAKE3；不能信任客户端自报摘要。重复 prepare 同 ID 不提交副作用；UI apply 始终复用首次返回的完整 envelope。

`ReviewWorkspaceView { stream_id, current: Option<StoredContinuousSnapshot>, source_checks, projection, recovery, migration, capabilities }`；`ReviewWorkspacePreview = Archive(ArchivePlan) | Restore(RestorePlan)`；`ReviewApplyResult { receipt, view }`，view 是重新读取的当前状态，可能已比 receipt 前进。`HistoryView { selector: HistorySelector, feedback, assets, evidence, limitations, restore_actions }`；其 role 在 DTO 固定为 history，不接受文件路径。

为保持本任务可独立编译，Task 12 的 UsageImportPreview／UsageImportError 和 Task 13 的
MigrationInspection／MigrationChoice／MigrationPlan／MigrationBinding 数据形状在本任务 ports.rs
声明，12／13 分别实现核验行为。未注入相应能力时返回 CapabilityUnavailable，不伪造成功，
这些入口在 Task 21 前不向普通用户开放。后续模块直接重用类型，不重复声明同名模型。

- [x] **Step 1 — RED：首条意见可读、第二张无需完成前一张。** support 文件实现有界内存 Repository、固定 Clock、可取消 FakeAsset／FakeEvidence 以及真实字段构造器；所有 Fake 都实现上述 port，不生产依赖 Infrastructure。

```rust
let first = service.apply(first_envelope).await.unwrap();
let second = service.apply(second_envelope).await.unwrap();
assert_ne!(first.receipt.snapshot, second.receipt.snapshot);
assert_eq!(second.view.current.as_ref().unwrap().state.feedback.len(), 2);
```

两个 envelope 由 service.prepare 获取；第二次 expected 使用第一次成功 snapshot ID。support fixture 使用固定 UUID、非空文字、两张 AssetVersion、已核验最小 PNG，不虚构完成状态。
- [x] **Step 2 — RED。** `cargo test --locked -p viewer-application --test continuous_review_service`。
- [x] **Step 3 — 编排小用例。** prepare/validate → 保存 recovery → 捕获／复用证据 → Domain 转移 → 最终源核验和 CAS → receipt/view。Service 不亲自序列化 JSON；editing／archiving／history／projection 各自只负责自己的用例。

```rust
let plan = plan_archive(&current.state, &bases, &selection, &coverage)?;
if plan.is_noop() { return Err(ReviewWorkspaceError::NoChanges); }
```

复用 Task 3 的 `is_noop()`；预览后 C 改变返回 StaleSnapshot，并要求新的预览，不自动扩大范围。
- [x] **Step 4 — 测试保存失败不丢输入、空标记不提交、取消只保留 recovery、当前空仍有身份、源变化进入待确认、历史继续提出、部分恢复、过期预览和幂等 receipt 不回退当前。** shared feedback 文字修改必须更新全部目标使用的 textRevision；同目标不能跨两种投影。
- [x] **Step 5 — GREEN。** `cargo test --locked -p viewer-application && cargo test --locked -p viewer-infrastructure --test continuous_review_repository && pnpm architecture:boundaries`。
- [x] **Step 6 — 提交。** `git commit -m "feat(review): coordinate continuous editing and manual archives"`。

### Task 12: 外部使用依据声明的显式导入与核验

**Files:** Create `crates/viewer-application/src/review_workspace/usage.rs`、`crates/viewer-infrastructure/src/review/continuous/usage.rs`、`crates/viewer-infrastructure/tests/continuous_review_usage.rs`。Modify `crates/viewer-application/src/review_workspace/mod.rs`、`crates/viewer-application/src/review_workspace/ports.rs`、`crates/viewer-application/src/review_workspace/service.rs`、`crates/viewer-infrastructure/src/review/continuous/mod.rs`。

**Interfaces:** `UsageImportPort::inspect(&RelativePath) -> Result<UsageImportPreview, UsageImportError>`，preview 含已核验声明、规范摘要、目标核验结果和独立的产出候选状态；错误为 InvalidDeclaration、WrongContext、UnknownBasis、InvalidScope、Conflict、UnsafePath、LimitExceeded、SourceChanged。Service `inspect_usage(path)` 不持久采用；`AdoptUsage` 或 Archive 采用指定 ID 时再次核验固定内容并随原子提交保存。导入句柄绑定会话和内容摘要，不作为任意路径读令牌。

Service 增加 `with_usage_importer(Arc<dyn UsageImportPort>) -> Self` 和 async
`inspect_usage(RelativePath) -> Result<UsageImportPreview, ReviewWorkspaceError>`；原构造器不隐式
扫描文件系统。preview 保存于当前 service 的有界候选集合，按 id＋digest 查找；重启后未采用
声明需要重新显式导入，不能从临时恢复文件猜为已采用。

- [x] **Step 1 — RED：合法声明也不证明已返工。** 临时工程中写 producer 自有 `handoff/usage.json`，其 basis 指向已提交 B，仅包含图 1 的目标键。

```rust
let preview = importer.inspect(&RelativePath::parse("handoff/usage.json").unwrap()).unwrap();
assert_eq!(preview.declaration.targets.len(), 1);
assert_eq!(preview.declaration.basis.snapshot_id, basis_id);
assert!(!project.path().join(".viewer/reviews/usage").exists());
```

测试对象 importer 是本任务的 `ProjectUsageImporter`，构造参数为项目根、项目／Stream 身份和只读 repository；不把声明自由文本作为路径或脚本解释。
- [x] **Step 2 — RED。** `cargo test --locked -p viewer-infrastructure --test continuous_review_usage`。
- [x] **Step 3 — 实现结构和引用核验后再展示。**

```rust
if declaration.project_id != project_id || declaration.stream_id != stream_id {
    return Err(UsageImportError::WrongContext);
}
```

核验 basis 摘要、所有 TargetVersionKey 实际属于 B；同 ID 同规范内容去重，不同内容冲突。输出映射单独核验项目相对路径、hash、旧 asset；映射异常不得变成已确认 lineage，不影响合法依据部分的人工选择。
- [x] **Step 4 — 测试伪造范围、旧 revision、跨项目／Stream、穿越／符号链接、64 MiB 边界、导入后替换、重复 ID、没有 outputs 和错误 outputs。** 无效声明保留错误原因；无需声明也能走 Unknown 手动存档。导入或采用都不自动存档、不执行 Agent。
- [x] **Step 5 — GREEN。** `cargo test --locked -p viewer-infrastructure --test continuous_review_usage && cargo test --locked -p viewer-application --test continuous_review_service`。
- [x] **Step 6 — 提交。** `git commit -m "feat(review): validate optional producer usage declarations"`。

### Task 13: v1/v2 安全迁移与未确认历史候选

**Files:** Create `crates/viewer-application/src/review_workspace/migration.rs`、`crates/viewer-infrastructure/src/review/continuous/migration.rs`、`crates/viewer-infrastructure/tests/continuous_review_migration.rs`。Modify `crates/viewer-application/src/review_workspace/mod.rs`、`crates/viewer-application/src/review_workspace/ports.rs`、`crates/viewer-application/src/review_workspace/service.rs`、`crates/viewer-infrastructure/src/review/continuous/mod.rs`、`crates/viewer-infrastructure/src/review/provider.rs`。

**Interfaces:** 复用 Task 11 ports.rs 中的 `MigrationInspection { legacy_protocol, index_digest, active_draft, completed_candidates, limitations }`；`MigrationChoice = ContinueSelected { legacy_targets: Vec<LegacyTargetRef>, bindings: Vec<MigrationBinding> } | KeepHistoryOnly`。`MigrationBinding { legacy_target, new_asset_version_id, anchor, position_confirmed }`，不能为不存在 v3 ID 的旧目标捏造 TargetVersionKey。`MigrationPlan { inspection_digest, choice }`；inspect 为只读，Migrate 命令携带计划并复查旧 index digest。provider 增加 `inspect_migration() -> Result<Option<MigrationInspection>, ReviewCommitError>`、`migrate(MigrationPlan, ReviewCommandId, [u8;32]) -> Result<ReviewCommitReceipt, ReviewCommitError>`，最后一参是已重算核验的 envelope payload digest；写入复用同一租约和 Task 8 幂等恢复。备份路径 `recovery/legacy-index-{blake3}.json`，不可变，v3 index 保留其摘要引用。读旧索引的 view 由 inspection 生成 migration_required，不先尝试用 v3 decoder 打开 legacy current。

- [x] **Step 1 — RED：只有 Completed 时不能自动变当前要求。** 复制已有 v1 与 v2 fixture 到 TempDir；记住旧文件摘要并执行 KeepHistoryOnly。

```rust
assert!(migrated.current.as_ref().unwrap().state.feedback.is_empty());
assert_eq!(std::fs::read(&old_round_path).unwrap(), old_round_bytes);
assert_eq!(std::fs::read(&old_evidence_path).unwrap(), old_evidence_bytes);
```

有旧历史的迁移生成合法空 current 状态；没有旧评审数据则保持 no_review_state，直到第一条成功保存。历史候选是独立 UI 数据，不塞进 actionable。
- [x] **Step 2 — RED。** `cargo test --locked -p viewer-infrastructure --test continuous_review_migration`。
- [x] **Step 3 — 核验、备份、转换、一次公布。** 活动 Draft 保留原意见身份并补齐新 revision／Target ID；无法核对源或旧缺失证据的限制显式保留。Completed 仅按用户选择建立新身份及 HistoryRef，未选内容仍只在 legacy history。

```rust
let should_activate = matches!(choice, MigrationChoice::ContinueSelected { .. });
```

这里的 `..` 是 Rust 模式语法；完整 choice 字段见 Interfaces。不得把 should_activate 应用于未选的全部 Completed。失败前后旧字节不变，旧索引备份不能被当前读取器当作有效入口。
- [x] **Step 4 — 测试 Draft 有效／源变、仅 Completed、混合 v1/v2、损坏必需证据、半途崩溃、回执丢失、重复迁移、只读工程、未知主版本和已有外部旧副本无法撤销的提示数据。** 不自动迁移，不建立 compatibility latest；旧 writer 读 v3 必须拒绝写。
- [x] **Step 5 — GREEN／检查点 C。** `cargo test --locked -p viewer-infrastructure --test continuous_review_migration --test continuous_review_usage --test review_repository && cargo test --locked -p viewer-application`。
- [x] **Step 6 — 提交。** `git commit -m "feat(review): migrate legacy reviews without reviving old instructions"`。

### Task 14: 原生只读 IO、legacy 兼容与 Node 桥接（2026-08-28 修订）

> 已完成，独立复核通过。首版 `cc7e13f` 及候选 `1dccf78` 未验收；改用已批准的原生架构，不删减安全覆盖。

**Files:** Create `crates/viewer-infrastructure/src/bin/viewer-review-reader.rs`、`crates/viewer-infrastructure/src/review/continuous/reader/{mod,request,project,legacy}.rs`、`scripts/review-protocol/native-reader.mjs`、`scripts/review-protocol/native-reader.test.mjs`。Modify `review/{mod,continuous/mod,continuous/owned_io}.rs`、`scripts/review-protocol/read-latest.mjs`、`package.json`、`scripts/repository-policy.test.mjs`。保留 `blake3.mjs` 及黄金测试供既有 fixture 调用；安全覆盖迁入 Rust 后删除未验收的 `safe-read.mjs`／测试。

**Interfaces:** 内部请求协议 `viewer.review.reader/1`，闭合操作 `legacyLatest`／`legacyList`，共同字段 `projectRoot`，选择字段 `reviewStreamId?` 或成对 `taskId?`／`batchId?`。stdin 最多 64 KiB，stdout 最多 64 MiB；错误为有界 code/message。Node `invokeReader(operation, options)` 通过无 shell 的进程调用发送一次请求。默认二进制位于脚本工作树的 `target/debug/viewer-review-reader`；仅允许调用者用绝对 `VIEWER_REVIEW_READER` 覆盖，不搜索项目/PATH、不自动构建。新 `build:review-reader` 为 `cargo build --locked --offline -p viewer-infrastructure --bin viewer-review-reader`；协议门禁显式先构建再测试。

- [x] **Step 1 — RED：固定 index 与目录句柄。** Rust `owned_io` 单元测试在打开 index 后原子替换，断言返回旧字节；将已打开目录替换为其他目录／符号链接，断言失败且不读新目录。补 descriptor 枚举、大小写别名、硬链接、FIFO、目录与预分配大小边界。真实 TempDir + 文件句柄，不能仅 mock stat。
- [x] **Step 2 — RED。** `cargo test --locked -p viewer-infrastructure review::continuous::owned_io`；Node 加实际二进制入口测试，请求未知字段／超限、缺二进制、相对 override 和 legacy 输出／CLI 失败方式。
- [x] **Step 3 — 实现共享只读访问与 native legacy。** 逐级目录句柄访问与枚举；单独的 pinned-index 读取容忍正常 unlink，其他对象仍严格核验。复用 Rust legacy decoder，v2 必需 PNG 逐块摘要并校验包内精确文件集合；返回核验后的原始 JSON，保持旧字段可选性。核心没有写仓储／迁移／Viewer 启动入口。
- [x] **Step 4 — 兼容与范围测试。** 现有 v1/v2 正负例全部通过；大小写、穿越、链接、非普通文件、损坏 PNG、总量上限和并发保存覆盖不撤销。源检查的 64 KiB 固定缓冲测试迁至 Task 15 Rust source 模块，旧 Node 实现不再充当安全边界。
- [x] **Step 5 — GREEN。** `pnpm test:review-protocol && cargo test --locked -p viewer-infrastructure`；精确同步 policy 的脚本清单，不放宽边界规则。
- [x] **Step 6 — 提交检查点。** 与 Task 15 共用原生入口和 DTO，以一个可构建实现 `7863439` 保存，再提交审阅修正 `76726a1`；不合并／推送／接 UI。

### Task 15: 当前／历史读取入口与可选精确增量

> 已完成，独立复核通过；与 Task 14 共享可构建提交，各独立边界的 RED→GREEN 证据保留。

**Files:** Create `crates/viewer-infrastructure/src/review/continuous/reader/{current,history,delta,source}.rs`、`scripts/review-protocol/read-current.mjs`、`scripts/review-protocol/read-history.mjs`。已有 RED `read-current.test.mjs`／`read-history.test.mjs` 纳入门禁。Modify `reader/{mod,request,project}.rs`、`continuous/history.rs`、`protocol/v3/read_result.rs`、`package.json`、`tests/review_protocol_v3_contract.rs`。不新增第二套 JS v3 校验器。

**Interfaces:** `readCurrentReview({ projectRoot, reviewStreamId?, taskId?, batchId?, sinceSnapshotId? }) -> Promise<ReviewReadResult>`；`listCurrentReviewStreams({projectRoot})`；`readReviewHistory({projectRoot, reviewStreamId, snapshotId?, archiveId?, legacyRoundId?}) -> Promise<ReviewReadResult>`，history 三个 selector 恰好一个。当前 CLI 为 `--project`、可选 `--stream` 或 `--task`／`--batch`、`--since`、`--list`；历史 CLI 必须指定 `--stream` 和一个 `--snapshot`／`--archive`／`--legacy-round`，没有隐式 latest。CLI 成功输出 JSON 到 stdout，错误 JSON 到 stderr 且非零退出。

`ReviewReadResult` 使用闭合判别联合：`ok/current` 含固定 snapshotRef、完整原文目标数据、sourceChecks、actionable、needsConfirmation、historyRefs、delta；`ok/history` 为不可执行历史内容及 evidence，不含 actionable；`no_review_state/current` 无快照；`error` 有 code/message，无成功清单。`migration_required` 是 error code。delta 为 `not_requested`／`unavailable { reason }`／`available { sinceSnapshotId, currentSnapshotId, targets }`；未知／跨上下文／超限 since 不破坏已核验的当前完整结果。

库入口将预期的协议／IO 失败转换为 error result，CLI 根据 status 决定退出码；内部安全 IO
可抛错误，但不能让 public reader 在同类失败上有时返回空 current、有时抛无类型异常。

- [x] **Step 1 — RED：空当前不回退旧存档。** 调用 `createProjectV3Case('current_empty')`，取得独占 projectRoot／streamId；用 try/finally 调用 cleanup。

```js
const result = await readCurrentReview({ projectRoot, reviewStreamId })
assert.equal(result.status, 'ok')
assert.equal(result.role, 'current')
assert.deepEqual(result.actionable, [])
assert.deepEqual(result.needsConfirmation, [])
assert.equal(Object.hasOwn(result, 'outcomes'), false)
```

- [x] **Step 2 — RED。** `node --test scripts/review-protocol/read-current.test.mjs scripts/review-protocol/read-history.test.mjs`。
- [x] **Step 3 — 固定一个 Rust View 后完整核验。** 通过共享 `history::read_state`／引用核验／v3 codec 加载，不能调用每次重开 index 的多次 repository 方法拼结果。`current`／`history`／`list` 为新增闭合请求操作。`CurrentReadResult` 等增加 crate 内构造器并调用既有编码校验；当前投影和净增量分别复用 Domain `project_current`／`diff_review`。source 以根目录句柄逐级打开，64 KiB 缓冲完整 hash，前后身份和重新打开核对；源失败映射 pending，不更改已保存状态。

历史 snapshot 必须可达；archive 按已验证依据分组、保持 selected keys；legacy 仅显式 round。历史结果保留字节累计最多 64 MiB。增量沿固定链最多 10,000 节点，累计 transition 内存最多 64 MiB，逐个读取而非留存所有状态；无法完整证明时返回 unavailable，不破坏已验证 current。未建索引时验证现有 `.viewer/project.json`，身份缺失返回 typed IO error，不生成新 UUID。
- [x] **Step 4 — 测试缺索引／空 current／仅 pending／损坏必需证据、同快照读时源变化、正在保存、未知 since、跨项目／Stream、10,000 节点边界、循环、withdrawn／archived 原因、legacy 无证据限制及未知主版本。** read-latest 遇 v3 明确拒绝并提示新入口；history 永不变成当前 Agent 指令。
- [x] **Step 5 — GREEN。** `pnpm test:review-protocol && cargo test --locked -p viewer-infrastructure --test review_protocol_v3_contract`；新 Node tests 加入门禁，记录 Rust 写入→Node 读取字节互通结果。
- [x] **Step 6 — 提交。** 可构建实现 `7863439`，审阅修正 `76726a1`；完整验证见[阶段 D 读取器记录](../../reviews/2026-08-28-continuous-review-reader-phase-d.md)。

### Task 16: 桌面命令、DTO 和历史证据授权

> 2026-08-28：接口核对和[契约补充](../../progress/2026-08-28-continuous-review-task16-contract-checkpoint.md)
> 已获用户确认。下列接口替代原来的九命令／四字段示例；实现和审阅修正已通过全仓门禁及独立复验，Task 16 验收完成。

**Files:** Create `src-tauri/src/commands/review_workspace.rs`、`src-tauri/src/dto/review_workspace.rs`、`src-tauri/src/state/review_workspace.rs`、`ui/src/api/reviewWorkspaceTypes.ts`。Modify `src-tauri/src/commands/mod.rs`、`src-tauri/src/dto/mod.rs`、`src-tauri/src/state/mod.rs`、`src-tauri/src/state/session.rs`、`src-tauri/src/lib.rs`、`src-tauri/src/image_protocol.rs`、`crates/viewer-infrastructure/src/image_cache.rs`、`ui/src/api/viewer.ts`、`ui/src/app/workspace/ports.ts`。Tests 随新 Rust 模块；Create `ui/src/api/reviewWorkspaceTypes.test.ts`。

**Interfaces:** Desktop commands：`get_review_workspace`、`prepare_review_assets`、`prepare_review_command`、`apply_review_command`、`preview_review_archive`、`preview_review_restore`、`get_review_history`、`inspect_review_usage`、`inspect_review_migration`、`get_review_evidence`、`cancel_review_workspace_task`。全部携带 sessionId 和 generation；写命令使用 Task 11 完整 envelope；prepared digest 在 apply 重新核验。`prepare_review_assets` 仅接受当前索引的 entityIds，返回 AssetVersion 及该版本的受会话授权预览；未经预绑定不可首次保存。`get_review_evidence` 只接受已提交 selector、AssetVersionId 和 Base／Annotated 角色，返回 session 绑定图片 URL 与尺寸，不接收本地路径。

TypeScript `ReviewWorkspacePort` 对应方法为 getWorkspace／prepareAssets／prepareCommand／applyCommand／previewArchive／previewRestore／getHistory／inspectUsage／inspectMigration／getEvidence／cancelTask，返回 typed Promise，DTO 与 Rust 使用相同判别字段；可沿用 UUID string，但不混淆 snapshot ID 与 legacy round ID。新增 registry 函数 `register_review_png(&ImageArtifactRegistry, SessionId, EntityId, &BoundReviewImage) -> Result<ImageArtifactToken, ImageArtifactRegistryError>`，使用既有 token／error 类型；已验证 immutable_bytes 通道不放开任意路径和 generic fs capability。DTO 按命令、视图、历史／迁移和校验拆分，Application 不依赖 serde；后端只读选择唯一人工 Stream，旧 UI 不切换。

- [x] **Step 1 — RED：关闭会话后历史 token 失效。** 在 image_protocol 模块测试中建立 ActiveImageSession，登记核验过的 PNG，关闭后用原 URL 请求。

```rust
let active = ActiveImageSession::default();
active.set(Some(session_id));
let token = register_review_png(&registry, session_id, entity_id, &verified).unwrap();
let resolver = ImageProtocolResolver::new(active.clone(), registry);
let request_path = format!("/{session_id}/{}", token.as_str());
assert!(resolver.resolve(&request_path).is_ok());
active.set(None);
assert_eq!(resolver.resolve(&request_path), Err(ProtocolError::Forbidden));
```

registry 为 Arc<ImageArtifactRegistry>；verified 来自 Task 9 仓储 load_evidence，session_id／entity_id 是固定测试 ID。resolver 接受 URI path 而非完整 URL；不能为配合测试扩展公网／文件权限。
- [x] **Step 2 — RED。** `cargo test --locked -p viewer-desktop review_workspace && cargo test --locked -p viewer-desktop image_protocol`；包名已由 `src-tauri/Cargo.toml` 核对。证据、DTO、预绑定和运行集成均保留 RED→GREEN 日志。
- [x] **Step 3 — 注入新 service 并保持边界。** 当前 active generation 与 cancellation 贯穿 prepare／render／commit／preview，切工程后旧任务不能向新会话发结果。旧流程仅处理 legacy，v3 的编辑不进入 CompletedReadOnly。

```ts
export interface PreparedReviewCommand {
  context: ReviewWorkspaceContext
  commandId: string
  expectedSnapshotId: string | null
  payloadDigest: string
  generated: GeneratedReviewIds
  usageSelections: PreparedUsageSelection[]
  command: ReviewWorkspaceCommand
}
```

此 DTO 无损对应 ReviewCommandEnvelope（包括迁移生成身份与声明来源）；内部 bytes 使用严格 64 小写 hex 传输。prepare 仅生成／规范化并返回 envelope，UI 不需要新 hash 运行库；apply 验证后端固定 context、session、digest、expected 和命令目标。已提交但刷新失败保留 receipt。关闭先撤销授权、取消全部工作，再等待任务退出后清理；普通 cancel 仅取消当前已登记任务，不毒化下一次调用。cancellation 传到素材准备、捕获、渲染、保存后 view；await 后重新检查 generation，旧回复不注册新 token。
- [x] **Step 4 — 加跨会话／过期 generation、伪造 evidence 路径／角色／摘要、关闭中渲染、取消、闭合 DTO 拒绝额外字段与旧 API mock 回归。** 预绑定当前源与按 selector 获取的历史证据分别带对应 AssetVersionId。补齐首保存失败重开恢复、排队取消仍查回执、响应编码预算和旧视频数值边界回归。
- [x] **Step 5 — GREEN／检查点 D。** 新 Rust 模块测试、`pnpm --dir ui typecheck`、`pnpm architecture:boundaries`、`pnpm test:review-protocol` 通过；修正提交 `384d4cc` 的完整 `pnpm verify:clean` exit 0，独立复验确认三项 Important 和一项 Minor 已解决。11 命令与 port 对照见桥接记录；趋势仍 48 项非阻断告警，未调基线。
- [x] **Step 6 — 提交。** 实现 `db456ef`（`feat(review): bridge continuous workflows with session-safe evidence`）；审阅修正 `384d4cc`。仅隔离分支，未合并或推送。

### Task 17: UI 持续评审协调器与输入保留

**Files:** Create `ui/src/app/review/continuousReviewModel.ts`、`ui/src/app/review/continuousReviewModel.test.ts`、`ui/src/app/review/useContinuousReviewCoordinator.ts`、`ui/src/app/review/useContinuousReviewCoordinator.test.tsx`。Modify `ui/src/app/workspace/ports.ts`。

实施细化：会话/写入生命周期与具名用例分别放入 `continuousReviewSession.ts` 和
`continuousReviewActions.ts`，测试按生命周期/预览/其他用例/守卫拆分，并共享测试夹具。
Task 16 已完成 ports.ts 的独立类型导出，本任务不提前修改旧 WorkspacePorts 工厂。
补充暴露现有端口的素材预绑定、适用性确认和 legacy 继续提出；输入增加显式刷新/版本依据
确认入口，刷新不自动重新绑定输入。细节和证据见[Task 17 记录](../../reviews/2026-08-28-continuous-review-coordinator-task17.md)。

**Interfaces:** `useContinuousReviewCoordinator({ sessionId, generation, port, onError }) -> ContinuousReviewCoordinator`；state 为 loading／ready／saving／save_failed／recovery_required／migration_required／unavailable 的闭合联合。Coordinator 暴露 view、editorInput、pendingEnvelope、saveFeedback、withdrawTargets、previewArchive、commitArchive、previewRestore、restore、continueHistorical、confirmSource、inspectUsage、adoptUsage、inspectMigration、migrate、getHistory、getEvidence、retry、cancel。所有 async 方法返回 Promise<void> 或对应 preview，交由明确错误状态表达失败，不能吞掉 reject。未提交输入与最近成功 snapshot 独立。

- [x] **Step 1 — RED：回执丢失时重试同一 envelope，失败不清空原文。** 用 React Testing Library renderHook 和实现 Task 16 全部方法的 vi.fn port；第一 apply 拒绝 OutcomeUnknown，第二返回已提交 receipt。

```ts
expect(result.current.editorInput.text).toBe('袖口收紧，保留褶皱')
await act(() => result.current.retry())
expect(port.applyCommand.mock.calls[1]?.[0]).toEqual(port.applyCommand.mock.calls[0]?.[0])
```

这里 applyCommand 参数固定为 `{ sessionId, generation, envelope }`；prepareCommand 参数固定为 `{ sessionId, generation, commandId, expectedSnapshotId, command }`，遵循 Task 16 已确认的会话契约，不再为同一 retry 调用 prepare 生成新 ID。
- [x] **Step 2 — RED。** `pnpm --dir ui test src/app/review/useContinuousReviewCoordinator.test.tsx src/app/review/continuousReviewModel.test.ts`。分批 RED→GREEN 日志见协调器记录。
- [x] **Step 3 — 串行化语义命令并隔离会话 epoch。** 保存前冻结输入并 prepare；失败保留 input／envelope；成功清理的只能是对应输入版本，不清掉保存过程中用户新打的字。

```ts
if (replyEpoch !== activeEpoch.current) return
setView(reply.view)
if (savedInputRevision === inputRevision.current) clearEditorInput()
```

`inputRevision` 是仅 UI 编辑保护计数；持久外部版本仍是 snapshot ID。setView 用 apply 返回的重新读取 view，不能将历史 receipt.snapshot 当当前 head。
- [x] **Step 4 — 测试双击保存／存档、乱序响应、切图有脏输入、切项目／关闭、StaleSnapshot 要重预览、只读项目、当前空和迁移状态。** 并发任务不能用新 ID 自动重发失败命令；用户明确改写载荷才新建操作。复核后补充刷新与重试互斥、重试/迁移恢复守卫、预览本地拒绝及订阅重入的失效回归。
- [x] **Step 5 — GREEN。** 评审目录 11 文件 / 90 项通过（本次新增 65），UI check、完整 `pnpm verify:clean` exit 0；UI 全仓 1209 通过 + 1 个既有跳过。独立复验确认三项 Important 全部解决，无剩余阻断。趋势 49 项非阻断告警，未改基线。
- [x] **Step 6 — 提交。** 实现 `da1dcff`（`feat(review): preserve edits across continuous save and retry`）；复核修正 `d69a36a`。仅隔离分支本地提交，未合并或推送。

### Task 18: 大图工作台接入持续编辑与动态素材范围

**Files:** Modify `ui/src/app/review/useImageReviewWorkbench.ts`、`ui/src/app/review/useImageReviewWorkbench.test.tsx`、`ui/src/app/workspace/WorkspaceImageReviewPreview.tsx`、`ui/src/components/review/ImageReviewWorkspace.tsx`、`ui/src/components/review/ImageReviewWorkspace.test.tsx`、`ui/src/components/review/ReviewFeedbackRail.tsx`、`ui/src/components/review/ReviewFeedbackRail.test.tsx`、`ui/src/components/review/AnnotationToolbar.tsx`、`ui/src/components/review/ReviewContextBar.tsx`、`ui/src/components/review/ImageReviewWorkbench.integration.test.tsx`。

**Interfaces:** workbench 消费 Task 17 coordinator 的窄 adapter，不直接 import viewer bridge；编辑对象带 Feedback ID 和 Target ID，显示编号独立。工具禁用原因只允许 loading／saving冲突／源待确认／只读目录／recovery_required，不含“已完成本轮”。preview resolver 对 v3 新图返回可新建意见的 workbench，而非 outside_scope。legacy 明确保留迁移入口，不假装已采用新协议。

- [x] **Step 1 — RED：图 1 保存后打开图 2 仍能画框评审。** 在现有 integration fixture 中注入 v3 coordinator，依次绘制矩形、填文字、保存、导航，再点击画笔。

```tsx
expect(screen.getByRole('button', { name: '画笔' })).toBeEnabled()
expect(screen.getByRole('button', { name: '矩形' })).toBeEnabled()
expect(screen.queryByRole('button', { name: '完成本轮评审' })).not.toBeInTheDocument()
```

- [x] **Step 2 — RED。** `pnpm --dir ui test src/components/review/ImageReviewWorkbench.integration.test.tsx src/app/review/useImageReviewWorkbench.test.tsx`。
- [x] **Step 3 — 接入成功保存／失败／待确认提示。** 保留画笔、矩形、整图意见、就地文字编辑、重绘、删除和脏输入离开确认。v3 工具栏显示“存档意见”“历史”，当前意见成功保存提示“已保存，可供外部读取”，不能声称 Agent 已读。

```tsx
<ViewerButton onClick={onArchive} disabled={hasUncommittedInput}>存档意见</ViewerButton>
```

onArchive 由父层注入 Task 19 的预览入口；未接通前仅用于组件测试载体。通用 ImagePreviewSurface／几何投影不新增协议依赖；网格只浏览不创建反馈。
- [x] **Step 4 — 测试四处标注、编辑／重绘／删除后编号同步、批量共同意见、缩放／旋转／适应窗口／导航、键盘操作及 1024×720 紧凑布局。** 工具启用时核对预览表示与已准备素材版本，不一致要求刷新；画完再发生同路径覆写时保存不能静默重绑。存档历史只读不会禁用当前新意见工具；删除最后一条不显示“全部通过”。
- [x] **Step 5 — GREEN。** 评审目录 18 文件 / 157 项通过；UI 全仓 140 文件 / 1235 通过 + 1 个既有跳过；UI check、完整 `pnpm verify:clean`、架构边界／contracts 通过。独立复验确认所有 Important 和 Minor 已解决，无剩余阻断；趋势 48 项非阻断告警，未改基线。
- [x] **Step 6 — 提交。** `c6d5d11`（`feat(review): enable continuous feedback in the image workbench`）。仅隔离分支，未合并或推送。

### Task 19: 手动存档预览、部分范围与后补保护 UI

**Files:** Create `ui/src/components/review/ReviewArchiveDialog.tsx`、`ui/src/components/review/ReviewArchiveDialog.test.tsx`。Modify `ui/src/components/review/ReviewWorkspaceLayer.tsx`、`ui/src/components/review/ReviewContextBar.tsx`、`ui/src/app/review/useContinuousReviewCoordinator.ts`、`ui/src/app/review/useContinuousReviewCoordinator.test.tsx`、`ui/src/styles/review.css`。

**Interfaces:** `ReviewArchiveDialogProps { preview: ArchivePlanDto, selection: ArchiveSelectionDto, busy, error, onSelectionChange, onConfirm, onCancel }`；两 DTO 对应 Task 3 原字段。依据显示“已核对交接版本”或“交接版本未确认”；默认不猜 Agent 实际读取过哪个技术版本。明确列出将移入历史、保留的后补意见、未选素材，绑定 expectedSnapshotId。

- [x] **Step 1 — RED：图 1 存档而图 2 和后补保留。** 用 B/C fixture 渲染预览；选择图 1 的一个目标，另一个共享目标仍属于图 2。

```tsx
expect(screen.getByText('交接版本未确认')).toBeVisible()
expect(screen.getByText('保留后补意见')).toBeVisible()
expect(screen.getByRole('button', { name: '确认存档' })).toBeEnabled()
```

- [x] **Step 2 — RED。** `pnpm --dir ui test src/components/review/ReviewArchiveDialog.test.tsx`；组件缺失、空范围可确认、错误 disposition、已覆盖 no-op、已知依据被替换、跨会话旧结果及最终取消选择等均留下预期失败证据。
- [x] **Step 3 — 展示版本差集，确认只提交固定 selection。** 不把“整批”解释成自动包含 future additions；编辑中需先保存或取消输入，不能把 recovery 当已提交依据。

```ts
if (currentSnapshotId !== preview.expectedSnapshotId) {
  setError('意见已变化，请重新查看存档范围')
  return
}
```

后端仍是最终版本守卫；前端守卫只是及时反馈。空范围／已有效覆盖不创建空存档；存档成功后提示可撤销，不显示“已修复”。
- [x] **Step 4 — 测试有效声明预选、无声明、手动选 B、预览后新增／删除／重绘、重复点击、取消、保存失败、共同目标拆分及误选恢复入口。** Unknown 始终有清晰确认，不用虚构“最新已处理版本”。补充覆盖 selection-only 已覆盖目标、空选择重新勾选、会话替换和旧异步结果失效；集成断言放入既有 `reviewComponents.test.tsx`。
- [x] **Step 5 — GREEN。** 聚焦 3 文件 / 37 项、UI 全仓 141 文件 / 1251 项通过 + 1 项既有跳过；UI check/build、架构边界、完整 `pnpm verify:clean` exit 0。趋势 47 项非阻断既有分支告警；本任务曾新增的 `ReviewWorkspaceLayer` 长函数／决策复杂度告警已通过职责拆分消除，未改趋势基线。
- [x] **Step 6 — 提交。** `d869049`（`feat(review): preview exact manual archive scope`）；修正 `9a722dc`、`a3305bd`、`e96831d`、`4537cba`。仅隔离分支，未合并或推送。

### Task 20: 历史查看、选择恢复与素材适用性确认

**Files:** Create `ui/src/components/review/ReviewHistoryPanel.tsx`、`ui/src/components/review/ReviewHistoryPanel.test.tsx`、`ui/src/components/review/ReviewSourceConfirmation.tsx`、`ui/src/components/review/ReviewSourceConfirmation.test.tsx`。Modify `ui/src/components/review/ReviewWorkspaceLayer.tsx`、`ui/src/app/review/useImageReviewWorkbench.ts`、`ui/src/app/review/useContinuousReviewCoordinator.ts`、`ui/src/styles/review.css`。

**Interfaces:** HistoryPanel 消费 typed `HistoryViewDto`（selector、role=history、旧原文、证据能力、目标引用、可恢复操作）和 `onContinue(HistoryRef)`／`onRestore(RestoreDecision[])`；SourceConfirmation 消费旧 AssetVersion、核验后的候选和原 Anchor，输出 `SourceBindingDecisionDto`，必须明确选择目标并确认位置。历史预览 URL 只来自 getEvidence，不拼 `.viewer` 路径。

- [ ] **Step 1 — RED：恢复不能静默覆盖已有修改。** 以同 Target ID 的旧文字和新文字渲染恢复冲突。

```tsx
expect(screen.getByText('保留当前意见')).toBeVisible()
expect(screen.getByText('使用历史意见')).toBeVisible()
expect(screen.getByText('作为新意见继续提出')).toBeVisible()
expect(screen.getByRole('button', { name: '确认恢复' })).toBeDisabled()
```

- [ ] **Step 2 — RED。** `pnpm --dir ui test src/components/review/ReviewHistoryPanel.test.tsx src/components/review/ReviewSourceConfirmation.test.tsx`。
- [ ] **Step 3 — 接通精确恢复／继续提出。** 无后续修改也追加恢复状态，不回写历史 JSON；后续已有变化逐项选择。历史定位基于旧证据，换新素材时保留原文并要求用户确认 Anchor。

```ts
const canConfirm = selectedAssetVersionId !== null && positionConfirmed && unresolvedConflicts === 0
```

位置确认不等于意见已执行；源文件 Missing 时仍能看旧证据，必需证据损坏则显示完整性错误，不假装空历史。legacy 缺失则显示能力限制。
- [ ] **Step 4 — 测试立即撤销、部分恢复、已恢复重复动作、历史继续提出使用新 ID、同路径新图／歧义候选／独立新图、过期 source confirmation、关闭会话 token 失效和重启恢复。** 不生成自动“改回原色”等反向意见。
- [ ] **Step 5 — GREEN。** `pnpm --dir ui test src/components/review/ReviewHistoryPanel.test.tsx src/components/review/ReviewSourceConfirmation.test.tsx src/app/review/useContinuousReviewCoordinator.test.tsx`。
- [ ] **Step 6 — 提交。** `git commit -m "feat(review): restore history with explicit source confirmation"`。

### Task 21: 可选声明／迁移入口与完整流程切换

**Files:** Create `ui/src/components/review/ReviewUsageImport.tsx`、`ui/src/components/review/ReviewUsageImport.test.tsx`、`ui/src/components/review/ReviewMigrationDialog.tsx`、`ui/src/components/review/ReviewMigrationDialog.test.tsx`。Modify `ui/src/components/review/ReviewWorkspaceLayer.tsx`、`ui/src/components/review/ReviewRecoveryNotice.tsx`、`ui/src/app/workspace/WorkspaceImageReviewPreview.tsx`、`ui/src/App.tsx`、`src-tauri/src/state/review_workspace.rs`、`src-tauri/src/commands/review_workspace.rs`、`ui/src/api/viewer.ts`、`ui/src/styles/review.css`。

**Interfaces:** UsageImport 输出用户显式选择的项目内声明相对路径，原生选择器使用既有 dialog 能力；桌面命令核验位于当前项目且不在 `.viewer` 内，UI 不自行读取文件。MigrationDialog 展示 MigrationInspection，明确“继续选中意见”或“仅保留历史”，回传 Task 13 的 MigrationPlan。Root 依据已核验 protocol/capabilities 选择 coordinator；不能因 UI flag 而跳过迁移确认。

- [ ] **Step 1 — RED：拒绝声明后仍能无声明存档。** 使用错误项目声明触发错误，关导入面板再进入存档预览。

```tsx
expect(screen.getByText('声明不属于当前项目')).toBeVisible()
fireEvent.click(screen.getByRole('button', { name: '不使用声明' }))
expect(screen.getByText('交接版本未确认')).toBeVisible()
```

- [ ] **Step 2 — RED。** `pnpm --dir ui test src/components/review/ReviewUsageImport.test.tsx src/components/review/ReviewMigrationDialog.test.tsx`。
- [ ] **Step 3 — 在阶段 E 所有测试通过后启用新流程。** 新项目第一次成功保存创建 v3；旧项目明确提示迁移，取消保持旧数据。历史 Completed 不自动全选成新要求；活动 Draft 的迁入结果和待确认原因可见。

```tsx
if (view.migration !== null) return <ReviewMigrationDialog inspection={view.migration} onConfirm={migrate} onCancel={closeMigration} />
```

MigrationDialogProps 在本任务定义为 `{ inspection, onConfirm(MigrationPlanDto), onCancel() }`；选择保存于面板内部。新流程不再渲染完成／放弃轮次入口；legacy adapter 暂留供历史和兼容测试，不做无关大规模删除。
- [ ] **Step 4 — 测试不自动扫描 Downloads、不修改 production manifest、导入取消、未知主版本、只读目录、迁移中断、旧软件拒写及新旧项目切换。** 用户不提供声明也可完成完整多轮流程；不增加 Agent 连接配置步骤。
- [ ] **Step 5 — GREEN／检查点 E。** `pnpm --dir ui test && pnpm --dir ui check && pnpm --dir ui build && pnpm architecture:boundaries`，并执行 Rust migration／usage／desktop 聚焦测试。再用隔离工程检查新入口，不能拿用户原工程做首次迁移实验。
- [ ] **Step 6 — 提交。** `git commit -m "feat(review): activate continuous review with explicit migration"`。

### Task 22: Rust→Node 多轮闭环与真实工作台验收

**Files:** Create `src-tauri/examples/continuous_review_harness.rs`、`scripts/review-protocol/continuous-review-e2e.mjs`、`scripts/review-protocol/continuous-review-e2e.test.mjs`、`ui/src/acceptance/scenes/continuousReviewScenes.tsx`、`ui/src/acceptance/scenes/continuousReviewScenes.test.tsx`。Modify `package.json`、`ui/src/acceptance/acceptanceStateCatalog.json`、`ui/src/acceptance/scenes/index.ts`、`ui/src/acceptance/acceptanceBridge.ts`、`ui/src/acceptance/acceptanceFixtures.ts`、`scripts/viewer-visual-acceptance.mjs`、`scripts/viewer-visual-acceptance.test.mjs`。

**Interfaces:** Rust harness 使用真实 ContinuousReviewService／Repository／Mac Evidence，命令 `cargo run --quiet --locked -p viewer-desktop --example continuous_review_harness -- --project <owned-temp-project> --scenario <scenario>`。scenario 闭合集为 `two_iterations`、`partial_shared`、`source_replaced`、`unknown_basis`、`restore_conflict`、`lost_receipt`、`migration`。输出 JSON 只含身份、摘要、检查结论，不含用户绝对路径或假造 Agent 执行事实。Node `createContinuousReviewScenario(name) -> Promise<{projectRoot, run(), cleanup()}>` 使用 mkdtemp 复制现有 fixture，验证所有返回内容，然后调用独立读取器。

验收载体新增 RVW-25 持续编辑／新图、26 部分存档预览、27 后补保留、28 旧证据与换版确认、29 恢复冲突、30 无声明存档、31 旧数据迁移、32 空当前有历史；复用真实组件与 typed port，不绘制截图替身。既有 RVW-17 四标注／24 缩放继续回归；RVW-21 明确成为 legacy 固定范围场景，不用其旧文案证明 v3 行为。

- [ ] **Step 1 — RED：交接后补意见不能被历史覆盖。** harness 建立 B，读取 B 后模拟用户改为 C，覆写返回图 1，按 B 只存档图 1，再读当前与历史。

```js
assert.equal(current.snapshotRef.snapshotId, result.currentSnapshotId)
assert.equal(history.role, 'history')
assert.equal(current.feedback.find(item => item.id === result.editedFeedbackId).text, '袖口收紧，保留褶皱')
assert.equal(history.feedback.find(item => item.id === result.editedFeedbackId).text, '袖口收紧')
```

`current.feedback` 是完整保存内容；actionable 另由 sourceChecks 决定，测试不能因为 source 已覆写就丢弃仍在待确认的后补原文。
- [ ] **Step 2 — RED。** `node --test scripts/review-protocol/continuous-review-e2e.test.mjs` 和 `pnpm --dir ui test src/acceptance/scenes/continuousReviewScenes.test.tsx`。
- [ ] **Step 3 — 串接真实写入、独立读取与场景适配。** 两轮均走保存→读取→外部文件变化→手动存档→新意见；不让 Node reader 写回执，外部声明文件由测试 producer 显式创建。

```js
const current = await readCurrentReview({ projectRoot, reviewStreamId: result.streamId })
const history = await readReviewHistory({ projectRoot, reviewStreamId: result.streamId, archiveId: result.archiveId })
```

把新 e2e 加入 test:review-loop；每个场景结束前校验原 fixture 无变化。临时目录只按创建时返回的精确所有权路径清理，不操作用户“下载/测试图”原目录。
- [ ] **Step 4 — 运行全部 scenario，捕获 RVW-17／24–32 的 1024×720 与宽窗口截图。** `pnpm accept:visual --id RVW-17 --id RVW-24 --id RVW-25 --id RVW-26 --id RVW-27 --id RVW-28 --id RVW-29 --id RVW-30 --id RVW-31 --id RVW-32 --output-root target/viewer-visual-acceptance/continuous-review`。随后用用户“下载/测试图”的已核对受控副本做原生四处标注、保存、重启、换版、存档、再编辑；记录精确命令、截图路径与结果。需要操作本机 UI 时先按 computer-use 技能读取并执行其约束。
- [ ] **Step 5 — GREEN。** `pnpm test:review-protocol && pnpm test:review-loop && pnpm test:visual-acceptance && pnpm --dir ui test src/acceptance`；截图逐张检查文字、按钮可点、编号／几何一致，原生验证不能仅由浏览器截图代替。
- [ ] **Step 6 — 提交。** `git commit -m "test(review): cover continuous review and agent handoff end to end"`。

### Task 23: 旧功能回归、产品事实同步与最终门禁

**Files:** Modify `docs/PRODUCT_SPEC.md`、`docs/product/README.md`、`docs/product/USER_GUIDE.md`、`docs/product/FEATURE_REFERENCE.md`、`docs/product/SHORTCUTS.md`、`docs/product/DATA_PRIVACY.md`、`docs/product/TROUBLESHOOTING.md`、`docs/protocol/README.md`、`docs/architecture/viewer-0.1-api-baseline.md`、`docs/adr/0006-continuous-review-snapshot-and-archive-protocol.md`、`docs/README.md`、`CHANGELOG.md`；按已落地边界更新本计划、对应设计，以及 `docs/superpowers/specs/2026-08-25-viewer-exception-driven-review-loop-design.md`、`docs/superpowers/specs/2026-08-26-viewer-image-annotation-review-workbench-design.md`。Create `docs/quality/2026-08-27-continuous-review-validation.md`。Tests 沿用本计划新增测试与既有门禁，不为文案另建无用框架。

**Interfaces:** 没有新运行时 API。产物为准确的 Current 产品／协议说明、部分替代关系和可复核验证报告。报告必须区分已执行／失败／未执行，不以计划勾选代替日志；公开 Rust API baseline 仅记录真正新增的端口，不放宽冻结的文件／网络安全边界。

- [ ] **Step 1 — 核对闭环验收与旧功能清单。** 逐项勾核本计划第 4 节；运行已有图片预览、视频播放、搜索、文件整理／撤销、网格／大图导航和只读工程聚焦测试。对需要交互证明的项目记录实际操作，发现问题先按 systematic-debugging 定位，不能无证据宣布全功能可用。
- [ ] **Step 2 — 同步已实现事实。** 用户指南明确无需“完成本轮”、成功保存才可读、存档非已解决、无声明主流程、部分存档／后补保留、历史继续提出与源确认。协议示例只给已运行通过的命令：

```sh
node scripts/review-protocol/read-current.mjs --project /absolute/test-project --stream 00000000-0000-4000-8000-000000000102
node scripts/review-protocol/read-history.mjs --project /absolute/test-project --stream 00000000-0000-4000-8000-000000000102 --archive 00000000-0000-4000-8000-000000000601
```

上述绝对路径是文档语法示例；实际验证使用临时工程返回的路径／ID，不声称示例工程已存在。读取说明要求完整当前清单替换旧待办、执行前核对目标版本，撤回不等于逆向修改。隐私说明列出本地证据／技术快照留存和没有自动清理，不宣称完整原图／视频备份。
- [ ] **Step 3 — 审核兼容与范围文案。** 旧设计只将固定范围、完成锁定、默认通过及旧 latest 当前语义标为被本设计替代，其余几何／安全约束继续有效；不整体作废已完成历史计划。保留开发早期政策，不新增签名、公证、正式安装包或发售待办。CHANGELOG 写本次功能事实，不改版本号。
- [ ] **Step 4 — 运行最终门禁并保存结果。**

```sh
git diff --check
pnpm test:policy
pnpm architecture:boundaries
pnpm architecture:trends
pnpm verify:clean
```

architecture:trends 是非阻塞报告，新增趋势需解释，不更新基线隐藏问题。其余应 exit 0；native 四标注／两轮流程证据单独列明，不能把既有 packaging 脚本的通过解读成产品发布准备。门禁运行中定期回报真实进展。
- [ ] **Step 5 — 检查点 F 与用户交付。** 只有 Tasks 1–22、完整验收矩阵、原生回归及门禁均有证据时，将实施状态标为完成；若有缺项明确停在对应任务，不写“全部可用”。按 requesting-code-review 与 verification-before-completion 技能完成必要检查；若需要代理审阅先遵守当时的授权和技能要求。
- [ ] **Step 6 — 提交已核验文档。** `git commit -m "docs(review): document continuous review and validated handoff workflow"`；仅暂存本任务文件，随后核对 git status 与最终 commit。合并／推送／发布不在本计划授权内。

## 4. 设计覆盖与验收矩阵

设计第 1–4 节的目标／取舍／非目标由 Global Constraints 和 Tasks 1、5、11、18、23 共同落实；
第 5 节对应 1–2／9–11／17–18，第 6 节对应 3–4／7–8／19–20，第 7 节对应 9–10／20，
第 8 节对应 5／14–15，第 9–11 节对应 1–11／16–17，第 12 节对应 12／21，第 13 节对应
6／13／15／21／23，第 14 节逐项见下表，第 15–16 节由 ADR／索引／验证报告留痕。

| 设计验收场景 | 主要任务 | 必须留下的证据 |
| --- | --- | --- |
| 首次局部意见、另一张图继续评审，无强制完成 | 1、9、11、18、22 | Domain／service／UI／原生流程 |
| 同一意见多次文字编辑和重绘 | 2、9、17、18 | ID 稳定、revision 改变、同快照编号预览 |
| 读 B 后写 C，存 B 保护后补与已删除项 | 3、7、19、22 | 差集测试及 Rust→Node B/C 原文对照 |
| 未知交接依据仍可手动存档 | 3、12、19、21 | usageBasis=null，不暗示已执行 |
| 只返图 1，图 2／共享意见另一目标保留 | 3、11、19、22 | 精确 TargetVersionKey 范围断言 |
| 目标重绘／共同文字改变的部分归档 | 2–3、19 | 文字与目标 revision 分别变化 |
| 同路径／相同 mtime 覆写 | 9–10、15、20、22 | 流式 hash 与旧底图证据 |
| 改名／移动／重复内容／多候选／缺失 | 10、15、20 | 不猜 lineage，保留待确认 |
| 继续提出历史问题 | 4、11、20 | 新 ID、HistoryRef、位置确认 |
| 撤销与有后续修改后的恢复 | 4、8、19–20 | 追加恢复事实、冲突选择、不覆盖 |
| 新增→编辑→删除、改后改回 | 2、5、15 | 净增量黄金用例，无无效中间任务 |
| 撤回“改蓝色” | 2、5、15、23 | withdrawn 原因，不生成反向自然语言 |
| no state／空 current／仅 pending／损坏 | 5–6、15、17 | 判别结果与闭合 Schema 契约 |
| 无／未知／跨上下文 since | 5、8、15 | 完整当前不依赖 delta，明确 unavailable |
| 保存／渲染／索引中断 | 7–9、11、17、22 | 每故障点与恢复输入测试 |
| 双击／回执丢失／乱序／过期 | 7–8、11、16–17、19 | 同操作幂等、旧响应不回退 |
| 两进程／只读／磁盘满／资源限额 | 6–8、10、14、17 | 单租约、上限边界、输入不丢 |
| 读取期间继续保存／改变源文件 | 10、15、22 | 固定 snapshotRef、独立检查时刻 |
| 同快照两次读取但源变化 | 5、10、15 | snapshot 不变，sourceChecks 变化 |
| legacy 原无证据 vs 声明证据损坏 | 6、9、13、15、20 | capability limitation 与 integrity error 分开 |
| 声明伪造范围／错项目／重复 ID／越界 | 12、16、21 | 拒绝导入但 Unknown 主流程可用 |
| v1/v2 迁移／旧读 v3／未来主版本 | 6、13、15、21–22 | 原字节哈希未变、拒绝降级／旧任务复活 |
| 跨快照共享证据／显式旧引用 | 7–9、15 | 无环可达引用、无后台 GC |
| 原生四标注／缩放／旋转／导航／重启 | 9、18、22–23 | 原生截图与实际交互结果 |
| 视频／搜索／文件操作等既有能力 | 16、18、22–23 | 原有专项门禁与受控工程 smoke |

## 5. 实施自检与交接要求

- 依赖顺序：1→2→3→4→5→6→7→8→9→10→11→12→13→14→15→16→17→18→19→20→21→22→23。
  虽然部分测试可以并行运行，产品状态链仍按阶段检查点推进，不提前接通真实用户写入。
- 测试沿用该包既有 imports 和测试运行器，输入／输出以本任务 Interfaces 与黄金夹具为准；
  不允许用空函数、总是成功的 mock 或忽略断言来满足门禁。既有依赖以基线锁文件为准。
- 原生 Evidence 适配器不进入 Domain；Application 不 import Infrastructure DTO；
  UI 不读写 `.viewer`；旧 API 只显式服务 legacy。复用原子 IO、租约与绘图原语，不复制安全实现。
- 阶段结束记录：完成任务／commit、聚焦测试命令与 exit code、已知限制、下一任务、未提交改动。
  中途暂停时只更新真实进度，不把待验收阶段标完成。
- 本次规划交付只标记“设计已批准、计划已形成”；23 个执行任务保持未勾选。
  后续建议在当前任务内按阶段执行并审阅检查点；未经用户选择不启动代理并行工作。
