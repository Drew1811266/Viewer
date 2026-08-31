import type {
  ReviewHistoryRef,
  ReviewHistoryView,
  ReviewRestoreChoice,
  ReviewRestoreDecision,
  ReviewTargetVersionKey,
  ReviewVersionedFeedback,
} from '../../api/reviewWorkspaceTypes'

export function displayHistoryFeedback(
  history: ReviewHistoryView,
  entry: ReviewHistoryView['entries'][number],
) {
  const selected = new Set(entry.selected.map(historyTargetKey))
  const wholeSnapshot = history.selector.kind === 'snapshot' && selected.size === 0
  return entry.feedback.flatMap((feedback) => {
    const targets = wholeSnapshot
      ? feedback.targets
      : feedback.targets.filter((target) =>
          selected.has(
            historyTargetKey({
              feedbackId: feedback.id,
              textRevisionId: feedback.textRevisionId,
              targetId: target.id,
              targetRevisionId: target.revisionId,
            }),
          ),
        )
    return targets.length === 0 ? [] : [{ ...feedback, targets }]
  })
}

export function restoreTargetState(
  restoreActions: ReviewTargetVersionKey[],
  currentFeedback: ReviewVersionedFeedback[],
) {
  const alreadyCurrent = restoreActions.filter(
    (key) => currentTargetVersion(currentFeedback, key.targetId) === historyTargetKey(key),
  )
  const selectable = restoreActions.filter(
    (key) => !alreadyCurrent.some((current) => historyTargetKey(current) === historyTargetKey(key)),
  )
  const conflicts = selectable.filter((key) => {
    const current = currentTargetVersion(currentFeedback, key.targetId)
    return current !== null && current !== historyTargetKey(key)
  })
  return { alreadyCurrent, selectable, conflicts: uniqueHistoryKeys(conflicts) }
}

export function restoreSubmission(
  selected: ReviewTargetVersionKey[],
  conflicts: ReviewTargetVersionKey[],
  choices: Record<string, ReviewRestoreChoice>,
):
  | { kind: 'needs_choice' }
  | { kind: 'empty' }
  | { kind: 'preview'; decisions: ReviewRestoreDecision[] } {
  const unresolved = conflicts.some(
    (key) =>
      selected.some((item) => historyTargetKey(item) === historyTargetKey(key)) &&
      choices[historyTargetKey(key)] === undefined,
  )
  if (unresolved) return { kind: 'needs_choice' }
  const decisions = selected.flatMap((historicalKey) => {
    const choice = choices[historyTargetKey(historicalKey)] ?? { kind: 'use_historical' as const }
    return choice.kind === 'preserve_current' ? [] : [{ historicalKey, choice }]
  })
  return decisions.length === 0 ? { kind: 'empty' } : { kind: 'preview', decisions }
}

export function historyRefsForEntries(
  history: ReviewHistoryView,
  projectId: string,
  streamId: string,
): ReviewHistoryRef[] {
  return history.entries.flatMap((entry) =>
    entry.selected.map((key) => ({
      projectId,
      streamId,
      source: {
        kind: 'snapshot' as const,
        snapshot: structuredClone(entry.snapshot),
        keys: [structuredClone(key)],
      },
    })),
  )
}

export function historyTargetKey(key: ReviewTargetVersionKey) {
  return `${key.feedbackId}\u0000${key.textRevisionId}\u0000${key.targetId}\u0000${key.targetRevisionId}`
}

function currentTargetVersion(feedback: ReviewVersionedFeedback[], targetId: string) {
  for (const item of feedback) {
    const target = item.targets.find((candidate) => candidate.id === targetId)
    if (target !== undefined) {
      return historyTargetKey({
        feedbackId: item.id,
        textRevisionId: item.textRevisionId,
        targetId: target.id,
        targetRevisionId: target.revisionId,
      })
    }
  }
  return null
}

function uniqueHistoryKeys(keys: ReviewTargetVersionKey[]) {
  return keys.filter(
    (key, index) =>
      keys.findIndex((candidate) => historyTargetKey(candidate) === historyTargetKey(key)) ===
      index,
  )
}
