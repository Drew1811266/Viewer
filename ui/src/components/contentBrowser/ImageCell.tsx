import type { DragEvent, MouseEvent, PointerEvent } from 'react'
import type { BrowserFile } from '../../api/types'
import { isPreviewableImage } from '../../fileKinds'
import type { AspectRect, ImageDimensions } from '../../layout/aspectLayout'
import AspectThumbnail from '../AspectThumbnail'
import ReviewFeedbackCountBadge from '../review/ReviewFeedbackCountBadge'
import UnsupportedFileState from '../UnsupportedFileState'
import ViewerIcon from '../ui/ViewerIcon'
import { OrganizationDragHandle } from './OrganizationDragHandle'

export function ImageCell({
  file,
  rect,
  dimensionsKnown,
  selected,
  active,
  loadThumbnail,
  onNaturalDimensions,
  markerLabel,
  feedbackCount,
  onClick,
  onPreview,
  onRadialMenuPointerDown,
  onRadialMenuContextMenu,
  organizationDragDisabled,
  onFinderDragStart,
  onPointerDown,
  onPointerMove,
  onPointerUp,
  onPointerCancel,
}: {
  file: BrowserFile
  rect: AspectRect
  dimensionsKnown: boolean
  selected: boolean
  active: boolean
  loadThumbnail: (file: BrowserFile, maxPixels: number, scaleMilli: number) => Promise<string>
  onNaturalDimensions: (
    identity: { entityId: string; modifiedNs: string },
    dimensions: ImageDimensions,
  ) => void
  markerLabel: string | null
  feedbackCount?: number
  onClick: (file: BrowserFile, event: MouseEvent) => void
  onPreview: (file: BrowserFile) => void
  onRadialMenuPointerDown: (file: BrowserFile, event: PointerEvent<HTMLElement>) => void
  onRadialMenuContextMenu: (file: BrowserFile, event: MouseEvent<HTMLElement>) => void
  organizationDragDisabled: boolean
  onFinderDragStart: (file: BrowserFile, event: DragEvent<HTMLElement>) => void
  onPointerDown: (file: BrowserFile, event: PointerEvent<HTMLElement>) => void
  onPointerMove: (event: PointerEvent<HTMLElement>) => void
  onPointerUp: (event: PointerEvent<HTMLElement>) => void
  onPointerCancel: (event: PointerEvent<HTMLElement>) => void
}) {
  return (
    <div
      role="option"
      id={`file-${file.entityId}`}
      aria-label={file.name}
      aria-selected={selected}
      tabIndex={undefined}
      data-active={active || undefined}
      className="image-cell"
      onPointerDown={(event) => onRadialMenuPointerDown(file, event)}
      onContextMenuCapture={(event) => event.preventDefault()}
      onContextMenu={(event) => onRadialMenuContextMenu(file, event)}
      onClick={(event) => onClick(file, event)}
      onDoubleClick={() => onPreview(file)}
    >
      <div
        className="file-export-surface"
        draggable
        title="拖到 Finder"
        onDragStart={(event) => onFinderDragStart(file, event)}
      >
        <div className="image-cell-thumbnail-frame">
          <div
            className="image-cell-preview"
            style={{ width: rect.imageWidth, height: rect.imageHeight }}
          >
            {isPreviewableImage(file) ? (
              <AspectThumbnail
                file={file}
                width={rect.imageWidth}
                height={rect.imageHeight}
                dimensionsKnown={dimensionsKnown}
                loadThumbnail={loadThumbnail}
                onNaturalDimensions={onNaturalDimensions}
              />
            ) : (
              <UnsupportedFileState file={file} compact />
            )}
          </div>
        </div>
        <div className="image-cell-meta">
          <span className="image-cell-meta-line">
            <span className="image-cell-name">{file.name}</span>
            <ReviewFeedbackCountBadge count={feedbackCount} />
          </span>
          <span className="image-cell-submeta">
            {fileTypeLabel(file)} · {formatFileSize(file.size)}
          </span>
        </div>
      </div>
      {markerLabel && (
        <span
          className="image-cell-marker"
          data-review={file.marker.reviewState ?? undefined}
          data-favorite={file.marker.favorite || undefined}
        >
          {markerLabel}
        </span>
      )}
      {selected && (
        <span className="image-cell-selected-check" aria-hidden="true">
          <ViewerIcon name="check" />
        </span>
      )}
      <OrganizationDragHandle
        file={file}
        disabled={organizationDragDisabled}
        revealed={selected}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerCancel={onPointerCancel}
      />
    </div>
  )
}

function fileTypeLabel(file: BrowserFile): string {
  const extension = file.name.includes('.') ? file.name.split('.').pop() : null
  if (extension !== null && extension !== undefined && extension.length > 0) {
    return extension.toUpperCase()
  }
  if (file.kind === 'markdown') return 'MARKDOWN'
  if (file.kind === 'text') return 'TXT'
  if (file.kind === 'video') return 'VIDEO'
  if (file.kind === 'image' || file.kind === 'jpeg' || file.kind === 'png') return 'IMAGE'
  return 'FILE'
}

function formatFileSize(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return '—'
  if (bytes < 1_024) return `${bytes} B`
  const units = ['KiB', 'MiB', 'GiB']
  let value = bytes / 1_024
  let unit = 0
  while (value >= 1_024 && unit < units.length - 1) {
    value /= 1_024
    unit += 1
  }
  const rounded = value >= 100 ? Math.round(value) : Math.round(value * 10) / 10
  return `${rounded} ${units[unit]}`
}
