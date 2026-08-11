import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { AcceptanceRequest } from '../acceptanceRequest'
import { VideoFeasibilityScene } from './videoFeasibilityScene'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }))

const request: AcceptanceRequest = {
  id: 'VIDEO-FEASIBILITY',
  viewport: '1024x720',
  width: 1024,
  height: 720,
}

const report = {
  fixtureId: 'h264-1080p',
  rect: { x: 152, y: 120, width: 720, height: 405 },
  firstFrameReady: true,
  decodedPictureType: 'I',
  forwardStep: false,
  backwardStep: false,
  playbackTimeUs: 0,
  diagnostics: {
    hwdec: 'videotoolbox',
    videoOutput: 'libmpv/opengl',
    activeClients: 1,
    activeRenderContexts: 1,
    activeSurfaces: 1,
    renderedFrames: 4,
  },
}

let forwardStep = false
let backwardStep = false
let playbackTimeUs = 0

beforeEach(() => {
  vi.clearAllMocks()
  forwardStep = false
  backwardStep = false
  playbackTimeUs = 0
  vi.mocked(listen).mockResolvedValue(() => undefined)
  vi.mocked(invoke).mockImplementation(async (_command, payload) => {
    const action = (payload as { request: { action: string } }).request.action
    if (action === 'frame_step_forward') {
      forwardStep = true
      playbackTimeUs = 33_333
    }
    if (action === 'frame_step_backward') {
      backwardStep = true
      playbackTimeUs = 0
    }
    const active = action !== 'close'
    return {
      ...report,
      forwardStep,
      backwardStep,
      playbackTimeUs,
      diagnostics: {
        ...report.diagnostics,
        activeClients: active ? 1 : 0,
        activeRenderContexts: active ? 1 : 0,
        activeSurfaces: active ? 1 : 0,
      },
    }
  })
  vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (
    this: HTMLElement,
  ) {
    if (this.dataset.nativeSurfaceSlot === 'true') {
      return rectangle(152, 120, 720, 405)
    }
    return rectangle(0, 0, 0, 0)
  })
})

describe('video feasibility scene', () => {
  it('sends only the final CSS rectangle and fixture ID across IPC', async () => {
    render(<VideoFeasibilityScene request={request} />)

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith('run_video_feasibility', {
        request: {
          fixtureId: 'h264-1080p',
          action: 'mount',
          rect: { x: 152, y: 120, width: 720, height: 405 },
        },
      }),
    )
    expect(screen.getByTestId('video-feasibility-slot')).toHaveAttribute(
      'data-first-frame-ready',
      'true',
    )
    expect(screen.getByText('首帧：就绪')).toBeInTheDocument()
    expect(screen.getByText('解码帧：I')).toBeInTheDocument()
    expect(screen.getByRole('status', { name: '已绘制：4' })).toBeInTheDocument()
    expect(JSON.stringify(vi.mocked(invoke).mock.calls)).not.toContain('frameBuffer')
  })

  it('exercises explicit forward and backward frame-step actions through React overlays', async () => {
    render(<VideoFeasibilityScene request={request} />)
    await screen.findByText('videotoolbox')

    fireEvent.click(screen.getByRole('button', { name: '前进一帧' }))
    await screen.findByRole('status', { name: '时间：33333 µs' })
    fireEvent.click(screen.getByRole('button', { name: '后退一帧' }))

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        'run_video_feasibility',
        expect.objectContaining({
          request: expect.objectContaining({ action: 'frame_step_forward' }),
        }),
      )
      expect(invoke).toHaveBeenCalledWith(
        'run_video_feasibility',
        expect.objectContaining({
          request: expect.objectContaining({ action: 'frame_step_backward' }),
        }),
      )
      expect(screen.getByText('前进：完成')).toBeInTheDocument()
      expect(screen.getByText('后退：完成')).toBeInTheDocument()
      expect(screen.getByRole('status', { name: '时间：0 µs' })).toBeInTheDocument()
    })
  })

  it('runs thirty mount/unmount cycles and exposes the released baseline', async () => {
    render(<VideoFeasibilityScene request={request} />)
    await screen.findByText('首帧：就绪')

    fireEvent.click(screen.getByRole('button', { name: '循环挂载与卸载 30 次' }))

    await screen.findByText('生命周期：30/30')
    const actions = vi
      .mocked(invoke)
      .mock.calls.map((call) => (call[1] as { request: { action: string } }).request.action)
    expect(actions.filter((action) => action === 'mount')).toHaveLength(30)
    expect(actions.filter((action) => action === 'close')).toHaveLength(30)
    expect(screen.getByText('0/0/0')).toBeInTheDocument()
  })

  it('closes a native session whose mount resolves after React cleanup', async () => {
    let resolveMount: ((value: typeof report) => void) | undefined
    vi.mocked(invoke).mockImplementation(async (_command, payload) => {
      const action = (payload as { request: { action: string } }).request.action
      if (action === 'mount') {
        return new Promise<typeof report>((resolve) => {
          resolveMount = resolve
        })
      }
      return {
        ...report,
        diagnostics: {
          ...report.diagnostics,
          activeClients: 0,
          activeRenderContexts: 0,
          activeSurfaces: 0,
        },
      }
    })
    const view = render(<VideoFeasibilityScene request={request} />)
    await waitFor(() => expect(resolveMount).toBeDefined())

    view.unmount()
    await act(async () => resolveMount?.(report))

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith(
        'run_video_feasibility',
        expect.objectContaining({ request: expect.objectContaining({ action: 'close' }) }),
      ),
    )
  })
})

function rectangle(x: number, y: number, width: number, height: number): DOMRect {
  return {
    x,
    y,
    left: x,
    top: y,
    right: x + width,
    bottom: y + height,
    width,
    height,
    toJSON: () => undefined,
  } as DOMRect
}
