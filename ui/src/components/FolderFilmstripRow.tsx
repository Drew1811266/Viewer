import { useCallback, useEffect, useRef, useState } from 'react'
import type { BrowserFile, ContentFolderCard } from '../api/types'

interface FolderFilmstripRowProps {
  folder: ContentFolderCard
  loadImages: (entityId: string, retry?: boolean) => Promise<BrowserFile[]>
  onSelect: (entityId: string) => void
  onPreview: (file: BrowserFile, files: BrowserFile[]) => void
  requestThumbnail?: (file: BrowserFile) => Promise<string>
}

type RowState =
  | { status: 'idle' | 'loading' }
  | { status: 'ready'; images: BrowserFile[] }
  | { status: 'failed' }

export default function FolderFilmstripRow({
  folder,
  loadImages,
  onSelect,
  onPreview,
  requestThumbnail,
}: FolderFilmstripRowProps) {
  const row = useRef<HTMLElement>(null)
  const requestSequence = useRef(0)
  const [visible, setVisible] = useState(() => typeof IntersectionObserver === 'undefined')
  const [state, setState] = useState<RowState>({ status: 'idle' })

  useEffect(() => {
    if (typeof IntersectionObserver === 'undefined' || row.current === null) return
    const observer = new IntersectionObserver(
      (entries) => {
        if (!entries.some((entry) => entry.isIntersecting)) return
        setVisible(true)
        observer.disconnect()
      },
      { rootMargin: '240px 0px' },
    )
    observer.observe(row.current)
    return () => observer.disconnect()
  }, [])

  const requestImages = useCallback(
    (retry: boolean) => {
      requestSequence.current += 1
      const requestId = requestSequence.current
      setState({ status: 'loading' })
      void loadImages(folder.entityId, retry).then(
        (images) => {
          if (requestId === requestSequence.current) {
            setState({ status: 'ready', images })
          }
        },
        () => {
          if (requestId === requestSequence.current) setState({ status: 'failed' })
        },
      )
    },
    [folder.entityId, loadImages],
  )

  useEffect(() => {
    if (!visible || state.status !== 'idle') return
    requestImages(false)
  }, [requestImages, state.status, visible])

  useEffect(
    () => () => {
      requestSequence.current += 1
    },
    [],
  )

  const reviewed = folder.reviewProgress.total - folder.reviewProgress.unmarked

  return (
    <article ref={row} className="folder-filmstrip-row">
      <button
        type="button"
        className="folder-filmstrip-identity"
        aria-label={`打开 ${folder.name}`}
        onClick={() => onSelect(folder.entityId)}
      >
        <strong>{folder.name}</strong>
        <span>{folder.relativePath}</span>
        <span className="folder-filmstrip-counts">
          <span>{`${folder.imageCount} 张图片`}</span> · <span>{`${folder.textCount} 个文本`}</span>
        </span>
        <span className="folder-filmstrip-marker">{`文件夹：${markerLabel(folder.marker)}`}</span>
        <span>{`已审阅 ${reviewed} / ${folder.reviewProgress.total}`}</span>
      </button>
      <div
        className="folder-filmstrip"
        role="region"
        aria-label={`${folder.name} 图片`}
        data-state={state.status}
      >
        {state.status === 'idle' && (
          <span className="folder-filmstrip-deferred" aria-label="等待加载图片" />
        )}
        {state.status === 'loading' &&
          Array.from({ length: 4 }, (_, index) => (
            <span
              className="folder-filmstrip-skeleton"
              aria-label="图片加载中"
              key={index}
            />
          ))}
        {state.status === 'failed' && (
          <div className="folder-filmstrip-error" role="alert">
            <span>无法加载图片</span>
            <button
              type="button"
              aria-label={`重试 ${folder.name}`}
              onClick={() => requestImages(true)}
            >
              重试
            </button>
          </div>
        )}
        {state.status === 'ready' && state.images.length === 0 && <p>无图片</p>}
        {state.status === 'ready' &&
          state.images.map((file) => (
            <button
              type="button"
              className="folder-filmstrip-thumbnail"
              aria-label={`预览 ${file.name}`}
              title={file.name}
              key={file.entityId}
              onClick={() => onPreview(file, state.images)}
            >
              <FolderThumbnail file={file} requestThumbnail={requestThumbnail} />
            </button>
          ))}
      </div>
    </article>
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
      if (!entries.some((entry) => entry.isIntersecting)) return
      setVisible(true)
      observer.disconnect()
    })
    observer.observe(element.current)
    return () => observer.disconnect()
  }, [])

  useEffect(() => {
    if (!visible || requestThumbnail === undefined) return
    let current = true
    void requestThumbnail(file).then(
      (url) => {
        if (current) setState({ status: 'ready', url })
      },
      () => {
        if (current) setState({ status: 'failed' })
      },
    )
    return () => {
      current = false
    }
  }, [file, requestThumbnail, visible])

  if (state.status === 'ready') return <img src={state.url} alt="" />
  return (
    <span
      ref={element}
      aria-label={state.status === 'failed' ? '缩略图不可用' : '缩略图加载中'}
    />
  )
}
