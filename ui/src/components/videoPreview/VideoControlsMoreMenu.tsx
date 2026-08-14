import { useEffect, useRef } from 'react'
import { ViewerIconButton } from '../ui/ViewerButton'
import ViewerPopover from '../ui/ViewerPopover'
import type {
  VideoControlCommands,
  VideoControlViewState,
  VideoPlaybackRate,
} from './VideoControls'

export interface VideoControlsMoreMenuProps {
  open: boolean
  onOpenChange(open: boolean): void
  disabled: boolean
  view: Pick<VideoControlViewState, 'volumePercent' | 'rate'>
  commands: Pick<VideoControlCommands, 'step' | 'setVolume' | 'setRate'>
  rates: readonly VideoPlaybackRate[]
  onActivity(): void
  onAdjustingChange(adjusting: boolean): void
}

export default function VideoControlsMoreMenu({
  open,
  onOpenChange,
  disabled,
  view,
  commands,
  rates,
  onActivity,
  onAdjustingChange,
}: VideoControlsMoreMenuProps) {
  const triggerRef = useRef<HTMLButtonElement>(null)

  useEffect(() => {
    onAdjustingChange(open)
    return () => {
      if (open) onAdjustingChange(false)
    }
  }, [onAdjustingChange, open])

  function setOpen(next: boolean) {
    onActivity()
    onOpenChange(next)
  }

  return (
    <div className="video-controls-more">
      <ViewerIconButton
        ref={triggerRef}
        icon="sliders-horizontal"
        label="更多播放控制"
        aria-expanded={open}
        disabled={disabled}
        active={open}
        onClick={() => setOpen(!open)}
      />
      <ViewerPopover
        className="video-controls-more__popover"
        open={open}
        label="更多播放控制"
        triggerRef={triggerRef}
        onOpenChange={setOpen}
      >
        <div className="video-controls-more__steps" role="group" aria-label="逐帧控制">
          <ViewerIconButton
            icon="skip-back"
            label="上一帧"
            title="上一帧（左方向键）"
            aria-keyshortcuts="ArrowLeft"
            disabled={disabled}
            onClick={() => runCommand(commands.step('backward'))}
          />
          <ViewerIconButton
            icon="skip-forward"
            label="下一帧"
            title="下一帧（右方向键）"
            aria-keyshortcuts="ArrowRight"
            disabled={disabled}
            onClick={() => runCommand(commands.step('forward'))}
          />
        </div>
        <label className="video-controls-more__field">
          <span>音量</span>
          <input
            type="range"
            aria-label="音量"
            min={0}
            max={100}
            step={1}
            value={view.volumePercent}
            disabled={disabled}
            onChange={(event) => {
              onActivity()
              runCommand(commands.setVolume(Number(event.currentTarget.value)))
            }}
          />
        </label>
        <label className="video-controls-more__field">
          <span>播放速度</span>
          <select
            aria-label="播放速度"
            value={view.rate}
            disabled={disabled}
            onChange={(event) => {
              const rate = Number(event.currentTarget.value) as VideoPlaybackRate
              if (!rates.includes(rate)) return
              onActivity()
              runCommand(commands.setRate(rate))
            }}
          >
            {rates.map((rate) => (
              <option key={rate} value={rate}>
                {rate}×
              </option>
            ))}
          </select>
        </label>
      </ViewerPopover>
    </div>
  )
}

function runCommand(command: Promise<void>): void {
  void command.catch(() => undefined)
}
