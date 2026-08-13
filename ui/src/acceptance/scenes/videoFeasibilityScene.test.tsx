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
  commandSerial: 0,
  lastCommandLatencyUs: 0,
  generationDiagnostics: {
    generation: 7,
    mountReturned: true,
    updateCallbacks: 3,
    drawEntries: 2,
    frameUpdates: 1,
    pictureFrames: 1,
    reveals: 1,
    eventEmits: 1,
  },
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
let commandSerial = 0

beforeEach(() => {
  vi.clearAllMocks()
  forwardStep = false
  backwardStep = false
  playbackTimeUs = 0
  commandSerial = 0
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
    if (action === 'play' || action === 'pause') commandSerial += 1
    const active = action !== 'close'
    return {
      ...report,
      forwardStep,
      backwardStep,
      playbackTimeUs,
      commandSerial,
      lastCommandLatencyUs: commandSerial > 0 ? 450 : 0,
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

  it('exposes the typed native command-to-engine latency', async () => {
    render(<VideoFeasibilityScene request={request} />)
    await screen.findByText('首帧：就绪')
    fireEvent.click(screen.getByRole('button', { name: '播放性能样本' }))
    await waitFor(() =>
      expect(screen.getByRole('status', { name: '引擎延迟：450' })).toBeInTheDocument(),
    )
    expect(screen.getByRole('status', { name: '命令：1' })).toBeInTheDocument()
  })

  it('remounts the current performance fixture without navigating through another video', async () => {
    let generation = 6
    vi.mocked(invoke).mockImplementation(async (_command, payload) => {
      const action = (payload as { request: { action: string } }).request.action
      if (action === 'mount') generation += 1
      return {
        ...report,
        generationDiagnostics: { ...report.generationDiagnostics, generation },
      }
    })
    render(<VideoFeasibilityScene request={request} />)
    await screen.findByText('首帧：就绪')
    const firstRemountCall = vi.mocked(invoke).mock.calls.length

    fireEvent.click(screen.getByRole('button', { name: '重新挂载当前样本' }))

    await waitFor(() => {
      const requests = vi
        .mocked(invoke)
        .mock.calls.slice(firstRemountCall)
        .map((call) => (call[1] as { request: { action: string; fixtureId: string } }).request)
      expect(requests).toEqual([
        expect.objectContaining({ action: 'close', fixtureId: 'h264-1080p' }),
        expect.objectContaining({ action: 'mount', fixtureId: 'h264-1080p' }),
      ])
    })
    expect(screen.getByRole('status', { name: '样本：h264-1080p' })).toBeInTheDocument()
    expect(await screen.findByRole('status', { name: '首帧：就绪' })).toBeInTheDocument()
  })

  it('rejects an old same-fixture frame while the remounted generation is pending', async () => {
    type DeferredReport = Omit<typeof report, 'decodedPictureType'> & {
      decodedPictureType: string | null
    }
    type FrameListener = (event: { payload: DeferredReport }) => void
    let frameListener: FrameListener | undefined
    let resolveRemount: ((value: DeferredReport) => void) | undefined
    let mountCount = 0
    vi.mocked(listen).mockImplementation(async (_event, handler) => {
      frameListener = handler as unknown as FrameListener
      return () => undefined
    })
    vi.mocked(invoke).mockImplementation(async (_command, payload) => {
      const action = (payload as { request: { action: string } }).request.action
      if (action === 'mount') {
        mountCount += 1
        if (mountCount === 2) {
          return new Promise<DeferredReport>((resolve) => {
            resolveRemount = resolve
          })
        }
      }
      return {
        ...report,
        firstFrameReady: action !== 'close',
        generationDiagnostics: { ...report.generationDiagnostics, generation: 7 },
      }
    })
    render(<VideoFeasibilityScene request={request} />)
    await screen.findByText('首帧：就绪')

    fireEvent.click(screen.getByRole('button', { name: '重新挂载当前样本' }))
    await waitFor(() => expect(resolveRemount).toBeDefined())
    expect(screen.getByRole('status', { name: '首帧：等待' })).toBeInTheDocument()

    act(() => frameListener?.({ payload: report }))
    expect(screen.getByRole('status', { name: '首帧：等待' })).toBeInTheDocument()

    await act(async () =>
      resolveRemount?.({
        ...report,
        firstFrameReady: false,
        decodedPictureType: null,
        generationDiagnostics: { ...report.generationDiagnostics, generation: 8 },
      }),
    )
    act(() =>
      frameListener?.({
        payload: {
          ...report,
          generationDiagnostics: { ...report.generationDiagnostics, generation: 8 },
        },
      }),
    )
    expect(screen.getByRole('status', { name: '首帧：就绪' })).toBeInTheDocument()
    expect(
      screen.getByRole('status', {
        name: '诊断代：8 挂载：1 回调：3 绘制入口：2 FRAME：1 图像：1 显示：1 事件：1',
      }),
    ).toBeInTheDocument()
  })

  it('closes an invalidated late remount by its native generation', async () => {
    let mountCount = 0
    let resolveRemount: ((value: typeof report) => void) | undefined
    vi.mocked(invoke).mockImplementation(async (_command, payload) => {
      const action = (payload as { request: { action: string } }).request.action
      if (action === 'mount') {
        mountCount += 1
        if (mountCount === 2) {
          return new Promise<typeof report>((resolve) => {
            resolveRemount = resolve
          })
        }
      }
      return report
    })
    const view = render(<VideoFeasibilityScene request={request} />)
    await screen.findByText('首帧：就绪')
    fireEvent.click(screen.getByRole('button', { name: '重新挂载当前样本' }))
    await waitFor(() => expect(resolveRemount).toBeDefined())

    view.unmount()
    await act(async () =>
      resolveRemount?.({
        ...report,
        generationDiagnostics: { ...report.generationDiagnostics, generation: 8 },
      }),
    )

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith('run_video_feasibility', {
        request: expect.objectContaining({
          action: 'close',
          fixtureId: 'h264-1080p',
          expectedGeneration: 8,
        }),
      }),
    )
  })

  it('exposes generation-bound native pipeline counters to accessibility diagnostics', async () => {
    render(<VideoFeasibilityScene request={request} />)

    await screen.findByRole('status', {
      name: '诊断代：7 挂载：1 回调：3 绘制入口：2 FRAME：1 图像：1 显示：1 事件：1',
    })
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

  it('samples a real baseline and alternates navigation fixtures across thirty lifecycle cycles', async () => {
    const baseline = { activeClients: 2, activeRenderContexts: 3, activeSurfaces: 4 }
    vi.mocked(invoke).mockImplementation(async (_command, payload) => {
      const request = (payload as { request: { action: string; fixtureId: string } }).request
      const active = request.action !== 'close'
      return {
        ...report,
        fixtureId: request.fixtureId,
        diagnostics: {
          ...report.diagnostics,
          activeClients: baseline.activeClients + (active ? 1 : 0),
          activeRenderContexts: baseline.activeRenderContexts + (active ? 1 : 0),
          activeSurfaces: baseline.activeSurfaces + (active ? 1 : 0),
        },
      }
    })
    render(<VideoFeasibilityScene request={request} />)
    await screen.findByText('首帧：就绪')

    const firstLifecycleCall = vi.mocked(invoke).mock.calls.length
    fireEvent.click(screen.getByRole('button', { name: '循环挂载与卸载 30 次' }))

    await screen.findByText('生命周期：30/30')
    const lifecycleRequests = vi
      .mocked(invoke)
      .mock.calls.slice(firstLifecycleCall)
      .map((call) => (call[1] as { request: { action: string; fixtureId: string } }).request)
    const mountedFixtures = lifecycleRequests
      .filter(({ action }) => action === 'mount')
      .map(({ fixtureId }) => fixtureId)
    expect(mountedFixtures).toHaveLength(30)
    expect(new Set(mountedFixtures)).toEqual(new Set(['h264-1080p', 'hevc-portrait']))
    expect(
      mountedFixtures.every(
        (fixture, index) => index === 0 || fixture !== mountedFixtures[index - 1],
      ),
    ).toBe(true)
    expect(lifecycleRequests.filter(({ action }) => action === 'close')).toHaveLength(31)
    expect(screen.getByRole('status', { name: '生命周期基线：2/3/4' })).toBeInTheDocument()
    expect(screen.getByRole('status', { name: '生命周期结束：2/3/4' })).toBeInTheDocument()
    expect(
      screen.getByRole('status', { name: `生命周期路径：${mountedFixtures.join(',')}` }),
    ).toBeInTheDocument()
  })

  it('keeps a same-generation event-ready frame after a delayed mount snapshot and isolates stale fixture generations', async () => {
    type DeferredReport = Omit<typeof report, 'decodedPictureType'> & {
      decodedPictureType: string | null
    }
    type FrameListener = (event: { payload: DeferredReport }) => void
    const frameListeners: FrameListener[] = []
    const resolveListeners: Array<() => void> = []
    const mounts: Array<{
      fixtureId: string
      resolve: (value: DeferredReport) => void
    }> = []
    const waitingReport = (fixtureId: string, generation: number) => ({
      ...report,
      fixtureId,
      firstFrameReady: false,
      decodedPictureType: null,
      generationDiagnostics: { ...report.generationDiagnostics, generation },
    })

    vi.mocked(listen).mockImplementation((_event, handler) => {
      frameListeners.push(handler as unknown as FrameListener)
      return new Promise<() => void>((resolve) => {
        resolveListeners.push(() => resolve(() => undefined))
      })
    })
    vi.mocked(invoke).mockImplementation((_command, payload) => {
      const request = (payload as { request: { action: string; fixtureId: string } }).request
      if (request.action === 'mount') {
        return new Promise<DeferredReport>((resolve) => {
          mounts.push({ fixtureId: request.fixtureId, resolve })
        })
      }
      return Promise.resolve(report)
    })

    render(<VideoFeasibilityScene request={request} />)

    await waitFor(() => expect(resolveListeners).toHaveLength(1))
    expect(invoke).not.toHaveBeenCalled()
    await act(async () => resolveListeners[0]?.())
    await waitFor(() => expect(mounts).toHaveLength(1))

    act(() =>
      frameListeners[0]?.({
        payload: {
          ...waitingReport('h264-1080p', 7),
          firstFrameReady: true,
          decodedPictureType: 'I',
        },
      }),
    )
    expect(screen.getByRole('status', { name: '首帧：就绪' })).toBeInTheDocument()

    await act(async () => mounts[0]?.resolve(waitingReport('h264-1080p', 7)))
    expect(screen.getByRole('status', { name: '首帧：就绪' })).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: '切换样本 hevc-4k30' }))
    await waitFor(() => expect(resolveListeners).toHaveLength(2))
    expect(screen.getByRole('status', { name: '样本：hevc-4k30' })).toBeInTheDocument()
    expect(screen.getByRole('status', { name: '首帧：等待' })).toBeInTheDocument()

    act(() =>
      frameListeners[0]?.({
        payload: {
          ...waitingReport('h264-1080p', 7),
          firstFrameReady: true,
          decodedPictureType: 'I',
        },
      }),
    )
    expect(screen.getByRole('status', { name: '首帧：等待' })).toBeInTheDocument()

    await act(async () => resolveListeners[1]?.())
    await waitFor(() => expect(mounts).toHaveLength(2))

    fireEvent.click(screen.getByRole('button', { name: '切换样本 h264-1080p' }))
    await waitFor(() => expect(resolveListeners).toHaveLength(3))
    expect(screen.getByRole('status', { name: '样本：h264-1080p' })).toBeInTheDocument()
    expect(screen.getByRole('status', { name: '首帧：等待' })).toBeInTheDocument()

    await act(async () =>
      mounts[1]?.resolve({
        ...waitingReport('hevc-4k30', 8),
        firstFrameReady: true,
        decodedPictureType: 'I',
      }),
    )
    act(() =>
      frameListeners[1]?.({
        payload: {
          ...waitingReport('hevc-4k30', 8),
          firstFrameReady: true,
          decodedPictureType: 'I',
        },
      }),
    )
    expect(screen.getByRole('status', { name: '样本：h264-1080p' })).toBeInTheDocument()
    expect(screen.getByRole('status', { name: '首帧：等待' })).toBeInTheDocument()
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
