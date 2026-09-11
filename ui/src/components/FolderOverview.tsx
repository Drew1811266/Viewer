import { useCallback, useRef } from 'react'
import type { BrowserFile, ContentFolderCard, ThumbnailDensity } from '../api/types'
import {
  findVerticalScrollAncestor,
  usePrefetchOnScroll,
} from '../app/workspace/usePrefetchOnScroll'
import { isPreviewableImage } from '../fileKinds'
import { THUMBNAIL_HEIGHT, thumbnailRequestSize } from '../settings/thumbnailDensity'
import FolderFilmstripRow from './FolderFilmstripRow'

const FILMSTRIP_ROW_BORDER_PX = 1
const FILMSTRIP_ROW_FOOTER_PX = 24
/** 预取下一行文件夹的前 N 张预览图（性能方案 §3.2：density 决定 ~4-6 张）。 */
const PREFETCH_IMAGE_COUNT = 6

interface FolderOverviewProps {
  folders: ContentFolderCard[]
  density: ThumbnailDensity
  onSelect: (entityId: string) => void
  onPreview: (file: BrowserFile, files: BrowserFile[]) => void
  requestFolderImages: (entityId: string) => Promise<BrowserFile[]>
  requestThumbnail?: (file: BrowserFile, maxPixels: number, scaleMilli: number) => Promise<string>
  prefetchThumbnails?: (
    files: BrowserFile[],
    maxPixels: number,
    scaleMilli: number,
    signal: AbortSignal,
  ) => Promise<void>
  feedbackCountByEntityId?: ReadonlyMap<string, number>
}

export default function FolderOverview({
  folders,
  density,
  onSelect,
  onPreview,
  requestFolderImages,
  requestThumbnail,
  prefetchThumbnails,
  feedbackCountByEntityId,
}: FolderOverviewProps) {
  const section = useRef<HTMLElement>(null)
  const requests = useRef(new Map<string, Promise<BrowserFile[]>>())
  const loadImages = useCallback(
    (entityId: string, retry = false) => {
      if (retry) requests.current.delete(entityId)
      const cached = requests.current.get(entityId)
      if (cached !== undefined) return cached
      const request = requestFolderImages(entityId)
      requests.current.set(entityId, request)
      return request
    },
    [requestFolderImages],
  )

  const prefetchRow = useCallback(
    (rowIndex: number, signal: AbortSignal) => {
      const folder = folders[rowIndex]
      if (folder === undefined || prefetchThumbnails === undefined) return
      void loadImages(folder.entityId).then(
        (images) => {
          if (signal.aborted) return
          const targets = images.filter(isPreviewableImage).slice(0, PREFETCH_IMAGE_COUNT)
          if (targets.length === 0) return
          const edge = THUMBNAIL_HEIGHT[density]
          const size = thumbnailRequestSize(edge, edge, window.devicePixelRatio)
          void prefetchThumbnails(targets, size.maxPixels, size.scaleMilli, signal)
        },
        () => undefined,
      )
    },
    [density, folders, loadImages, prefetchThumbnails],
  )

  usePrefetchOnScroll({
    getContainer: () => findVerticalScrollAncestor(section.current),
    rowCount: folders.length,
    rowHeight: THUMBNAIL_HEIGHT[density] + FILMSTRIP_ROW_FOOTER_PX + FILMSTRIP_ROW_BORDER_PX,
    onPrefetchRow: prefetchRow,
  })

  return (
    <section ref={section} className="folder-overview" aria-label="内容文件夹概览">
      <div className="folder-filmstrip-list">
        {folders.map((folder) => (
          <FolderFilmstripRow
            folder={folder}
            density={density}
            key={folder.entityId}
            loadImages={loadImages}
            requestThumbnail={requestThumbnail}
            feedbackCountByEntityId={feedbackCountByEntityId}
            onSelect={onSelect}
            onPreview={onPreview}
          />
        ))}
      </div>
    </section>
  )
}
