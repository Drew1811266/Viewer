import { act, renderHook } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { BrowserFile } from '../../api/types'
import type { ViewerState } from '../../state/viewerState'
import { initialViewerState } from '../../state/viewerState'
import type { OrganizationCommands } from './useOrganizationCoordinator'
import { useOrganizationCoordinator } from './useOrganizationCoordinator'

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((next) => {
    resolve = next
  })
  return { promise, resolve }
}

function file(entityId: string): BrowserFile {
  return {
    entityId,
    relativePath: `${entityId}.jpg`,
    name: `${entityId}.jpg`,
    kind: 'jpeg',
    size: 1,
    modifiedNs: '1',
    marker: { reviewState: null, favorite: false },
    imageMetadata: { width: 1, height: 1 },
    imageUrl: null,
    videoMetadata: null,
  }
}

function state(sessionId = 'session-1'): ViewerState {
  return {
    ...initialViewerState,
    status: 'active',
    project: {
      projectId: 'project-1',
      sessionId,
      generation: 1,
      displayName: 'Catalog',
      access: 'read_write',
    },
    operation: {
      kind: null,
      active: null,
      results: null,
      finishing: false,
      pending: false,
    },
  }
}

function commands(): OrganizationCommands {
  return {
    setSelectedEntityIds: vi.fn(),
    setReviewState: vi.fn().mockResolvedValue(undefined),
    toggleFavorite: vi.fn().mockResolvedValue(undefined),
    previewRename: vi.fn().mockResolvedValue(null),
    preflightFileCommand: vi.fn().mockResolvedValue(null),
    executeFileCommand: vi.fn().mockResolvedValue(null),
    cancelOperation: vi.fn().mockResolvedValue(false),
    loadOperationResults: vi.fn().mockResolvedValue(null),
    undoLastOperation: vi.fn().mockResolvedValue(false),
    consumeContextRepair: vi.fn(),
    beginFinderDrag: vi.fn().mockResolvedValue(undefined),
  }
}

describe('useOrganizationCoordinator', () => {
  it('owns selection and clears all session-local organization state on project change', async () => {
    const controller = commands()
    const pending = deferred<Awaited<ReturnType<OrganizationCommands['executeFileCommand']>>>()
    vi.mocked(controller.executeFileCommand).mockReturnValue(pending.promise)
    const hook = renderHook(
      ({ viewerState }) => useOrganizationCoordinator({ state: viewerState, commands: controller }),
      { initialProps: { viewerState: state() } },
    )

    act(() => {
      hook.result.current.selectFiles([file('image-1')])
    })
    act(() => {
      hook.result.current.openRenameDialog()
      hook.result.current.showResults('batch-1')
    })
    await act(async () => {
      vi.mocked(controller.beginFinderDrag).mockRejectedValueOnce(new Error('nope'))
      hook.result.current.exportToFinder(['image-1'])
      await Promise.resolve()
    })
    let submission!: ReturnType<typeof hook.result.current.submitFileCommand>
    act(() => {
      submission = hook.result.current.submitFileCommand('trash', [
        { entityId: 'image-1', action: { kind: 'trash' } },
      ])
    })

    expect(controller.setSelectedEntityIds).toHaveBeenCalledWith(['image-1'])
    expect(hook.result.current.operationSubmitting).toBe(true)
    expect(hook.result.current.finderDragMessage).toBe('无法拖到 Finder，请重新拖动。')

    hook.rerender({ viewerState: state('session-2') })
    expect(hook.result.current.selectedFiles).toEqual([])
    expect(hook.result.current.finderDragMessage).toBeNull()
    expect(hook.result.current.operationDialog).toBeNull()
    expect(hook.result.current.resultsBatchId).toBeNull()
    expect(hook.result.current.operationSubmitting).toBe(false)

    await act(async () => {
      pending.resolve(null)
      await submission
    })
  })

  it.each([
    ['finder_drag_selection_stale', '部分文件已发生变化，请刷新后重试。'],
    ['stale_project_session', '部分文件已发生变化，请刷新后重试。'],
    ['backend_unavailable', '无法拖到 Finder，请重新拖动。'],
  ])('preserves Finder failure copy for %s', async (code, message) => {
    const controller = commands()
    vi.mocked(controller.beginFinderDrag).mockRejectedValueOnce({ code })
    const hook = renderHook(() =>
      useOrganizationCoordinator({ state: state(), commands: controller }),
    )

    await act(async () => {
      hook.result.current.exportToFinder(['image-1'])
      await Promise.resolve()
    })

    expect(hook.result.current.finderDragMessage).toBe(message)
  })

  it('opens completed results once and does not reopen them after dismissal', () => {
    const controller = commands()
    const completedState: ViewerState = {
      ...state(),
      operation: {
        kind: 'move',
        active: {
          sessionId: 'session-1',
          generation: 1,
          batchId: 'batch-1',
          lifecycle: 'completed',
          requested: 1,
          completed: 1,
          failed: 0,
          skipped: 0,
          cancelled: 0,
          activeEntityId: null,
        },
        results: { total: 0, offset: 0, items: [] },
        finishing: false,
        pending: false,
      },
    }
    const hook = renderHook(
      ({ viewerState }) => useOrganizationCoordinator({ state: viewerState, commands: controller }),
      { initialProps: { viewerState: completedState } },
    )

    expect(hook.result.current.resultsBatchId).toBe('batch-1')
    act(() => hook.result.current.closeResults())
    hook.rerender({ viewerState: completedState })
    expect(hook.result.current.resultsBatchId).toBeNull()
  })

  it('always clears submitting and closes the dialog only after a started command', async () => {
    const controller = commands()
    const hook = renderHook(() =>
      useOrganizationCoordinator({ state: state(), commands: controller }),
    )

    act(() => {
      hook.result.current.selectFiles([file('image-1')])
    })
    act(() => {
      hook.result.current.openTrashDialog()
    })
    await act(async () => {
      await expect(
        hook.result.current.submitFileCommand('trash', [
          { entityId: 'image-1', action: { kind: 'trash' } },
        ]),
      ).resolves.toBe(false)
    })
    expect(hook.result.current.operationSubmitting).toBe(false)
    expect(hook.result.current.operationDialog?.kind).toBe('trash')

    vi.mocked(controller.executeFileCommand).mockRejectedValueOnce(new Error('unexpected'))
    await act(async () => {
      await expect(
        hook.result.current.submitFileCommand('trash', [
          { entityId: 'image-1', action: { kind: 'trash' } },
        ]),
      ).rejects.toThrow('unexpected')
    })
    expect(hook.result.current.operationSubmitting).toBe(false)
    expect(hook.result.current.operationDialog?.kind).toBe('trash')

    vi.mocked(controller.executeFileCommand).mockResolvedValueOnce({ batchId: 'batch-1' })
    await act(async () => {
      await expect(
        hook.result.current.submitFileCommand('trash', [
          { entityId: 'image-1', action: { kind: 'trash' } },
        ]),
      ).resolves.toBe(true)
    })
    expect(hook.result.current.operationSubmitting).toBe(false)
    expect(hook.result.current.operationDialog).toBeNull()
  })
})
