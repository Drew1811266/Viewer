import { act, renderHook, waitFor } from '@testing-library/react'
import { StrictMode } from 'react'
import { expect, it, vi } from 'vitest'
import type {
  PreparedReviewCommand,
  ReviewApplyResult,
  ReviewWorkspaceView,
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

it('reports an initial load failure and allows an explicit read-only retry', async () => {
  const port = reviewPort()
  const onError = vi.fn()
  vi.mocked(port.getWorkspace).mockRejectedValueOnce(failure('integrity', false))
  const { result } = renderHook(() => useContinuousReviewCoordinator({ ...session, port, onError }))
  await waitFor(() => expect(result.current.state.kind).toBe('unavailable'))
  expect(result.current.error?.code).toBe('integrity')
  await act(() => result.current.retry())
  expect(result.current.state.kind).toBe('ready')
  expect(port.applyCommand).not.toHaveBeenCalled()
  expect(onError).toHaveBeenCalledOnce()
})

it('switching generation cancels the old session and prevents a late preparation from applying', async () => {
  const port = reviewPort()
  const preparation = deferred<PreparedReviewCommand>()
  vi.mocked(port.prepareCommand).mockReturnValueOnce(preparation.promise)
  const hook = renderHook(
    ({ generation }) => useContinuousReviewCoordinator({ ...session, generation, port }),
    { initialProps: { generation: 1 } },
  )
  await waitFor(() => expect(hook.result.current.state.kind).toBe('ready'))
  act(() => {
    hook.result.current.beginEditor(input)
  })
  let saveResult!: Promise<unknown>
  act(() => {
    saveResult = hook.result.current.saveFeedback().catch((error) => error)
  })
  const request = vi.mocked(port.prepareCommand).mock.calls[0]?.[0]
  if (!request) throw new Error('Missing preparation')
  hook.rerender({ generation: 2 })
  await waitFor(() => expect(hook.result.current.state.kind).toBe('ready'))
  act(() => {
    hook.result.current.beginEditor({ ...input, text: '新工程意见' })
  })
  await act(async () => {
    preparation.resolve(prepared(request))
    expect(await saveResult).toMatchObject({ code: 'stale_session' })
  })
  expect(port.applyCommand).not.toHaveBeenCalled()
  expect(port.cancelTask).toHaveBeenCalledWith(session)
  expect(hook.result.current.editorInput.text).toBe('新工程意见')
})

it('a late committed reply cannot overwrite the next project but its caller still receives the receipt', async () => {
  const port = reviewPort(workspace('original'))
  const writing = deferred<ReviewApplyResult>()
  const onError = vi.fn()
  vi.mocked(port.applyCommand).mockReturnValueOnce(writing.promise)
  const hook = renderHook(
    ({ sessionId }) => useContinuousReviewCoordinator({ ...session, sessionId, port, onError }),
    { initialProps: { sessionId: 'session-1' } },
  )
  await waitFor(() => expect(hook.result.current.state.kind).toBe('ready'))
  const oldSave = hook.result.current.saveFeedback
  act(() => {
    hook.result.current.beginEditor(input)
  })
  let saveResult!: Promise<unknown>
  act(() => {
    saveResult = oldSave().catch((error) => error)
  })
  await waitFor(() => expect(port.applyCommand).toHaveBeenCalledOnce())
  const envelope = vi.mocked(port.applyCommand).mock.calls[0]?.[0].envelope
  if (!envelope) throw new Error('Missing envelope')
  vi.mocked(port.getWorkspace).mockResolvedValue(workspace('other-project'))
  hook.rerender({ sessionId: 'session-2' })
  await waitFor(() =>
    expect(hook.result.current.view?.current?.reference.snapshotId).toBe('other-project'),
  )
  await act(async () => {
    writing.resolve(applied(envelope))
    expect(await saveResult).toMatchObject({
      code: 'stale_session',
      committedReceipt: { commandId: envelope.commandId },
    })
  })
  expect(hook.result.current.view?.current?.reference.snapshotId).toBe('other-project')
  expect(hook.result.current.lastReceipt).toBeNull()
  await expect(oldSave()).rejects.toMatchObject({ code: 'stale_session' })
  expect(onError).not.toHaveBeenCalled()
  expect(port.prepareCommand).toHaveBeenCalledOnce()
})

it('unmount cancels owned work and never continues a prepared write', async () => {
  const port = reviewPort()
  const preparation = deferred<PreparedReviewCommand>()
  vi.mocked(port.prepareCommand).mockReturnValueOnce(preparation.promise)
  const hook = renderHook(() => useContinuousReviewCoordinator({ ...session, port }))
  await waitFor(() => expect(hook.result.current.state.kind).toBe('ready'))
  act(() => {
    hook.result.current.beginEditor(input)
  })
  let saveResult!: Promise<unknown>
  act(() => {
    saveResult = hook.result.current.saveFeedback().catch((error) => error)
  })
  const request = vi.mocked(port.prepareCommand).mock.calls[0]?.[0]
  if (!request) throw new Error('Missing preparation')
  hook.unmount()
  preparation.resolve(prepared(request))
  expect(await saveResult).toMatchObject({ code: 'stale_session' })
  expect(port.cancelTask).toHaveBeenCalledWith(session)
  expect(port.applyCommand).not.toHaveBeenCalled()
})

it('ignores the disposed StrictMode load even if it arrives after the replacement load', async () => {
  const port = reviewPort(workspace('new-head'))
  const oldLoad = deferred<ReviewWorkspaceView>()
  vi.mocked(port.getWorkspace).mockReturnValueOnce(oldLoad.promise)
  const { result } = renderHook(() => useContinuousReviewCoordinator({ ...session, port }), {
    wrapper: StrictMode,
  })
  await waitFor(() => expect(result.current.view?.current?.reference.snapshotId).toBe('new-head'))
  await act(async () => {
    oldLoad.resolve(workspace('old-head'))
  })
  expect(result.current.view?.current?.reference.snapshotId).toBe('new-head')
})

it('a replacement transport waits for the previous coordinator cancellation before loading', async () => {
  const oldPort = reviewPort(workspace('old-head'))
  const nextPort = reviewPort(workspace('new-head'))
  const acknowledgement = deferred<number>()
  vi.mocked(oldPort.cancelTask).mockReturnValueOnce(acknowledgement.promise)
  const hook = renderHook(({ port }) => useContinuousReviewCoordinator({ ...session, port }), {
    initialProps: { port: oldPort },
  })
  await waitFor(() => expect(hook.result.current.state.kind).toBe('ready'))
  hook.rerender({ port: nextPort })
  expect(nextPort.getWorkspace).not.toHaveBeenCalled()
  await act(async () => {
    acknowledgement.resolve(0)
  })
  await waitFor(() =>
    expect(hook.result.current.view?.current?.reference.snapshotId).toBe('new-head'),
  )
})

it('cancel during prepare retains the envelope for retry but does not start applying it', async () => {
  const port = reviewPort()
  const preparation = deferred<PreparedReviewCommand>()
  vi.mocked(port.prepareCommand).mockReturnValueOnce(preparation.promise)
  const { result } = renderHook(() => useContinuousReviewCoordinator({ ...session, port }))
  await waitFor(() => expect(result.current.state.kind).toBe('ready'))
  act(() => {
    result.current.beginEditor(input)
  })
  let saveResult!: Promise<unknown>
  act(() => {
    saveResult = result.current.saveFeedback().catch((error) => error)
  })
  const request = vi.mocked(port.prepareCommand).mock.calls[0]?.[0]
  if (!request) throw new Error('Missing preparation')
  await act(() => result.current.cancel())
  await act(async () => {
    preparation.resolve(prepared(request))
    expect(await saveResult).toMatchObject({ code: 'cancelled' })
  })
  expect(result.current.pendingEnvelope?.commandId).toBe(request.commandId)
  expect(result.current.editorInput.text).toBe(input.text)
  expect(port.applyCommand).not.toHaveBeenCalled()
  await act(() => result.current.retry())
  expect(port.prepareCommand).toHaveBeenCalledOnce()
  expect(port.applyCommand).toHaveBeenCalledOnce()
})

it('cancel is not rollback: an apply that already committed still updates the saved view', async () => {
  const port = reviewPort()
  const writing = deferred<ReviewApplyResult>()
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
  const envelope = vi.mocked(port.applyCommand).mock.calls[0]?.[0].envelope
  if (!envelope) throw new Error('Missing envelope')
  await act(() => result.current.cancel())
  expect(result.current.state.kind).toBe('saving')
  await act(async () => {
    writing.resolve(applied(envelope))
    await saving
  })
  expect(result.current.view?.current?.reference.snapshotId).toBe('saved-snapshot')
  expect(result.current.lastReceipt?.commandId).toBe(envelope.commandId)
})

it('read-only capabilities block new input and all submission before desktop preparation', async () => {
  const view = workspace()
  view.capabilities.continuousEditing = false
  const port = reviewPort(view)
  const { result } = renderHook(() => useContinuousReviewCoordinator({ ...session, port }))
  await waitFor(() => expect(result.current.state.kind).toBe('unavailable'))
  act(() => {
    expect(result.current.beginEditor(input)).toBe(false)
  })
  await act(async () => {
    await expect(result.current.saveFeedback()).rejects.toMatchObject({ code: 'read_only' })
  })
  expect(port.prepareCommand).not.toHaveBeenCalled()
})

it('does not silently replace newer typing with a queued second save', async () => {
  const port = reviewPort()
  const writing = deferred<ReviewApplyResult>()
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
    result.current.setEditorText('新文字')
  })
  await act(async () => {
    await expect(result.current.saveFeedback()).rejects.toMatchObject({ code: 'busy' })
  })
  const envelope = vi.mocked(port.applyCommand).mock.calls[0]?.[0].envelope
  if (!envelope) throw new Error('Missing envelope')
  await act(async () => {
    writing.resolve(applied(envelope))
    await saving
  })
  expect(result.current.editorInput.text).toBe('新文字')
  expect(port.applyCommand).toHaveBeenCalledOnce()
})
