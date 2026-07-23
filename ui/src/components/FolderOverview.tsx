import { useCallback, useRef } from 'react'
import type { BrowserFile, ContentFolderCard } from '../api/types'
import FolderFilmstripRow from './FolderFilmstripRow'

interface FolderOverviewProps {
  folders: ContentFolderCard[]
  currentPath: string
  onSelect: (entityId: string) => void
  onShowAll: () => void
  onPreview: (file: BrowserFile, files: BrowserFile[]) => void
  requestFolderImages: (entityId: string) => Promise<BrowserFile[]>
  requestThumbnail?: (file: BrowserFile) => Promise<string>
}

export default function FolderOverview({
  folders,
  currentPath,
  onSelect,
  onShowAll,
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
      <header className="workspace-heading">
        <p>{currentPath || '项目根目录'}</p>
        <button type="button" onClick={onShowAll}>
          显示全部后代文件
        </button>
      </header>
      <div className="folder-filmstrip-list">
        {folders.map((folder) => (
          <FolderFilmstripRow
            folder={folder}
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
