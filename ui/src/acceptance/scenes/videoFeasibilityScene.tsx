import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react'
import type { AcceptanceSceneRegistry } from '../AcceptanceApp'
import type { AcceptanceRequest } from '../acceptanceRequest'

type FixtureId = 'h264-1080p' | 'hevc-portrait' | 'vfr-step' | 'h264-1080p60' | 'hevc-4k30'
type FeasibilityAction =
  | 'mount'
  | 'update_geometry'
  | 'frame_step_forward'
  | 'frame_step_backward'
  | 'play'
  | 'pause'
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
  mistimedFrames: number
  decoderDroppedFrames: number
}

interface ResourceCounts {
  clients: number
  renderContexts: number
  surfaces: number
}

interface VideoFeasibilityReport {
  fixtureId: string
  rect: SurfaceRect
  firstFrameReady: boolean
  decodedPictureType: string | null
  forwardStep: boolean
  backwardStep: boolean
  playbackTimeUs: number | null
  commandSerial: number
  lastCommandLatencyUs: number
  generationDiagnostics: {
    generation: number
    mountReturned: boolean
    updateCallbacks: number
    drawEntries: number
    frameUpdates: number
    pictureFrames: number
    reveals: number
    eventEmits: number
  }
  diagnostics: VideoRenderDiagnostics
}

export const VIDEO_FEASIBILITY_SCENES: AcceptanceSceneRegistry = {
  'VIDEO-FEASIBILITY': VideoFeasibilityScene,
}

export function VideoFeasibilityScene({ request }: { request: AcceptanceRequest }) {
  const [fixtureId, setFixtureId] = useState<FixtureId>(() => fixtureFromHash(window.location.hash))
  const slot = useRef<HTMLDivElement>(null)
  const mounted = useRef(false)
  const rect = useRef<SurfaceRect | null>(null)
  const activeReportGeneration = useRef(0)
  const nativeGeneration = useRef(0)
  const minimumAcceptedNativeGeneration = useRef(0)
  const [slotWidth, setSlotWidth] = useState(Math.min(720, request.width - 240))
  const [report, setReport] = useState<VideoFeasibilityReport | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [lifecycleCycles, setLifecycleCycles] = useState(0)
  const [lifecycleBaseline, setLifecycleBaseline] = useState<ResourceCounts | null>(null)
  const [lifecycleAfter, setLifecycleAfter] = useState<ResourceCounts | null>(null)
  const [lifecycleSequence, setLifecycleSequence] = useState<FixtureId[]>([])
  const [cycling, setCycling] = useState(false)
  const [remounting, setRemounting] = useState(false)
  const slotHeight = Math.round((slotWidth * 9) / 16)
  const slotX = Math.round((request.width - slotWidth) / 2)
  const slotY = 120

  const publishReport = useCallback((next: VideoFeasibilityReport) => {
    if (next.generationDiagnostics.generation < minimumAcceptedNativeGeneration.current) return
    setReport((previous) => {
      const merged = mergeFirstFrameReport(previous, next)
      nativeGeneration.current = merged.generationDiagnostics.generation
      return merged
    })
  }, [])

  const send = useCallback(
    async (
      action: FeasibilityAction,
      currentRect = rect.current,
      publish = true,
      reportGeneration = activeReportGeneration.current,
      expectedGeneration?: number,
    ) => {
      if (currentRect === null) throw new Error('Native video surface rectangle is unavailable')
      const next = await invoke<VideoFeasibilityReport>('run_video_feasibility', {
        request: {
          fixtureId,
          action,
          rect: currentRect,
          ...(expectedGeneration === undefined ? {} : { expectedGeneration }),
        },
      })
      if (publish && reportGeneration === activeReportGeneration.current) {
        publishReport(next)
      }
      return next
    },
    [fixtureId, publishReport],
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
    const reportGeneration = activeReportGeneration.current + 1
    activeReportGeneration.current = reportGeneration
    let disposed = false
    let unlisten: (() => void) | undefined
    void (async () => {
      try {
        const release = await listen<VideoFeasibilityReport>(
          'viewer://video-feasibility-frame',
          ({ payload }) => {
            if (
              !disposed &&
              reportGeneration === activeReportGeneration.current &&
              payload.fixtureId === fixtureId
            ) {
              publishReport(payload)
            }
          },
        )
        if (disposed) {
          release()
          return
        }
        unlisten = release
        const mountedReport = await send('mount', finalRect, false, reportGeneration)
        if (disposed) {
          void send(
            'close',
            finalRect,
            false,
            reportGeneration,
            mountedReport.generationDiagnostics.generation,
          ).catch(() => undefined)
        } else {
          mounted.current = true
          publishReport(mountedReport)
        }
      } catch (caught: unknown) {
        if (!disposed) setError(errorMessage(caught))
      }
    })()
    return () => {
      disposed = true
      if (activeReportGeneration.current === reportGeneration) {
        activeReportGeneration.current += 1
      }
      unlisten?.()
      if (mounted.current) {
        void send('close', rect.current, false, reportGeneration, nativeGeneration.current).catch(
          () => undefined,
        )
      }
      mounted.current = false
    }
  }, [fixtureId, publishReport, send])

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

  const switchFixture = (next: FixtureId) => {
    if (next === fixtureId || cycling || remounting) return
    void (async () => {
      try {
        if (mounted.current) {
          await send(
            'close',
            rect.current,
            false,
            activeReportGeneration.current,
            nativeGeneration.current,
          )
          mounted.current = false
        }
        window.location.hash = next
        setReport(null)
        setFixtureId(next)
      } catch (caught: unknown) {
        setError(errorMessage(caught))
      }
    })()
  }

  const remountCurrentFixture = () => {
    if (cycling || remounting) return
    const reportGeneration = activeReportGeneration.current
    const closingGeneration = nativeGeneration.current
    minimumAcceptedNativeGeneration.current = closingGeneration + 1
    setRemounting(true)
    setError(null)
    void (async () => {
      try {
        if (mounted.current) {
          await send('close', rect.current, false, reportGeneration, closingGeneration)
          mounted.current = false
        }
        if (reportGeneration !== activeReportGeneration.current) return
        setReport(null)
        const next = await send('mount', rect.current, false, reportGeneration)
        if (reportGeneration !== activeReportGeneration.current) {
          await send(
            'close',
            rect.current,
            false,
            reportGeneration,
            next.generationDiagnostics.generation,
          )
          return
        }
        mounted.current = true
        publishReport(next)
      } catch (caught: unknown) {
        if (reportGeneration === activeReportGeneration.current) setError(errorMessage(caught))
      } finally {
        if (reportGeneration === activeReportGeneration.current) setRemounting(false)
      }
    })()
  }

  const cycleLifecycle = () => {
    if (cycling || remounting) return
    setCycling(true)
    setError(null)
    void (async () => {
      try {
        const initiallyClosed = await send('close')
        mounted.current = false
        const baseline = resourceCounts(initiallyClosed)
        setLifecycleBaseline(baseline)
        setLifecycleAfter(null)
        setLifecycleSequence([])
        for (let cycle = 1; cycle <= 30; cycle += 1) {
          const lifecycleFixture: FixtureId = cycle % 2 === 1 ? 'h264-1080p' : 'hevc-portrait'
          const opened = await sendForFixture(lifecycleFixture, 'mount')
          mounted.current = true
          requireResourceCounts(opened, incremented(baseline))
          const closed = await sendForFixture(lifecycleFixture, 'close')
          mounted.current = false
          requireResourceCounts(closed, baseline)
          setLifecycleCycles(cycle)
          setLifecycleAfter(resourceCounts(closed))
          setLifecycleSequence((sequence) => [...sequence, lifecycleFixture])
        }
      } catch (caught: unknown) {
        setError(errorMessage(caught))
      } finally {
        setCycling(false)
      }
    })()
  }

  const sendForFixture = async (targetFixture: FixtureId, action: FeasibilityAction) => {
    if (rect.current === null) throw new Error('Native video surface rectangle is unavailable')
    return invoke<VideoFeasibilityReport>('run_video_feasibility', {
      request: { fixtureId: targetFixture, action, rect: rect.current },
    })
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
          <nav
            aria-label="原生视频验收样本"
            style={{ display: 'flex', gap: 4, marginLeft: 'auto' }}
          >
            {(
              ['h264-1080p', 'hevc-portrait', 'vfr-step', 'h264-1080p60', 'hevc-4k30'] as const
            ).map((candidate) => (
              <button
                aria-label={`切换样本 ${candidate}`}
                disabled={candidate === fixtureId || cycling || remounting}
                key={candidate}
                onClick={() => switchFixture(candidate)}
                type="button"
              >
                {candidate}
              </button>
            ))}
          </nav>
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
          <dd
            aria-label={`已绘制：${report?.diagnostics.renderedFrames ?? 0}`}
            role="status"
            style={{ margin: 0 }}
          >
            {report?.diagnostics.renderedFrames ?? 0} 帧
          </dd>
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
          <span aria-label={`样本：${fixtureId}`} role="status">
            样本：{fixtureId}
          </span>
          <span aria-label={`解码帧：${report?.decodedPictureType ?? '等待'}`} role="status">
            解码帧：{report?.decodedPictureType ?? '等待'}
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
          <span
            aria-label={`生命周期基线：${formatResourceCounts(lifecycleBaseline)}`}
            role="status"
          >
            生命周期基线：{formatResourceCounts(lifecycleBaseline)}
          </span>
          <span aria-label={`生命周期结束：${formatResourceCounts(lifecycleAfter)}`} role="status">
            生命周期结束：{formatResourceCounts(lifecycleAfter)}
          </span>
          <span aria-label={`生命周期路径：${lifecycleSequence.join(',')}`} role="status">
            生命周期路径：{lifecycleSequence.join(',')}
          </span>
          <span aria-label={`命令：${report?.commandSerial ?? 0}`} role="status">
            命令：{report?.commandSerial ?? 0}
          </span>
          <span aria-label={`引擎延迟：${report?.lastCommandLatencyUs ?? 0}`} role="status">
            引擎延迟：{report?.lastCommandLatencyUs ?? 0}
          </span>
          <span aria-label={`误时帧：${report?.diagnostics.mistimedFrames ?? 0}`} role="status">
            误时帧：{report?.diagnostics.mistimedFrames ?? 0}
          </span>
          <span
            aria-label={`解码丢帧：${report?.diagnostics.decoderDroppedFrames ?? 0}`}
            role="status"
          >
            解码丢帧：{report?.diagnostics.decoderDroppedFrames ?? 0}
          </span>
          <span aria-label={generationDiagnosticLabel(report)} role="status">
            {generationDiagnosticLabel(report)}
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
          <button aria-label="播放性能样本" onClick={() => run('play')} type="button">
            ▶
          </button>
          <button aria-label="暂停性能样本" onClick={() => run('pause')} type="button">
            ‖
          </button>
          <button
            aria-label="重新挂载当前样本"
            disabled={cycling || remounting}
            onClick={remountCurrentFixture}
            type="button"
          >
            重挂
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
            disabled={cycling || remounting}
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
  if (
    candidate === 'hevc-portrait' ||
    candidate === 'vfr-step' ||
    candidate === 'h264-1080p60' ||
    candidate === 'hevc-4k30'
  )
    return candidate
  return 'h264-1080p'
}

function errorMessage(caught: unknown): string {
  if (caught instanceof Error) return caught.message
  if (typeof caught === 'object' && caught !== null && 'message' in caught) {
    return String(caught.message)
  }
  return String(caught)
}

function requireResourceCounts(report: VideoFeasibilityReport, expected: ResourceCounts) {
  const counts = resourceCounts(report)
  if (
    counts.clients !== expected.clients ||
    counts.renderContexts !== expected.renderContexts ||
    counts.surfaces !== expected.surfaces
  ) {
    throw new Error(
      `Native resource baseline mismatch: ${formatResourceCounts(counts)} (expected ${formatResourceCounts(expected)})`,
    )
  }
}

function resourceCounts(report: VideoFeasibilityReport): ResourceCounts {
  return {
    clients: report.diagnostics.activeClients,
    renderContexts: report.diagnostics.activeRenderContexts,
    surfaces: report.diagnostics.activeSurfaces,
  }
}

function incremented(counts: ResourceCounts): ResourceCounts {
  return {
    clients: counts.clients + 1,
    renderContexts: counts.renderContexts + 1,
    surfaces: counts.surfaces + 1,
  }
}

function formatResourceCounts(counts: ResourceCounts | null): string {
  return counts === null ? '—' : `${counts.clients}/${counts.renderContexts}/${counts.surfaces}`
}

function mergeFirstFrameReport(
  previous: VideoFeasibilityReport | null,
  next: VideoFeasibilityReport,
): VideoFeasibilityReport {
  if (
    previous !== null &&
    previous.fixtureId === next.fixtureId &&
    previous.generationDiagnostics.generation > next.generationDiagnostics.generation
  ) {
    return previous
  }
  if (
    previous?.fixtureId === next.fixtureId &&
    previous.generationDiagnostics.generation === next.generationDiagnostics.generation &&
    previous.firstFrameReady &&
    !next.firstFrameReady
  ) {
    return { ...next, firstFrameReady: true }
  }
  return next
}

function generationDiagnosticLabel(report: VideoFeasibilityReport | null): string {
  const diagnostic = report?.generationDiagnostics
  return [
    `诊断代：${diagnostic?.generation ?? 0}`,
    `挂载：${diagnostic?.mountReturned ? 1 : 0}`,
    `回调：${diagnostic?.updateCallbacks ?? 0}`,
    `绘制入口：${diagnostic?.drawEntries ?? 0}`,
    `FRAME：${diagnostic?.frameUpdates ?? 0}`,
    `图像：${diagnostic?.pictureFrames ?? 0}`,
    `显示：${diagnostic?.reveals ?? 0}`,
    `事件：${diagnostic?.eventEmits ?? 0}`,
  ].join(' ')
}
