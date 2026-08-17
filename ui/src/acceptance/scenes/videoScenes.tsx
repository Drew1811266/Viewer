import { useCallback, useEffect, useMemo, useRef } from 'react'
import type { VideoEvent, VideoSession } from '../../api/types'
import SettingsDialog from '../../components/SettingsDialog'
import VideoPreview from '../../components/VideoPreview'
import type { VideoPreviewBridge } from '../../components/videoPreview/useVideoBridge'
import { defined } from '../../defined'
import type { AcceptanceSceneRegistry } from '../AcceptanceApp'
import type { AcceptanceBridgeOverrides } from '../acceptanceBridge'
import {
  ACCEPTANCE_VIDEO_FILES,
  ACCEPTANCE_VIDEO_FRAME_URL,
  acceptanceVideoCacheBridge,
} from '../acceptanceFixtures'
import AcceptanceProductScene from './sceneHarness'

export const VIDEO_ACCEPTANCE_SCENE_IDS = [
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
] as const

type VideoAcceptanceSceneId = (typeof VIDEO_ACCEPTANCE_SCENE_IDS)[number]
type PreviewAcceptanceState = Exclude<
  VideoAcceptanceSceneId,
  'workspace-video-expanded' | 'workspace-video-unavailable' | 'settings-video-cache'
>

const noOp = () => undefined

export const VIDEO_SCENES: AcceptanceSceneRegistry = {
  'workspace-video-expanded': () => <VideoWorkspaceScene unavailable={false} />,
  'workspace-video-unavailable': () => <VideoWorkspaceScene unavailable />,
  'video-preparing': () => <VideoPreviewScene state="video-preparing" />,
  'video-playing-controls': () => <VideoPreviewScene state="video-playing-controls" />,
  'video-paused-controls': () => <VideoPreviewScene state="video-paused-controls" />,
  'video-timeline-pending': () => <VideoPreviewScene state="video-timeline-pending" />,
  'video-timeline-ready': () => <VideoPreviewScene state="video-timeline-ready" />,
  'video-ended': () => <VideoPreviewScene state="video-ended" />,
  'video-failed-retry': () => <VideoPreviewScene state="video-failed-retry" />,
  'video-fullscreen-controls': () => <VideoPreviewScene state="video-fullscreen-controls" />,
  'settings-video-cache': () => <VideoCacheSettingsScene />,
  'video-reduced-motion': () => <VideoPreviewScene state="video-reduced-motion" />,
}

function VideoWorkspaceScene({ unavailable }: { unavailable: boolean }) {
  const bridgeOverrides = useMemo<AcceptanceBridgeOverrides>(() => {
    const videos = unavailable
      ? [defined(ACCEPTANCE_VIDEO_FILES[2], 'Missing unavailable video fixture')]
      : ACCEPTANCE_VIDEO_FILES.slice(0, 2)
    return {
      async queryFolder() {
        return { workspace: 'content' as const, images: [], videos, otherFiles: [] }
      },
    }
  }, [unavailable])
  const ready = useCallback(
    () =>
      document.querySelector('[aria-label="视频文件"]') !== null &&
      (!unavailable || document.querySelector('.video-card-unavailable') !== null),
    [unavailable],
  )
  return (
    <AcceptanceProductScene
      ready={ready}
      bridgeOverrides={bridgeOverrides}
      attributes={{ 'data-video-bridge': 'fake' }}
    >
      {null}
    </AcceptanceProductScene>
  )
}

function VideoCacheSettingsScene() {
  const bridge = useMemo(acceptanceVideoCacheBridge, [])
  const ready = useCallback(
    () => document.body.textContent?.includes('256 MiB / 1 GiB') === true,
    [],
  )
  return (
    <AcceptanceProductScene ready={ready} attributes={{ 'data-video-bridge': 'fake' }}>
      <SettingsDialog
        bridge={bridge}
        density="standard"
        magnifier={{ shape: 'circle', magnification: 2, area: 'small' }}
        error={null}
        onDensityChange={noOp}
        onMagnifierShapeChange={noOp}
        onMagnifierMagnificationChange={noOp}
        onMagnifierAreaChange={noOp}
        onClose={noOp}
      />
    </AcceptanceProductScene>
  )
}

function VideoPreviewScene({ state }: { state: PreviewAcceptanceState }) {
  const bridge = useMemo(() => fakeVideoBridge(state), [state])
  const hovered = useRef(false)
  useEffect(() => {
    if (state !== 'video-timeline-pending' && state !== 'video-timeline-ready') return
    const hover = () => {
      if (hovered.current) return
      const timeline = document.querySelector<HTMLElement>('[aria-label="视频时间轴"]')
      if (timeline === null) return
      hovered.current = true
      const rect = timeline.getBoundingClientRect()
      timeline.dispatchEvent(
        new MouseEvent('pointermove', {
          bubbles: true,
          clientX: rect.left + Math.max(rect.width * 0.58, 1),
          clientY: rect.top + 4,
        }),
      )
    }
    const observer = new MutationObserver(hover)
    observer.observe(document.body, { attributes: true, childList: true, subtree: true })
    hover()
    return () => observer.disconnect()
  }, [state])
  const ready = useCallback(() => previewSceneReady(state), [state])
  const file = defined(ACCEPTANCE_VIDEO_FILES[0], 'Missing playable video fixture')
  return (
    <AcceptanceProductScene
      ready={ready}
      attributes={{
        'data-video-bridge': 'fake',
        'data-video-acceptance-state': state,
        ...(state === 'video-reduced-motion' ? { 'data-acceptance-motion': 'reduced' } : {}),
      }}
    >
      {state !== 'video-preparing' && state !== 'video-failed-retry' && (
        <img
          className="acceptance-video-native-frame"
          src={ACCEPTANCE_VIDEO_FRAME_URL}
          alt="视频静态验收帧"
        />
      )}
      <VideoPreview file={file} files={ACCEPTANCE_VIDEO_FILES.slice(0, 2)} bridge={bridge} />
    </AcceptanceProductScene>
  )
}

function fakeVideoBridge(state: PreviewAcceptanceState): VideoPreviewBridge {
  let listener: ((event: VideoEvent) => void) | null = null
  let nextGeneration = 0
  return {
    async listenVideo(handler) {
      listener = handler
      return () => {
        if (listener === handler) listener = null
      }
    },
    async videoOpen() {
      const session = videoSession(++nextGeneration)
      queueMicrotask(() => publishSceneEvents(listener, session.generation, state))
      return session
    },
    async videoCancelOpen() {
      return true
    },
    async videoClose() {
      return undefined
    },
    async videoPlay() {
      return undefined
    },
    async videoPause() {
      return undefined
    },
    async videoSeek() {
      return undefined
    },
    async videoStep() {
      return undefined
    },
    async videoSetVolume() {
      return undefined
    },
    async videoSetMuted() {
      return undefined
    },
    async videoSetRate() {
      return undefined
    },
    async videoSetFullscreen() {
      return undefined
    },
    async videoRequestCover() {
      return ACCEPTANCE_VIDEO_FRAME_URL
    },
    async videoRequestThumbnail(request) {
      if (state !== 'video-timeline-ready') return
      window.setTimeout(() => {
        listener?.({
          type: 'timelineThumbnailReady',
          generation: request.generation,
          requestId: request.requestId,
          bucketUs: request.timeUs,
          artifactUrl: ACCEPTANCE_VIDEO_FRAME_URL,
        })
      }, 0)
    },
  }
}

function publishSceneEvents(
  listener: ((event: VideoEvent) => void) | null,
  generation: number,
  state: PreviewAcceptanceState,
) {
  if (listener === null || state === 'video-preparing') return
  if (state === 'video-failed-retry') {
    listener({
      type: 'failed',
      generation,
      error: { code: 'damaged', retryable: true },
    })
    return
  }
  listener({ type: 'firstFrameReady', generation })
  listener({
    type: 'progress',
    generation,
    timeUs: state === 'video-ended' ? 92_400_000 : 38_200_000,
    durationUs: 92_400_000,
  })
  if (state === 'video-ended') listener({ type: 'ended', generation })
  else {
    listener({
      type: 'stateChanged',
      generation,
      state: state === 'video-playing-controls' ? 'playing' : 'paused',
    })
  }
  if (state === 'video-fullscreen-controls') {
    listener({ type: 'fullscreenChanged', generation, fullscreen: true })
  }
}

function videoSession(generation: number): VideoSession {
  return {
    generation,
    sessionId: `acceptance-video-${generation}`,
    media: {
      durationUs: 92_400_000,
      displayWidth: 1_920,
      displayHeight: 1_080,
      rotationDegrees: 0,
    },
  }
}

function previewSceneReady(state: PreviewAcceptanceState): boolean {
  if (state === 'video-preparing') {
    return document.querySelector('[aria-label="正在加载视频"]') !== null
  }
  if (state === 'video-failed-retry') {
    return document.body.textContent?.includes('视频已损坏或无法读取') === true
  }
  if (state === 'video-timeline-pending') {
    return document.querySelector('[data-testid="timeline-thumbnail-pending"]') !== null
  }
  if (state === 'video-timeline-ready') {
    return document.querySelector('.video-timeline__thumbnail img') !== null
  }
  const preview = document.querySelector('.video-preview')
  if (preview?.getAttribute('data-surface-visible') !== 'true') return false
  if (state === 'video-playing-controls') {
    return document.querySelector('[aria-label="暂停"]') !== null
  }
  if (state === 'video-ended') {
    return document.querySelector('[aria-label="视频时间轴"][aria-valuenow="92.4"]') !== null
  }
  if (state === 'video-fullscreen-controls') {
    return preview.getAttribute('data-fullscreen') === 'true'
  }
  return document.querySelector('[aria-label="播放"]') !== null
}
