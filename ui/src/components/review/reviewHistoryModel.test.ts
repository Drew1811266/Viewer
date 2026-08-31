import { expect, it } from 'vitest'
import type {
  ReviewHistoryView,
  ReviewTargetVersionKey,
  ReviewVersionedFeedback,
} from '../../api/reviewWorkspaceTypes'
import {
  displayHistoryFeedback,
  historyRefsForEntries,
  restoreSubmission,
  restoreTargetState,
} from './reviewHistoryModel'

const historicalKey: ReviewTargetVersionKey = {
  feedbackId: 'feedback-old',
  textRevisionId: 'text-old',
  targetId: 'target-old',
  targetRevisionId: 'target-old',
}

const anotherHistoricalKey: ReviewTargetVersionKey = {
  feedbackId: 'feedback-other',
  textRevisionId: 'text-other',
  targetId: 'target-other',
  targetRevisionId: 'target-other',
}

const history: ReviewHistoryView = {
  selector: { kind: 'archive', archiveId: 'archive-1' },
  entries: [
    {
      snapshot: { snapshotId: 'snapshot-old', blake3: 'a'.repeat(64) },
      feedback: [
        feedback(historicalKey, '历史意见'),
        feedback(anotherHistoricalKey, '未选中的历史意见'),
      ],
      assets: [],
      evidence: [],
      selected: [historicalKey],
    },
  ],
  legacy: null,
  limitations: ['background_only'],
  restoreActions: [historicalKey, anotherHistoricalKey],
}

function feedback(key: ReviewTargetVersionKey, text: string): ReviewVersionedFeedback {
  return {
    id: key.feedbackId,
    textRevisionId: key.textRevisionId,
    text,
    createdAtMs: 1,
    historyRef: null,
    targets: [
      {
        id: key.targetId,
        revisionId: key.targetRevisionId,
        assetVersionId: 'asset-old',
        anchor: { kind: 'asset' },
        availability: { kind: 'ready' },
      },
    ],
  }
}

it('shows only selected archive targets but shows every target for an empty snapshot selection', () => {
  expect(displayHistoryFeedback(history, firstEntry(history))).toEqual([
    feedback(historicalKey, '历史意见'),
  ])

  const snapshotHistory = structuredClone(history)
  const snapshotEntry = firstEntry(snapshotHistory)
  snapshotHistory.selector = { kind: 'snapshot', snapshot: snapshotEntry.snapshot }
  snapshotEntry.selected = []
  expect(displayHistoryFeedback(snapshotHistory, snapshotEntry)).toHaveLength(2)
})

it('does not offer a duplicate restore when the exact historical target is already current', () => {
  const state = restoreTargetState(history.restoreActions, [feedback(historicalKey, '历史意见')])
  expect(state.alreadyCurrent).toEqual([historicalKey])
  expect(state.selectable).toEqual([anotherHistoricalKey])
})

it('requires an explicit choice only for selected conflicts and preserves current is not a write', () => {
  const state = restoreTargetState(history.restoreActions, [
    feedback({ ...historicalKey, targetRevisionId: 'target-current' }, '新意见'),
  ])
  expect(state.conflicts).toEqual([historicalKey])
  expect(
    restoreSubmission([historicalKey], state.conflicts, {
      [keyOf(historicalKey)]: { kind: 'preserve_current' },
    }),
  ).toEqual({ kind: 'empty' })
  expect(
    restoreSubmission([historicalKey], state.conflicts, {
      [keyOf(historicalKey)]: { kind: 'use_historical' },
    }),
  ).toEqual({
    kind: 'preview',
    decisions: [{ historicalKey, choice: { kind: 'use_historical' } }],
  })
})

it('keeps every selected target as a separate typed continuation candidate', () => {
  expect(historyRefsForEntries(history, 'project-1', 'stream-1')).toEqual([
    {
      projectId: 'project-1',
      streamId: 'stream-1',
      source: { kind: 'snapshot', snapshot: firstEntry(history).snapshot, keys: [historicalKey] },
    },
  ])
})

function firstEntry(value: ReviewHistoryView) {
  const entry = value.entries[0]
  if (entry === undefined) throw new Error('Fixture requires one history entry')
  return entry
}

function keyOf(key: ReviewTargetVersionKey) {
  return `${key.feedbackId}\u0000${key.textRevisionId}\u0000${key.targetId}\u0000${key.targetRevisionId}`
}
