import { expect, it, vi } from 'vitest'
import type {
  PreparedReviewAsset,
  ReviewTargetVersionKey,
  ReviewWorkspaceView,
} from '../../api/reviewWorkspaceTypes'
import type { ContinuousReviewSnapshot } from './continuousReviewModel'
import {
  continuousImageReviewWorkbenchAdapter,
  type ImageReviewPreparation,
} from './imageReviewWorkbenchAdapter'
import type { ContinuousReviewCoordinator } from './useContinuousReviewCoordinator'

const RECT = { kind: 'image_rect' as const, x: 0.1, y: 0.2, width: 0.3, height: 0.4 }
const KEY: ReviewTargetVersionKey = {
  feedbackId: 'feedback-1',
  textRevisionId: 'text-1',
  targetId: 'target-1',
  targetRevisionId: 'target-revision-1',
}

function asset(entityId: string, id: string) {
  return {
    id,
    sourceEntityId: entityId,
    relativePath: `${entityId}.png`,
    evidence: { sizeBytes: 100, modifiedNs: '1', blake3: 'ab'.repeat(32) },
    media: { kind: 'image' as const, width: 640, height: 480 },
    producerAssetId: null,
    parentAssetVersionId: null,
  }
}

function prepared(entityId: string, id: string): PreparedReviewAsset {
  return {
    asset: asset(entityId, id),
    preview: {
      assetVersionId: id,
      role: 'base',
      url: `viewer-review-image://localhost/${id}`,
      width: 640,
      height: 480,
      sourceWidth: 640,
      sourceHeight: 480,
    },
  }
}

function view(): ReviewWorkspaceView {
  return {
    streamId: 'stream-1',
    historySelectors: [],
    current: {
      authoring: {
        head: { sequence: 1, snapshotId: 'snapshot-1' },
        state: {
          projectId: 'project-1',
          streamId: 'stream-1',
          snapshotId: 'snapshot-1',
          parent: null,
          assets: [asset('image-1', 'asset-1')],
          feedback: [
            {
              id: KEY.feedbackId,
              textRevisionId: KEY.textRevisionId,
              text: '调整领口',
              createdAtMs: 10,
              historyRef: null,
              targets: [
                {
                  id: KEY.targetId,
                  revisionId: KEY.targetRevisionId,
                  assetVersionId: 'asset-1',
                  anchor: RECT,
                  availability: { kind: 'ready' },
                },
              ],
            },
          ],
        },
      },
      publishedRef: { snapshotId: 'snapshot-1', blake3: 'cd'.repeat(32) },
      evidence: [],
    },
    sourceChecks: [],
    projection: { actionable: [KEY.targetId], needsConfirmation: [] },
    recovery: [],
    migration: null,
    capabilities: { continuousEditing: true, usageImport: true, migration: false },
  }
}

function coordinator(currentView = view(), workbenchSessionKey = 'session-1:1:instance-1') {
  let snapshot: ContinuousReviewSnapshot = {
    state: { kind: 'ready' },
    view: currentView,
    editorInput: {
      contextKey: null,
      feedbackId: null,
      text: '',
      targets: [],
      baseSnapshotId: null,
    },
    pendingEnvelope: null,
    lastReceipt: null,
    lastChangedAssetVersionIds: [],
    error: null,
  }
  return {
    value: {
      ...snapshot,
      workbenchSessionKey,
      getSnapshot: vi.fn(() => snapshot),
      prepareAssets: vi.fn(async (entityIds: string[]) =>
        entityIds.map((entityId) => prepared(entityId, `asset-${entityId.at(-1)}`)),
      ),
      beginEditor: vi.fn(() => true),
      setEditorText: vi.fn(),
      setEditorTargets: vi.fn(() => true),
      discardEditor: vi.fn(() => true),
      saveFeedback: vi.fn(async () => undefined),
      withdrawTargets: vi.fn(async () => undefined),
    } as unknown as ContinuousReviewCoordinator,
    setSnapshot(next: ContinuousReviewSnapshot) {
      snapshot = next
    },
  }
}

it('maps one continuous target to a stable workbench item with separate display ordinal', async () => {
  const review = coordinator()
  const adapter = continuousImageReviewWorkbenchAdapter(review.value)
  const preparation = await adapter.prepareEntity('image-1')

  expect(adapter.view('image-1', preparation).feedback).toEqual([
    {
      itemId: 'target-1',
      feedbackId: 'feedback-1',
      targetKey: KEY,
      assetVersionId: 'asset-1',
      ordinal: 1,
      text: '调整领口',
      createdAtMs: 10,
      anchor: RECT,
    },
  ])
})

it('adds a newly opened image with its prepared asset instead of a fixed member scope', async () => {
  const review = coordinator()
  const adapter = continuousImageReviewWorkbenchAdapter(review.value)
  const preparation = await adapter.prepareEntity('image-2')
  const current = adapter.view('image-2', preparation)

  expect(current.readOnlyReason).toBeNull()
  await adapter.saveFeedback({
    entityId: 'image-2',
    item: null,
    operation: 'text',
    text: '修正袖口',
    anchor: RECT,
    preparation,
  })

  expect(review.value.beginEditor).toHaveBeenCalledWith({
    contextKey: 'new:image-2',
    feedbackId: null,
    text: '修正袖口',
    targets: [{ kind: 'add', assetVersionId: 'asset-2', anchor: RECT }],
  })
  expect(review.value.saveFeedback).toHaveBeenCalledOnce()
})

it('uses the client mutation id to keep a new opinion retry bound to one provisional scene item', async () => {
  const review = coordinator()
  const adapter = continuousImageReviewWorkbenchAdapter(review.value)
  const preparation = await adapter.prepareEntity('image-2')

  await adapter.saveFeedback({
    entityId: 'image-2',
    clientMutationId: 'mutation-42',
    item: null,
    operation: 'text',
    text: '修正袖口',
    anchor: RECT,
    preparation,
  })

  expect(review.value.beginEditor).toHaveBeenCalledWith(
    expect.objectContaining({ contextKey: 'new:image-2:mutation-42' }),
  )
})

it('keeps an orphaned retained asset version from blocking the prepared current version', async () => {
  const orphaned = view()
  if (orphaned.current === null) throw new Error('Expected current continuous review fixture')
  orphaned.current.authoring.state.assets = [asset('image-1', 'asset-old')]
  orphaned.current.authoring.state.feedback = []
  orphaned.projection = { actionable: [], needsConfirmation: [] }
  const review = coordinator(orphaned)
  const adapter = continuousImageReviewWorkbenchAdapter(review.value)
  const preparation = await adapter.prepareEntity('image-1')

  expect(adapter.view('image-1', preparation)).toMatchObject({
    feedback: [],
    readOnlyReason: null,
  })
  await adapter.saveFeedback({
    entityId: 'image-1',
    item: null,
    operation: 'text',
    text: '新版本可正常编辑',
    anchor: RECT,
    preparation,
  })

  expect(review.value.beginEditor).toHaveBeenCalledWith({
    contextKey: 'new:image-1',
    feedbackId: null,
    text: '新版本可正常编辑',
    targets: [{ kind: 'add', assetVersionId: 'asset-1', anchor: RECT }],
  })
  expect(review.value.saveFeedback).toHaveBeenCalledOnce()
})

it('edits text by feedback identity and redraws or withdraws only the selected target version', async () => {
  const review = coordinator()
  const adapter = continuousImageReviewWorkbenchAdapter(review.value)
  const preparation = await adapter.prepareEntity('image-1')
  const item = adapter.view('image-1', preparation).feedback[0]
  if (!item) throw new Error('Missing feedback item')

  await adapter.saveFeedback({
    entityId: 'image-1',
    item,
    operation: 'text',
    text: '调整领口边缘',
    anchor: RECT,
    preparation,
  })
  expect(review.value.beginEditor).toHaveBeenLastCalledWith({
    contextKey: 'target-1',
    feedbackId: 'feedback-1',
    text: '调整领口边缘',
    targets: [],
  })

  await adapter.saveFeedback({
    entityId: 'image-1',
    item,
    operation: 'geometry',
    text: item.text,
    anchor: { ...RECT, x: 0.2 },
    preparation,
  })
  expect(review.value.beginEditor).toHaveBeenLastCalledWith({
    contextKey: 'target-1',
    feedbackId: 'feedback-1',
    text: '调整领口',
    targets: [
      {
        kind: 'redraw',
        key: KEY,
        assetVersionId: 'asset-1',
        anchor: { ...RECT, x: 0.2 },
      },
    ],
  })

  await adapter.deleteFeedback({ entityId: 'image-1', item, preparation })
  expect(review.value.withdrawTargets).toHaveBeenCalledWith([KEY])
})

it('keeps tools disabled until the exact prepared preview is safe and current', async () => {
  const changed = view()
  changed.sourceChecks = [{ assetVersionId: 'asset-1', checkedAtMs: 20, status: 'changed' }]
  const review = coordinator(changed)
  const adapter = continuousImageReviewWorkbenchAdapter(review.value)
  const good = await adapter.prepareEntity('image-1')
  const mismatched: ImageReviewPreparation = {
    key: good.key,
    prepared: good.prepared && {
      ...good.prepared,
      preview: good.prepared.preview && {
        ...good.prepared.preview,
        assetVersionId: 'different-version',
      },
    },
  }

  expect(adapter.view('image-1', null).readOnlyReason).toBe('loading')
  expect(adapter.view('image-1', mismatched).readOnlyReason).toBe('source_confirmation')
  expect(adapter.view('image-1', good).readOnlyReason).toBe('source_confirmation')
  await expect(
    adapter.saveFeedback({
      entityId: 'image-1',
      item: null,
      operation: 'text',
      text: '不应写入错误版本',
      anchor: RECT,
      preparation: mismatched,
    }),
  ).rejects.toMatchObject({ code: 'preview_required' })
  expect(review.value.beginEditor).not.toHaveBeenCalled()

  await expect(
    adapter.saveFeedback({
      entityId: 'image-1',
      item: null,
      operation: 'text',
      text: '也不应绕过来源确认',
      anchor: RECT,
      preparation: good,
    }),
  ).rejects.toMatchObject({ code: 'needs_confirmation' })
  expect(review.value.beginEditor).not.toHaveBeenCalled()
})

it('retries the retained editor input without regenerating its identity or target selection', async () => {
  const review = coordinator()
  const adapter = continuousImageReviewWorkbenchAdapter(review.value)
  const preparation = await adapter.prepareEntity('image-1')
  const item = adapter.view('image-1', preparation).feedback[0]
  if (!item) throw new Error('Missing feedback item')
  vi.mocked(review.value.beginEditor).mockReturnValue(false)
  review.setSnapshot({
    ...review.value.getSnapshot(),
    state: {
      kind: 'save_failed',
      error: {
        code: 'io',
        message: '写入失败',
        retryable: true,
        committedReceipt: null,
        committedAuthoringReceipt: null,
      },
    },
    editorInput: {
      contextKey: 'target-1',
      feedbackId: 'feedback-1',
      text: '调整领口',
      targets: [
        {
          kind: 'redraw',
          key: KEY,
          assetVersionId: 'asset-1',
          anchor: RECT,
        },
      ],
      baseSnapshotId: 'snapshot-1',
    },
  })

  await adapter.saveFeedback({
    entityId: 'image-1',
    item,
    operation: 'geometry',
    text: item.text,
    anchor: RECT,
    preparation,
  })

  expect(review.value.setEditorText).not.toHaveBeenCalled()
  expect(review.value.setEditorTargets).not.toHaveBeenCalled()
  expect(review.value.saveFeedback).toHaveBeenCalledOnce()
})

it('turns an explicit text change after a definite failure into a new retained payload', async () => {
  const review = coordinator()
  const adapter = continuousImageReviewWorkbenchAdapter(review.value)
  const preparation = await adapter.prepareEntity('image-1')
  const item = adapter.view('image-1', preparation).feedback[0]
  if (!item) throw new Error('Missing feedback item')
  vi.mocked(review.value.beginEditor).mockReturnValue(false)
  review.setSnapshot({
    ...review.value.getSnapshot(),
    state: {
      kind: 'save_failed',
      error: {
        code: 'io',
        message: '写入失败',
        retryable: true,
        committedReceipt: null,
        committedAuthoringReceipt: null,
      },
    },
    editorInput: {
      contextKey: 'target-1',
      feedbackId: 'feedback-1',
      text: '调整领口',
      targets: [],
      baseSnapshotId: 'snapshot-1',
    },
  })

  await adapter.saveFeedback({
    entityId: 'image-1',
    item,
    operation: 'text',
    text: '调整领口并保留材质',
    anchor: RECT,
    preparation,
  })

  expect(review.value.setEditorText).toHaveBeenCalledWith('调整领口并保留材质')
  expect(review.value.saveFeedback).toHaveBeenCalledOnce()
})

it('discards retained definite-failure input but preserves reconciliation input', () => {
  const review = coordinator()
  const adapter = continuousImageReviewWorkbenchAdapter(review.value)
  review.setSnapshot({
    ...review.value.getSnapshot(),
    editorInput: {
      contextKey: 'target-1',
      feedbackId: 'feedback-1',
      text: '调整领口',
      targets: [],
      baseSnapshotId: 'snapshot-1',
    },
  })

  expect(adapter.discardPendingInput()).toBe(true)
  expect(review.value.discardEditor).toHaveBeenCalledOnce()
  vi.mocked(review.value.discardEditor).mockReturnValue(false)
  expect(adapter.discardPendingInput()).toBe(false)
})

it('reports a saved target that still needs confirmation without claiming agent consumption', async () => {
  const initial = view()
  if (initial.current === null) throw new Error('Missing current snapshot')
  initial.current.authoring.state.feedback = []
  initial.projection = { actionable: [], needsConfirmation: [] }
  const review = coordinator(initial)
  const adapter = continuousImageReviewWorkbenchAdapter(review.value)
  const preparation = await adapter.prepareEntity('image-1')
  vi.mocked(review.value.saveFeedback).mockImplementationOnce(async () => {
    const pending = view()
    const target = pending.current?.authoring.state.feedback[0]?.targets[0]
    if (!target) throw new Error('Missing pending target')
    target.availability = {
      kind: 'needs_confirmation',
      reasons: ['applicability_unconfirmed'],
    }
    pending.projection = { actionable: [], needsConfirmation: ['target-1'] }
    review.setSnapshot({ ...review.value.getSnapshot(), view: pending })
  })

  const result = await adapter.saveFeedback({
    entityId: 'image-1',
    item: null,
    operation: 'text',
    text: '修正领口',
    anchor: RECT,
    preparation,
  })

  expect(result.statusMessage).toBe('已保存，待确认')
  expect(result.statusMessage).not.toContain('Agent')
  expect(result.statusMessage).not.toContain('已读取')

  const confirmed = view()
  review.setSnapshot({ ...review.value.getSnapshot(), view: confirmed })
  expect(adapter.view('image-1', preparation).statusMessage).toBe('已保存，可供外部读取')
})

it('never carries one entity save status into another entity view', async () => {
  const review = coordinator()
  const adapter = continuousImageReviewWorkbenchAdapter(review.value)
  const firstPreparation = await adapter.prepareEntity('image-1')
  await adapter.saveFeedback({
    entityId: 'image-1',
    item: null,
    operation: 'text',
    text: '修正领口',
    anchor: RECT,
    preparation: firstPreparation,
  })
  const secondPreparation = await adapter.prepareEntity('image-2')
  const replacementVersion: ImageReviewPreparation = {
    key: firstPreparation.key,
    prepared: prepared('image-1', 'asset-1-replaced'),
  }

  expect(adapter.view('image-1', firstPreparation).statusMessage).toBe('已保存，可供外部读取')
  expect(adapter.view('image-2', secondPreparation).statusMessage).toBeNull()
  expect(adapter.view('image-1', replacementVersion).statusMessage).toBeNull()
})

it('maps publication progress for the saved asset to the exact Agent-facing status copy', async () => {
  const review = coordinator()
  const adapter = continuousImageReviewWorkbenchAdapter(review.value)
  const preparation = await adapter.prepareEntity('image-1')
  const base = review.value.getSnapshot()

  review.setSnapshot({
    ...base,
    state: { kind: 'saved_pending_publication', pendingRevisions: 2 },
    lastChangedAssetVersionIds: ['asset-1'],
  })
  expect(adapter.view('image-1', preparation).statusMessage).toBe('已保存，Agent 数据生成中')

  review.setSnapshot({
    ...review.value.getSnapshot(),
    state: { kind: 'publication_blocked', code: 'source_changed' },
  })
  expect(adapter.view('image-1', preparation).statusMessage).toBe('评审已保存，Agent 数据生成失败')

  review.setSnapshot({ ...review.value.getSnapshot(), state: { kind: 'ready' } })
  expect(adapter.view('image-1', preparation).statusMessage).toBe('已保存，可供外部读取')
})

it('never claims a deletion is Agent-readable while publication is still pending', async () => {
  const review = coordinator()
  const adapter = continuousImageReviewWorkbenchAdapter(review.value)
  const preparation = await adapter.prepareEntity('image-1')
  const item = adapter.view('image-1', preparation).feedback[0]
  if (!item) throw new Error('Missing feedback item')
  review.setSnapshot({
    ...review.value.getSnapshot(),
    state: { kind: 'saved_pending_publication', pendingRevisions: 1 },
  })

  const result = await adapter.deleteFeedback({ entityId: 'image-1', item, preparation })

  expect(result.statusMessage).toBe('已保存，Agent 数据生成中')
  expect(result.statusMessage).not.toContain('可供外部读取')
})

it('refuses to delete an item retained from a previously opened asset', async () => {
  const review = coordinator()
  const adapter = continuousImageReviewWorkbenchAdapter(review.value)
  const firstPreparation = await adapter.prepareEntity('image-1')
  const staleItem = adapter.view('image-1', firstPreparation).feedback[0]
  if (!staleItem) throw new Error('Missing feedback item')
  const secondPreparation = await adapter.prepareEntity('image-2')
  adapter.view('image-2', secondPreparation)

  await expect(
    adapter.deleteFeedback({
      entityId: 'image-2',
      item: staleItem,
      preparation: secondPreparation,
    }),
  ).rejects.toMatchObject({ code: 'wrong_context' })
  expect(review.value.withdrawTargets).not.toHaveBeenCalled()
})

it('keeps preparation identity scoped to the coordinator session instance', () => {
  const first = continuousImageReviewWorkbenchAdapter(
    coordinator(view(), 'session-1:1:instance-1').value,
  )
  const replacement = continuousImageReviewWorkbenchAdapter(
    coordinator(view(), 'session-1:1:instance-2').value,
  )

  expect(first.preparationKey('image-1')).not.toBe(replacement.preparationKey('image-1'))
})

it('deletes with explicit current context even when React recreated the adapter', async () => {
  const review = coordinator()
  const firstAdapter = continuousImageReviewWorkbenchAdapter(review.value)
  const preparation = await firstAdapter.prepareEntity('image-1')
  const item = firstAdapter.view('image-1', preparation).feedback[0]
  if (!item) throw new Error('Missing feedback item')

  const recreatedAdapter = continuousImageReviewWorkbenchAdapter(review.value)
  await recreatedAdapter.deleteFeedback({ entityId: 'image-1', item, preparation })

  expect(review.value.withdrawTargets).toHaveBeenCalledWith([KEY])
})

it('rejects a text edit when the target or text revision changed after editing began', async () => {
  const review = coordinator()
  const adapter = continuousImageReviewWorkbenchAdapter(review.value)
  const preparation = await adapter.prepareEntity('image-1')
  const frozenItem = adapter.view('image-1', preparation).feedback[0]
  if (!frozenItem) throw new Error('Missing feedback item')
  const refreshed = view()
  const refreshedFeedback = refreshed.current?.authoring.state.feedback[0]
  const refreshedTarget = refreshedFeedback?.targets[0]
  if (!refreshedFeedback || !refreshedTarget) throw new Error('Missing refreshed target')
  refreshedFeedback.textRevisionId = 'text-2'
  refreshedTarget.revisionId = 'target-revision-2'
  review.setSnapshot({ ...review.value.getSnapshot(), view: refreshed })

  await expect(
    adapter.saveFeedback({
      entityId: 'image-1',
      item: frozenItem,
      operation: 'text',
      text: '不覆盖并发更新',
      anchor: RECT,
      preparation,
    }),
  ).rejects.toMatchObject({ code: 'needs_confirmation' })
  expect(review.value.beginEditor).not.toHaveBeenCalled()
})

it('turns a same-path source change failure into an explicit confirmation state', async () => {
  const review = coordinator()
  review.setSnapshot({
    ...review.value.getSnapshot(),
    state: {
      kind: 'save_failed',
      error: {
        code: 'source_changed',
        message: '素材已被覆盖',
        retryable: false,
        committedReceipt: null,
        committedAuthoringReceipt: null,
      },
    },
  })
  const adapter = continuousImageReviewWorkbenchAdapter(review.value)
  const preparation = await adapter.prepareEntity('image-1')

  expect(adapter.view('image-1', preparation).readOnlyReason).toBe('source_confirmation')
})
