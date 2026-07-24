import type { DragEvent, MouseEvent, PointerEvent } from 'react'
import { useEffect, useState } from 'react'
import type { BrowserFile } from '../../api/types'
import { OrganizationDragHandle } from './OrganizationDragHandle'

export function ImageCell({
  file,
  selected,
  active,
  maxPixels,
  scaleMilli,
  loadThumbnail,
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
  selected: boolean
  active: boolean
  maxPixels: number
  scaleMilli: number
  loadThumbnail: (file: BrowserFile, maxPixels: number, scaleMilli: number) => Promise<string>
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
  const [url, setUrl] = useState<string | null>(file.imageUrl)
  const [failed, setFailed] = useState(false)

  useEffect(() => {
    let current = true
    setFailed(false)
    void loadThumbnail(file, maxPixels, scaleMilli).then(
      (nextUrl) => {
        if (current) setUrl(nextUrl)
      },
      () => {
        if (current) setFailed(true)
      },
    )
    return () => {
      current = false
    }
  }, [file, loadThumbnail, maxPixels, scaleMilli])

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
        <div className="image-cell-preview">
          {url ? (
            <img src={url} alt="" />
          ) : (
            <span aria-label={failed ? '缩略图不可用' : '缩略图加载中'} />
          )}
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
