import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react'
import type { BrowserFile, ContentFolderCard } from '../api/types'

const THUMBNAIL_SIZE = 132
const THUMBNAIL_GAP = 8
const FILMSTRIP_INLINE_PADDING = 12
const THUMBNAIL_STRIDE = THUMBNAIL_SIZE + THUMBNAIL_GAP
const OVERSCAN_CELLS = 2

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
  const filmstrip = useRef<HTMLDivElement>(null)
  const requestSequence = useRef(0)
  const [visible, setVisible] = useState(() => typeof IntersectionObserver === 'undefined')
  const [state, setState] = useState<RowState>({ status: 'idle' })
  const [viewport, setViewport] = useState({ scrollLeft: 0, width: 0 })
  const [focusedImageIndex, setFocusedImageIndex] = useState<number | null>(null)

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

  const updateViewport = useCallback(() => {
    const element = filmstrip.current
    if (element === null) return
    const next = {
      scrollLeft: element.scrollLeft,
      width: element.clientWidth,
    }
    setViewport((current) =>
      current.scrollLeft === next.scrollLeft && current.width === next.width ? current : next,
    )
  }, [])

  useLayoutEffect(() => {
    const element = filmstrip.current
    if (element === null) return
    updateViewport()
    if (typeof ResizeObserver !== 'undefined') {
      const observer = new ResizeObserver(() => updateViewport())
      observer.observe(element)
      return () => observer.disconnect()
    }
    window.addEventListener('resize', updateViewport)
    return () => window.removeEventListener('resize', updateViewport)
  }, [updateViewport])

  const reviewed = folder.reviewProgress.total - folder.reviewProgress.unmarked
  const imageWindow =
    state.status === 'ready'
      ? getImageWindow(state.images.length, viewport.scrollLeft, viewport.width)
      : null

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
        ref={filmstrip}
        className="folder-filmstrip"
        role="region"
        aria-label={`${folder.name} 图片`}
        data-state={state.status}
        onScroll={updateViewport}
      >
        {state.status === 'idle' && (
          <span className="folder-filmstrip-deferred" aria-label="等待加载图片" />
        )}
        {state.status === 'loading' &&
          ['first', 'second', 'third', 'fourth'].map((key) => (
            <span className="folder-filmstrip-skeleton" aria-label="图片加载中" key={key} />
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
        {state.status === 'ready' && imageWindow !== null && state.images.length > 0 && (
          <div
            className="folder-filmstrip-track"
            role="list"
            style={{ width: `${imageWindow.totalWidth}px` }}
          >
            {getMountedImageIndexes(imageWindow, focusedImageIndex).map((index) => {
              const file = state.images[index]!
              return (
                <div
                  className="folder-filmstrip-item"
                  role="listitem"
                  aria-posinset={index + 1}
                  aria-setsize={state.images.length}
                  key={file.entityId}
                  style={{ left: `${index * THUMBNAIL_STRIDE}px` }}
                >
                  <button
                    type="button"
                    className="folder-filmstrip-thumbnail"
                    aria-label={`预览 ${file.name}`}
                    title={file.name}
                    onFocus={() => setFocusedImageIndex(index)}
                    onBlur={() =>
                      setFocusedImageIndex((current) => (current === index ? null : current))
                    }
                    onClick={() => onPreview(file, state.images)}
                  >
                    <FolderThumbnail file={file} requestThumbnail={requestThumbnail} />
                  </button>
                </div>
              )
            })}
          </div>
        )}
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
  const [state, setState] = useState<{ status: 'loading' | 'ready' | 'failed'; url?: string }>({
    status: 'loading',
  })

  useEffect(() => {
    if (requestThumbnail === undefined) return
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
  }, [file, requestThumbnail])

  if (state.status === 'ready') return <img src={state.url} alt="" />
  return (
    <span role="img" aria-label={state.status === 'failed' ? '缩略图不可用' : '缩略图加载中'} />
  )
}

function getImageWindow(imageCount: number, scrollLeft: number, viewportWidth: number) {
  const totalWidth =
    imageCount === 0 ? 0 : imageCount * THUMBNAIL_SIZE + (imageCount - 1) * THUMBNAIL_GAP
  const viewportStart = Math.max(0, scrollLeft - FILMSTRIP_INLINE_PADDING)
  const viewportEnd = Math.max(viewportStart, scrollLeft + viewportWidth - FILMSTRIP_INLINE_PADDING)
  let firstVisible = Math.min(imageCount, Math.floor(viewportStart / THUMBNAIL_STRIDE))
  if (
    firstVisible < imageCount &&
    firstVisible * THUMBNAIL_STRIDE + THUMBNAIL_SIZE <= viewportStart
  ) {
    firstVisible += 1
  }
  const visibleEnd = Math.min(
    imageCount,
    Math.max(firstVisible + 1, Math.ceil(viewportEnd / THUMBNAIL_STRIDE)),
  )
  return {
    start: Math.max(0, firstVisible - OVERSCAN_CELLS),
    end: Math.min(imageCount, visibleEnd + OVERSCAN_CELLS),
    totalWidth,
  }
}

function getMountedImageIndexes(
  imageWindow: ReturnType<typeof getImageWindow>,
  focusedImageIndex: number | null,
) {
  const indexes = Array.from(
    { length: imageWindow.end - imageWindow.start },
    (_, offset) => imageWindow.start + offset,
  )
  if (
    focusedImageIndex === null ||
    (focusedImageIndex >= imageWindow.start && focusedImageIndex < imageWindow.end)
  ) {
    return indexes
  }
  if (focusedImageIndex < imageWindow.start) indexes.unshift(focusedImageIndex)
  else indexes.push(focusedImageIndex)
  return indexes
}
