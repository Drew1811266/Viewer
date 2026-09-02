import type { ReviewWorkspacePatch } from '../../api/reviewWorkspaceTypes'
import type {
  ReviewAssetVersion,
  ReviewVersionedFeedback,
  ReviewWorkspaceView,
} from '../../api/reviewWorkspaceViewTypes'

export class ReviewPatchMismatch extends Error {
  readonly reason: 'stale_basis' | 'invalid_patch'

  constructor(reason: 'stale_basis' | 'invalid_patch') {
    super(
      reason === 'stale_basis'
        ? 'Review patch basis does not match the current authoring head'
        : 'Review patch is internally inconsistent',
    )
    this.name = 'ReviewPatchMismatch'
    this.reason = reason
  }
}

/** Applies a trusted desktop delta without manufacturing a published v3 snapshot. */
export function applyReviewWorkspacePatch(
  view: ReviewWorkspaceView,
  patch: ReviewWorkspacePatch,
): ReviewWorkspaceView {
  const currentSnapshotId = view.current?.authoring.head.snapshotId ?? null
  if (patch.streamId !== view.streamId) throw new ReviewPatchMismatch('invalid_patch')
  if (currentSnapshotId !== patch.basisSnapshotId) throw new ReviewPatchMismatch('stale_basis')
  if (
    (view.current !== null &&
      (patch.projectId !== view.current.authoring.state.projectId ||
        patch.streamId !== view.current.authoring.state.streamId)) ||
    patch.head.sequence <= 0 ||
    patch.head.snapshotId.length === 0 ||
    (patch.basisSnapshotId === null && patch.parent !== null) ||
    (patch.basisSnapshotId !== null &&
      (patch.parent?.snapshotId !== patch.basisSnapshotId || patch.parent.blake3.length === 0)) ||
    (view.current !== null && patch.head.sequence <= view.current.authoring.head.sequence)
  ) {
    throw new ReviewPatchMismatch('invalid_patch')
  }
  validateIdentities(
    patch.upsertAssets.map((asset) => asset.id),
    patch.removeAssetVersionIds,
  )
  validateIdentities(
    patch.upsertFeedback.map((feedback) => feedback.id),
    patch.removeFeedbackIds,
  )

  const assets = applyUpserts(
    view.current?.authoring.state.assets ?? [],
    patch.upsertAssets,
    patch.removeAssetVersionIds,
    (asset) => asset.id,
  )
  const feedback = applyUpserts(
    view.current?.authoring.state.feedback ?? [],
    patch.upsertFeedback,
    patch.removeFeedbackIds,
    (item) => item.id,
  )
  if (!sameProjection(logicalProjection(feedback), patch.projection)) {
    throw new ReviewPatchMismatch('invalid_patch')
  }

  const previous = view.current
  const state = {
    ...(previous?.authoring.state ?? emptyState(view, patch.projectId, patch.head.snapshotId)),
    snapshotId: patch.head.snapshotId,
    parent: patch.parent === null ? null : { ...patch.parent },
    assets,
    feedback,
  }
  return {
    ...view,
    historySelectors: patch.historySelectors ?? view.historySelectors,
    projection: cloneProjection(patch.projection),
    current: {
      authoring: {
        head: { ...patch.head },
        state,
      },
      publishedRef: previous?.publishedRef ?? null,
      evidence: previous?.evidence ?? [],
    },
  }
}

function validateIdentities(upserts: string[], removals: string[]) {
  const upsertIds = new Set(upserts)
  const removalIds = new Set(removals)
  if (
    upsertIds.size !== upserts.length ||
    removalIds.size !== removals.length ||
    [...upsertIds].some((id) => removalIds.has(id))
  ) {
    throw new ReviewPatchMismatch('invalid_patch')
  }
}

function applyUpserts<T>(
  current: readonly T[],
  upserts: readonly T[],
  removals: readonly string[],
  id: (value: T) => string,
) {
  const removed = new Set(removals)
  const next = current
    .filter((value) => !removed.has(id(value)))
    .map((value) => structuredClone(value))
  for (const upsert of upserts) {
    const index = next.findIndex((value) => id(value) === id(upsert))
    if (index === -1) next.push(structuredClone(upsert))
    else next[index] = structuredClone(upsert)
  }
  return next
}

function logicalProjection(feedback: readonly ReviewVersionedFeedback[]) {
  const projection = { actionable: [] as string[], needsConfirmation: [] as string[] }
  for (const target of feedback.flatMap((item) => item.targets)) {
    if (target.availability.kind === 'ready') projection.actionable.push(target.id)
    else projection.needsConfirmation.push(target.id)
  }
  return projection
}

function sameProjection(
  left: { actionable: string[]; needsConfirmation: string[] },
  right: { actionable: string[]; needsConfirmation: string[] },
) {
  return (
    sameStrings(left.actionable, right.actionable) &&
    sameStrings(left.needsConfirmation, right.needsConfirmation)
  )
}

function sameStrings(left: string[], right: string[]) {
  return left.length === right.length && left.every((value, index) => value === right[index])
}

function cloneProjection(value: { actionable: string[]; needsConfirmation: string[] }) {
  return {
    actionable: [...value.actionable],
    needsConfirmation: [...value.needsConfirmation],
  }
}

function emptyState(
  view: ReviewWorkspaceView,
  projectId: string,
  snapshotId: string,
): {
  projectId: string
  streamId: string
  snapshotId: string
  parent: { snapshotId: string; blake3: string } | null
  assets: ReviewAssetVersion[]
  feedback: ReviewVersionedFeedback[]
} {
  return {
    projectId,
    streamId: view.streamId,
    snapshotId,
    parent: null,
    assets: [],
    feedback: [],
  }
}
