import type {
  ProjectAccess,
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
    unreviewable: [],
    conflicts: [],
    counts: { total: 0, feedbackItems: 0, revise: 0, unreviewable: 0, pass: 0 },
    error: null,
  }
}

function uniqueIds(entityIds: readonly string[]): string[] {
  return [...new Set(entityIds)]
}
