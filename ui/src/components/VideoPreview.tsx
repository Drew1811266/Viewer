import type { FocusEvent, ReactNode } from 'react'
import { useEffect, useRef, useState } from 'react'
import type { VideoFile } from '../api/types'
import ViewerButton, { ViewerIconButton } from './ui/ViewerButton'
import ViewerLocalFeedback from './ui/ViewerLocalFeedback'
import ViewerToolbar from './ui/ViewerToolbar'
import { useVideoBridge, type VideoPreviewBridge } from './videoPreview/useVideoBridge'
import {
  useReducedMotionPreference,
  useVideoControlsVisibility,
} from './videoPreview/useVideoControlsVisibility'
import { useVideoPreviewScrollLock } from './videoPreview/useVideoPreviewScrollLock'
import { useVideoShortcuts } from './videoPreview/useVideoShortcuts'
import VideoControls, { type VideoControlViewState } from './videoPreview/VideoControls'
import { formatVideoTime } from './videoPreview/VideoTimeline'

export interface VideoPreviewProps {
  file: VideoFile
  files: VideoFile[]
  bridge: VideoPreviewBridge
  onNavigate?: (file: VideoFile) => void
  onClose?: () => void
}

const IGNORE = () => undefined

const VIDEO_ERROR_MESSAGES: Readonly<Record<string, string>> = {
  unsupported: '不支持此视频的容器或编码',
  damaged: '视频已损坏或无法读取',
  unreadable: '视频已损坏或无法读取',
  missing: '视频文件已移动或删除',
  engine_initialization: '视频引擎无法启动',
  engineInitialization: '视频引擎无法启动',
  decode_fallback_failed: '硬件与软件解码均失败',
  decodeFallbackFailed: '硬件与软件解码均失败',
  render_surface: '视频显示区域无法创建',
  renderSurface: '视频显示区域无法创建',
}

export default function VideoPreview({
  file,
  files,
  bridge,
  onNavigate = IGNORE,
  onClose = IGNORE,
}: VideoPreviewProps) {
  const dialog = useRef<HTMLElement>(null)
  const [retryRequest, setRetryRequest] = useState({ entityId: file.entityId, key: 0 })
  const [seeking, setSeeking] = useState(false)
  const [adjusting, setAdjusting] = useState(false)
  const [focusedWithin, setFocusedWithin] = useState(false)
  const [posterUrl, setPosterUrl] = useState(file.videoMetadata.coverUrl)
  const reducedMotion = useReducedMotionPreference()
  const retryKey = retryRequest.entityId === file.entityId ? retryRequest.key : 0
  const { state, controlState, commands } = useVideoBridge({
    bridge,
    file,
    retryKey,
  })
  const currentIndex = files.findIndex((candidate) => candidate.entityId === file.entityId)
  const failed = state.phase === 'failed'
  const controls = useVideoControlsVisibility({
    playing: state.phase === 'playing',
    seeking,
    adjusting,
    focusedWithin,
    error: state.error,
    reducedMotion,
  })

  useEffect(() => {
    const previous = document.activeElement
    dialog.current?.focus()
    return () => {
      if (previous instanceof HTMLElement && previous.isConnected) previous.focus()
    }
  }, [])

  useEffect(() => {
    let disposed = false
    setPosterUrl(file.videoMetadata.coverUrl)
    if (file.videoMetadata.coverUrl !== null)
      return () => {
        disposed = true
      }
    void bridge
      .videoRequestCover(file.entityId)
      .then((url) => {
        if (!disposed) setPosterUrl(url)
      })
      .catch(() => undefined)
    return () => {
      disposed = true
    }
  }, [bridge, file.entityId, file.videoMetadata.coverUrl])

  function navigate(delta: number) {
    const next = files[currentIndex + delta]
    if (next !== undefined) onNavigate(next)
  }

  function chromeFocusLeft(event: FocusEvent<HTMLDivElement>) {
    if (
      !(event.relatedTarget instanceof Node) ||
      !event.currentTarget.contains(event.relatedTarget)
    ) {
      setFocusedWithin(false)
    }
  }

  useVideoShortcuts({
    active: true,
    root: dialog,
    playing: state.phase === 'playing',
    fullscreen: controlState.fullscreen,
    onTogglePlayback: commands.togglePlayback,
    onStep: commands.step,
    onToggleMuted: commands.toggleMuted,
    onToggleFullscreen: commands.toggleFullscreen,
    onExitFullscreen: () => commands.setFullscreen(false),
    onClose,
    onOwnedShortcut: controls.reveal,
  })
  useVideoPreviewScrollLock(dialog)

  const controlView: VideoControlViewState = {
    generation: state.generation,
    phase: state.phase,
    timeUs: state.phase === 'ended' && state.durationUs !== null ? state.durationUs : state.timeUs,
    durationUs: state.durationUs,
    error: state.error,
    ...controlState,
  }

  const actions: ReactNode = (
    <>
      {failed && (
        <ViewerButton
          tone="quiet"
          onClick={() =>
            setRetryRequest((current) => ({
              entityId: file.entityId,
              key: current.entityId === file.entityId ? current.key + 1 : 1,
            }))
          }
        >
          重试
        </ViewerButton>
      )}
      <ViewerButton
        tone="quiet"
        className="preview-complete-action"
        aria-label="完成"
        onClick={onClose}
      >
        完成
      </ViewerButton>
    </>
  )

  const durationLabel = formatVideoTime(file.videoMetadata.durationUs ?? state.durationUs ?? 0)
  const navigation = (
    <nav className="preview-navigation-float video-preview-top-navigation" aria-label="视频导航">
      <ViewerIconButton
        icon="chevron-left"
        label="上一个视频"
        tone="quiet"
        disabled={currentIndex <= 0}
        onClick={() => navigate(-1)}
      />
      <span>
        {currentIndex + 1} / {files.length}
      </span>
      <ViewerIconButton
        icon="chevron-right"
        label="下一个视频"
        tone="quiet"
        disabled={currentIndex < 0 || currentIndex >= files.length - 1}
        onClick={() => navigate(1)}
      />
    </nav>
  )
  return (
    <section
      ref={dialog}
      className="preview-overlay video-preview"
      data-surface-visible={state.surfaceVisible}
      data-chrome-visible={controls.visible}
      data-fullscreen={controlState.fullscreen}
      data-reduced-motion={reducedMotion}
      role="dialog"
      aria-label={`视频预览 ${file.name}`}
      tabIndex={-1}
      onPointerMove={controls.reveal}
    >
      <div
        className="video-preview-stage"
        data-testid="video-preview-stage"
        data-surface-visible={state.surfaceVisible}
      >
        <div
          onFocusCapture={() => {
            controls.reveal()
            setFocusedWithin(true)
          }}
          onBlurCapture={chromeFocusLeft}
        >
          <ViewerToolbar
            label="视频预览工具"
            className="video-preview-topbar"
            leading={
              <span className="video-preview-title">
                <strong>{file.name}</strong>
                <span>{durationLabel}</span>
              </span>
            }
            center={navigation}
            actions={actions}
          />
        </div>
        {state.phase === 'ended' && posterUrl !== null && (
          <div className="video-preview-ended-poster">
            <img src={posterUrl} alt={`${file.name} 的视频封面`} draggable={false} />
          </div>
        )}
        {!failed && !state.surfaceVisible && (
          <section className="video-preview-loading" role="status" aria-label="正在加载视频">
            <strong>正在加载视频</strong>
            <div className="video-preview-loading__progress" role="progressbar">
              <span />
            </div>
          </section>
        )}
        {failed && (
          <ViewerLocalFeedback tone="danger" title={videoErrorMessage(state.error?.code)}>
            此错误仅影响当前视频。你可以重试或完成预览。
          </ViewerLocalFeedback>
        )}
        <div
          className="video-preview-bottom-chrome"
          data-chrome-visible={controls.visible}
          onFocusCapture={() => setFocusedWithin(true)}
          onBlurCapture={chromeFocusLeft}
        >
          {state.generation > 0 && (
            <VideoControls
              view={controlView}
              commands={commands}
              visible={controls.visible}
              reducedMotion={controls.reducedMotion}
              onActivity={controls.reveal}
              onSeekingChange={setSeeking}
              onAdjustingChange={setAdjusting}
              onFocusWithinChange={setFocusedWithin}
            />
          )}
        </div>
      </div>
    </section>
  )
}

export function videoErrorMessage(code: string | undefined): string {
  return code === undefined
    ? '无法播放这个视频'
    : (VIDEO_ERROR_MESSAGES[code] ?? '无法播放这个视频')
}
