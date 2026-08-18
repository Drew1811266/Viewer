import { type FocusEvent, useEffect, useState } from 'react'
import type { VideoError, VideoEvent } from '../../api/types'
import { ViewerIconButton } from '../ui/ViewerButton'
import { useVideoResponsiveLayout } from './useVideoResponsiveLayout'
import VideoControlsMoreMenu from './VideoControlsMoreMenu'
import VideoRateMenu from './VideoRateMenu'
import VideoTimeline from './VideoTimeline'

export const VIDEO_RATES = [0.5, 0.75, 1, 1.25, 1.5, 2] as const
export type VideoPlaybackRate = (typeof VIDEO_RATES)[number]

export interface VideoControlViewState {
  generation: number
  phase: 'preparing' | 'playing' | 'paused' | 'seeking' | 'ended' | 'failed' | 'closing'
  timeUs: number
  durationUs: number | null
  volumePercent: number
  muted: boolean
  rate: VideoPlaybackRate
  fullscreen: boolean
  error: VideoError | null
  timelineThumbnail: Extract<VideoEvent, { type: 'timelineThumbnailReady' }> | null
}

export interface VideoControlCommands {
  play(): Promise<void>
  pause(): Promise<void>
  togglePlayback(): Promise<void>
  step(direction: 'backward' | 'forward'): Promise<void>
  previewSeek(timeUs: number): Promise<void>
  seek(timeUs: number): Promise<void>
  setVolume(volumePercent: number): Promise<void>
  setMuted(muted: boolean): Promise<void>
  toggleMuted(): Promise<void>
  setRate(rate: VideoPlaybackRate): Promise<void>
  setFullscreen(fullscreen: boolean): Promise<void>
  toggleFullscreen(): Promise<void>
  requestThumbnail(request: { requestId: string; timeUs: number }): Promise<void>
}

export interface VideoControlsProps {
  view: VideoControlViewState
  commands: VideoControlCommands
  visible: boolean
  reducedMotion: boolean
  onActivity(): void
  onSeekingChange(seeking: boolean): void
  onAdjustingChange(adjusting: boolean): void
  onFocusWithinChange?(focused: boolean): void
}

export default function VideoControls({
  view,
  commands,
  visible,
  reducedMotion,
  onActivity,
  onSeekingChange,
  onAdjustingChange,
  onFocusWithinChange = () => undefined,
}: VideoControlsProps) {
  const { compact } = useVideoResponsiveLayout()
  const [moreOpen, setMoreOpen] = useState(false)
  const playing = view.phase === 'playing'
  const endedTimeUs =
    view.phase === 'ended' && view.durationUs !== null ? view.durationUs : view.timeUs
  const disabled =
    view.generation <= 0 ||
    view.phase === 'preparing' ||
    view.phase === 'failed' ||
    view.phase === 'closing'

  useEffect(() => {
    if (!compact) setMoreOpen(false)
  }, [compact])

  function focusLeft(event: FocusEvent<HTMLDivElement>) {
    if (
      !(event.relatedTarget instanceof Node) ||
      !event.currentTarget.contains(event.relatedTarget)
    ) {
      onFocusWithinChange(false)
    }
  }

  return (
    <div
      className="video-controls"
      role="group"
      aria-label="视频播放控制"
      data-visible={String(visible)}
      data-reduced-motion={String(reducedMotion)}
      data-layout={compact ? 'compact' : 'wide'}
      onPointerMove={onActivity}
      onFocusCapture={() => onFocusWithinChange(true)}
      onBlurCapture={focusLeft}
    >
      <div className="video-controls__transport">
        {!compact && (
          <ViewerIconButton
            icon="skip-back"
            label="上一帧"
            title="上一帧（左方向键）"
            aria-keyshortcuts="ArrowLeft"
            disabled={disabled}
            onClick={() => runCommand(commands.step('backward'))}
          />
        )}
        <ViewerIconButton
          icon={playing ? 'pause' : 'play'}
          className="video-controls__play"
          label={playing ? '暂停' : '播放'}
          title={playing ? '暂停（空格）' : '播放（空格）'}
          aria-keyshortcuts="Space"
          disabled={disabled}
          active={playing}
          onClick={() => runCommand(commands.togglePlayback())}
        />
        {!compact && (
          <ViewerIconButton
            icon="skip-forward"
            label="下一帧"
            title="下一帧（右方向键）"
            aria-keyshortcuts="ArrowRight"
            disabled={disabled}
            onClick={() => runCommand(commands.step('forward'))}
          />
        )}
      </div>
      <div className="video-controls__timeline">
        <VideoTimeline
          generation={view.generation}
          durationUs={view.durationUs}
          timeUs={endedTimeUs}
          thumbnail={view.timelineThumbnail}
          onPreviewSeek={commands.previewSeek}
          onCommitSeek={commands.seek}
          onRequestThumbnail={commands.requestThumbnail}
          onSeekingChange={onSeekingChange}
          onActivity={onActivity}
        />
      </div>
      <div className="video-controls__settings">
        <ViewerIconButton
          icon={view.muted ? 'volume-x' : 'volume-2'}
          label={view.muted ? '取消静音' : '静音'}
          title={view.muted ? '取消静音（M）' : '静音（M）'}
          aria-keyshortcuts="M"
          disabled={disabled}
          active={view.muted}
          onClick={() => runCommand(commands.toggleMuted())}
        />
        {!compact && (
          <>
            <label className="video-controls__volume">
              <span>音量</span>
              <input
                type="range"
                aria-label="音量"
                title="音量（0 到 100）"
                min={0}
                max={100}
                step={1}
                value={view.volumePercent}
                disabled={disabled}
                onPointerDown={() => onAdjustingChange(true)}
                onPointerUp={() => onAdjustingChange(false)}
                onPointerCancel={() => onAdjustingChange(false)}
                onChange={(event) => {
                  onActivity()
                  runCommand(commands.setVolume(Number(event.currentTarget.value)))
                }}
              />
            </label>
            <div className="video-controls__rate">
              <span>速度</span>
              <VideoRateMenu
                rate={view.rate}
                rates={VIDEO_RATES}
                disabled={disabled}
                onActivity={onActivity}
                onOpenChange={onAdjustingChange}
                onSelect={(rate) => {
                  runCommand(commands.setRate(rate))
                }}
              />
            </div>
          </>
        )}
        {compact && (
          <VideoControlsMoreMenu
            open={moreOpen}
            onOpenChange={setMoreOpen}
            disabled={disabled}
            view={view}
            commands={commands}
            rates={VIDEO_RATES}
            onActivity={onActivity}
            onAdjustingChange={onAdjustingChange}
          />
        )}
        <ViewerIconButton
          icon={view.fullscreen ? 'minimize' : 'maximize'}
          label={view.fullscreen ? '退出全屏' : '进入全屏'}
          title={view.fullscreen ? '退出全屏（F 或 Escape）' : '进入全屏（F）'}
          aria-keyshortcuts={view.fullscreen ? 'F Escape' : 'F'}
          disabled={disabled}
          onClick={() => runCommand(commands.toggleFullscreen())}
        />
      </div>
    </div>
  )
}

function runCommand(command: Promise<void>): void {
  void command.catch(() => undefined)
}
