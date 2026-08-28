import type {
  PreparedReviewCommand,
  ReviewCommitReceipt,
  ReviewTargetEdit,
  ReviewWorkspaceError,
  ReviewWorkspaceView,
} from '../../api/reviewWorkspaceTypes'

export type ContinuousReviewState =
  | { kind: 'loading' }
  | { kind: 'ready' }
  | { kind: 'saving'; stage: 'preparing' | 'applying' }
  | { kind: 'save_failed'; error: ReviewWorkspaceError }
  | { kind: 'recovery_required'; error: ReviewWorkspaceError | null }
  | { kind: 'migration_required' }
  | { kind: 'unavailable'; error: ReviewWorkspaceError }

export interface ContinuousReviewEditorSeed {
  contextKey: string | null
  feedbackId: string | null
  text: string
  targets: ReviewTargetEdit[]
}
export interface ContinuousReviewEditorInput extends ContinuousReviewEditorSeed {
  baseSnapshotId: string | null
}
export interface ContinuousReviewSnapshot {
  state: ContinuousReviewState
  view: ReviewWorkspaceView | null
  editorInput: ContinuousReviewEditorInput
  pendingEnvelope: PreparedReviewCommand | null
  lastReceipt: ReviewCommitReceipt | null
  error: ReviewWorkspaceError | null
}

export function continuousReviewState(view: ReviewWorkspaceView): ContinuousReviewState {
  if (view.migration !== null) return { kind: 'migration_required' }
  if (view.recovery.length > 0) return { kind: 'recovery_required', error: null }
  if (!view.capabilities.continuousEditing) {
    return { kind: 'unavailable', error: reviewWorkspaceError('read_only', '项目不可写', false) }
  }
  return { kind: 'ready' }
}

export function emptyContinuousEditor(): ContinuousReviewEditorInput {
  return { contextKey: null, feedbackId: null, text: '', targets: [], baseSnapshotId: null }
}

export function reviewWorkspaceError(
  code: ReviewWorkspaceError['code'],
  message: string,
  retryable: boolean,
): ReviewWorkspaceError {
  return { code, message, retryable, committedReceipt: null }
}

const errorCodes = new Set<ReviewWorkspaceError['code']>([
  'invalid_data',
  'stale_session',
  'cancelled',
  'busy',
  'internal',
  'migration_required',
  'unsupported_protocol',
  'stale_snapshot',
  'command_conflict',
  'read_only',
  'lease_busy',
  'integrity',
  'limit_exceeded',
  'lookup_unavailable',
  'evidence_absent',
  'ambiguous_evidence',
  'io',
  'outcome_unknown',
  'asset_unavailable',
  'source_changed',
  'unsafe_source',
  'render_failed',
  'usage_invalid',
  'no_changes',
  'capability_unavailable',
  'wrong_context',
  'preview_required',
  'needs_confirmation',
  'selection_conflict',
  'committed_view_unavailable',
])

export function normalizeReviewWorkspaceError(cause: unknown): ReviewWorkspaceError {
  if (typeof cause === 'object' && cause !== null) {
    const candidate = cause as Partial<ReviewWorkspaceError>
    if (
      typeof candidate.code === 'string' &&
      errorCodes.has(candidate.code as ReviewWorkspaceError['code']) &&
      typeof candidate.message === 'string' &&
      typeof candidate.retryable === 'boolean' &&
      (candidate.committedReceipt === null || isCommitReceipt(candidate.committedReceipt))
    ) {
      return {
        code: candidate.code as ReviewWorkspaceError['code'],
        message: candidate.message,
        retryable: candidate.retryable,
        committedReceipt: candidate.committedReceipt ?? null,
      }
    }
  }
  const message =
    typeof cause === 'object' &&
    cause !== null &&
    'message' in cause &&
    typeof cause.message === 'string'
      ? cause.message
      : typeof cause === 'string'
        ? cause
        : '未知桌面错误'
  return reviewWorkspaceError('internal', message, true)
}

function isCommitReceipt(value: unknown): value is ReviewCommitReceipt {
  if (typeof value !== 'object' || value === null) return false
  const receipt = value as Partial<ReviewCommitReceipt>
  return (
    typeof receipt.commandId === 'string' &&
    typeof receipt.payloadDigest === 'string' &&
    typeof receipt.snapshot === 'object' &&
    receipt.snapshot !== null &&
    typeof receipt.snapshot.snapshotId === 'string' &&
    typeof receipt.snapshot.blake3 === 'string'
  )
}

export function requiresReviewReconciliation(error: ReviewWorkspaceError): boolean {
  return (
    error.committedReceipt !== null ||
    [
      'outcome_unknown',
      'lookup_unavailable',
      'integrity',
      'command_conflict',
      'wrong_context',
      'internal',
      'committed_view_unavailable',
    ].includes(error.code)
  )
}

export function deepFreezeReviewValue<T>(value: T): T {
  if (typeof value !== 'object' || value === null || Object.isFrozen(value)) return value
  Object.freeze(value)
  for (const child of Object.values(value)) deepFreezeReviewValue(child)
  return value
}

/** Local duplicate-click comparison only; never a replacement for the desktop payload digest. */
export function sameReviewValue(left: unknown, right: unknown): boolean {
  if (Object.is(left, right)) return true
  if (typeof left !== 'object' || left === null || typeof right !== 'object' || right === null)
    return false
  if (Array.isArray(left) || Array.isArray(right))
    return (
      Array.isArray(left) &&
      Array.isArray(right) &&
      left.length === right.length &&
      left.every((value, index) => sameReviewValue(value, right[index]))
    )
  const a = left as Record<string, unknown>
  const b = right as Record<string, unknown>
  const keys = Object.keys(a)
  return (
    keys.length === Object.keys(b).length &&
    keys.every((key) => Object.hasOwn(b, key) && sameReviewValue(a[key], b[key]))
  )
}
