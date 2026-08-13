import type { DragEvent, MouseEvent, PointerEvent } from 'react'
import { useEffect, useState } from 'react'
import type { VideoFile } from '../../api/types'
import { OrganizationDragHandle } from './OrganizationDragHandle'

export interface VideoCardProps {
  video: VideoFile
  selected: boolean
  active: boolean
  onOpen(entityId: string): void
  onClick?: (video: VideoFile, event: MouseEvent<HTMLElement>) => void
  onRadialMenuPointerDown?: (video: VideoFile, event: PointerEvent<HTMLElement>) => void
  onRadialMenuContextMenu?: (video: VideoFile, event: MouseEvent<HTMLElement>) => void
  organizationDragDisabled?: boolean
  onFinderDragStart?: (video: VideoFile, event: DragEvent<HTMLElement>) => void
  onOrganizationPointerDown?: (video: VideoFile, event: PointerEvent<HTMLElement>) => void
  onOrganizationPointerMove?: (event: PointerEvent<HTMLElement>) => void
  onOrganizationPointerUp?: (event: PointerEvent<HTMLElement>) => void
  onOrganizationPointerCancel?: (event: PointerEvent<HTMLElement>) => void
}

export function VideoCard({
  video,
  selected,
  active,
  onOpen,
  onClick,
  onRadialMenuPointerDown,
  onRadialMenuContextMenu,
  organizationDragDisabled = false,
  onFinderDragStart,
  onOrganizationPointerDown,
  onOrganizationPointerMove,
  onOrganizationPointerUp,
  onOrganizationPointerCancel,
}: VideoCardProps) {
  const coverUrl = video.videoMetadata.coverUrl
  const [coverLoaded, setCoverLoaded] = useState(false)
  const [coverFailed, setCoverFailed] = useState(false)

  useEffect(() => {
    setCoverLoaded(false)
    setCoverFailed(false)
  }, [coverUrl])

  const supportsOrganization =
    onOrganizationPointerDown !== undefined &&
    onOrganizationPointerMove !== undefined &&
    onOrganizationPointerUp !== undefined &&
    onOrganizationPointerCancel !== undefined

  return (
    <div
      role="option"
      id={`file-${video.entityId}`}
      aria-label={video.name}
      aria-selected={selected}
      tabIndex={-1}
      data-active={active || undefined}
      className="video-card"
      onPointerDown={(event) => onRadialMenuPointerDown?.(video, event)}
      onContextMenuCapture={(event) => event.preventDefault()}
      onContextMenu={(event) => onRadialMenuContextMenu?.(video, event)}
      onClick={(event) => onClick?.(video, event)}
      onDoubleClick={() => onOpen(video.entityId)}
    >
      <div
        className="file-export-surface"
        draggable={onFinderDragStart !== undefined}
        title={onFinderDragStart === undefined ? undefined : '拖到 Finder'}
        onDragStart={(event) => onFinderDragStart?.(video, event)}
      >
        <div
          className="video-card-cover-stage"
          data-thumbnail-state={coverFailed ? 'failed' : coverLoaded ? 'ready' : 'pending'}
          style={{ aspectRatio: '16 / 9' }}
        >
          {coverUrl !== null && !coverFailed && (
            <img
              alt=""
              aria-hidden="true"
              draggable={false}
              className={`video-card-cover${coverLoaded ? ' video-card-cover--loaded' : ''}`}
              src={coverUrl}
              onLoad={() => setCoverLoaded(true)}
              onError={() => {
                setCoverLoaded(false)
                setCoverFailed(true)
              }}
            />
          )}
          {coverFailed && (
            <span className="video-card-thumbnail-failed" aria-label="视频缩略图不可用" />
          )}
          <span className="video-card-play" aria-hidden="true" />
          <span className="video-card-duration">
            {formatDuration(video.videoMetadata.durationUs)}
          </span>
          {video.videoMetadata.probeStatus === 'failed' && (
            <span className="video-card-unavailable">不可用</span>
          )}
        </div>
        <div className="video-card-meta">
          <span className="video-card-name">{video.name}</span>
        </div>
      </div>
      {supportsOrganization && (
        <OrganizationDragHandle
          file={video}
          disabled={organizationDragDisabled}
          revealed={selected}
          onPointerDown={(_file, event) => onOrganizationPointerDown(video, event)}
          onPointerMove={onOrganizationPointerMove}
          onPointerUp={onOrganizationPointerUp}
          onPointerCancel={onOrganizationPointerCancel}
        />
      )}
    </div>
  )
}

export function formatDuration(durationUs: number | null): string {
  if (durationUs === null || !Number.isFinite(durationUs) || durationUs < 0) return '--:--'
  const totalSeconds = Math.floor(durationUs / 1_000_000)
  const seconds = totalSeconds % 60
  const totalMinutes = Math.floor(totalSeconds / 60)
  if (totalMinutes < 60) return `${totalMinutes}:${String(seconds).padStart(2, '0')}`
  const hours = Math.floor(totalMinutes / 60)
  return `${hours}:${String(totalMinutes % 60).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`
}
