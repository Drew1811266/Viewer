import type { ReviewTargetVersionKey } from '../../api/reviewWorkspaceTypes'
import type {
  ProjectAccess,
  ReviewAnchor,
  ReviewCompletionProposal,
  ReviewConflictSnapshot,
  ReviewEditorState,
  ReviewMemberSnapshot,
  ReviewScopeProposal,
  ReviewScopeRequest,
  ReviewSessionSnapshot,
} from '../../api/types'

export type {
  ProjectAccess,
  ReviewCompletionProposal,
  ReviewConflictSnapshot,
  ReviewEditorState,
  ReviewMemberSnapshot,
  ReviewScopeProposal,
  ReviewScopeRequest,
  ReviewSessionSnapshot,
}

export type ReviewScopeContext =
  | {
      kind: 'folder'
      folderId: string | null
      includeDescendants: boolean
      selectedEntityIds: string[]
    }
  | { kind: 'search'; selectedEntityIds: string[] }

export interface SavedImageFeedback {
  itemId: string
  feedbackId: string
  targetKey: ReviewTargetVersionKey | null
  assetVersionId: string
  text: string
  createdAtMs: number
  ordinal: number | null
  anchor: ReviewAnchor
}

export type ImageReviewReadOnlyReason =
  | 'outside_scope'
  | 'write_unavailable'
  | 'recovery_required'
  | 'loading'
  | 'saving'
  | 'source_confirmation'
  | 'migration_required'
  | null

export function deriveReviewScope(context: ReviewScopeContext): ReviewScopeRequest | null {
  const entityIds = uniqueIds(context.selectedEntityIds)
  switch (context.kind) {
    case 'folder':
      return entityIds.length > 0
        ? { kind: 'selection', entityIds }
        : {
            kind: 'folder',
            folderId: context.folderId,
            includeDescendants: context.includeDescendants,
          }
    case 'search':
      return entityIds.length > 0 ? { kind: 'selection', entityIds } : null
  }
}

export function eligibleReviewTargetIds(
  selectedEntityIds: readonly string[],
  members: readonly ReviewMemberSnapshot[],
): string[] {
  const memberIds = new Set(
    members.flatMap((member) => (member.entityId === null ? [] : [member.entityId])),
  )
  return uniqueIds(selectedEntityIds).filter((entityId) => memberIds.has(entityId))
}

export function hasUnsavedReviewText(editor: ReviewEditorState): boolean {
  return editor.text.trim().length > 0 && editor.text !== editor.savedText
}

export function emptyReviewEditor(): ReviewEditorState {
  return {
    mode: 'create',
    feedbackId: null,
    text: '',
    savedText: '',
    targetEntityIds: [],
    saveState: 'idle',
    error: null,
  }
}

export function idleReviewSnapshot(): ReviewSessionSnapshot {
  return {
    phase: 'idle',
    resume: null,
    reviewStreamId: null,
    reviewRoundId: null,
    revision: 0,
    members: [],
    feedback: [],
    restorableFeedbackId: null,
    unreviewable: [],
    conflicts: [],
    counts: { total: 0, feedbackItems: 0, revise: 0, unreviewable: 0, pass: 0 },
    error: null,
  }
}

export function imageFeedbackForEntity(
  snapshot: ReviewSessionSnapshot,
  entityId: string,
): SavedImageFeedback[] {
  let localOrdinal = 0
  return [...snapshot.feedback]
    .sort(
      (left, right) =>
        left.createdAtMs - right.createdAtMs ||
        (left.feedbackId < right.feedbackId ? -1 : left.feedbackId > right.feedbackId ? 1 : 0),
    )
    .flatMap((feedback) => {
      const target = feedback.targets.find((candidate) => candidate.entityId === entityId)
      if (target === undefined) return []
      let ordinal: number | null = null
      if (target.anchor.kind !== 'asset') {
        localOrdinal += 1
        ordinal = localOrdinal
      }
      return [
        {
          itemId: feedback.feedbackId,
          feedbackId: feedback.feedbackId,
          targetKey: null,
          assetVersionId: target.assetVersionId,
          text: feedback.text,
          createdAtMs: feedback.createdAtMs,
          ordinal,
          anchor: target.anchor,
        },
      ]
    })
}

export function imageReviewReadOnlyReason(
  snapshot: ReviewSessionSnapshot,
  entityId: string,
): ImageReviewReadOnlyReason {
  if (snapshot.phase === 'recovery_required') return 'recovery_required'
  if (
    snapshot.phase === 'preparing' ||
    snapshot.phase === 'completing' ||
    snapshot.phase === 'write_unavailable' ||
    snapshot.phase === 'completed_read_only'
  ) {
    return 'write_unavailable'
  }
  if (
    snapshot.reviewRoundId !== null &&
    !snapshot.members.some((member) => member.entityId === entityId)
  ) {
    return 'outside_scope'
  }
  return null
}

function uniqueIds(entityIds: readonly string[]): string[] {
  return [...new Set(entityIds)]
}
