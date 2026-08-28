import type {
  ReviewArchiveGroup,
  ReviewCommitReceipt,
  ReviewHistoryRef,
  ReviewHistorySelector,
  ReviewMigrationPlan,
  ReviewProductionScope,
  ReviewSnapshotRef,
  ReviewTargetVersionKey,
} from './reviewWorkspaceTypes'
import type { ReviewAnchor } from './types'

export interface ReviewAssetVersion {
  id: string
  sourceEntityId: string | null
  relativePath: string
  evidence: { sizeBytes: number; modifiedNs: string; blake3: string | null }
  media:
    | { kind: 'image'; width: number | null; height: number | null }
    | {
        kind: 'video'
        durationUs: number | null
        displayWidth: number | null
        displayHeight: number | null
      }
  producerAssetId: string | null
  parentAssetVersionId: string | null
}
export type ReviewPendingReason =
  | 'source_changed'
  | 'source_missing'
  | 'source_unreadable'
  | 'source_unverified'
  | 'legacy_usage_unknown'
  | 'legacy_evidence_absent'
  | 'applicability_unconfirmed'
export interface ReviewVersionedTarget {
  id: string
  revisionId: string
  assetVersionId: string
  anchor: ReviewAnchor
  availability: { kind: 'ready' } | { kind: 'needs_confirmation'; reasons: ReviewPendingReason[] }
}
export interface ReviewVersionedFeedback {
  id: string
  textRevisionId: string
  text: string
  createdAtMs: number
  targets: ReviewVersionedTarget[]
  historyRef: ReviewHistoryRef | null
}
export interface ReviewContinuousState {
  projectId: string
  streamId: string
  snapshotId: string
  parent: ReviewSnapshotRef | null
  assets: ReviewAssetVersion[]
  feedback: ReviewVersionedFeedback[]
}
export interface ReviewEvidenceRef {
  blake3: string
  sizeBytes: number
  width: number
  height: number
}
export interface ReviewEvidenceBinding {
  assetVersionId: string
  capability:
    | {
        kind: 'image'
        base: ReviewEvidenceRef
        annotated: ReviewEvidenceRef | null
        annotations: { ordinal: number; key: ReviewTargetVersionKey }[]
      }
    | { kind: 'legacy_absent' }
    | { kind: 'not_image' }
}
/** Session-scoped cache URL; re-request by selector after eviction, never persist as evidence ID. */
export interface ReviewEvidenceImage {
  assetVersionId: string
  role: 'base' | 'annotated'
  url: string
  width: number
  height: number
  sourceWidth: number
  sourceHeight: number
}
export interface PreparedReviewAsset {
  asset: ReviewAssetVersion
  preview: ReviewEvidenceImage | null
}
export interface ReviewChange {
  targetId: string
  before: ReviewTargetVersionKey | null
  after: ReviewTargetVersionKey | null
  kind:
    | 'added'
    | 'edited'
    | 'withdrawn'
    | 'archived'
    | 'restored'
    | 'rebound'
    | 'availability_changed'
  archiveId: string | null
  historicalKey: ReviewTargetVersionKey | null
}
export interface ReviewStoredSnapshot {
  reference: ReviewSnapshotRef
  production: ReviewProductionScope | null
  state: ReviewContinuousState
  commandId: string
  payloadDigest: string
  changes: ReviewChange[]
  evidence: ReviewEvidenceBinding[]
}
export interface ReviewSourceCheck {
  assetVersionId: string
  checkedAtMs: number
  status: 'match' | 'changed' | 'missing' | 'unreadable' | 'unverified'
}
export interface ReviewWorkspaceView {
  streamId: string
  current: ReviewStoredSnapshot | null
  sourceChecks: ReviewSourceCheck[]
  projection: { actionable: string[]; needsConfirmation: string[] }
  recovery: ReviewRecoveryDraft[]
  migration: ReviewMigrationInspection | null
  capabilities: { continuousEditing: boolean; usageImport: boolean; migration: boolean }
}
export interface ReviewApplyResult {
  receipt: ReviewCommitReceipt
  view: ReviewWorkspaceView
}
export interface ReviewArchiveRetention {
  basis: ReviewTargetVersionKey
  current: ReviewTargetVersionKey | null
  disposition: 'remove_current' | 'retain_later_edit' | 'already_absent'
}
export interface ReviewArchivePlan {
  expectedSnapshotId: string
  groups: ReviewArchiveGroup[]
  removed: ReviewTargetVersionKey[]
  retained: ReviewArchiveRetention[]
  alreadyCovered: ReviewTargetVersionKey[]
}
export interface ReviewRestorePlan {
  expectedSnapshotId: string
  restored: {
    historicalKey: ReviewTargetVersionKey
    feedback: Omit<ReviewVersionedFeedback, 'targets'>
    target: ReviewVersionedTarget
    asset: ReviewAssetVersion
    continuedAsNew: boolean
  }[]
  conflicts: ReviewTargetVersionKey[]
  coverageReversals: { archiveId: string; key: ReviewTargetVersionKey; active: boolean }[]
  requiresSourceCheck: string[]
}
export type ReviewHistoryLimitation =
  | 'background_only'
  | 'legacy_evidence_absent'
  | 'external_copies_cannot_be_revoked'
  | 'usage_unconfirmed'
export interface ReviewHistoryEntry {
  snapshot: ReviewSnapshotRef
  feedback: ReviewVersionedFeedback[]
  assets: ReviewAssetVersion[]
  evidence: ReviewEvidenceBinding[]
  selected: ReviewTargetVersionKey[]
}
/** Explicit background; intentionally no actionable/current fields. */
export interface ReviewHistoryView {
  selector: ReviewHistorySelector
  entries: ReviewHistoryEntry[]
  legacy: ReviewLegacyRecord | null
  limitations: ReviewHistoryLimitation[]
  restoreActions: ReviewTargetVersionKey[]
}
export interface ReviewUsageOutput {
  relativePath: string
  blake3: string
  previousAssetVersionId: string
}
export interface ReviewUsageDeclaration {
  id: string
  projectId: string
  streamId: string
  basis: ReviewSnapshotRef
  targets: ReviewTargetVersionKey[]
  outputs: ReviewUsageOutput[]
}
export interface ReviewUsageImportPreview {
  declaration: ReviewUsageDeclaration
  canonicalDigest: string
  source: string
  sourceDigest: string
  outputs: {
    output: ReviewUsageOutput
    status:
      | 'verified_candidate'
      | 'unknown_previous_asset'
      | 'missing'
      | 'changed'
      | 'unsafe'
      | 'unreadable'
  }[]
}
export type ReviewLegacyProtocol = 'viewer.review/1' | 'viewer.review/2'
export interface ReviewLegacyReference {
  streamId: string
  roundId: string
  protocol: ReviewLegacyProtocol
  isDraft: boolean
  blake3: string
}
export type ReviewLegacyFailure =
  | 'unsupported'
  | 'damaged'
  | 'unreadable'
  | 'permission_denied'
  | 'missing'
  | 'decode_failed'
export interface ReviewLegacyFeedback {
  id: string
  text: string
  createdAtMs: number
  targets: { assetVersionId: string; anchor: ReviewAnchor }[]
}
interface ReviewLegacyContent {
  projectId: string
  reviewStreamId: string
  reviewRoundId: string
  production: ReviewProductionScope | null
  previousCompletedRoundId: string | null
  createdAtMs: number
  assets: ReviewAssetVersion[]
  feedback: ReviewLegacyFeedback[]
}
export interface ReviewLegacyDraft extends ReviewLegacyContent {
  unreviewable: { assetVersionId: string; failure: ReviewLegacyFailure }[]
}
export interface ReviewLegacyCompleted extends ReviewLegacyContent {
  completedAtMs: number
  outcomes: {
    assetVersionId: string
    kind: 'pass' | 'revise' | 'unreviewable'
    feedbackIds: string[]
    failure: ReviewLegacyFailure | null
  }[]
}
export interface ReviewLegacyRecord {
  reference: ReviewLegacyReference
  contents:
    | { kind: 'draft'; record: ReviewLegacyDraft }
    | { kind: 'completed'; record: ReviewLegacyCompleted }
}
export interface ReviewMigrationInspection {
  legacyProtocol: ReviewLegacyProtocol
  indexDigest: string
  inspectionDigest: string
  legacyRecords: ReviewLegacyReference[]
  activeDraft: { protocolVersion: ReviewLegacyProtocol; draft: ReviewLegacyDraft } | null
  completedCandidates: ReviewLegacyCompleted[]
  limitations: ReviewHistoryLimitation[]
}
export interface ReviewRecoverySelection {
  origin:
    | { kind: 'current' | 'snapshot'; key: ReviewTargetVersionKey }
    | { kind: 'legacy'; roundId: string; feedbackId: string; targetIndex: number }
    | { kind: 'archive'; archiveId: string; key: ReviewTargetVersionKey }
  feedbackId: string
  textRevisionId: string
  targetId: string
  targetRevisionId: string
  assetVersionId: string
  confirmation:
    | { kind: 'unconfirmed' | 'user_confirmed' }
    | { kind: 'producer_verified'; usageId: string }
}
export interface ReviewRecoveryDraft {
  streamId: string
  commandId: string
  expectedSnapshotId: string | null
  payloadDigest: string
  editorInput: {
    migration: ReviewMigrationPlan | null
    selections: ReviewRecoverySelection[]
    text: string
    feedbackId: string | null
    targets: ReviewVersionedTarget[]
    historyRef: ReviewHistoryRef | null
  }
  failure:
    | 'render_failed'
    | 'source_changed'
    | 'write_failed'
    | 'commit_unknown'
    | 'cancelled'
    | 'stale_snapshot'
}
