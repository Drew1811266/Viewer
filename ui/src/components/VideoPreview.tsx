import type { ReactNode } from 'react'
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
import { useVideoShortcuts } from './videoPreview/useVideoShortcuts'
import VideoControls, { type VideoControlViewState } from './videoPreview/VideoControls'

export interface VideoPreviewProps {
  file: VideoFile
  files: VideoFile[]
  bridge: VideoPreviewBridge
  onNavigate?: (file: VideoFile) => void
  onClose?: () => void
}

const IGNORE = () => undefined

export default function VideoPreview({
  file,
  files,
  bridge,
  onNavigate = IGNORE,
  onClose = IGNORE,
}: VideoPreviewProps) {
  const dialog = useRef<HTMLElement>(null)
  const stage = useRef<HTMLDivElement>(null)
  const [retryKey, setRetryKey] = useState(0)
  const [seeking, setSeeking] = useState(false)
  const [adjusting, setAdjusting] = useState(false)
  const [focusedWithin, setFocusedWithin] = useState(false)
  const reducedMotion = useReducedMotionPreference()
  const { state, controlState, commands } = useVideoBridge({ bridge, file, retryKey, stage })
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

  function navigate(delta: number) {
    const next = files[currentIndex + delta]
    if (next !== undefined) onNavigate(next)
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
        <ViewerButton tone="quiet" onClick={() => setRetryKey((current) => current + 1)}>
          重试
        </ViewerButton>
      )}
      <ViewerButton
        tone="quiet"
        className="preview-complete-action"
        aria-label="关闭预览"
        onClick={onClose}
      >
        完成
      </ViewerButton>
    </>
  )

  return (
    <section
      ref={dialog}
      className="preview-overlay video-preview"
      data-surface-visible={state.surfaceVisible}
      data-fullscreen={controlState.fullscreen}
      data-reduced-motion={reducedMotion}
      role="dialog"
      aria-label={`视频预览 ${file.name}`}
      tabIndex={-1}
      onPointerMove={controls.reveal}
    >
      <ViewerToolbar
        label="视频预览工具"
        leading={<strong>{file.name}</strong>}
        actions={actions}
      />
      <div
        ref={stage}
        className="video-preview-stage"
        data-testid="video-preview-stage"
        data-surface-visible={state.surfaceVisible}
      >
        {!failed && !state.surfaceVisible && (
          <section className="video-preview-loading" role="status" aria-label="正在加载视频">
            <strong>正在加载视频</strong>
            <div className="video-preview-loading__progress" role="progressbar">
              <span />
            </div>
          </section>
        )}
        {failed && (
          <ViewerLocalFeedback tone="danger" title="无法播放这个视频">
            请重试；如果问题持续出现，请确认视频文件仍可访问。
          </ViewerLocalFeedback>
        )}
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
      <nav className="preview-navigation-float" aria-label="视频导航">
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
    </section>
  )
}
