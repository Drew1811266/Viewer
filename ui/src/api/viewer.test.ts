import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const invoke = vi.hoisted(() => vi.fn())
const listen = vi.hoisted(() => vi.fn())
const onDragDropEvent = vi.hoisted(() => vi.fn())

vi.mock('@tauri-apps/api/core', () => ({ invoke }))
vi.mock('@tauri-apps/api/event', () => ({ listen }))
vi.mock('@tauri-apps/api/webview', () => ({
  getCurrentWebview: () => ({ onDragDropEvent }),
}))

import { tauriViewerBridge } from './viewer'

beforeEach(() => {
  invoke.mockReset()
  listen.mockReset()
  onDragDropEvent.mockReset()
})

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('tauriViewerBridge', () => {
  it('maps every video operation and event to its one matching native contract', async () => {
    const handler = vi.fn()

    await tauriViewerBridge.videoOpen({
      entityId: 'video-1',
      surfaceRect: { x: 10, y: 20, width: 640, height: 360 },
    })
    await tauriViewerBridge.videoClose({ generation: 8 })
    await tauriViewerBridge.videoPlay({ generation: 8 })
    await tauriViewerBridge.videoPause({ generation: 8 })
    await tauriViewerBridge.videoSeek({ generation: 8, timeUs: 2_500_000 })
    await tauriViewerBridge.videoStep({ generation: 8, direction: 'forward' })
    await tauriViewerBridge.videoSetVolume({ generation: 8, volumePercent: 64 })
    await tauriViewerBridge.videoSetMuted({ generation: 8, muted: true })
    await tauriViewerBridge.videoSetRate({ generation: 8, rate: 'one_and_half' })
    await tauriViewerBridge.videoSetSurfaceRect({
      generation: 8,
      x: 12,
      y: 24,
      width: 960,
      height: 540,
    })
    await tauriViewerBridge.videoSetFullscreen({ generation: 8, fullscreen: true })
    await tauriViewerBridge.videoRequestThumbnail({
      generation: 8,
      requestId: 'thumbnail-1',
      timeUs: 3_000_000,
    })
    await tauriViewerBridge.videoCacheStats()
    await tauriViewerBridge.videoCacheClear()
    await tauriViewerBridge.listenVideo(handler)

    expect(invoke.mock.calls).toEqual([
      [
        'video_open',
        {
          request: {
            entityId: 'video-1',
            surfaceRect: { x: 10, y: 20, width: 640, height: 360 },
          },
        },
      ],
      ['video_close', { request: { generation: 8 } }],
      ['video_play', { request: { generation: 8 } }],
      ['video_pause', { request: { generation: 8 } }],
      ['video_seek', { request: { generation: 8, timeUs: 2_500_000 } }],
      ['video_step', { request: { generation: 8, direction: 'forward' } }],
      ['video_set_volume', { request: { generation: 8, volumePercent: 64 } }],
      ['video_set_muted', { request: { generation: 8, muted: true } }],
      ['video_set_rate', { request: { generation: 8, rate: 'one_and_half' } }],
      [
        'video_set_surface_rect',
        { request: { generation: 8, x: 12, y: 24, width: 960, height: 540 } },
      ],
      ['video_set_fullscreen', { request: { generation: 8, fullscreen: true } }],
      [
        'video_request_thumbnail',
        { request: { generation: 8, requestId: 'thumbnail-1', timeUs: 3_000_000 } },
      ],
      ['video_cache_stats'],
      ['video_cache_clear'],
    ])
    expect(listen).toHaveBeenCalledWith('viewer://video-event', expect.any(Function))
  })

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

  it('updates the complete viewer settings atomically', async () => {
    await tauriViewerBridge.updateViewerSettings({
      thumbnailDensity: 'large',
      magnifier: { shape: 'rounded_rectangle', magnification: 2, area: 'medium' },
    })

    expect(invoke).toHaveBeenCalledWith('update_viewer_settings', {
      settings: {
        thumbnailDensity: 'large',
        magnifier: { shape: 'rounded_rectangle', magnification: 2, area: 'medium' },
      },
    })
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
