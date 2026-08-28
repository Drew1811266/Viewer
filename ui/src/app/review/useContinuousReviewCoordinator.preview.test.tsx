import { act, renderHook, waitFor } from '@testing-library/react'
import { expect, it, vi } from 'vitest'
import type {
  ReviewApplyResult,
  ReviewArchivePlan,
  ReviewRestorePlan,
} from '../../api/reviewWorkspaceTypes'
import { ContinuousReviewSession } from './continuousReviewSession'
import {
  applied,
  archivePlan,
  archiveSelection,
  deferred,
  failure,
  input,
  restorePlan,
  reviewPort,
  session,
  targetKey,
  workspace,
} from './continuousReviewTestFixtures'
import { useContinuousReviewCoordinator } from './useContinuousReviewCoordinator'

it('freezes the exact archive selection, coalesces double confirmation, and rejects competing commands', async () => {
  const port = reviewPort(workspace('base'))
  const writing = deferred<ReviewApplyResult>()
  vi.mocked(port.previewArchive).mockImplementation(async ({ selection }) => archivePlan(selection))
  vi.mocked(port.applyCommand).mockReturnValueOnce(writing.promise)
  const { result } = renderHook(() => useContinuousReviewCoordinator({ ...session, port }))
  await waitFor(() => expect(result.current.state.kind).toBe('ready'))
  const selection = archiveSelection()
  await act(() => result.current.previewArchive(selection))
  selection.groups[0]?.targets.push({ ...targetKey, targetId: 'unselected-target' })
  let first!: Promise<void>
  let second!: Promise<void>
  act(() => {
    first = result.current.commitArchive()
    second = result.current.commitArchive()
  })
  await waitFor(() => expect(port.applyCommand).toHaveBeenCalledOnce())
  await act(async () => {
    await expect(result.current.withdrawTargets([targetKey])).rejects.toMatchObject({
      code: 'busy',
    })
  })
  const envelope = vi.mocked(port.applyCommand).mock.calls[0]?.[0].envelope
  if (!envelope) throw new Error('Missing envelope')
  expect(envelope.command).toEqual({
    kind: 'archive',
    expectedSnapshotId: 'base',
    groups: [{ basis: { kind: 'unknown' }, targets: [targetKey] }],
  })
  await act(async () => {
    writing.resolve(applied(envelope))
    await Promise.all([first, second])
  })
  expect(port.prepareCommand).toHaveBeenCalledOnce()
})

it('requires a fresh preview after a stale archive instead of silently rebasing or resending', async () => {
  const port = reviewPort(workspace('base'))
  vi.mocked(port.previewArchive).mockImplementation(async ({ selection }) => archivePlan(selection))
  vi.mocked(port.applyCommand).mockRejectedValueOnce(failure('stale_snapshot', false))
  const { result } = renderHook(() => useContinuousReviewCoordinator({ ...session, port }))
  await waitFor(() => expect(result.current.state.kind).toBe('ready'))
  await act(() => result.current.previewArchive(archiveSelection()))
  await act(async () => {
    await expect(result.current.commitArchive()).rejects.toMatchObject({ code: 'stale_snapshot' })
  })
  await act(async () => {
    await expect(result.current.retry()).rejects.toMatchObject({ code: 'stale_snapshot' })
  })
  expect(port.applyCommand).toHaveBeenCalledOnce()
  vi.mocked(port.getWorkspace).mockResolvedValue(workspace('new-head'))
  await act(() => result.current.refresh())
  await act(async () => {
    await expect(result.current.commitArchive()).rejects.toMatchObject({ code: 'preview_required' })
  })
  await act(() => result.current.previewArchive(archiveSelection('new-head')))
  await act(() => result.current.commitArchive())
  const calls = vi.mocked(port.prepareCommand).mock.calls
  expect(calls[1]?.[0].expectedSnapshotId).toBe('new-head')
  expect(calls[1]?.[0].commandId).not.toBe(calls[0]?.[0].commandId)
})

it('cannot archive dirty input or treat it as part of the submitted snapshot', async () => {
  const port = reviewPort(workspace('base'))
  const { result } = renderHook(() => useContinuousReviewCoordinator({ ...session, port }))
  await waitFor(() => expect(result.current.state.kind).toBe('ready'))
  act(() => {
    result.current.beginEditor(input)
  })
  await act(async () => {
    await expect(result.current.previewArchive(archiveSelection())).rejects.toMatchObject({
      code: 'needs_confirmation',
    })
  })
  expect(result.current.editorInput.text).toBe(input.text)
  expect(port.previewArchive).not.toHaveBeenCalled()
  expect(port.prepareCommand).not.toHaveBeenCalled()
})

it('an older preview response cannot replace the newest user selection', async () => {
  const port = reviewPort(workspace('base'))
  const firstPreview = deferred<ReviewArchivePlan>()
  vi.mocked(port.previewArchive)
    .mockReturnValueOnce(firstPreview.promise)
    .mockImplementationOnce(async ({ selection }) => archivePlan(selection))
  const { result } = renderHook(() => useContinuousReviewCoordinator({ ...session, port }))
  await waitFor(() => expect(result.current.state.kind).toBe('ready'))
  let first!: Promise<unknown>
  act(() => {
    first = result.current.previewArchive(archiveSelection()).catch((error) => error)
  })
  const second = archiveSelection()
  const secondKey = { ...targetKey, targetId: 'second-target' }
  second.groups[0] = { basis: { kind: 'unknown' }, targets: [secondKey] }
  await act(() => result.current.previewArchive(second))
  await act(async () => {
    firstPreview.resolve(archivePlan())
    expect(await first).toMatchObject({ code: 'cancelled' })
  })
  await act(() => result.current.commitArchive())
  expect(vi.mocked(port.prepareCommand).mock.calls[0]?.[0].command).toMatchObject({
    groups: [{ basis: { kind: 'unknown' }, targets: [secondKey] }],
  })
})

it('a locally stale new preview invalidates an older in-flight archive preview and its result', async () => {
  const port = reviewPort(workspace('base'))
  const oldPreview = deferred<ReviewArchivePlan>()
  vi.mocked(port.previewArchive).mockReturnValueOnce(oldPreview.promise)
  const { result } = renderHook(() => useContinuousReviewCoordinator({ ...session, port }))
  await waitFor(() => expect(result.current.state.kind).toBe('ready'))
  let first!: Promise<unknown>
  act(() => {
    first = result.current.previewArchive(archiveSelection()).catch((error) => error)
  })
  await act(async () => {
    await expect(
      result.current.previewArchive(archiveSelection('stale-head')),
    ).rejects.toMatchObject({
      code: 'stale_snapshot',
    })
  })
  await act(async () => {
    oldPreview.resolve(archivePlan())
    expect(await first).toMatchObject({ code: 'cancelled' })
  })
  expect(result.current.error?.code).toBe('stale_snapshot')
  await act(async () => {
    await expect(result.current.commitArchive()).rejects.toMatchObject({ code: 'preview_required' })
  })
  expect(port.previewArchive).toHaveBeenCalledOnce()
  expect(port.prepareCommand).not.toHaveBeenCalled()
})

it('cannot revive an archive preview when a subscriber requests a newer preview during result notification', async () => {
  const port = reviewPort(workspace('base'))
  const oldPreview = deferred<ReviewArchivePlan>()
  vi.mocked(port.previewArchive).mockReturnValueOnce(oldPreview.promise)
  const coordinator = new ContinuousReviewSession(port, session, () => {})
  const stop = coordinator.start()
  let unsubscribe = () => {}
  try {
    await waitFor(() => expect(coordinator.getSnapshot().state.kind).toBe('ready'))
    const first = coordinator.actions.previewArchive(archiveSelection()).catch((error) => error)
    let next: Promise<unknown> | null = null
    let requested = false
    unsubscribe = coordinator.subscribe(() => {
      if (requested) return
      requested = true
      next = coordinator.actions
        .previewArchive(archiveSelection('stale-head'))
        .catch((error) => error)
    })
    oldPreview.resolve(archivePlan())
    expect(await first).toMatchObject({ code: 'cancelled' })
    expect(await next).toMatchObject({ code: 'stale_snapshot' })
    expect(coordinator.getSnapshot().error?.code).toBe('stale_snapshot')
    await expect(coordinator.actions.commitArchive()).rejects.toMatchObject({
      code: 'preview_required',
    })
    expect(port.prepareCommand).not.toHaveBeenCalled()
  } finally {
    unsubscribe()
    await stop()
  }
})

it.each(['archive', 'restore'] as const)(
  'dirty-input rejection invalidates an older in-flight %s preview even after discard',
  async (kind) => {
    const port = reviewPort(workspace('base'))
    const oldArchive = deferred<ReviewArchivePlan>()
    const oldRestore = deferred<ReviewRestorePlan>()
    vi.mocked(port.previewArchive).mockReturnValueOnce(oldArchive.promise)
    vi.mocked(port.previewRestore).mockReturnValueOnce(oldRestore.promise)
    const { result } = renderHook(() => useContinuousReviewCoordinator({ ...session, port }))
    await waitFor(() => expect(result.current.state.kind).toBe('ready'))
    const preview = () =>
      kind === 'archive'
        ? result.current.previewArchive(archiveSelection())
        : result.current.previewRestore('archive-1', [])
    let first!: Promise<unknown>
    act(() => {
      first = preview().catch((error) => error)
      expect(result.current.beginEditor(input)).toBe(true)
    })
    await act(async () => {
      await expect(preview()).rejects.toMatchObject({ code: 'needs_confirmation' })
    })
    act(() => {
      expect(result.current.discardEditor()).toBe(true)
    })
    await act(async () => {
      if (kind === 'archive') oldArchive.resolve(archivePlan())
      else oldRestore.resolve(restorePlan())
      expect(await first).toMatchObject({ code: 'cancelled' })
    })
    await act(async () => {
      const commit = kind === 'archive' ? result.current.commitArchive() : result.current.restore()
      await expect(commit).rejects.toMatchObject({ code: 'preview_required' })
    })
    expect(port.prepareCommand).not.toHaveBeenCalled()
  },
)

it('refresh invalidates an old archive preview even before a new current head is received', async () => {
  const port = reviewPort(workspace('base'))
  vi.mocked(port.previewArchive).mockResolvedValue(archivePlan())
  const { result } = renderHook(() => useContinuousReviewCoordinator({ ...session, port }))
  await waitFor(() => expect(result.current.state.kind).toBe('ready'))
  await act(() => result.current.previewArchive(archiveSelection()))
  await act(() => result.current.refresh())
  await act(async () => {
    await expect(result.current.commitArchive()).rejects.toMatchObject({ code: 'preview_required' })
  })
  expect(port.prepareCommand).not.toHaveBeenCalled()
})

it('restores only confirmed decisions from a conflict-free preview', async () => {
  const port = reviewPort(workspace('base'))
  const decisions = [{ historicalKey: targetKey, choice: { kind: 'use_historical' as const } }]
  vi.mocked(port.previewRestore)
    .mockResolvedValueOnce({ ...restorePlan(), conflicts: [targetKey] })
    .mockResolvedValueOnce(restorePlan())
  const { result } = renderHook(() => useContinuousReviewCoordinator({ ...session, port }))
  await waitFor(() => expect(result.current.state.kind).toBe('ready'))
  await act(() => result.current.previewRestore('archive-1', []))
  await act(async () => {
    await expect(result.current.restore()).rejects.toMatchObject({ code: 'needs_confirmation' })
  })
  expect(port.prepareCommand).not.toHaveBeenCalled()
  await act(() => result.current.previewRestore('archive-1', decisions))
  await act(() => result.current.restore())
  expect(vi.mocked(port.prepareCommand).mock.calls[0]?.[0]).toMatchObject({
    ...session,
    expectedSnapshotId: 'base',
    command: { kind: 'restore', archiveId: 'archive-1', decisions },
  })
})

it('does not publish a preview whose backend head differs from the locally displayed snapshot', async () => {
  const port = reviewPort(workspace('base'))
  vi.mocked(port.previewRestore).mockResolvedValue(restorePlan('external-head'))
  const { result } = renderHook(() => useContinuousReviewCoordinator({ ...session, port }))
  await waitFor(() => expect(result.current.state.kind).toBe('ready'))
  await act(async () => {
    await expect(result.current.previewRestore('archive-1', [])).rejects.toMatchObject({
      code: 'stale_snapshot',
    })
  })
  await act(async () => {
    await expect(result.current.restore()).rejects.toMatchObject({ code: 'preview_required' })
  })
  expect(port.prepareCommand).not.toHaveBeenCalled()
})
