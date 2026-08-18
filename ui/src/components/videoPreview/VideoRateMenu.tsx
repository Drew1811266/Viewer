import { useEffect, useId, useRef, useState } from 'react'
import ViewerIcon from '../ui/ViewerIcon'
import ViewerPopover from '../ui/ViewerPopover'
import type { VideoPlaybackRate } from './VideoControls'

export interface VideoRateMenuProps {
  rate: VideoPlaybackRate
  rates: readonly VideoPlaybackRate[]
  disabled: boolean
  onSelect(rate: VideoPlaybackRate): void
  onActivity?(): void
  onOpenChange?(open: boolean): void
  className?: string
}

export default function VideoRateMenu({
  rate,
  rates,
  disabled,
  onSelect,
  onActivity = () => undefined,
  onOpenChange,
  className,
}: VideoRateMenuProps) {
  const [open, setOpenState] = useState(false)
  const triggerRef = useRef<HTMLButtonElement>(null)
  const optionRefs = useRef<Array<HTMLButtonElement | null>>([])
  const pendingFocusIndex = useRef<number | null>(null)
  const listboxId = useId()
  const selectedIndex = Math.max(0, rates.indexOf(rate))

  useEffect(() => {
    if (!open || pendingFocusIndex.current === null) return
    optionRefs.current[pendingFocusIndex.current]?.focus()
    pendingFocusIndex.current = null
  }, [open])

  function setOpen(next: boolean) {
    if (disabled && next) return
    onActivity()
    setOpenState(next)
    onOpenChange?.(next)
  }

  function openAndFocus(index: number) {
    pendingFocusIndex.current = clamp(index, 0, rates.length - 1)
    setOpen(true)
  }

  function select(nextRate: VideoPlaybackRate) {
    onActivity()
    onSelect(nextRate)
    setOpenState(false)
    onOpenChange?.(false)
    triggerRef.current?.focus()
  }

  return (
    <div className={className === undefined ? 'video-rate-menu' : `video-rate-menu ${className}`}>
      <button
        ref={triggerRef}
        className="video-rate-menu__trigger"
        type="button"
        aria-label={`播放速度，当前 ${formatRate(rate)}`}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={listboxId}
        disabled={disabled}
        onClick={() => setOpen(!open)}
        onKeyDown={(event) => {
          if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return
          event.preventDefault()
          openAndFocus(selectedIndex + (event.key === 'ArrowDown' ? 1 : -1))
        }}
      >
        <span>{formatRate(rate)}</span>
        <ViewerIcon name="chevron-down" size={15} />
      </button>
      <ViewerPopover
        className="video-rate-menu__popover"
        open={open}
        label="播放速度"
        triggerRef={triggerRef}
        onOpenChange={setOpen}
      >
        <div
          id={listboxId}
          className="video-rate-menu__listbox"
          role="listbox"
          aria-label="选择播放速度"
        >
          {rates.map((candidate, index) => (
            <button
              key={candidate}
              ref={(node) => {
                optionRefs.current[index] = node
              }}
              className="video-rate-menu__option"
              type="button"
              role="option"
              aria-selected={candidate === rate}
              onClick={() => select(candidate)}
              onKeyDown={(event) => {
                const nextIndex = keyboardTargetIndex(event.key, index, rates.length)
                if (nextIndex === null) return
                event.preventDefault()
                optionRefs.current[nextIndex]?.focus()
              }}
            >
              <span>{formatRate(candidate)}</span>
              <ViewerIcon name="check" size={15} />
            </button>
          ))}
        </div>
      </ViewerPopover>
    </div>
  )
}

function formatRate(rate: VideoPlaybackRate): string {
  return `${rate}×`
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.max(minimum, Math.min(maximum, value))
}

function keyboardTargetIndex(key: string, current: number, count: number): number | null {
  if (key === 'ArrowDown') return clamp(current + 1, 0, count - 1)
  if (key === 'ArrowUp') return clamp(current - 1, 0, count - 1)
  if (key === 'Home') return 0
  if (key === 'End') return count - 1
  return null
}
