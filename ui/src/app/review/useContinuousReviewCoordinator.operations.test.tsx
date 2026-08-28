import { act, renderHook, waitFor } from '@testing-library/react'
import { expect, it, vi } from 'vitest'
import type {
  ReviewHistoryRef,
  ReviewHistoryView,
  ReviewRecoveryDraft,
  ReviewSourceBindingDecision,
  ReviewWorkspaceCommand,
} from '../../api/reviewWorkspaceTypes'
import {
  archivePlan,
  archiveSelection,
  context,
  failure,
  input,
  migrationInspection,
  reviewPort,
  session,
  targetKey,
  workspace,
} from './continuousReviewTestFixtures'
import {
  type ContinuousReviewCoordinator,
  useContinuousReviewCoordinator,
} from './useContinuousReviewCoordinator'

const historyRef: ReviewHistoryRef = {
  projectId: context.projectId,
  streamId: context.streamId,
  source: {
    kind: 'snapshot',
    snapshot: { snapshotId: 'historical', blake3: 'ab'.repeat(32) },
    keys: [targetKey],
  },
}
const source: ReviewSourceBindingDecision = {
  targetKey,
  newAssetVersionId: 'new-asset',
  anchor: { kind: 'asset' },
  confirmation: { kind: 'user_confirmed' },
}
const legacyRef: ReviewHistoryRef = {
  projectId: context.projectId,
  streamId: context.streamId,
  source: {
    kind: 'legacy',
    roundId: 'round-1',
    recordBlake3: '12'.repeat(32),
    targets: [{ roundId: 'round-1', feedbackId: 'legacy-feedback', targetIndex: 0 }],
  },
}

const operations: {
  name: string
  run: (coordinator: ContinuousReviewCoordinator) => Promise<void>
  command: ReviewWorkspaceCommand
}[] = [
  {
    name: 'withdraw',
    run: (coordinator) => coordinator.withdrawTargets([targetKey]),
    command: { kind: 'withdraw', targets: [targetKey] },
  },
  {
    name: 'continue historical',
    run: (coordinator) => coordinator.continueHistorical(historyRef, [source]),
    command: { kind: 'continue_historical', historyRef, bindings: [source] },
  },
  {
    name: 'confirm source',
    run: (coordinator) => coordinator.confirmSource(source),
    command: { kind: 'confirm_source', ...source },
  },
  {
    name: 'confirm applicability',
    run: (coordinator) => coordinator.confirmApplicability(targetKey, 'asset-1', { kind: 'asset' }),
    command: {
      kind: 'confirm_applicability',
      key: targetKey,
      assetVersionId: 'asset-1',
      anchor: { kind: 'asset' },
    },
  },
  {
    name: 'continue legacy',
    run: (coordinator) => coordinator.continueLegacy(legacyRef, []),
    command: { kind: 'continue_legacy', historyRef: legacyRef, bindings: [] },
  },
  {
    name: 'adopt usage',
    run: (coordinator) => coordinator.adoptUsage('usage-1'),
    command: { kind: 'adopt_usage', declarationId: 'usage-1' },
  },
]

it.each(operations)(
  '$name preserves exact identities and guards the displayed snapshot',
  async ({ run, command }) => {
    const port = reviewPort(workspace('base'))
    const { result } = renderHook(() => useContinuousReviewCoordinator({ ...session, port }))
    await waitFor(() => expect(result.current.state.kind).toBe('ready'))
    await act(() => run(result.current))
    expect(port.prepareCommand).toHaveBeenCalledOnce()
    expect(vi.mocked(port.prepareCommand).mock.calls[0]?.[0]).toMatchObject({
      ...session,
      expectedSnapshotId: 'base',
      command,
    })
    expect(result.current.view?.current?.reference.snapshotId).toBe('saved-snapshot')
  },
)

it.each(operations)('$name cannot write to a read-only project', async ({ run }) => {
  const view = workspace('base')
  view.capabilities = { continuousEditing: false, migration: false, usageImport: false }
  const port = reviewPort(view)
  const { result } = renderHook(() => useContinuousReviewCoordinator({ ...session, port }))
  await waitFor(() => expect(result.current.state.kind).toBe('unavailable'))
  await act(async () => {
    await expect(run(result.current)).rejects.toMatchObject({ code: 'read_only' })
  })
  expect(port.prepareCommand).not.toHaveBeenCalled()
})

it('usage inspection returns the verified candidate without adopting it, and failed import does not block manual archive', async () => {
  const port = reviewPort(workspace('base'))
  const preview = {
    declaration: {
      id: 'usage-1',
      projectId: 'project-1',
      streamId: 'stream-1',
      basis: { snapshotId: 'base', blake3: '12'.repeat(32) },
      targets: [targetKey],
      outputs: [],
    },
    canonicalDigest: '34'.repeat(32),
    source: 'producer/usage.json',
    sourceDigest: '56'.repeat(32),
    outputs: [],
  }
  vi.mocked(port.inspectUsage)
    .mockResolvedValueOnce(preview)
    .mockRejectedValueOnce(failure('usage_invalid', false))
  vi.mocked(port.previewArchive).mockResolvedValue(archivePlan())
  const { result } = renderHook(() => useContinuousReviewCoordinator({ ...session, port }))
  await waitFor(() => expect(result.current.state.kind).toBe('ready'))
  await act(async () => {
    expect(await result.current.inspectUsage('usage-file')).toEqual(preview)
  })
  expect(port.inspectUsage).toHaveBeenCalledWith({ ...session, entityId: 'usage-file' })
  expect(port.prepareCommand).not.toHaveBeenCalled()
  await act(async () => {
    await expect(result.current.inspectUsage('invalid-file')).rejects.toMatchObject({
      code: 'usage_invalid',
    })
  })
  expect(result.current.state.kind).toBe('ready')
  await act(() => result.current.previewArchive(archiveSelection()))
  await act(() => result.current.commitArchive())
  expect(vi.mocked(port.prepareCommand).mock.calls[0]?.[0].command).toMatchObject({
    kind: 'archive',
    groups: [{ basis: { kind: 'unknown' } }],
  })
})

it('exposes migration as a separate state and writes only after explicit confirmation of its inspection', async () => {
  const view = workspace()
  view.migration = migrationInspection()
  view.capabilities = { continuousEditing: false, usageImport: false, migration: true }
  const port = reviewPort(view)
  vi.mocked(port.inspectMigration).mockResolvedValue(view.migration)
  const { result } = renderHook(() => useContinuousReviewCoordinator({ ...session, port }))
  await waitFor(() => expect(result.current.state.kind).toBe('migration_required'))
  expect(result.current.view?.current).toBeNull()
  expect(port.prepareCommand).not.toHaveBeenCalled()
  act(() => {
    expect(result.current.beginEditor(input)).toBe(false)
  })
  const plan = {
    inspectionDigest: view.migration.inspectionDigest,
    choice: { kind: 'keep_history_only' as const },
  }
  await act(async () => {
    expect(await result.current.inspectMigration()).toEqual(view.migration)
  })
  await act(() => result.current.migrate(plan))
  expect(vi.mocked(port.prepareCommand).mock.calls[0]?.[0]).toMatchObject({
    ...session,
    expectedSnapshotId: null,
    command: { kind: 'migrate', ...plan },
  })
  expect(result.current.state.kind).toBe('ready')
})

it('rejects a migration plan not matching the displayed or explicitly inspected legacy contents', async () => {
  const view = workspace()
  view.migration = migrationInspection()
  view.capabilities = { continuousEditing: false, usageImport: false, migration: true }
  const port = reviewPort(view)
  const { result } = renderHook(() => useContinuousReviewCoordinator({ ...session, port }))
  await waitFor(() => expect(result.current.state.kind).toBe('migration_required'))
  await act(async () => {
    await expect(
      result.current.migrate({
        inspectionDigest: 'old-inspection',
        choice: { kind: 'keep_history_only' },
      }),
    ).rejects.toMatchObject({ code: 'preview_required' })
  })
  expect(port.prepareCommand).not.toHaveBeenCalled()
})

it('history, evidence and asset preparation do not replace the current snapshot or publish feedback', async () => {
  const port = reviewPort(workspace('base'))
  const selector = { kind: 'archive' as const, archiveId: 'archive-1' }
  const history: ReviewHistoryView = {
    selector,
    entries: [],
    legacy: null,
    limitations: ['background_only'],
    restoreActions: [],
  }
  const image = {
    assetVersionId: 'historical-asset',
    role: 'base' as const,
    url: 'viewer-image://temporary',
    width: 100,
    height: 100,
    sourceWidth: 1000,
    sourceHeight: 1000,
  }
  const assets = [
    {
      asset: {
        id: 'asset-1',
        sourceEntityId: 'image-1',
        relativePath: 'image.jpg',
        evidence: { sizeBytes: 123, modifiedNs: '1234567890123456789', blake3: 'ab'.repeat(32) },
        media: { kind: 'image' as const, width: 100, height: 100 },
        producerAssetId: null,
        parentAssetVersionId: null,
      },
      preview: { ...image, assetVersionId: 'asset-1' },
    },
  ]
  vi.mocked(port.getHistory).mockResolvedValue(history)
  vi.mocked(port.getEvidence).mockResolvedValue(image)
  vi.mocked(port.prepareAssets).mockResolvedValue(assets)
  const { result } = renderHook(() => useContinuousReviewCoordinator({ ...session, port }))
  await waitFor(() => expect(result.current.state.kind).toBe('ready'))
  await act(async () => {
    expect(await result.current.getHistory(selector)).toEqual(history)
    expect(await result.current.getEvidence(selector, 'historical-asset', 'base')).toEqual(image)
    expect(await result.current.prepareAssets(['image-1'])).toEqual(assets)
  })
  expect(port.getEvidence).toHaveBeenCalledWith({
    ...session,
    selector,
    assetVersionId: 'historical-asset',
    role: 'base',
  })
  expect(port.prepareAssets).toHaveBeenCalledWith({ ...session, entityIds: ['image-1'] })
  expect(result.current.view?.current?.reference.snapshotId).toBe('base')
  expect(port.prepareCommand).not.toHaveBeenCalled()
})

it('unresolved recovery remains unpublished and blocks new writes without automatically replaying input', async () => {
  const recovery: ReviewRecoveryDraft = {
    streamId: 'stream-1',
    commandId: 'failed-command',
    expectedSnapshotId: null,
    payloadDigest: 'ab'.repeat(32),
    failure: 'commit_unknown',
    editorInput: {
      migration: null,
      selections: [],
      text: '必须保留的恢复原文',
      feedbackId: null,
      targets: [],
      historyRef: null,
    },
  }
  const view = workspace()
  view.recovery = [recovery]
  const port = reviewPort(view)
  const { result } = renderHook(() => useContinuousReviewCoordinator({ ...session, port }))
  await waitFor(() => expect(result.current.state.kind).toBe('recovery_required'))
  act(() => {
    expect(result.current.beginEditor(input)).toBe(false)
  })
  await act(() => result.current.retry())
  expect(result.current.view?.recovery[0]?.editorInput.text).toBe('必须保留的恢复原文')
  expect(result.current.pendingEnvelope).toBeNull()
  expect(port.prepareCommand).not.toHaveBeenCalled()
  expect(port.applyCommand).not.toHaveBeenCalled()
})
