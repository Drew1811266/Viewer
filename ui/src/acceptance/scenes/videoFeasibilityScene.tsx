import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react'
import type { AcceptanceSceneRegistry } from '../AcceptanceApp'
import type { AcceptanceRequest } from '../acceptanceRequest'

type FixtureId = 'h264-1080p' | 'hevc-portrait' | 'vfr-step'
type FeasibilityAction =
  | 'mount'
  | 'update_geometry'
  | 'frame_step_forward'
  | 'frame_step_backward'
  | 'status'
  | 'close'

interface SurfaceRect {
  x: number
  y: number
  width: number
  height: number
}

interface VideoRenderDiagnostics {
  hwdec: string
  videoOutput: string
  activeClients: number
  activeRenderContexts: number
  activeSurfaces: number
  renderedFrames: number
}

interface VideoFeasibilityReport {
  fixtureId: string
  rect: SurfaceRect
  firstFrameReady: boolean
  forwardStep: boolean
  backwardStep: boolean
  playbackTimeUs: number | null
  diagnostics: VideoRenderDiagnostics
}

export const VIDEO_FEASIBILITY_SCENES: AcceptanceSceneRegistry = {
  'VIDEO-FEASIBILITY': VideoFeasibilityScene,
}

export function VideoFeasibilityScene({ request }: { request: AcceptanceRequest }) {
  const fixtureId = fixtureFromHash(window.location.hash)
  const slot = useRef<HTMLDivElement>(null)
  const mounted = useRef(false)
  const rect = useRef<SurfaceRect | null>(null)
  const [slotWidth, setSlotWidth] = useState(Math.min(720, request.width - 240))
  const [report, setReport] = useState<VideoFeasibilityReport | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [lifecycleCycles, setLifecycleCycles] = useState(0)
  const [cycling, setCycling] = useState(false)
  const slotHeight = Math.round((slotWidth * 9) / 16)
  const slotX = Math.round((request.width - slotWidth) / 2)
  const slotY = 120

  const send = useCallback(
    async (action: FeasibilityAction, currentRect = rect.current, publish = true) => {
      if (currentRect === null) throw new Error('Native video surface rectangle is unavailable')
      const next = await invoke<VideoFeasibilityReport>('run_video_feasibility', {
        request: { fixtureId, action, rect: currentRect },
      })
      if (publish) setReport(next)
      return next
    },
    [fixtureId],
  )

  useEffect(() => {
    const rootBackground = document.documentElement.style.background
    const bodyBackground = document.body.style.background
    document.documentElement.style.background = 'transparent'
    document.body.style.background = 'transparent'
    return () => {
      document.documentElement.style.background = rootBackground
      document.body.style.background = bodyBackground
    }
  }, [])

  useLayoutEffect(() => {
    const element = slot.current
    if (element === null) return
    const finalRect = surfaceRect(element)
    rect.current = finalRect
    let disposed = false
    let unlisten: (() => void) | undefined
    void listen<VideoFeasibilityReport>('viewer://video-feasibility-frame', ({ payload }) => {
      if (!disposed && payload.fixtureId === fixtureId) setReport(payload)
    }).then((release) => {
      if (disposed) release()
      else unlisten = release
    })
    void send('mount', finalRect).then(
      () => {
        if (disposed) void send('close', finalRect, false).catch(() => undefined)
        else mounted.current = true
      },
      (caught: unknown) => setError(errorMessage(caught)),
    )
    return () => {
      disposed = true
      unlisten?.()
      if (mounted.current) void send('close', rect.current, false).catch(() => undefined)
      mounted.current = false
    }
  }, [fixtureId, send])

  useLayoutEffect(() => {
    const element = slot.current
    if (element === null) return
    const finalRect = surfaceRect(element)
    rect.current = finalRect
    if (mounted.current) {
      void send('update_geometry', finalRect).catch((caught: unknown) =>
        setError(errorMessage(caught)),
      )
    }
  }, [send, slotWidth])

  const run = (action: FeasibilityAction) => {
    void send(action).catch((caught: unknown) => setError(errorMessage(caught)))
  }

  const cycleLifecycle = () => {
    if (cycling) return
    setCycling(true)
    setError(null)
    void (async () => {
      try {
        for (let cycle = 1; cycle <= 30; cycle += 1) {
          const closed = await send('close')
          mounted.current = false
          requireResourceCounts(closed, 0)
          setLifecycleCycles(cycle)
          if (cycle < 30) {
            const reopened = await send('mount')
            mounted.current = true
            requireResourceCounts(reopened, 1)
          }
        }
      } catch (caught: unknown) {
        setError(errorMessage(caught))
      } finally {
        setCycling(false)
      }
    })()
  }

  const opaque = 'var(--viewer-application)'
  return (
    <section
      aria-label="原生视频渲染可行性"
      data-acceptance-scene-ready={report?.firstFrameReady ? 'true' : 'false'}
      style={{ height: '100%', position: 'relative', width: '100%' }}
    >
      <div style={{ background: opaque, inset: '0 0 auto', height: slotY, position: 'absolute' }}>
        <header style={{ alignItems: 'center', display: 'flex', height: 72, padding: '0 32px' }}>
          <div>
            <strong>Viewer 原生视频渲染门禁</strong>
            <div style={{ color: 'var(--viewer-text-secondary)', fontSize: 12 }}>{fixtureId}</div>
          </div>
        </header>
      </div>
      <div
        style={{
          background: opaque,
          height: slotHeight,
          left: 0,
          position: 'absolute',
          top: slotY,
          width: slotX,
        }}
      />
      <div
        style={{
          background: opaque,
          height: slotHeight,
          left: slotX + slotWidth,
          position: 'absolute',
          right: 0,
          top: slotY,
        }}
      />
      <div
        style={{
          background: opaque,
          bottom: 0,
          left: 0,
          position: 'absolute',
          right: 0,
          top: slotY + slotHeight,
        }}
      >
        <dl
          style={{
            display: 'grid',
            fontSize: 12,
            gap: '4px 16px',
            gridTemplateColumns: 'max-content max-content max-content max-content',
            margin: '28px auto',
            width: 'fit-content',
          }}
        >
          <dt>硬件路径</dt>
          <dd style={{ margin: 0 }}>{report?.diagnostics.hwdec ?? '等待首帧'}</dd>
          <dt>视频输出</dt>
          <dd style={{ margin: 0 }}>{report?.diagnostics.videoOutput ?? '等待首帧'}</dd>
          <dt>资源</dt>
          <dd style={{ margin: 0 }}>
            {report === null
              ? '—'
              : `${report.diagnostics.activeClients}/${report.diagnostics.activeRenderContexts}/${report.diagnostics.activeSurfaces}`}
          </dd>
          <dt>已绘制</dt>
          <dd style={{ margin: 0 }}>{report?.diagnostics.renderedFrames ?? 0} 帧</dd>
        </dl>
        <p
          aria-live="polite"
          style={{
            display: 'flex',
            fontSize: 12,
            gap: 16,
            justifyContent: 'center',
            margin: '-20px auto 0',
          }}
        >
          <span aria-label={`首帧：${report?.firstFrameReady ? '就绪' : '等待'}`} role="status">
            首帧：{report?.firstFrameReady ? '就绪' : '等待'}
          </span>
          <span aria-label={`前进：${report?.forwardStep ? '完成' : '未运行'}`} role="status">
            前进：{report?.forwardStep ? '完成' : '未运行'}
          </span>
          <span aria-label={`后退：${report?.backwardStep ? '完成' : '未运行'}`} role="status">
            后退：{report?.backwardStep ? '完成' : '未运行'}
          </span>
          <span aria-label={`时间：${report?.playbackTimeUs ?? '—'} µs`} role="status">
            时间：{report?.playbackTimeUs ?? '—'} µs
          </span>
          <span aria-label={`生命周期：${lifecycleCycles}/30`} role="status">
            生命周期：{lifecycleCycles}/30
          </span>
        </p>
        {error !== null && <p role="alert">{error}</p>}
      </div>
      <div
        data-backward-step={report?.backwardStep ? 'true' : 'false'}
        data-first-frame-ready={report?.firstFrameReady ? 'true' : 'false'}
        data-forward-step={report?.forwardStep ? 'true' : 'false'}
        data-native-surface-slot="true"
        data-testid="video-feasibility-slot"
        ref={slot}
        style={{
          border: '2px solid var(--viewer-accent)',
          height: slotHeight,
          left: slotX,
          position: 'absolute',
          top: slotY,
          width: slotWidth,
        }}
      >
        <div
          aria-label="React 视频控制覆盖层"
          style={{
            alignItems: 'center',
            background: 'rgb(20 22 26 / 78%)',
            borderRadius: 10,
            bottom: 16,
            display: 'flex',
            gap: 8,
            left: '50%',
            padding: 8,
            position: 'absolute',
            transform: 'translateX(-50%)',
            zIndex: 2,
          }}
        >
          <button aria-label="后退一帧" onClick={() => run('frame_step_backward')} type="button">
            ←
          </button>
          <button aria-label="前进一帧" onClick={() => run('frame_step_forward')} type="button">
            →
          </button>
          <button
            aria-label="调整原生表面尺寸"
            onClick={() => setSlotWidth((width) => (width > 640 ? 600 : 720))}
            type="button"
          >
            Retina
          </button>
          <button
            aria-label="循环挂载与卸载 30 次"
            disabled={cycling}
            onClick={cycleLifecycle}
            type="button"
          >
            30×
          </button>
        </div>
      </div>
    </section>
  )
}

function surfaceRect(element: HTMLElement): SurfaceRect {
  const bounds = element.getBoundingClientRect()
  return {
    x: bounds.left,
    y: bounds.top,
    width: bounds.width,
    height: bounds.height,
  }
}

function fixtureFromHash(hash: string): FixtureId {
  const candidate = hash.replace(/^#/, '')
  if (candidate === 'hevc-portrait' || candidate === 'vfr-step') return candidate
  return 'h264-1080p'
}

function errorMessage(caught: unknown): string {
  if (caught instanceof Error) return caught.message
  if (typeof caught === 'object' && caught !== null && 'message' in caught) {
    return String(caught.message)
  }
  return String(caught)
}

function requireResourceCounts(report: VideoFeasibilityReport, expected: number) {
  const diagnostics = report.diagnostics
  const counts = [
    diagnostics.activeClients,
    diagnostics.activeRenderContexts,
    diagnostics.activeSurfaces,
  ]
  if (counts.some((count) => count !== expected)) {
    throw new Error(`Native resource baseline mismatch: ${counts.join('/')} (expected ${expected})`)
  }
}
