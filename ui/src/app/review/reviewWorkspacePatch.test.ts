import { expect, it } from 'vitest'
import type { ReviewWorkspacePatch } from '../../api/reviewWorkspaceTypes'
import { workspace } from './continuousReviewTestFixtures'
import { applyReviewWorkspacePatch, ReviewPatchMismatch } from './reviewWorkspacePatch'

function patch(basisSnapshotId: string | null = 'base'): ReviewWorkspacePatch {
  return {
    projectId: 'project-1',
    streamId: 'stream-1',
    parent:
      basisSnapshotId === null ? null : { snapshotId: basisSnapshotId, blake3: 'ab'.repeat(32) },
    basisSnapshotId,
    head: { sequence: 2, snapshotId: 'next' },
    upsertAssets: [],
    removeAssetVersionIds: [],
    upsertFeedback: [],
    removeFeedbackIds: [],
    projection: { actionable: [], needsConfirmation: [] },
    historySelectors: null,
  }
}

it('creates the first authoring state from explicit patch identity without a full reread', () => {
  const before = workspace()

  const after = applyReviewWorkspacePatch(before, patch(null))

  expect(after.current?.authoring).toEqual({
    head: { sequence: 2, snapshotId: 'next' },
    state: {
      projectId: 'project-1',
      streamId: 'stream-1',
      snapshotId: 'next',
      parent: null,
      assets: [],
      feedback: [],
    },
  })
  expect(after.current?.publishedRef).toBeNull()
  expect(after.current?.evidence).toEqual([])
})

it('advances only the authoring view and preserves last-known publication evidence', () => {
  const before = workspace('base')
  const publishedRef = before.current?.publishedRef
  const evidence = before.current?.evidence

  const after = applyReviewWorkspacePatch(before, patch())

  expect(after.current?.authoring.head).toEqual({ sequence: 2, snapshotId: 'next' })
  expect(after.current?.authoring.state.snapshotId).toBe('next')
  expect(after.current?.publishedRef).toBe(publishedRef)
  expect(after.current?.evidence).toBe(evidence)
})

it('patches extended geometry in place without replacing publication or unrelated workspace state', () => {
  const before = workspace('base')
  if (before.current === null) throw new Error('Expected current workspace')
  before.current.authoring.state.feedback = [
    {
      id: 'feedback-1',
      textRevisionId: 'text-1',
      text: '调整箭头方向',
      createdAtMs: 1,
      historyRef: null,
      targets: [
        {
          id: 'target-1',
          revisionId: 'revision-1',
          assetVersionId: 'asset-1',
          anchor: { kind: 'image_point', x: 0.2, y: 0.3 },
          availability: { kind: 'ready' },
        },
      ],
    },
  ]
  before.projection.actionable = ['target-1']
  const originalFeedback = before.current.authoring.state.feedback[0]
  const originalTarget = originalFeedback?.targets[0]
  if (originalFeedback === undefined || originalTarget === undefined)
    throw new Error('Expected feedback target')
  const delta = patch()
  delta.projection.actionable = ['target-1']
  delta.upsertFeedback = [
    {
      ...originalFeedback,
      targets: [
        {
          ...originalTarget,
          revisionId: 'revision-2',
          anchor: {
            kind: 'image_arrow',
            tail: { x: 0.2, y: 0.3 },
            head: { x: 0.7, y: 0.6 },
          },
        },
      ],
    },
  ]
  const publishedRef = before.current.publishedRef
  const evidence = before.current.evidence

  const after = applyReviewWorkspacePatch(before, delta)

  expect(after.current?.authoring.state.feedback[0]).toMatchObject({
    id: 'feedback-1',
    textRevisionId: 'text-1',
    targets: [
      {
        id: 'target-1',
        revisionId: 'revision-2',
        anchor: { kind: 'image_arrow' },
      },
    ],
  })
  expect(after.current?.publishedRef).toBe(publishedRef)
  expect(after.current?.evidence).toBe(evidence)
})

it('rejects stale bases and conflicting identities without partially changing the view', () => {
  const before = workspace('base')
  expect(() => applyReviewWorkspacePatch(before, patch('stale'))).toThrow(ReviewPatchMismatch)

  const invalid = patch()
  invalid.removeAssetVersionIds = ['asset-1']
  invalid.upsertAssets = [
    {
      id: 'asset-1',
      sourceEntityId: null,
      relativePath: 'asset.png',
      evidence: { sizeBytes: 1, modifiedNs: '1', blake3: null },
      media: { kind: 'image', width: 1, height: 1 },
      producerAssetId: null,
      parentAssetVersionId: null,
    },
  ]
  expect(() => applyReviewWorkspacePatch(before, invalid)).toThrow(ReviewPatchMismatch)
  expect(before.current?.authoring.head.snapshotId).toBe('base')
})
