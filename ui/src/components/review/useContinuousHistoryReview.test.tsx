import { act, renderHook, waitFor } from '@testing-library/react'
import { expect, it, vi } from 'vitest'
import type { ReviewHistorySelector } from '../../api/reviewWorkspaceTypes'
import { targetKey } from '../../app/review/continuousReviewTestFixtures'
import type { ContinuousReviewCoordinator } from '../../app/review/useContinuousReviewCoordinator'
import { useContinuousHistoryReview } from './useContinuousHistoryReview'

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((done) => {
    resolve = done
  })
  return { promise, resolve }
}

function coordinator(
  overrides: Partial<ContinuousReviewCoordinator> = {},
): ContinuousReviewCoordinator {
  return {
    workbenchSessionKey: 'session-a',
    view: {
      current: {
        reference: { snapshotId: 'current', blake3: 'c'.repeat(64) },
        production: null,
        state: {
          projectId: 'project-1',
          streamId: 'stream-1',
          snapshotId: 'current',
          parent: null,
          assets: [],
          feedback: [],
        },
        commandId: 'current-command',
        payloadDigest: 'd'.repeat(64),
        changes: [],
        evidence: [],
      },
      streamId: 'stream-1',
      sourceChecks: [],
      projection: { actionable: [], needsConfirmation: [] },
      recovery: [],
      migration: null,
      capabilities: { continuousEditing: true, usageImport: true, migration: false },
    },
    hasUncommittedInput: false,
    getHistory: vi.fn().mockResolvedValue({
      selector: { kind: 'archive', archiveId: 'archive-1' },
      entries: [
        {
          snapshot: { snapshotId: 'old', blake3: 'a'.repeat(64) },
          feedback: [
            {
              id: targetKey.feedbackId,
              textRevisionId: targetKey.textRevisionId,
              text: '历史意见',
              createdAtMs: 1,
              historyRef: null,
              targets: [
                {
                  id: targetKey.targetId,
                  revisionId: targetKey.targetRevisionId,
                  assetVersionId: 'asset-old',
                  anchor: { kind: 'asset' },
                  availability: { kind: 'ready' },
                },
              ],
            },
          ],
          assets: [
            {
              id: 'asset-old',
              sourceEntityId: null,
              relativePath: 'old.png',
              evidence: { sizeBytes: 1, modifiedNs: '1', blake3: 'a'.repeat(64) },
              media: { kind: 'image', width: 10, height: 10 },
              producerAssetId: null,
              parentAssetVersionId: null,
            },
          ],
          evidence: [
            {
              assetVersionId: 'asset-old',
              capability: {
                kind: 'image',
                base: { blake3: 'b'.repeat(64), sizeBytes: 1, width: 10, height: 10 },
                annotated: null,
                annotations: [],
              },
            },
          ],
          selected: [targetKey],
        },
      ],
      legacy: null,
      limitations: ['background_only'],
      restoreActions: [targetKey],
    }),
    getEvidence: vi.fn().mockResolvedValue({
      assetVersionId: 'asset-old',
      role: 'annotated',
      url: 'viewer-image://session-authorized',
      width: 10,
      height: 10,
      sourceWidth: 10,
      sourceHeight: 10,
    }),
    ...overrides,
  } as unknown as ContinuousReviewCoordinator
}

it('uses only coordinator getEvidence output for historical previews and drops an old session result', async () => {
  const old = coordinator()
  const rendered = renderHook(
    ({ value }) =>
      useContinuousHistoryReview(value, [], { kind: 'archive', archiveId: 'archive-1' }),
    { initialProps: { value: old } },
  )
  await act(() => rendered.result.current.open())
  await waitFor(() => expect(rendered.result.current.panel?.history.entries).toHaveLength(1))
  await act(() => rendered.result.current.requestEvidence('asset-old', 'annotated'))
  expect(old.getEvidence).toHaveBeenCalledWith(
    { kind: 'archive', archiveId: 'archive-1' },
    'asset-old',
    'annotated',
  )
  expect(rendered.result.current.panel?.evidence[0]?.url).toBe('viewer-image://session-authorized')
})

it('ignores an older same-session history response after the user opens history again', async () => {
  const first = deferred<Awaited<ReturnType<ContinuousReviewCoordinator['getHistory']>>>()
  const current = coordinator()
  const second = await current.getHistory({ kind: 'archive', archiveId: 'archive-1' })
  const entry = second.entries[0]
  const feedback = entry?.feedback[0]
  if (entry === undefined || feedback === undefined)
    throw new Error('Fixture requires one history entry')
  feedback.text = '第二次读取的历史'
  current.getHistory = vi.fn().mockReturnValueOnce(first.promise).mockResolvedValueOnce(second)
  const rendered = renderHook(() =>
    useContinuousHistoryReview(current, [], { kind: 'archive', archiveId: 'archive-1' }),
  )
  act(() => void rendered.result.current.open())
  await waitFor(() => expect(current.getHistory).toHaveBeenCalledTimes(1))
  act(() => void rendered.result.current.open())
  await waitFor(() =>
    expect(rendered.result.current.panel?.history.entries[0]?.feedback[0]?.text).toBe(
      '第二次读取的历史',
    ),
  )
  await act(async () => {
    first.resolve({
      ...second,
      entries: [
        {
          ...entry,
          feedback: [{ ...feedback, text: '过期历史' }],
        },
      ],
    })
    await first.promise
  })
  expect(rendered.result.current.panel?.history.entries[0]?.feedback[0]?.text).toBe(
    '第二次读取的历史',
  )
})

it('continues one typed historical key only after an explicit source binding, leaving IDs to the coordinator', async () => {
  const current = coordinator({
    prepareAssets: vi.fn().mockResolvedValue([
      {
        asset: {
          id: 'asset-new',
          sourceEntityId: 'new-entity',
          relativePath: 'new.png',
          evidence: { sizeBytes: 2, modifiedNs: '2', blake3: 'b'.repeat(64) },
          media: { kind: 'image', width: 10, height: 10 },
          producerAssetId: null,
          parentAssetVersionId: 'asset-old',
        },
        preview: null,
      },
    ]),
    continueHistorical: vi.fn().mockResolvedValue(undefined),
  })
  const rendered = renderHook(() =>
    useContinuousHistoryReview(current, ['new-entity'], {
      kind: 'archive',
      archiveId: 'archive-1',
    }),
  )
  await act(() => rendered.result.current.open())
  const reference = rendered.result.current.panel?.historyRef
  if (reference === null || reference === undefined)
    throw new Error('Missing typed history reference')
  await act(() => rendered.result.current.continueHistorical(reference))
  await waitFor(() => expect(rendered.result.current.source?.candidates[0]?.id).toBe('asset-new'))
  await act(() =>
    rendered.result.current.confirmSource({
      targetKey,
      newAssetVersionId: 'asset-new',
      anchor: { kind: 'asset' },
      confirmation: { kind: 'user_confirmed' },
    }),
  )
  expect(current.continueHistorical).toHaveBeenCalledWith(reference, [
    {
      targetKey,
      newAssetVersionId: 'asset-new',
      anchor: { kind: 'asset' },
      confirmation: { kind: 'user_confirmed' },
    },
  ])
})

it('does not commit a restore when the confirmed preview restores no targets', async () => {
  const current = coordinator({
    previewRestore: vi.fn().mockResolvedValue({
      expectedSnapshotId: 'current',
      restored: [],
      conflicts: [],
      coverageReversals: [],
      requiresSourceCheck: [],
    }),
    restore: vi.fn().mockResolvedValue(undefined),
  })
  const rendered = renderHook(() =>
    useContinuousHistoryReview(current, [], { kind: 'archive', archiveId: 'archive-1' }),
  )
  await act(() => rendered.result.current.open())
  await act(() =>
    rendered.result.current.restore([
      { historicalKey: targetKey, choice: { kind: 'use_historical' } },
    ]),
  )
  expect(rendered.result.current.panel?.restorePreview?.plan).not.toBeNull()
  await act(() =>
    rendered.result.current.restore([
      { historicalKey: targetKey, choice: { kind: 'use_historical' } },
    ]),
  )
  expect(current.restore).not.toHaveBeenCalled()
  expect(rendered.result.current.notice).toBe('未恢复任何历史目标；当前意见保持不变。')
})

it('commits only after an explicitly reconfirmed partial restore preview', async () => {
  const current = coordinator({
    previewRestore: vi.fn().mockResolvedValue({
      expectedSnapshotId: 'current',
      restored: [targetKey],
      conflicts: [],
      coverageReversals: [],
      requiresSourceCheck: [],
    }),
    restore: vi.fn().mockResolvedValue(undefined),
  })
  const rendered = renderHook(() =>
    useContinuousHistoryReview(current, [], { kind: 'archive', archiveId: 'archive-1' }),
  )
  await act(() => rendered.result.current.open())
  await act(() =>
    rendered.result.current.restore([
      { historicalKey: targetKey, choice: { kind: 'use_historical' } },
    ]),
  )
  expect(current.restore).not.toHaveBeenCalled()
  await act(() =>
    rendered.result.current.restore([
      { historicalKey: targetKey, choice: { kind: 'use_historical' } },
    ]),
  )
  expect(current.restore).toHaveBeenCalledOnce()
  expect(rendered.result.current.notice).toBe('已追加恢复状态；历史记录没有被改写。')
})

it('never commits a restore plan after the user changes the decisions that were previewed', async () => {
  const current = coordinator({
    previewRestore: vi.fn().mockResolvedValue({
      expectedSnapshotId: 'current',
      restored: [targetKey],
      conflicts: [],
      coverageReversals: [],
      requiresSourceCheck: [],
    }),
    restore: vi.fn().mockResolvedValue(undefined),
  })
  const rendered = renderHook(() =>
    useContinuousHistoryReview(current, [], { kind: 'archive', archiveId: 'archive-1' }),
  )
  await act(() => rendered.result.current.open())
  await act(() =>
    rendered.result.current.restore([
      { historicalKey: targetKey, choice: { kind: 'use_historical' } },
    ]),
  )
  await act(() =>
    rendered.result.current.restore([
      {
        historicalKey: targetKey,
        choice: {
          kind: 'continue_as_new',
          feedbackId: 'new-feedback',
          textRevisionId: 'new-text',
          targetId: 'new-target',
          targetRevisionId: 'new-target-revision',
          targetAssetVersionId: 'asset-old',
          confirmedAnchor: null,
          createdAtMs: 2,
        },
      },
    ]),
  )

  expect(current.previewRestore).toHaveBeenCalledTimes(2)
  expect(current.restore).not.toHaveBeenCalled()
})

it('invalidates a reviewed restore plan when the panel selection changes', async () => {
  const current = coordinator({
    previewRestore: vi.fn().mockResolvedValue({
      expectedSnapshotId: 'current',
      restored: [targetKey],
      conflicts: [],
      coverageReversals: [],
      requiresSourceCheck: [],
    }),
  })
  const rendered = renderHook(() =>
    useContinuousHistoryReview(current, [], { kind: 'archive', archiveId: 'archive-1' }),
  )
  await act(() => rendered.result.current.open())
  await act(() =>
    rendered.result.current.restore([
      { historicalKey: targetKey, choice: { kind: 'use_historical' } },
    ]),
  )
  expect(rendered.result.current.panel?.restorePreview).not.toBeNull()

  act(() => rendered.result.current.invalidateRestorePreview())
  expect(rendered.result.current.panel?.restorePreview).toBeNull()
})

it('rejects a continuation ref with zero or multiple snapshot keys instead of selecting keys[0]', async () => {
  const prepareAssets = vi.fn()
  const current = coordinator({ prepareAssets })
  const rendered = renderHook(() =>
    useContinuousHistoryReview(current, ['new-entity'], {
      kind: 'archive',
      archiveId: 'archive-1',
    }),
  )
  await act(() => rendered.result.current.open())
  const reference = rendered.result.current.panel?.historyRef
  if (reference === null || reference === undefined || reference.source.kind !== 'snapshot')
    throw new Error('Fixture requires a snapshot history reference')
  const snapshot = reference.source.snapshot

  await act(() =>
    rendered.result.current.continueHistorical({
      ...reference,
      source: {
        kind: 'snapshot',
        snapshot,
        keys: [targetKey, { ...targetKey, targetId: 'other' }],
      },
    }),
  )

  expect(prepareAssets).not.toHaveBeenCalled()
  expect(rendered.result.current.error).toMatchObject({ code: 'invalid_data' })
})

it('drops a stale source-candidate response when the history selector changes', async () => {
  const pendingAssets =
    deferred<Awaited<ReturnType<ContinuousReviewCoordinator['prepareAssets']>>>()
  const current = coordinator({
    prepareAssets: vi.fn().mockReturnValue(pendingAssets.promise),
  })
  const initialSelector: ReviewHistorySelector = { kind: 'archive', archiveId: 'archive-1' }
  const rendered = renderHook<
    ReturnType<typeof useContinuousHistoryReview>,
    { selector: ReviewHistorySelector }
  >(
    ({ selector }: { selector: ReviewHistorySelector }) =>
      useContinuousHistoryReview(current, ['new-entity'], selector),
    { initialProps: { selector: initialSelector } },
  )
  await act(() => rendered.result.current.open())
  const reference = rendered.result.current.panel?.historyRef
  if (reference === null || reference === undefined)
    throw new Error('Missing typed history reference')
  act(() => void rendered.result.current.continueHistorical(reference))
  await waitFor(() => expect(current.prepareAssets).toHaveBeenCalledOnce())
  rendered.rerender({
    selector: { kind: 'snapshot', snapshot: { snapshotId: 'new', blake3: 'n'.repeat(64) } },
  })
  expect(rendered.result.current.busy).toBe(false)
  await act(async () => {
    pendingAssets.resolve([])
    await pendingAssets.promise
  })
  expect(rendered.result.current.source).toBeNull()
})

it('releases UI busy state when a pending source preparation is closed', async () => {
  const pendingAssets =
    deferred<Awaited<ReturnType<ContinuousReviewCoordinator['prepareAssets']>>>()
  const current = coordinator({ prepareAssets: vi.fn().mockReturnValue(pendingAssets.promise) })
  const rendered = renderHook(() =>
    useContinuousHistoryReview(current, ['new-entity'], {
      kind: 'archive',
      archiveId: 'archive-1',
    }),
  )
  await act(() => rendered.result.current.open())
  const reference = rendered.result.current.panel?.historyRef
  if (reference === null || reference === undefined)
    throw new Error('Fixture requires a history reference')
  act(() => void rendered.result.current.continueHistorical(reference))
  await waitFor(() => expect(rendered.result.current.busy).toBe(true))

  act(() => rendered.result.current.close())
  expect(rendered.result.current.busy).toBe(false)
  expect(rendered.result.current.panel).toBeNull()
})

it('drops a pending source-candidate result after the workbench session is replaced', async () => {
  const pendingAssets =
    deferred<Awaited<ReturnType<ContinuousReviewCoordinator['prepareAssets']>>>()
  const old = coordinator({ prepareAssets: vi.fn().mockReturnValue(pendingAssets.promise) })
  const next = { ...old, workbenchSessionKey: 'session-b' }
  const rendered = renderHook(
    ({ value }) =>
      useContinuousHistoryReview(value, ['new-entity'], {
        kind: 'archive',
        archiveId: 'archive-1',
      }),
    { initialProps: { value: old } },
  )
  await act(() => rendered.result.current.open())
  const reference = rendered.result.current.panel?.historyRef
  if (reference === null || reference === undefined)
    throw new Error('Missing typed history reference')
  act(() => void rendered.result.current.continueHistorical(reference))
  await waitFor(() => expect(old.prepareAssets).toHaveBeenCalledOnce())
  rendered.rerender({ value: next })
  expect(rendered.result.current.busy).toBe(false)
  await act(async () => {
    pendingAssets.resolve([])
    await pendingAssets.promise
  })
  expect(rendered.result.current.panel).toBeNull()
  expect(rendered.result.current.source).toBeNull()
})

it('releases UI busy state and drops candidates when the selected entity changes', async () => {
  const pendingAssets =
    deferred<Awaited<ReturnType<ContinuousReviewCoordinator['prepareAssets']>>>()
  const current = coordinator({ prepareAssets: vi.fn().mockReturnValue(pendingAssets.promise) })
  const rendered = renderHook(
    ({ entities }) =>
      useContinuousHistoryReview(current, entities, { kind: 'archive', archiveId: 'archive-1' }),
    { initialProps: { entities: ['new-entity'] } },
  )
  await act(() => rendered.result.current.open())
  const reference = rendered.result.current.panel?.historyRef
  if (reference === null || reference === undefined)
    throw new Error('Fixture requires a history reference')
  act(() => void rendered.result.current.continueHistorical(reference))
  await waitFor(() => expect(rendered.result.current.busy).toBe(true))

  rendered.rerender({ entities: ['replacement-entity'] })
  expect(rendered.result.current.busy).toBe(false)
  expect(rendered.result.current.source).toBeNull()
})
