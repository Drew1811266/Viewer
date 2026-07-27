import { describe, expect, it, vi } from 'vitest'

const invoke = vi.hoisted(() => vi.fn())

vi.mock('@tauri-apps/api/core', () => ({ invoke }))

import { tauriViewerBridge } from './viewer'

describe('tauriViewerBridge settings commands', () => {
  it('gets the viewer settings without arguments', async () => {
    await tauriViewerBridge.getViewerSettings()

    expect(invoke).toHaveBeenCalledWith('get_viewer_settings')
  })

  it('updates thumbnail density with the narrow density argument', async () => {
    await tauriViewerBridge.updateThumbnailDensity('large')

    expect(invoke).toHaveBeenCalledWith('update_thumbnail_density', { density: 'large' })
  })
})
