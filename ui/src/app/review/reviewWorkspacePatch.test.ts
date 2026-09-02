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
