import type {
  PreparedReviewAsset,
  ReviewApplyResult,
  ReviewArchivePlan,
  ReviewEvidenceImage,
  ReviewHistoryView,
  ReviewMigrationInspection,
  ReviewRestorePlan,
  ReviewUsageImportPreview,
  ReviewWorkspaceView,
} from './reviewWorkspaceViewTypes'
import type { ReviewAnchor } from './types'

export type * from './reviewWorkspaceViewTypes'

export interface ReviewWorkspaceSessionRequest {
  sessionId: string
  generation: number
}
export interface ReviewSnapshotRef {
  snapshotId: string
  blake3: string
}
export interface ReviewTargetVersionKey {
  feedbackId: string
  textRevisionId: string
  targetId: string
  targetRevisionId: string
}
export interface ReviewProductionScope {
  taskId: string
  batchId: string
}
export interface ReviewWorkspaceContext {
  projectId: string
  streamId: string
  production: ReviewProductionScope | null
}
export interface ReviewLegacyTargetRef {
  roundId: string
  feedbackId: string
  targetIndex: number
}
export interface ReviewHistoryRef {
  projectId: string
  streamId: string
  source:
    | { kind: 'snapshot'; snapshot: ReviewSnapshotRef; keys: ReviewTargetVersionKey[] }
    | { kind: 'legacy'; roundId: string; recordBlake3: string; targets: ReviewLegacyTargetRef[] }
}
export type ReviewHistorySelector =
  | { kind: 'snapshot'; snapshot: ReviewSnapshotRef }
  | { kind: 'archive'; archiveId: string }
  | { kind: 'legacy'; roundId: string }
export type ReviewTargetEdit =
  | { kind: 'add'; assetVersionId: string; anchor: ReviewAnchor }
  | { kind: 'redraw'; key: ReviewTargetVersionKey; assetVersionId: string; anchor: ReviewAnchor }
export type ReviewSourceBindingConfirmation =
  | { kind: 'user_confirmed' }
  | { kind: 'producer_verified_and_position_confirmed'; usageId: string }
export interface ReviewSourceBindingDecision {
  targetKey: ReviewTargetVersionKey
  newAssetVersionId: string
  anchor: ReviewAnchor
  confirmation: ReviewSourceBindingConfirmation
}
export type ReviewArchiveBasis =
  | { kind: 'unknown' }
  | {
      kind: 'known'
      snapshot: ReviewSnapshotRef
      source: { kind: 'user_selected' } | { kind: 'agent_declared'; usageId: string }
    }
export interface ReviewArchiveGroup {
  basis: ReviewArchiveBasis
  targets: ReviewTargetVersionKey[]
}
export interface ReviewArchiveSelection {
  expectedSnapshotId: string
  groups: ReviewArchiveGroup[]
}
export type ReviewRestoreChoice =
  | { kind: 'preserve_current' }
  | { kind: 'use_historical' }
  | {
      kind: 'continue_as_new'
      feedbackId: string
      textRevisionId: string
      targetId: string
      targetRevisionId: string
      targetAssetVersionId: string
      confirmedAnchor: ReviewAnchor | null
      createdAtMs: number
    }
export interface ReviewRestoreDecision {
  historicalKey: ReviewTargetVersionKey
  choice: ReviewRestoreChoice
}
export interface ReviewMigrationBinding {
  legacyTarget: ReviewLegacyTargetRef
  newAssetVersionId: string
  anchor: ReviewAnchor
  positionConfirmed: boolean
}
export interface ReviewMigrationPlan {
  inspectionDigest: string
  choice:
    | { kind: 'keep_history_only' }
    | {
        kind: 'continue_selected'
        legacyTargets: ReviewLegacyTargetRef[]
        bindings: ReviewMigrationBinding[]
      }
}
export type ReviewWorkspaceCommand =
  | { kind: 'save_feedback'; feedbackId: string | null; text: string; targets: ReviewTargetEdit[] }
  | { kind: 'withdraw'; targets: ReviewTargetVersionKey[] }
  | ({ kind: 'archive' } & ReviewArchiveSelection)
  | { kind: 'restore'; archiveId: string; decisions: ReviewRestoreDecision[] }
  | {
      kind: 'continue_historical'
      historyRef: ReviewHistoryRef
      bindings: ReviewSourceBindingDecision[]
    }
  | ({ kind: 'confirm_source' } & ReviewSourceBindingDecision)
  | {
      kind: 'confirm_applicability'
      key: ReviewTargetVersionKey
      assetVersionId: string
      anchor: ReviewAnchor
    }
  | { kind: 'adopt_usage'; declarationId: string }
  | ({ kind: 'migrate' } & ReviewMigrationPlan)
  | { kind: 'continue_legacy'; historyRef: ReviewHistoryRef; bindings: ReviewMigrationBinding[] }
export interface GeneratedReviewIds {
  snapshotId: string
  feedbackId: string
  textRevisionId: string
  archiveId: string
  targets: { targetId: string; targetRevisionId: string }[]
  migration: {
    roundId: string
    legacyFeedbackId: string
    feedbackId: string
    textRevisionId: string
    targets: { targetIndex: number; targetId: string; targetRevisionId: string }[]
  }[]
  createdAtMs: number
}
export interface PreparedUsageSelection {
  id: string
  candidate: { canonicalDigest: string; sourceDigest: string; source: string } | null
}
/** Keep this complete object for retry. Do not regenerate identities or digest in the UI. */
export interface PreparedReviewCommand {
  context: ReviewWorkspaceContext
  commandId: string
  expectedSnapshotId: string | null
  payloadDigest: string
  generated: GeneratedReviewIds
  usageSelections: PreparedUsageSelection[]
  command: ReviewWorkspaceCommand
}
export interface ReviewCommitReceipt {
  commandId: string
  payloadDigest: string
  snapshot: ReviewSnapshotRef
}
export type ReviewWorkspaceErrorCode =
  | 'invalid_data'
  | 'stale_session'
  | 'cancelled'
  | 'busy'
  | 'internal'
  | 'migration_required'
  | 'unsupported_protocol'
  | 'stale_snapshot'
  | 'command_conflict'
  | 'read_only'
  | 'lease_busy'
  | 'integrity'
  | 'limit_exceeded'
  | 'lookup_unavailable'
  | 'evidence_absent'
  | 'ambiguous_evidence'
  | 'io'
  | 'outcome_unknown'
  | 'asset_unavailable'
  | 'source_changed'
  | 'unsafe_source'
  | 'render_failed'
  | 'usage_invalid'
  | 'no_changes'
  | 'capability_unavailable'
  | 'wrong_context'
  | 'preview_required'
  | 'needs_confirmation'
  | 'selection_conflict'
  | 'committed_view_unavailable'
export interface ReviewWorkspaceError {
  code: ReviewWorkspaceErrorCode
  message: string
  retryable: boolean
  committedReceipt: ReviewCommitReceipt | null
}
export interface PrepareReviewAssetsRequest extends ReviewWorkspaceSessionRequest {
  entityIds: string[]
}
export interface PrepareReviewCommandRequest extends ReviewWorkspaceSessionRequest {
  commandId: string
  expectedSnapshotId: string | null
  command: ReviewWorkspaceCommand
}
export interface ApplyReviewCommandRequest extends ReviewWorkspaceSessionRequest {
  envelope: PreparedReviewCommand
}
export interface PreviewReviewArchiveRequest extends ReviewWorkspaceSessionRequest {
  selection: ReviewArchiveSelection
}
export interface PreviewReviewRestoreRequest extends ReviewWorkspaceSessionRequest {
  archiveId: string
  decisions: ReviewRestoreDecision[]
}
export interface ReviewHistoryRequest extends ReviewWorkspaceSessionRequest {
  selector: ReviewHistorySelector
}
export interface InspectReviewUsageRequest extends ReviewWorkspaceSessionRequest {
  entityId: string
}
export interface ReviewEvidenceRequest extends ReviewHistoryRequest {
  assetVersionId: string
  role: 'base' | 'annotated'
}

/** No UI is activated by declaring this port. Errors retain any known committed receipt. */
export interface ReviewWorkspacePort {
  getWorkspace(request: ReviewWorkspaceSessionRequest): Promise<ReviewWorkspaceView>
  prepareAssets(request: PrepareReviewAssetsRequest): Promise<PreparedReviewAsset[]>
  prepareCommand(request: PrepareReviewCommandRequest): Promise<PreparedReviewCommand>
  applyCommand(request: ApplyReviewCommandRequest): Promise<ReviewApplyResult>
  previewArchive(request: PreviewReviewArchiveRequest): Promise<ReviewArchivePlan>
  previewRestore(request: PreviewReviewRestoreRequest): Promise<ReviewRestorePlan>
  getHistory(request: ReviewHistoryRequest): Promise<ReviewHistoryView>
  inspectUsage(request: InspectReviewUsageRequest): Promise<ReviewUsageImportPreview>
  /** Opens one native picker and inspects only its explicit selection. */
  selectUsage(request: ReviewWorkspaceSessionRequest): Promise<ReviewUsageImportPreview | null>
  inspectMigration(
    request: ReviewWorkspaceSessionRequest,
  ): Promise<ReviewMigrationInspection | null>
  getEvidence(request: ReviewEvidenceRequest): Promise<ReviewEvidenceImage>
  cancelTask(request: ReviewWorkspaceSessionRequest): Promise<number>
}
