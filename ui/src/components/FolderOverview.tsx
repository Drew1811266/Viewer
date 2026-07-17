import { useEffect, useRef, useState } from 'react'
import type { BrowserFile, ContentFolderCard } from '../api/types'

interface FolderOverviewProps {
  folders: ContentFolderCard[]
  currentPath: string
  onSelect: (entityId: string) => void
  onShowAll: () => void
  requestThumbnail?: (file: BrowserFile) => Promise<string>
}

export default function FolderOverview({
  folders,
  currentPath,
  onSelect,
  onShowAll,
  requestThumbnail,
}: FolderOverviewProps) {
  return (
    <section className="folder-overview" aria-label="内容文件夹概览">
      <header className="workspace-heading">
        <p>{currentPath || '项目根目录'}</p>
        <button type="button" onClick={onShowAll}>
          显示全部后代文件
        </button>
      </header>
      <div className="folder-card-grid">
        {folders.map((folder) => (
          <article className="folder-card" key={folder.entityId}>
            <button
              type="button"
              className="folder-card-open"
              aria-label={`打开 ${folder.name}`}
              onClick={() => onSelect(folder.entityId)}
            >
              <span>{folder.name}</span>
              <span>{folder.relativePath}</span>
            </button>
            <div className="folder-card-thumbnails">
              {Array.from({ length: 4 }, (_, index) => {
                const image = folder.representativeImages[index]
                return image ? (
                  <FolderThumbnail
                    key={image.entityId}
                    file={image}
                    requestThumbnail={requestThumbnail}
                  />
                ) : (
                  <span key={`empty-${index}`} aria-label="无缩略图" />
                )
              })}
            </div>
            <p className="folder-card-counts">
              <span>{folder.imageCount} 张图片</span>
              <span>{folder.textCount} 个文本</span>
            </p>
            <p className="folder-card-marker">文件夹：{markerLabel(folder.marker)}</p>
            <p className="folder-card-review-total">
              已审阅 {folder.reviewProgress.total - folder.reviewProgress.unmarked} /{' '}
              {folder.reviewProgress.total}
            </p>
            <p className="folder-card-review-breakdown">
              保留 {folder.reviewProgress.keep} · 待定 {folder.reviewProgress.pending} · 淘汰{' '}
              {folder.reviewProgress.reject} · 未标记 {folder.reviewProgress.unmarked} · 收藏{' '}
              {folder.reviewProgress.favorite}
            </p>
          </article>
        ))}
      </div>
    </section>
  )
}

function markerLabel(marker: ContentFolderCard['marker']): string {
  const review =
    marker.reviewState === 'keep'
      ? '保留'
      : marker.reviewState === 'pending'
        ? '待定'
        : marker.reviewState === 'reject'
          ? '淘汰'
          : '未标记'
  return marker.favorite ? `${review} · 收藏` : review
}

function FolderThumbnail({
  file,
  requestThumbnail,
}: {
  file: BrowserFile
  requestThumbnail?: (file: BrowserFile) => Promise<string>
}) {
  const element = useRef<HTMLSpanElement>(null)
  const [visible, setVisible] = useState(() => typeof IntersectionObserver === 'undefined')
  const [state, setState] = useState<{ status: 'loading' | 'ready' | 'failed'; url?: string }>(
    { status: 'loading' },
  )

  useEffect(() => {
    if (typeof IntersectionObserver === 'undefined' || element.current === null) return
    const observer = new IntersectionObserver((entries) => {
      if (entries.some((entry) => entry.isIntersecting)) setVisible(true)
    })
    observer.observe(element.current)
    return () => observer.disconnect()
  }, [])

  useEffect(() => {
    if (!visible || requestThumbnail === undefined) return
    let cancelled = false
    void requestThumbnail(file).then(
      (url) => {
        if (!cancelled) setState({ status: 'ready', url })
      },
      () => {
        if (!cancelled) setState({ status: 'failed' })
      },
    )
    return () => {
      cancelled = true
    }
  }, [file.entityId, file.modifiedNs, requestThumbnail, visible])

  if (state.status === 'ready') {
    return <img src={state.url} alt={file.name} />
  }
  return (
    <span
      ref={element}
      aria-label={state.status === 'failed' ? '缩略图不可用' : '缩略图加载中'}
    />
  )
}
