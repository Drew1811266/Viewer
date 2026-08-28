import { act, renderHook, waitFor } from '@testing-library/react'
import { expect, it, vi } from 'vitest'
import type { ReviewApplyResult, ReviewWorkspaceView } from '../../api/reviewWorkspaceTypes'
import {
  applied,
  deferred,
  failure,
  input,
  migrationInspection,
  recoveryDraft,
  reviewPort,
  session,
  workspace,
} from './continuousReviewTestFixtures'
import { useContinuousReviewCoordinator } from './useContinuousReviewCoordinator'

it('refresh keeps the editing basis until the user explicitly confirms rebasing retained input', async () => {
  const port = reviewPort(workspace('base'))
  vi.mocked(port.applyCommand).mockRejectedValueOnce(failure('stale_snapshot', false))
  const { result } = renderHook(() => useContinuousReviewCoordinator({ ...session, port }))
  await waitFor(() => expect(result.current.state.kind).toBe('ready'))
  act(() => {
    result.current.beginEditor(input)
  })
  await act(async () => {
    await expect(result.current.saveFeedback()).rejects.toMatchObject({ code: 'stale_snapshot' })
  })
  vi.mocked(port.getWorkspace).mockResolvedValue(workspace('new-base'))
  await act(() => result.current.refresh())
  expect(result.current.editorInput.baseSnapshotId).toBe('base')
  expect(result.current.editorInput.text).toBe(input.text)
  act(() => {
    expect(result.current.rebaseEditor()).toBe(true)
  })
  await act(() => result.current.saveFeedback())
  expect(vi.mocked(port.prepareCommand).mock.calls[1]?.[0].expectedSnapshotId).toBe('new-base')
  expect(vi.mocked(port.prepareCommand).mock.calls[1]?.[0].commandId).not.toBe(
    vi.mocked(port.prepareCommand).mock.calls[0]?.[0].commandId,
  )
})

it('uncertain outcomes cannot be discarded or rebased, even after a refresh', async () => {
  const port = reviewPort(workspace('base'))
  vi.mocked(port.applyCommand).mockRejectedValueOnce(failure('outcome_unknown'))
  const { result } = renderHook(() => useContinuousReviewCoordinator({ ...session, port }))
  await waitFor(() => expect(result.current.state.kind).toBe('ready'))
  act(() => {
    result.current.beginEditor(input)
  })
  await act(async () => {
    await expect(result.current.saveFeedback()).rejects.toMatchObject({ code: 'outcome_unknown' })
  })
  await act(() => result.current.refresh())
  act(() => {
    expect(result.current.discardEditor()).toBe(false)
    expect(result.current.rebaseEditor()).toBe(false)
    expect(result.current.setEditorTargets([])).toBe(false)
  })
  expect(result.current.pendingEnvelope).not.toBeNull()
})

it('retry cannot clear geometry edited after a definite failure; saving the new payload remains explicit', async () => {
  const port = reviewPort(workspace('base'))
  vi.mocked(port.applyCommand).mockRejectedValueOnce(failure('io'))
  const { result } = renderHook(() => useContinuousReviewCoordinator({ ...session, port }))
  await waitFor(() => expect(result.current.state.kind).toBe('ready'))
  act(() => {
    result.current.beginEditor(input)
  })
  await act(async () => {
    await expect(result.current.saveFeedback()).rejects.toMatchObject({ code: 'io' })
  })
  const newTargets = [
    {
      kind: 'add' as const,
      assetVersionId: 'asset-1',
      anchor: { kind: 'image_rect' as const, x: 0.1, y: 0.2, width: 0.3, height: 0.4 },
    },
  ]
  act(() => {
    expect(result.current.setEditorTargets(newTargets)).toBe(true)
  })
  await act(async () => {
    await expect(result.current.retry()).rejects.toMatchObject({ code: 'needs_confirmation' })
  })
  expect(result.current.editorInput.targets).toEqual(newTargets)
  expect(port.applyCommand).toHaveBeenCalledOnce()
  await act(() => result.current.saveFeedback())
  expect(vi.mocked(port.prepareCommand).mock.calls[1]?.[0].command).toMatchObject({
    targets: newTargets,
  })
})

it('retry waits for an in-flight refresh before applying the retained envelope', async () => {
  const port = reviewPort(workspace('base'))
  const read = deferred<ReviewWorkspaceView>()
  vi.mocked(port.applyCommand).mockRejectedValueOnce(failure('io'))
  const { result } = renderHook(() => useContinuousReviewCoordinator({ ...session, port }))
  await waitFor(() => expect(result.current.state.kind).toBe('ready'))
  act(() => {
    result.current.beginEditor(input)
  })
  await act(async () => {
    await expect(result.current.saveFeedback()).rejects.toMatchObject({ code: 'io' })
  })
  const retained = result.current.pendingEnvelope
  vi.mocked(port.getWorkspace).mockReturnValueOnce(read.promise)
  let reading!: Promise<void>
  act(() => {
    reading = result.current.refresh()
  })
  await act(async () => {
    await expect(result.current.retry()).rejects.toMatchObject({ code: 'busy' })
  })
  expect(port.applyCommand).toHaveBeenCalledOnce()
  expect(result.current.pendingEnvelope).toBe(retained)
  expect(result.current.editorInput.text).toBe(input.text)
  await act(async () => {
    read.resolve(workspace('base'))
    await reading
  })
  await act(() => result.current.retry())
  expect(port.prepareCommand).toHaveBeenCalledOnce()
  expect(vi.mocked(port.applyCommand).mock.calls[1]?.[0].envelope).toBe(retained)
  expect(result.current.view?.current?.reference.snapshotId).toBe('saved-snapshot')
  expect(result.current.lastReceipt?.snapshot.snapshotId).toBe('saved-snapshot')
})

it('late refresh responses neither roll back the view nor overwrite a newer load error', async () => {
  const port = reviewPort(workspace('base'))
  const oldRead = deferred<ReviewWorkspaceView>()
  const { result } = renderHook(() => useContinuousReviewCoordinator({ ...session, port }))
  await waitFor(() => expect(result.current.state.kind).toBe('ready'))
  vi.mocked(port.getWorkspace)
    .mockReturnValueOnce(oldRead.promise)
    .mockResolvedValueOnce(workspace('new-head'))
  let first!: Promise<unknown>
  act(() => {
    first = result.current.refresh().catch((error) => error)
  })
  await act(() => result.current.refresh())
  await act(async () => {
    oldRead.resolve(workspace('old-head'))
    expect(await first).toMatchObject({ code: 'cancelled' })
  })
  expect(result.current.view?.current?.reference.snapshotId).toBe('new-head')
  expect(result.current.state.kind).toBe('ready')
  expect(result.current.error).toBeNull()
})

it('cancelling a refresh exits loading while preserving the last verified view and input', async () => {
  const port = reviewPort(workspace('base'))
  const read = deferred<ReviewWorkspaceView>()
  const { result } = renderHook(() => useContinuousReviewCoordinator({ ...session, port }))
  await waitFor(() => expect(result.current.state.kind).toBe('ready'))
  act(() => {
    result.current.beginEditor(input)
  })
  vi.mocked(port.getWorkspace).mockReturnValueOnce(read.promise)
  let reading!: Promise<unknown>
  act(() => {
    reading = result.current.refresh().catch((error) => error)
  })
  await act(() => result.current.cancel())
  expect(result.current.state.kind).not.toBe('loading')
  expect(result.current.error?.code).toBe('cancelled')
  await act(async () => {
    read.resolve(workspace('too-late'))
    expect(await reading).toMatchObject({ code: 'cancelled' })
  })
  expect(result.current.view?.current?.reference.snapshotId).toBe('base')
  expect(result.current.editorInput.text).toBe(input.text)
})

it('does not start a fresh command while cancellation acknowledgement is still in flight', async () => {
  const port = reviewPort(workspace('base'))
  const cancelling = deferred<number>()
  vi.mocked(port.cancelTask).mockReturnValueOnce(cancelling.promise)
  const { result } = renderHook(() => useContinuousReviewCoordinator({ ...session, port }))
  await waitFor(() => expect(result.current.state.kind).toBe('ready'))
  act(() => {
    result.current.beginEditor(input)
  })
  let cancellation!: Promise<void>
  act(() => {
    cancellation = result.current.cancel()
  })
  await act(async () => {
    await expect(result.current.saveFeedback()).rejects.toMatchObject({ code: 'busy' })
  })
  expect(port.prepareCommand).not.toHaveBeenCalled()
  await act(async () => {
    cancelling.resolve(0)
    await cancellation
  })
  await act(() => result.current.saveFeedback())
  expect(port.applyCommand).toHaveBeenCalledOnce()
})

it('a late cancellation acknowledgement cannot relabel an already successful save as cancelled', async () => {
  const port = reviewPort()
  const writing = deferred<ReviewApplyResult>()
  const cancelling = deferred<number>()
  vi.mocked(port.applyCommand).mockReturnValueOnce(writing.promise)
  vi.mocked(port.cancelTask).mockReturnValueOnce(cancelling.promise)
  const { result } = renderHook(() => useContinuousReviewCoordinator({ ...session, port }))
  await waitFor(() => expect(result.current.state.kind).toBe('ready'))
  act(() => {
    result.current.beginEditor(input)
  })
  let saving!: Promise<void>
  act(() => {
    saving = result.current.saveFeedback()
  })
  await waitFor(() => expect(port.applyCommand).toHaveBeenCalledOnce())
  let cancellation!: Promise<void>
  act(() => {
    cancellation = result.current.cancel()
  })
  const envelope = vi.mocked(port.applyCommand).mock.calls[0]?.[0].envelope
  if (!envelope) throw new Error('Missing envelope')
  await act(async () => {
    writing.resolve(applied(envelope))
    await saving
  })
  await act(async () => {
    cancelling.resolve(1)
    await cancellation
  })
  expect(result.current.error).toBeNull()
  expect(result.current.lastReceipt?.snapshot.snapshotId).toBe('saved-snapshot')
})

it.each(['save', 'retry'] as const)(
  '%s cannot bypass a different unresolved recovery discovered on refresh',
  async (action) => {
    const port = reviewPort(workspace('base'))
    vi.mocked(port.applyCommand).mockRejectedValueOnce(failure('io'))
    const { result } = renderHook(() => useContinuousReviewCoordinator({ ...session, port }))
    await waitFor(() => expect(result.current.state.kind).toBe('ready'))
    act(() => {
      result.current.beginEditor(input)
    })
    await act(async () => {
      await expect(result.current.saveFeedback()).rejects.toMatchObject({ code: 'io' })
    })
    const latest = workspace('base')
    latest.recovery = [recoveryDraft()]
    vi.mocked(port.getWorkspace).mockResolvedValue(latest)
    await act(() => result.current.refresh())
    act(() => {
      result.current.setEditorText('后续编辑')
    })
    await act(async () => {
      const pending = action === 'save' ? result.current.saveFeedback() : result.current.retry()
      await expect(pending).rejects.toMatchObject({
        code: 'needs_confirmation',
      })
    })
    expect(port.prepareCommand).toHaveBeenCalledOnce()
    expect(port.applyCommand).toHaveBeenCalledOnce()
    expect(result.current.view?.recovery).toHaveLength(1)
    expect(result.current.editorInput.text).toBe('后续编辑')
    expect(result.current.pendingEnvelope).not.toBeNull()
  },
)

it('unresolved recovery also blocks a new migration command despite migration capability', async () => {
  const view = workspace()
  view.migration = migrationInspection()
  view.capabilities = { continuousEditing: false, usageImport: false, migration: true }
  view.recovery = [recoveryDraft('uncertain-migration')]
  const port = reviewPort(view)
  const { result } = renderHook(() => useContinuousReviewCoordinator({ ...session, port }))
  await waitFor(() => expect(result.current.state.kind).toBe('migration_required'))
  await act(async () => {
    await expect(
      result.current.migrate({
        inspectionDigest: migrationInspection().inspectionDigest,
        choice: { kind: 'keep_history_only' },
      }),
    ).rejects.toMatchObject({ code: 'needs_confirmation' })
  })
  expect(port.prepareCommand).not.toHaveBeenCalled()
  expect(port.applyCommand).not.toHaveBeenCalled()
  expect(result.current.view?.recovery).toEqual(view.recovery)
})

it('reconciles the same uncertain command without inventing a new envelope after refresh', async () => {
  const port = reviewPort(workspace('base'))
  vi.mocked(port.applyCommand).mockRejectedValueOnce(failure('outcome_unknown'))
  const { result } = renderHook(() => useContinuousReviewCoordinator({ ...session, port }))
  await waitFor(() => expect(result.current.state.kind).toBe('ready'))
  act(() => {
    result.current.beginEditor(input)
  })
  await act(async () => {
    await expect(result.current.saveFeedback()).rejects.toMatchObject({ code: 'outcome_unknown' })
  })
  const retained = result.current.pendingEnvelope
  if (!retained) throw new Error('Missing envelope')
  const view = workspace('later-head')
  view.recovery = [{ ...recoveryDraft(retained.commandId), payloadDigest: retained.payloadDigest }]
  vi.mocked(port.getWorkspace).mockResolvedValue(view)
  await act(() => result.current.refresh())
  await act(() => result.current.retry())
  expect(port.prepareCommand).toHaveBeenCalledOnce()
  expect(vi.mocked(port.applyCommand).mock.calls[1]?.[0].envelope).toBe(retained)
  expect(result.current.pendingEnvelope).toBeNull()
})

it('reconciles a known migration receipt even after refresh no longer requires migration', async () => {
  const view = workspace()
  view.migration = migrationInspection()
  view.capabilities = { continuousEditing: false, usageImport: false, migration: true }
  const port = reviewPort(view)
  vi.mocked(port.applyCommand).mockImplementationOnce(async ({ envelope }) => {
    throw {
      ...failure('committed_view_unavailable', false),
      committedReceipt: applied(envelope).receipt,
    }
  })
  const { result } = renderHook(() => useContinuousReviewCoordinator({ ...session, port }))
  await waitFor(() => expect(result.current.state.kind).toBe('migration_required'))
  await act(async () => {
    await expect(
      result.current.migrate({
        inspectionDigest: migrationInspection().inspectionDigest,
        choice: { kind: 'keep_history_only' },
      }),
    ).rejects.toMatchObject({ code: 'committed_view_unavailable' })
  })
  const retained = result.current.pendingEnvelope
  vi.mocked(port.getWorkspace).mockResolvedValue(workspace('saved-snapshot'))
  await act(() => result.current.refresh())
  await act(() => result.current.retry())
  expect(port.prepareCommand).toHaveBeenCalledOnce()
  expect(vi.mocked(port.applyCommand).mock.calls[1]?.[0].envelope).toBe(retained)
  expect(result.current.state.kind).toBe('ready')
})
