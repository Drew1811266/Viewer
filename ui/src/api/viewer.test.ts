import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const invoke = vi.hoisted(() => vi.fn())
const onDragDropEvent = vi.hoisted(() => vi.fn())

vi.mock('@tauri-apps/api/core', () => ({ invoke }))
vi.mock('@tauri-apps/api/webview', () => ({
  getCurrentWebview: () => ({ onDragDropEvent }),
}))

import { tauriViewerBridge } from './viewer'

beforeEach(() => {
  invoke.mockReset()
  onDragDropEvent.mockReset()
})

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('tauriViewerBridge', () => {
  it('forwards the complete native project drag lifecycle', async () => {
    let receive:
      | ((event: {
          payload:
            | { type: 'enter'; paths: string[]; position: { x: number; y: number } }
            | { type: 'over'; position: { x: number; y: number } }
            | { type: 'drop'; paths: string[]; position: { x: number; y: number } }
            | { type: 'leave' }
        }) => void)
      | undefined
    const unlisten = vi.fn()
    onDragDropEvent.mockImplementation(async (handler) => {
      receive = handler
      return unlisten
    })
    const handler = vi.fn()

    await tauriViewerBridge.listenProjectDropEvents(handler)
    receive?.({ payload: { type: 'enter', paths: ['/fixture/project'], position: { x: 1, y: 2 } } })
    receive?.({ payload: { type: 'over', position: { x: 2, y: 3 } } })
    receive?.({ payload: { type: 'drop', paths: ['/fixture/project'], position: { x: 3, y: 4 } } })
    receive?.({ payload: { type: 'leave' } })

    expect(handler.mock.calls.map(([event]) => event)).toEqual([
      { type: 'enter', paths: ['/fixture/project'] },
      { type: 'over' },
      { type: 'drop', paths: ['/fixture/project'] },
      { type: 'leave' },
    ])
  })

  it('gets the viewer settings without arguments', async () => {
    await tauriViewerBridge.getViewerSettings()

    expect(invoke).toHaveBeenCalledWith('get_viewer_settings')
  })

  it('updates thumbnail density with the narrow density argument', async () => {
    await tauriViewerBridge.updateThumbnailDensity('maximum')

    expect(invoke).toHaveBeenCalledWith('update_thumbnail_density', { density: 'maximum' })
  })

  it('rejects an aborted image request immediately and cancels its native request', async () => {
    vi.stubGlobal('crypto', { randomUUID: () => 'image-request-1' })
    const nativeRequest = new Promise(() => undefined)
    invoke.mockImplementation((command: string) => {
      if (command === 'request_image_representation') return nativeRequest
      if (command === 'cancel_image_request') return Promise.resolve(true)
      throw new Error(`Unexpected command: ${command}`)
    })
    const controller = new AbortController()

    const result = tauriViewerBridge.requestImage(
      {
        entityId: 'image-1',
        representation: { kind: 'original100_percent' },
      },
      controller.signal,
    )
    controller.abort()

    await expect(result).rejects.toMatchObject({ name: 'AbortError' })
    expect(invoke).toHaveBeenCalledWith('request_image_representation', {
      entityId: 'image-1',
      representation: { kind: 'original100_percent' },
      requestId: 'image-request-1',
    })
    expect(invoke).toHaveBeenCalledWith('cancel_image_request', {
      requestId: 'image-request-1',
    })
  })

  it('does not invoke native image rendering when the signal is already aborted', async () => {
    const controller = new AbortController()
    controller.abort()

    const result = tauriViewerBridge.requestImage(
      {
        entityId: 'image-1',
        representation: { kind: 'original100_percent' },
      },
      controller.signal,
    )

    await expect(result).rejects.toMatchObject({ name: 'AbortError' })
    expect(invoke).not.toHaveBeenCalled()
  })
})
