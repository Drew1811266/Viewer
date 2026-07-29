import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const invoke = vi.hoisted(() => vi.fn())

vi.mock('@tauri-apps/api/core', () => ({ invoke }))

import { tauriViewerBridge } from './viewer'

beforeEach(() => {
  invoke.mockReset()
})

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('tauriViewerBridge', () => {
  it('gets the viewer settings without arguments', async () => {
    await tauriViewerBridge.getViewerSettings()

    expect(invoke).toHaveBeenCalledWith('get_viewer_settings')
  })

  it('updates thumbnail density with the narrow density argument', async () => {
    await tauriViewerBridge.updateThumbnailDensity('large')

    expect(invoke).toHaveBeenCalledWith('update_thumbnail_density', { density: 'large' })
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
