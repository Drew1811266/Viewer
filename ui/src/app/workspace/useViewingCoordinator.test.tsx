import { act, renderHook } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { BrowserFile } from '../../api/types'
import type { ViewerState } from '../../state/viewerState'
import { initialViewerState } from '../../state/viewerState'
import type { PreviewDataPort, VideoPlaybackPort } from './ports'
import type { ViewingCommands } from './useViewingCoordinator'
import { useViewingCoordinator } from './useViewingCoordinator'

function file(entityId: string, kind: BrowserFile['kind'] = 'jpeg'): BrowserFile {
  return {
    entityId,
    relativePath: `${entityId}.file`,
    name: `${entityId}.file`,
    kind,
    size: 1,
    modifiedNs: '1',
    marker: { reviewState: null, favorite: false },
    imageMetadata: kind === 'jpeg' ? { width: 1, height: 1 } : null,
    imageUrl: null,
    videoMetadata: null,
  }
}

function state(sessionId: string, workspace: ViewerState['workspace'] = null): ViewerState {
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
    workspace,
  }
}

function previewPort(): PreviewDataPort {
  return {
    queryFolder: vi.fn().mockResolvedValue({ workspace: 'empty' }),
    requestImage: vi.fn().mockResolvedValue({
      cacheKey: 'image-1',
      url: 'viewer-image://image-1',
      width: 1,
      height: 1,
      backend: 'image_io',
    }),
    previewText: vi.fn().mockResolvedValue({
      entityId: 'text-1',
      format: 'plain_text',
      plainText: 'text',
      markdownHtml: null,
      encoding: 'utf8',
      truncated: false,
    }),
    openExternalLink: vi.fn().mockResolvedValue(undefined),
    videoRequestCover: vi.fn().mockResolvedValue('viewer-video-cover://video-1'),
  }
}

function commands(): ViewingCommands {
  return {
    setPreviewEntityId: vi.fn(),
    setCompareEntityIds: vi.fn(),
  }
}

function options(
  projectSessionId: string,
  viewerState = state(projectSessionId),
  port = previewPort(),
  viewingCommands = commands(),
) {
  return {
    state: viewerState,
    projectSessionId,
    port,
    playbackPort: {} as VideoPlaybackPort,
    commands: viewingCommands,
  }
}

describe('useViewingCoordinator', () => {
  it('invalidates preview, repair, and thumbnail cache ownership on project change', async () => {
    const image = file('image-1')
    const port = previewPort()
    const hook = renderHook(
      ({ sessionId }) => useViewingCoordinator(options(sessionId, state(sessionId), port)),
      { initialProps: { sessionId: 'session-1' } },
    )

    act(() => hook.result.current.openPreview(image))
    await act(() => hook.result.current.requestThumbnail(image, 200, 1000))
    hook.rerender({ sessionId: 'session-2' })

    expect(hook.result.current.activePreview).toBeNull()
    expect(hook.result.current.unavailablePreviewEntityIds.size).toBe(0)
    await act(() => hook.result.current.requestThumbnail(image, 200, 1000))
    expect(port.requestImage).toHaveBeenCalledTimes(2)
  })

  it('does not let stale navigation reopen a closed preview', () => {
    const image = file('image-1')
    const next = file('image-2')
    const hook = renderHook(() => useViewingCoordinator(options('session-1')))

    act(() => hook.result.current.openPreview(image))
    act(() => hook.result.current.closePreview())
    act(() => hook.result.current.navigatePreview(next))

    expect(hook.result.current.activePreview).toBeNull()
  })

  it('closes a filmstrip preview when its workspace projection identity changes', () => {
    const image = file('image-1')
    const initialWorkspace = {
      workspace: 'content' as const,
      images: [image],
      videos: [],
      otherFiles: [],
    }
    const hook = renderHook(
      ({ viewerState }) =>
        useViewingCoordinator(options('session-1', viewerState, previewPort(), commands())),
      { initialProps: { viewerState: state('session-1', initialWorkspace) } },
    )

    act(() => hook.result.current.openFilmstripPreview(image, [image]))
    expect(hook.result.current.activePreview).not.toBeNull()
    hook.rerender({
      viewerState: state('session-1', {
        ...initialWorkspace,
        images: [...initialWorkspace.images],
      }),
    })

    expect(hook.result.current.activePreview).toBeNull()
  })

  it('accumulates context repair IDs for the active ordered session', () => {
    const first = file('first')
    const second = file('second')
    const workspace = {
      workspace: 'content' as const,
      images: [first, second],
      videos: [],
      otherFiles: [],
    }
    const base = state('session-1', workspace)
    const hook = renderHook(
      ({ viewerState }) => useViewingCoordinator(options('session-1', viewerState)),
      { initialProps: { viewerState: base } },
    )

    act(() =>
      hook.result.current.openIntentPreview({
        kind: 'open-preview',
        file: first,
        files: [first, second],
        folderOverviewIdentity: null,
      }),
    )
    hook.rerender({
      viewerState: {
        ...base,
        contextRepair: {
          removedEntityIds: ['first'],
          suggestedEntityId: 'second',
          message: '第一次修复',
        },
      },
    })
    hook.rerender({
      viewerState: {
        ...base,
        contextRepair: {
          removedEntityIds: ['second'],
          suggestedEntityId: null,
          message: '第二次修复',
        },
      },
    })

    expect([...hook.result.current.unavailablePreviewEntityIds]).toEqual(['first', 'second'])
    expect(hook.result.current.activePreviewFiles).toEqual([first, second])
  })

  it('forwards image cancellation and does not request data merely to open unsupported files', async () => {
    const port = previewPort()
    const unsupported = file('archive-1', 'other')
    const image = file('image-1')
    const hook = renderHook(() =>
      useViewingCoordinator(options('session-1', state('session-1'), port)),
    )
    const abort = new AbortController()

    act(() => hook.result.current.openPreview(unsupported))
    expect(hook.result.current.activePreviewFile).toBe(unsupported)
    expect(port.requestImage).not.toHaveBeenCalled()
    expect(port.previewText).not.toHaveBeenCalled()

    await act(() =>
      hook.result.current.requestPreviewImage(
        image,
        { kind: 'fit_preview', maxWidth: 800, maxHeight: 600, scaleMilli: 1000 },
        abort.signal,
      ),
    )
    expect(port.requestImage).toHaveBeenCalledWith(
      {
        entityId: 'image-1',
        representation: {
          kind: 'fit_preview',
          maxWidth: 800,
          maxHeight: 600,
          scaleMilli: 1000,
        },
      },
      abort.signal,
    )
  })
})
