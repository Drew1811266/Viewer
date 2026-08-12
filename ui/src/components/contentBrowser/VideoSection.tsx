import type { DragEvent, KeyboardEvent, MouseEvent, PointerEvent } from 'react'
import type { VideoFile } from '../../api/types'
import { ViewerIconButton } from '../ui/ViewerButton'
import { VideoCard } from './VideoCard'

const VIDEO_LIST_ID = 'content-video-list'
const VIDEO_TITLE_ID = 'video-section-title'

export interface VideoSectionProps {
  videos: readonly VideoFile[]
  expanded: boolean
  onExpandedChange(expanded: boolean): void
  selection: ReadonlySet<string>
  activeId: string | null
  onOpen(entityId: string): void
  onListKeyDown?: (event: KeyboardEvent<HTMLElement>) => void
  onSelect?: (video: VideoFile, event: MouseEvent<HTMLElement>) => void
  onRadialMenuPointerDown?: (video: VideoFile, event: PointerEvent<HTMLElement>) => void
  onRadialMenuContextMenu?: (video: VideoFile, event: MouseEvent<HTMLElement>) => void
  organizationDragDisabled?: boolean
  onFinderDragStart?: (video: VideoFile, event: DragEvent<HTMLElement>) => void
  onOrganizationPointerDown?: (video: VideoFile, event: PointerEvent<HTMLElement>) => void
  onOrganizationPointerMove?: (event: PointerEvent<HTMLElement>) => void
  onOrganizationPointerUp?: (event: PointerEvent<HTMLElement>) => void
  onOrganizationPointerCancel?: (event: PointerEvent<HTMLElement>) => void
}

export function VideoSection({
  videos,
  expanded,
  onExpandedChange,
  selection,
  activeId,
  onOpen,
  onListKeyDown,
  onSelect,
  onRadialMenuPointerDown,
  onRadialMenuContextMenu,
  organizationDragDisabled,
  onFinderDragStart,
  onOrganizationPointerDown,
  onOrganizationPointerMove,
  onOrganizationPointerUp,
  onOrganizationPointerCancel,
}: VideoSectionProps) {
  if (videos.length === 0) return null
  return (
    <section className="video-section" aria-labelledby={VIDEO_TITLE_ID}>
      <div className="video-section-disclosure" data-expanded={expanded || undefined}>
        <ViewerIconButton
          className="video-section-disclosure-button"
          icon={expanded ? 'chevron-up' : 'chevron-down'}
          label={expanded ? '收起视频' : '展开视频'}
          tone="quiet"
          aria-expanded={expanded}
          aria-controls={VIDEO_LIST_ID}
          onClick={() => onExpandedChange(!expanded)}
        />
        <span id={VIDEO_TITLE_ID} className="video-section-title">
          视频 · {videos.length}
        </span>
      </div>
      {expanded && (
        <div
          id={VIDEO_LIST_ID}
          role="listbox"
          aria-label="视频文件"
          aria-multiselectable="true"
          aria-activedescendant={activeId === null ? undefined : `file-${activeId}`}
          tabIndex={0}
          className="video-card-list"
          onKeyDown={onListKeyDown}
        >
          {videos.map((video) => (
            <VideoCard
              key={video.entityId}
              video={video}
              selected={selection.has(video.entityId)}
              active={activeId === video.entityId}
              onOpen={onOpen}
              onClick={onSelect}
              onRadialMenuPointerDown={onRadialMenuPointerDown}
              onRadialMenuContextMenu={onRadialMenuContextMenu}
              organizationDragDisabled={organizationDragDisabled}
              onFinderDragStart={onFinderDragStart}
              onOrganizationPointerDown={onOrganizationPointerDown}
              onOrganizationPointerMove={onOrganizationPointerMove}
              onOrganizationPointerUp={onOrganizationPointerUp}
              onOrganizationPointerCancel={onOrganizationPointerCancel}
            />
          ))}
        </div>
      )}
    </section>
  )
}
