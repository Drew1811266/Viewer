import { type CSSProperties, type PointerEvent, useEffect, useMemo, useRef, useState } from 'react'
import type { VideoEvent } from '../../api/types'

const TIMELINE_BUCKET_US = 500_000
const THUMBNAIL_DEBOUNCE_MS = 80
const THUMBNAIL_WIDTH_PX = 160

type TimelineThumbnail = Extract<VideoEvent, { type: 'timelineThumbnailReady' }>

interface PendingThumbnail {
  generation: number
  requestId: string
  bucketUs: number
}

export interface VideoTimelineProps {
  generation: number
  durationUs: number | null
  timeUs: number
  thumbnail: TimelineThumbnail | null
  onSeek(timeUs: number): Promise<void>
  onRequestThumbnail(request: { requestId: string; timeUs: number }): Promise<void>
  onSeekingChange(seeking: boolean): void
  onActivity(): void
}

export default function VideoTimeline({
  generation,
  durationUs,
  timeUs,
  thumbnail,
  onSeek,
  onRequestThumbnail,
  onSeekingChange,
  onActivity,
}: VideoTimelineProps) {
  const slider = useRef<HTMLDivElement>(null)
  const requestSequence = useRef(0)
  const dragPointer = useRef<number | null>(null)
  const seekFrame = useRef<number | null>(null)
  const pendingSeek = useRef<number | null>(null)
  const lastSeek = useRef<number | null>(null)
  const seekingChange = useRef(onSeekingChange)
  seekingChange.current = onSeekingChange
  const [pointerTimeUs, setPointerTimeUs] = useState<number | null>(null)
  const [pointerOffsetPx, setPointerOffsetPx] = useState(0)
  const [trackWidthPx, setTrackWidthPx] = useState(0)
  const [dragging, setDragging] = useState(false)
  const [pendingThumbnail, setPendingThumbnail] = useState<PendingThumbnail | null>(null)
  const [acceptedThumbnail, setAcceptedThumbnail] = useState<TimelineThumbnail | null>(null)
  const boundedDurationUs = finiteDuration(durationUs)
  const currentTimeUs = clampTime(timeUs, boundedDurationUs)
  const displayedTimeUs = dragging && pointerTimeUs !== null ? pointerTimeUs : currentTimeUs
  const pointerBucketUs =
    pointerTimeUs === null ? null : quantizeTimelineTime(pointerTimeUs, boundedDurationUs)

  useEffect(() => {
    const wasDragging = dragPointer.current !== null
    if (seekFrame.current !== null) window.cancelAnimationFrame(seekFrame.current)
    dragPointer.current = null
    seekFrame.current = null
    pendingSeek.current = null
    lastSeek.current = null
    setDragging(false)
    setPointerTimeUs(null)
    setPendingThumbnail(null)
    setAcceptedThumbnail(null)
    if (wasDragging) seekingChange.current(false)
  }, [generation])

  useEffect(() => {
    if (pointerBucketUs === null || generation <= 0 || boundedDurationUs <= 0) return
    if (
      pendingThumbnail?.generation === generation &&
      pendingThumbnail.bucketUs === pointerBucketUs
    ) {
      return
    }
    const timeout = window.setTimeout(() => {
      const requestId = `timeline-${generation}-${++requestSequence.current}`
      setPendingThumbnail({ generation, requestId, bucketUs: pointerBucketUs })
      void onRequestThumbnail({ requestId, timeUs: pointerBucketUs }).catch(() => undefined)
    }, THUMBNAIL_DEBOUNCE_MS)
    return () => window.clearTimeout(timeout)
  }, [boundedDurationUs, generation, onRequestThumbnail, pendingThumbnail, pointerBucketUs])

  useEffect(
    () => () => {
      if (seekFrame.current !== null) window.cancelAnimationFrame(seekFrame.current)
    },
    [],
  )

  useEffect(() => {
    if (matchesCurrentThumbnail(thumbnail, pendingThumbnail, generation, pointerBucketUs)) {
      setAcceptedThumbnail(thumbnail)
    }
  }, [generation, pendingThumbnail, pointerBucketUs, thumbnail])

  const readyThumbnail = useMemo(
    () =>
      matchesCurrentThumbnail(acceptedThumbnail, pendingThumbnail, generation, pointerBucketUs)
        ? acceptedThumbnail
        : null,
    [acceptedThumbnail, generation, pendingThumbnail, pointerBucketUs],
  )

  function targetFromPointer(event: PointerEvent<HTMLDivElement>): number {
    const rect = event.currentTarget.getBoundingClientRect()
    const width = Number.isFinite(rect.width) && rect.width > 0 ? rect.width : 0
    const offset = clamp(event.clientX - rect.left, 0, width)
    setPointerOffsetPx(offset)
    setTrackWidthPx(width)
    return width === 0 ? 0 : Math.round((offset / width) * boundedDurationUs)
  }

  function point(event: PointerEvent<HTMLDivElement>) {
    onActivity()
    const targetUs = targetFromPointer(event)
    setPointerTimeUs(targetUs)
    return targetUs
  }

  function scheduleSeek(targetUs: number) {
    pendingSeek.current = targetUs
    if (seekFrame.current !== null) return
    seekFrame.current = window.requestAnimationFrame(() => flushSeek())
  }

  function flushSeek() {
    if (seekFrame.current !== null) window.cancelAnimationFrame(seekFrame.current)
    seekFrame.current = null
    const targetUs = pendingSeek.current
    pendingSeek.current = null
    if (targetUs === null || targetUs === lastSeek.current) return
    lastSeek.current = targetUs
    void onSeek(targetUs).catch(() => undefined)
  }

  function pointerDown(event: PointerEvent<HTMLDivElement>) {
    if (event.button !== 0) return
    const targetUs = point(event)
    dragPointer.current = event.pointerId
    setDragging(true)
    onSeekingChange(true)
    event.currentTarget.setPointerCapture?.(event.pointerId)
    scheduleSeek(targetUs)
  }

  function pointerMove(event: PointerEvent<HTMLDivElement>) {
    const targetUs = point(event)
    if (dragPointer.current === event.pointerId) scheduleSeek(targetUs)
  }

  function pointerUp(event: PointerEvent<HTMLDivElement>) {
    if (dragPointer.current !== event.pointerId) return
    const targetUs = point(event)
    scheduleSeek(targetUs)
    flushSeek()
    dragPointer.current = null
    setDragging(false)
    onSeekingChange(false)
    event.currentTarget.releasePointerCapture?.(event.pointerId)
  }

  const previewStyle = timelinePreviewStyle(pointerOffsetPx, trackWidthPx)
  const progress = boundedDurationUs === 0 ? 0 : (displayedTimeUs / boundedDurationUs) * 100

  return (
    <div className="video-timeline">
      <div
        ref={slider}
        className="video-timeline__slider"
        role="slider"
        aria-label="视频时间轴"
        aria-valuemin={0}
        aria-valuemax={microsecondsToSeconds(boundedDurationUs)}
        aria-valuenow={microsecondsToSeconds(displayedTimeUs)}
        aria-valuetext={`${formatVideoTime(displayedTimeUs)} / ${formatVideoTime(boundedDurationUs)}`}
        tabIndex={0}
        onPointerEnter={point}
        onPointerMove={pointerMove}
        onPointerDown={pointerDown}
        onPointerUp={pointerUp}
        onPointerCancel={pointerUp}
        onPointerLeave={() => {
          if (dragPointer.current === null) setPointerTimeUs(null)
        }}
      >
        <span className="video-timeline__track" aria-hidden="true">
          <span className="video-timeline__progress" style={{ width: `${progress}%` }} />
          <span className="video-timeline__thumb" style={{ left: `${progress}%` }} />
        </span>
        {pointerTimeUs !== null && (
          <span
            className="video-timeline__thumbnail"
            data-testid="timeline-thumbnail"
            style={previewStyle}
          >
            {readyThumbnail === null ? (
              <span
                className="video-timeline__thumbnail-pending"
                data-testid="timeline-thumbnail-pending"
              />
            ) : (
              <img
                src={readyThumbnail.artifactUrl}
                alt={`${formatVideoTime(readyThumbnail.bucketUs)} 预览`}
              />
            )}
            <span className="video-timeline__thumbnail-time">
              {formatVideoTime(pointerBucketUs ?? pointerTimeUs)}
            </span>
          </span>
        )}
      </div>
      <span className="video-timeline__time">
        {formatVideoTime(displayedTimeUs)} / {formatVideoTime(boundedDurationUs)}
      </span>
    </div>
  )
}

export function formatVideoTime(timeUs: number): string {
  const bounded = Math.max(0, Number.isFinite(timeUs) ? Math.floor(timeUs) : 0)
  const tenths = Math.floor(bounded / 100_000)
  const totalSeconds = Math.floor(tenths / 10)
  const fraction = tenths % 10
  const hours = Math.floor(totalSeconds / 3_600)
  const minutes = Math.floor((totalSeconds % 3_600) / 60)
  const seconds = totalSeconds % 60
  const base =
    hours > 0
      ? `${String(hours).padStart(2, '0')}:${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`
      : `${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`
  return fraction === 0 ? base : `${base}.${fraction}`
}

function finiteDuration(durationUs: number | null): number {
  return durationUs !== null && Number.isFinite(durationUs) && durationUs > 0
    ? Math.floor(durationUs)
    : 0
}

function clampTime(timeUs: number, durationUs: number): number {
  return Math.round(clamp(Number.isFinite(timeUs) ? timeUs : 0, 0, durationUs))
}

function quantizeTimelineTime(timeUs: number, durationUs: number): number {
  const bounded = clampTime(timeUs, durationUs)
  return Math.floor(bounded / TIMELINE_BUCKET_US) * TIMELINE_BUCKET_US
}

function microsecondsToSeconds(timeUs: number): number {
  return timeUs / 1_000_000
}

function timelinePreviewStyle(offsetPx: number, trackWidthPx: number): CSSProperties {
  if (offsetPx <= THUMBNAIL_WIDTH_PX / 2) return { left: '0px' }
  if (offsetPx >= trackWidthPx - THUMBNAIL_WIDTH_PX / 2) return { right: '0px' }
  return { left: `${offsetPx}px`, transform: 'translateX(-50%)' }
}

function matchesCurrentThumbnail(
  thumbnail: TimelineThumbnail | null,
  pending: PendingThumbnail | null,
  generation: number,
  pointerBucketUs: number | null,
): thumbnail is TimelineThumbnail {
  return (
    thumbnail !== null &&
    pending !== null &&
    pointerBucketUs !== null &&
    thumbnail.generation === generation &&
    thumbnail.generation === pending.generation &&
    thumbnail.requestId === pending.requestId &&
    thumbnail.bucketUs === pending.bucketUs &&
    thumbnail.bucketUs === pointerBucketUs
  )
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(maximum, Math.max(minimum, value))
}
