import type { DragEvent, MouseEvent, PointerEvent } from 'react'
import type { BrowserFile } from '../../api/types'
import { isPreviewableImage } from '../../fileKinds'
import type { AspectRect, ImageDimensions } from '../../layout/aspectLayout'
import AspectThumbnail from '../AspectThumbnail'
import UnsupportedFileState from '../UnsupportedFileState'
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
        <div className="image-cell-thumbnail-frame" data-selected={selected || undefined}>
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
        <span>{file.name}</span>
        {markerLabel && <span className="file-marker">{markerLabel}</span>}
      </div>
      <OrganizationDragHandle
        file={file}
        disabled={organizationDragDisabled}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerCancel={onPointerCancel}
      />
    </div>
  )
}
