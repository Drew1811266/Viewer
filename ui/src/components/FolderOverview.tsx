import { useCallback, useRef } from 'react'
import type { BrowserFile, ContentFolderCard, ThumbnailDensity } from '../api/types'
import FolderFilmstripRow from './FolderFilmstripRow'

interface FolderOverviewProps {
  folders: ContentFolderCard[]
  density: ThumbnailDensity
  onSelect: (entityId: string) => void
  onPreview: (file: BrowserFile, files: BrowserFile[]) => void
  requestFolderImages: (entityId: string) => Promise<BrowserFile[]>
  requestThumbnail?: (file: BrowserFile, maxPixels: number, scaleMilli: number) => Promise<string>
}

export default function FolderOverview({
  folders,
  density,
  onSelect,
  onPreview,
  requestFolderImages,
  requestThumbnail,
}: FolderOverviewProps) {
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

  return (
    <section className="folder-overview" aria-label="内容文件夹概览">
      <div className="folder-filmstrip-list">
        {folders.map((folder) => (
          <FolderFilmstripRow
            folder={folder}
            density={density}
            key={folder.entityId}
            loadImages={loadImages}
            requestThumbnail={requestThumbnail}
            onSelect={onSelect}
            onPreview={onPreview}
          />
        ))}
      </div>
    </section>
  )
}
