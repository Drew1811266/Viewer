import type { ReviewWorkspaceView } from '../../api/reviewWorkspaceTypes'
import type { ReviewFeedbackSnapshot } from '../../api/types'

interface VisibleReviewFeedbackCountOptions {
  continuousEnabled: boolean
  continuousView: ReviewWorkspaceView | null
  legacyFeedback: readonly ReviewFeedbackSnapshot[]
}

export function visibleReviewFeedbackCounts({
  continuousEnabled,
  continuousView,
  legacyFeedback,
}: VisibleReviewFeedbackCountOptions): ReadonlyMap<string, number> {
  return continuousEnabled
    ? continuousFeedbackCounts(continuousView)
    : legacyFeedbackCounts(legacyFeedback)
}

function continuousFeedbackCounts(view: ReviewWorkspaceView | null) {
  const counts = new Map<string, number>()
  if (view === null || view.migration !== null || view.current === null) return counts
  const entityByAssetVersion = new Map(
    view.current.state.assets.flatMap((asset) =>
      asset.sourceEntityId === null ? [] : [[asset.id, asset.sourceEntityId] as const],
    ),
  )
  for (const feedback of view.current.state.feedback) {
    const entityIds = new Set(
      feedback.targets.flatMap((target) => {
        const entityId = entityByAssetVersion.get(target.assetVersionId)
        return entityId === undefined ? [] : [entityId]
      }),
    )
    incrementCounts(counts, entityIds)
  }
  return counts
}

function legacyFeedbackCounts(feedback: readonly ReviewFeedbackSnapshot[]) {
  const counts = new Map<string, number>()
  for (const item of feedback) {
    const entityIds = new Set(
      item.targets.flatMap((target) => (target.entityId === null ? [] : [target.entityId])),
    )
    incrementCounts(counts, entityIds)
  }
  return counts
}

function incrementCounts(counts: Map<string, number>, entityIds: ReadonlySet<string>) {
  for (const entityId of entityIds) counts.set(entityId, (counts.get(entityId) ?? 0) + 1)
}
