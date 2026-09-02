import { act, renderHook, waitFor } from '@testing-library/react'
import { expect, it, vi } from 'vitest'
import type {
  PreparedReviewCommand,
  ReviewAuthoringApplyResult,
} from '../../api/reviewWorkspaceTypes'
import {
  applied,
  deferred,
  failure,
  input,
  prepared,
  reviewPort,
  session,
  workspace,
} from './continuousReviewTestFixtures'
import { useContinuousReviewCoordinator } from './useContinuousReviewCoordinator'

it('changes the workbench session identity when the coordinator session instance is replaced', async () => {
  const firstPort = reviewPort()
  const replacementPort = reviewPort()
  const hook = renderHook(({ port }) => useContinuousReviewCoordinator({ ...session, port }), {
    initialProps: { port: firstPort },
  })
  await waitFor(() => expect(hook.result.current.state.kind).toBe('ready'))
  const firstIdentity = hook.result.current.workbenchSessionKey

  hook.rerender({ port: replacementPort })
  await waitFor(() => expect(replacementPort.getWorkspace).toHaveBeenCalledOnce())

  expect(hook.result.current.workbenchSessionKey).not.toBe(firstIdentity)
})

it('exposes the current snapshot ID for an archive preview guard', async () => {
  const port = reviewPort(workspace('archive-head'))
  const { result } = renderHook(() => useContinuousReviewCoordinator({ ...session, port }))
  await waitFor(() => expect(result.current.state.kind).toBe('ready'))

  expect(result.current.currentSnapshotId).toBe('archive-head')
})

it('retains text after a lost receipt and retries the complete original envelope, installing the reread head', async () => {
  const port = reviewPort()
  const onError = vi.fn()
  vi.mocked(port.applyCommand)
    .mockRejectedValueOnce(failure('outcome_unknown'))
    .mockImplementationOnce(async ({ envelope }) => applied(envelope, workspace('later-head')))
  const { result } = renderHook(() => useContinuousReviewCoordinator({ ...session, port, onError }))
  await waitFor(() => expect(result.current.state.kind).toBe('ready'))
  act(() => {
    result.current.beginEditor(input)
  })
  await act(async () => {
    await expect(result.current.saveFeedback()).rejects.toMatchObject({ code: 'outcome_unknown' })
  })
  expect(result.current.editorInput.text).toBe('袖口收紧，保留褶皱')
  expect(result.current.state.kind).toBe('recovery_required')
  expect(result.current.view?.current).toBeNull()
  expect(result.current.pendingEnvelope?.usageSelections[0]?.candidate?.source).toBe(
    'producer/usage.json',
  )
  expect(onError).toHaveBeenCalledOnce()
  await act(() => result.current.retry())
  expect(port.applyCommand).toHaveBeenCalledTimes(2)
  expect(vi.mocked(port.applyCommand).mock.calls[1]?.[0]).toEqual(
    vi.mocked(port.applyCommand).mock.calls[0]?.[0],
  )
  expect(port.prepareCommand).toHaveBeenCalledOnce()
  expect(result.current.view?.current?.authoring.head.snapshotId).toBe('later-head')
  expect(result.current.lastReceipt?.head.snapshotId).toBe('later-head')
  expect(result.current.editorInput.text).toBe('')
  expect(result.current.pendingEnvelope).toBeNull()
})

it('coalesces double saves before preparation finishes instead of minting a second command', async () => {
  const port = reviewPort(workspace('base'))
  const preparation = deferred<PreparedReviewCommand>()
  vi.mocked(port.prepareCommand).mockReturnValueOnce(preparation.promise)
  const { result } = renderHook(() => useContinuousReviewCoordinator({ ...session, port }))
  await waitFor(() => expect(result.current.state.kind).toBe('ready'))
  act(() => {
    result.current.beginEditor(input)
  })
  let first!: Promise<void>
  let second!: Promise<void>
  act(() => {
    first = result.current.saveFeedback()
    second = result.current.saveFeedback()
  })
  expect(port.prepareCommand).toHaveBeenCalledOnce()
  const request = vi.mocked(port.prepareCommand).mock.calls[0]?.[0]
  expect(request).toMatchObject({
    ...session,
    expectedSnapshotId: 'base',
    command: { text: input.text },
  })
  if (!request) throw new Error('Missing preparation')
  await act(async () => {
    preparation.resolve(prepared(request))
    await Promise.all([first, second])
  })
  expect(port.applyCommand).toHaveBeenCalledOnce()
})

it('retains newer typing on save and associates it with the saved feedback identity for the next edit', async () => {
  const port = reviewPort()
  const writing = deferred<ReviewAuthoringApplyResult>()
  vi.mocked(port.applyCommand).mockReturnValueOnce(writing.promise)
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
  act(() => {
    result.current.setEditorText('袖口收紧，保留褶皱和原材质')
  })
  const envelope = vi.mocked(port.applyCommand).mock.calls[0]?.[0].envelope
  if (!envelope) throw new Error('Missing envelope')
  expect(envelope.command).toMatchObject({ text: '袖口收紧，保留褶皱' })
  await act(async () => {
    writing.resolve(applied(envelope))
    await saving
  })
  expect(result.current.editorInput).toMatchObject({
    text: '袖口收紧，保留褶皱和原材质',
    feedbackId: 'saved-feedback',
    targets: [],
    baseSnapshotId: 'saved-snapshot',
  })
  await act(() => result.current.saveFeedback())
  expect(vi.mocked(port.prepareCommand).mock.calls[1]?.[0]).toMatchObject({
    expectedSnapshotId: 'saved-snapshot',
    command: {
      kind: 'save_feedback',
      feedbackId: 'saved-feedback',
      text: '袖口收紧，保留褶皱和原材质',
      targets: [],
    },
  })
})

it('freezes geometry and input ownership while saving, but permits further typing', async () => {
  const port = reviewPort()
  const writing = deferred<ReviewAuthoringApplyResult>()
  vi.mocked(port.applyCommand).mockReturnValueOnce(writing.promise)
  const { result } = renderHook(() => useContinuousReviewCoordinator({ ...session, port }))
  await waitFor(() => expect(result.current.state.kind).toBe('ready'))
  const mutable = structuredClone(input)
  act(() => {
    result.current.beginEditor(mutable)
  })
  const mutableTarget = mutable.targets[0]
  if (!mutableTarget) throw new Error('Missing target')
  mutableTarget.assetVersionId = 'different-asset'
  let saving!: Promise<void>
  act(() => {
    saving = result.current.saveFeedback()
  })
  await waitFor(() => expect(port.applyCommand).toHaveBeenCalledOnce())
  act(() => {
    expect(result.current.beginEditor({ ...input, contextKey: 'image-2' })).toBe(false)
    expect(result.current.setEditorTargets([])).toBe(false)
    expect(result.current.discardEditor()).toBe(false)
  })
  expect(result.current.editorInput.targets[0]?.assetVersionId).toBe('asset-1')
  const envelope = vi.mocked(port.applyCommand).mock.calls[0]?.[0].envelope
  if (!envelope) throw new Error('Missing envelope')
  await act(async () => {
    writing.resolve(applied(envelope))
    await saving
  })
})

it('retries a failed prepare with the same request and command ID', async () => {
  const port = reviewPort()
  vi.mocked(port.prepareCommand).mockRejectedValueOnce(failure('io'))
  const { result } = renderHook(() => useContinuousReviewCoordinator({ ...session, port }))
  await waitFor(() => expect(result.current.state.kind).toBe('ready'))
  act(() => {
    result.current.beginEditor(input)
  })
  await act(async () => {
    await expect(result.current.saveFeedback()).rejects.toMatchObject({ code: 'io' })
  })
  expect(result.current.state.kind).toBe('save_failed')
  expect(result.current.pendingEnvelope).toBeNull()
  expect(port.applyCommand).not.toHaveBeenCalled()
  await act(() => result.current.retry())
  expect(vi.mocked(port.prepareCommand).mock.calls[1]?.[0]).toEqual(
    vi.mocked(port.prepareCommand).mock.calls[0]?.[0],
  )
})

it('an unchanged explicit save after failure uses the retry envelope, not another prepare', async () => {
  const port = reviewPort()
  vi.mocked(port.applyCommand).mockRejectedValueOnce(failure('io'))
  const { result } = renderHook(() => useContinuousReviewCoordinator({ ...session, port }))
  await waitFor(() => expect(result.current.state.kind).toBe('ready'))
  act(() => {
    result.current.beginEditor(input)
  })
  await act(async () => {
    await expect(result.current.saveFeedback()).rejects.toMatchObject({ code: 'io' })
  })
  await act(() => result.current.saveFeedback())
  expect(port.prepareCommand).toHaveBeenCalledOnce()
  expect(vi.mocked(port.applyCommand).mock.calls[1]?.[0]).toEqual(
    vi.mocked(port.applyCommand).mock.calls[0]?.[0],
  )
})

it('blocks a replacement command until an uncertain previous outcome is reconciled', async () => {
  const port = reviewPort()
  vi.mocked(port.applyCommand).mockRejectedValueOnce(failure('outcome_unknown'))
  const { result } = renderHook(() => useContinuousReviewCoordinator({ ...session, port }))
  await waitFor(() => expect(result.current.state.kind).toBe('ready'))
  act(() => {
    result.current.beginEditor(input)
  })
  await act(async () => {
    await expect(result.current.saveFeedback()).rejects.toMatchObject({ code: 'outcome_unknown' })
  })
  const envelope = result.current.pendingEnvelope
  act(() => {
    result.current.setEditorText('新想法，暂时保留原形状')
  })
  await act(async () => {
    await expect(result.current.saveFeedback()).rejects.toMatchObject({ code: 'outcome_unknown' })
  })
  expect(result.current.pendingEnvelope).toEqual(envelope)
  expect(port.prepareCommand).toHaveBeenCalledOnce()
  expect(port.applyCommand).toHaveBeenCalledOnce()
  expect(result.current.editorInput.text).toBe('新想法，暂时保留原形状')
})

it('allows an explicitly changed payload after a definite failure, retaining its original snapshot guard', async () => {
  const port = reviewPort(workspace('base'))
  vi.mocked(port.applyCommand).mockRejectedValueOnce(failure('source_changed', false))
  const { result } = renderHook(() => useContinuousReviewCoordinator({ ...session, port }))
  await waitFor(() => expect(result.current.state.kind).toBe('ready'))
  act(() => {
    result.current.beginEditor(input)
  })
  await act(async () => {
    await expect(result.current.saveFeedback()).rejects.toMatchObject({ code: 'source_changed' })
  })
  act(() => {
    result.current.setEditorText('保留原形状，只调整颜色')
  })
  await act(() => result.current.saveFeedback())
  const requests = vi.mocked(port.prepareCommand).mock.calls
  expect(requests[1]?.[0].commandId).not.toBe(requests[0]?.[0].commandId)
  expect(requests[1]?.[0].expectedSnapshotId).toBe('base')
})

it('distinguishes a known committed write with failed refresh from an unsaved failure', async () => {
  const port = reviewPort(workspace('base'))
  vi.mocked(port.applyCommand).mockImplementationOnce(async ({ envelope }) => {
    throw {
      ...failure('committed_view_unavailable'),
      committedAuthoringReceipt: applied(envelope).receipt,
    }
  })
  const { result } = renderHook(() => useContinuousReviewCoordinator({ ...session, port }))
  await waitFor(() => expect(result.current.state.kind).toBe('ready'))
  act(() => {
    result.current.beginEditor(input)
  })
  await act(async () => {
    await expect(result.current.saveFeedback()).rejects.toMatchObject({
      code: 'committed_view_unavailable',
    })
  })
  expect(result.current.lastReceipt?.head.snapshotId).toBe('saved-snapshot')
  expect(result.current.view?.current?.authoring.head.snapshotId).toBe('base')
  expect(result.current.state.kind).toBe('recovery_required')
  expect(result.current.editorInput.text).toBe(input.text)
  await act(() => result.current.retry())
  expect(port.prepareCommand).toHaveBeenCalledOnce()
})

it('handles an untyped transport rejection without discarding input or permitting duplicate submission', async () => {
  const port = reviewPort()
  vi.mocked(port.applyCommand).mockRejectedValueOnce('Transport disconnected')
  const { result } = renderHook(() => useContinuousReviewCoordinator({ ...session, port }))
  await waitFor(() => expect(result.current.state.kind).toBe('ready'))
  act(() => {
    result.current.beginEditor(input)
  })
  await act(async () => {
    await expect(result.current.saveFeedback()).rejects.toMatchObject({
      code: 'internal',
      message: 'Transport disconnected',
    })
  })
  expect(result.current.state.kind).toBe('recovery_required')
  expect(result.current.editorInput.text).toBe(input.text)
  await act(() => result.current.retry())
  expect(port.prepareCommand).toHaveBeenCalledOnce()
})

it('requires explicit discard before changing a dirty editor to another image', async () => {
  const port = reviewPort()
  const { result } = renderHook(() => useContinuousReviewCoordinator({ ...session, port }))
  await waitFor(() => expect(result.current.state.kind).toBe('ready'))
  act(() => {
    result.current.beginEditor(input)
    expect(result.current.beginEditor({ ...input, contextKey: 'image-2' })).toBe(false)
  })
  expect(result.current.editorInput.contextKey).toBe('image-1')
  expect(result.current.hasUncommittedInput).toBe(true)
  act(() => {
    expect(result.current.discardEditor()).toBe(true)
    expect(result.current.beginEditor({ ...input, contextKey: 'image-2' })).toBe(true)
  })
  expect(result.current.editorInput.contextKey).toBe('image-2')
  expect(port.prepareCommand).not.toHaveBeenCalled()
})

it('performs one full reconciliation read only when a committed patch basis is stale', async () => {
  const port = reviewPort(workspace('base'))
  const reconciled = workspace('saved-snapshot')
  if (reconciled.current === null) throw new Error('Missing reconciled current')
  reconciled.current.authoring.head.sequence = 2
  vi.mocked(port.getWorkspace)
    .mockResolvedValueOnce(workspace('base'))
    .mockResolvedValueOnce(reconciled)
  vi.mocked(port.applyCommand).mockImplementationOnce(async ({ envelope }) => {
    const reply = applied(envelope, workspace('saved-snapshot'), 2)
    reply.patch.basisSnapshotId = 'unexpected-base'
    return reply
  })
  const { result } = renderHook(() => useContinuousReviewCoordinator({ ...session, port }))
  await waitFor(() => expect(result.current.state.kind).toBe('ready'))
  act(() => {
    result.current.beginEditor(input)
  })

  await act(() => result.current.saveFeedback())

  expect(port.getWorkspace).toHaveBeenCalledTimes(2)
  expect(result.current.view?.current?.authoring.head.snapshotId).toBe('saved-snapshot')
  expect(result.current.editorInput.contextKey).toBeNull()
})
