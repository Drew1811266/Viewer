import { useCallback, useMemo, useRef } from 'react'
import type { BrowserFile, ContentFolderCard, ThumbnailDensity } from '../api/types'
import {
  findVerticalScrollAncestor,
  usePrefetchOnScroll,
} from '../app/workspace/usePrefetchOnScroll'
import { isPreviewableImage } from '../fileKinds'
import { THUMBNAIL_HEIGHT, thumbnailRequestSize } from '../settings/thumbnailDensity'
import FolderFilmstripRow from './FolderFilmstripRow'

const FILMSTRIP_ROW_EXTRA_PX = 26
const FILMSTRIP_ROW_GAP_PX = 12
const PREFETCH_IMAGE_COUNT = 6

interface FolderOverviewProps {
  folders: ContentFolderCard[]
  density: ThumbnailDensity
  projectName?: string
  onSelect: (entityId: string) => void
  onPreview: (file: BrowserFile, files: BrowserFile[]) => void
  requestFolderImages?: (entityId: string) => Promise<BrowserFile[]>
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
  projectName = '全部素材',
  onSelect,
  onPreview,
  requestFolderImages,
  requestThumbnail,
  prefetchThumbnails,
  feedbackCountByEntityId,
}: FolderOverviewProps) {
  const section = useRef<HTMLElement>(null)
  const requests = useRef(new Map<string, Promise<BrowserFile[]>>())
  const representativeImages = useMemo(
    () => new Map(folders.map((folder) => [folder.entityId, folder.representativeImages])),
    [folders],
  )
  const totals = useMemo(
    () =>
      folders.reduce(
        (sum, folder) => ({
          folders: sum.folders + 1,
          assets: sum.assets + folder.imageCount + folder.videoCount + folder.otherFileCount,
        }),
        { folders: 0, assets: 0 },
      ),
    [folders],
  )

  const loadImages = useCallback(
    (entityId: string, retry = false) => {
      if (retry) requests.current.delete(entityId)
      const cached = requests.current.get(entityId)
      if (cached !== undefined) return cached
      const request =
        requestFolderImages === undefined
          ? Promise.resolve(representativeImages.get(entityId) ?? [])
          : requestFolderImages(entityId)
      requests.current.set(entityId, request)
      return request
    },
    [representativeImages, requestFolderImages],
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
    rowHeight: THUMBNAIL_HEIGHT[density] + FILMSTRIP_ROW_EXTRA_PX + FILMSTRIP_ROW_GAP_PX,
    onPrefetchRow: prefetchRow,
  })

  return (
    <section ref={section} className="folder-overview" aria-label="内容文件夹概览">
      <header className="folder-overview-header">
        <h1>{projectName}</h1>
        <p>
          {totals.folders} 个文件夹 · {totals.assets} 个素材
        </p>
      </header>
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
      <p className="folder-overview-note">横向滚动查看文件夹内容；纵向滚动浏览全部素材。</p>
    </section>
  )
}
