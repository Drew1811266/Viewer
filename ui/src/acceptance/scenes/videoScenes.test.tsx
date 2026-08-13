import { render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { defined } from '../../defined'
import type { AcceptanceRequest } from '../acceptanceRequest'
import { VIDEO_ACCEPTANCE_SCENE_IDS, VIDEO_SCENES } from './videoScenes'

const request: AcceptanceRequest = {
  id: 'video-preparing',
  viewport: '1024x720',
  width: 1024,
  height: 720,
}

beforeEach(() => {
  vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue({
    x: 100,
    y: 50,
    left: 100,
    top: 50,
    right: 900,
    bottom: 650,
    width: 800,
    height: 600,
    toJSON: () => undefined,
  })
  vi.stubGlobal(
    'ResizeObserver',
    class {
      observe() {}
      disconnect() {}
    },
  )
})

afterEach(() => {
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
})

describe('video acceptance scenes', () => {
  it('registers the exact twelve approved scene IDs', () => {
    expect(Object.keys(VIDEO_SCENES)).toEqual(VIDEO_ACCEPTANCE_SCENE_IDS)
    expect(VIDEO_ACCEPTANCE_SCENE_IDS).toEqual([
      'workspace-video-expanded',
      'workspace-video-unavailable',
      'video-preparing',
      'video-playing-controls',
      'video-paused-controls',
      'video-timeline-pending',
      'video-timeline-ready',
      'video-ended',
      'video-failed-retry',
      'video-fullscreen-controls',
      'settings-video-cache',
      'video-reduced-motion',
    ])
  })

  it('renders preparing and failed states through a fake bridge without native media', async () => {
    const preparing = renderScene('video-preparing')
    expect(await screen.findByRole('status', { name: '正在加载视频' })).toBeVisible()
    expect(document.querySelector('[data-video-bridge="fake"]')).not.toBeNull()
    preparing.unmount()

    const failed = renderScene('video-failed-retry')
    expect(await screen.findByText('视频已损坏或无法读取')).toBeVisible()
    expect(screen.getByRole('button', { name: '重试' })).toBeEnabled()
    failed.unmount()
  })

  it('uses a static registered image artifact for ready playback scenes', async () => {
    const rendered = renderScene('video-playing-controls')
    const frame = await screen.findByRole('img', { name: '视频静态验收帧' })
    expect(frame).toHaveAttribute('src', expect.stringMatching(/^\/.+\.jpg$/))
    expect(screen.getByRole('group', { name: '视频播放控制' })).toBeVisible()
    rendered.unmount()
  })

  it('drives the production reduced-motion state for the reduced-motion scene', async () => {
    vi.stubGlobal(
      'matchMedia',
      vi.fn().mockReturnValue({
        matches: true,
        media: '(prefers-reduced-motion: reduce)',
        onchange: null,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
        addListener: vi.fn(),
        removeListener: vi.fn(),
        dispatchEvent: vi.fn(),
      }),
    )

    const rendered = renderScene('video-reduced-motion')

    expect(await screen.findByRole('dialog', { name: /视频预览/ })).toHaveAttribute(
      'data-reduced-motion',
      'true',
    )
    expect(screen.getByRole('group', { name: '视频播放控制' })).toHaveAttribute(
      'data-reduced-motion',
      'true',
    )
    rendered.unmount()
  })
})

function renderScene(id: string) {
  const Scene = defined(VIDEO_SCENES[id], `Missing ${id} acceptance scene`)
  return render(<Scene request={{ ...request, id }} />)
}
