import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import type { KeyboardEvent, MouseEvent } from 'react'
import type { BrowserFile, FolderWorkspace } from '../api/types'
import VirtualGrid from './VirtualGrid'
import type { TaskFeedback } from './TaskBar'

type ContentWorkspace = Extract<FolderWorkspace, { workspace: 'content' }>
type GridSize = 'small' | 'medium' | 'large'

interface ContentBrowserProps {
  workspace: ContentWorkspace
  viewportHeight?: number
  requestThumbnail?: (
    file: BrowserFile,
    maxPixels: number,
    scaleMilli: number,
  ) => Promise<string>
  onPreview?: (file: BrowserFile) => void
  onSelectionChange?: (files: BrowserFile[]) => void
  onThumbnailTaskChange?: (task: TaskFeedback | null) => void
}

interface ThumbnailWork {
  requested: number
  completed: number
  failed: number
}

const GRID_PIXELS: Record<GridSize, number> = {
  small: 132,
  medium: 180,
  large: 240,
}

export default function ContentBrowser({
  workspace,
  viewportHeight = 520,
  requestThumbnail,
  onPreview,
  onSelectionChange,
  onThumbnailTaskChange,
}: ContentBrowserProps) {
  const [gridSize, setGridSize] = useState<GridSize>('medium')
  const [selected, setSelected] = useState<Set<string>>(() => new Set())
  const [activeId, setActiveId] = useState<string | null>(null)
  const anchorId = useRef<string | null>(null)
  const cache = useRef(new Map<string, string>())
  const pending = useRef(new Map<string, Promise<string>>())
  const [work, setWork] = useState<ThumbnailWork>({ requested: 0, completed: 0, failed: 0 })
  const allFiles = useMemo(
    () => [...workspace.images, ...workspace.textFiles],
    [workspace.images, workspace.textFiles],
  )
  const fileById = useMemo(
    () => new Map(allFiles.map((file) => [file.entityId, file])),
    [allFiles],
  )

  useEffect(() => {
    const ids = new Set(allFiles.map((file) => file.entityId))
    const filtered = new Set([...selected].filter((id) => ids.has(id)))
    if (filtered.size !== selected.size) {
      setSelected(filtered)
    }
    if (filtered.size > 0 || filtered.size !== selected.size) {
      onSelectionChange?.(allFiles.filter((file) => filtered.has(file.entityId)))
    }
    if (activeId && !ids.has(activeId)) setActiveId(null)
  }, [activeId, allFiles, onSelectionChange, selected])

  useEffect(() => {
    if (onThumbnailTaskChange === undefined || work.requested === 0) {
      onThumbnailTaskChange?.(null)
      return
    }
    const settled = work.completed + work.failed
    onThumbnailTaskChange({
      id: 'visible-thumbnails',
      label: '加载可见缩略图',
      status:
        settled < work.requested ? 'running' : work.failed > 0 ? 'failed' : 'complete',
      requested: work.requested,
      completed: work.completed,
      failed: work.failed,
      cancellable: false,
      failures: [],
    })
  }, [onThumbnailTaskChange, work])

  const loadThumbnail = useCallback(
    (file: BrowserFile, maxPixels: number, scaleMilli: number) => {
      const key = `${file.entityId}:${file.modifiedNs}:${maxPixels}:${scaleMilli}`
      const existing = cache.current.get(key)
      if (existing) return Promise.resolve(existing)
      const inFlight = pending.current.get(key)
      if (inFlight) return inFlight
      if (requestThumbnail === undefined) return Promise.reject(new Error('thumbnail unavailable'))

      setWork((current) => ({ ...current, requested: current.requested + 1 }))
      const request = requestThumbnail(file, maxPixels, scaleMilli).then(
        (url) => {
          cache.current.set(key, url)
          setWork((current) => ({ ...current, completed: current.completed + 1 }))
          return url
        },
        (error: unknown) => {
          setWork((current) => ({ ...current, failed: current.failed + 1 }))
          throw error
        },
      )
      pending.current.set(key, request)
      void request.finally(() => pending.current.delete(key)).catch(() => undefined)
      return request
    },
    [requestThumbnail],
  )

  function commitSelection(next: Set<string>) {
    setSelected(next)
    onSelectionChange?.(
      allFiles.filter((file) => next.has(file.entityId)),
    )
  }

  function selectFile(file: BrowserFile, event: MouseEvent) {
    setActiveId(file.entityId)
    if (event.shiftKey && anchorId.current !== null) {
      const anchor = allFiles.findIndex((candidate) => candidate.entityId === anchorId.current)
      const target = allFiles.findIndex((candidate) => candidate.entityId === file.entityId)
      if (anchor >= 0 && target >= 0) {
        const next = new Set(selected)
        for (const candidate of allFiles.slice(Math.min(anchor, target), Math.max(anchor, target) + 1)) {
          next.add(candidate.entityId)
        }
        commitSelection(next)
        return
      }
    }
    anchorId.current = file.entityId
    if (event.metaKey) {
      const next = new Set(selected)
      if (next.has(file.entityId)) next.delete(file.entityId)
      else next.add(file.entityId)
      commitSelection(next)
    } else {
      commitSelection(new Set([file.entityId]))
    }
  }

  function handleKeyboard(event: KeyboardEvent<HTMLElement>) {
    const target = event.target as HTMLElement
    if (
      target instanceof HTMLInputElement ||
      target instanceof HTMLTextAreaElement ||
      target.isContentEditable
    ) {
      return
    }
    const activeIndex = Math.max(
      0,
      allFiles.findIndex((file) => file.entityId === activeId),
    )
    let nextIndex: number | null = null
    if (event.key === 'ArrowRight') nextIndex = activeIndex + 1
    if (event.key === 'ArrowLeft') nextIndex = activeIndex - 1
    if (event.key === 'ArrowDown') nextIndex = activeIndex + 4
    if (event.key === 'ArrowUp') nextIndex = activeIndex - 4
    if (nextIndex !== null) {
      event.preventDefault()
      const file = allFiles[Math.max(0, Math.min(allFiles.length - 1, nextIndex))]
      if (file) {
        setActiveId(file.entityId)
        anchorId.current = file.entityId
        commitSelection(new Set([file.entityId]))
      }
      return
    }
    if ((event.key === ' ' || event.key === 'Spacebar') && activeId) {
      event.preventDefault()
      const file = fileById.get(activeId)
      if (file) onPreview?.(file)
    }
  }

  const cellPixels = GRID_PIXELS[gridSize]
  const scaleMilli = Math.max(1_000, Math.round(window.devicePixelRatio * 1_000))

  return (
    <section className="content-browser" aria-label="文件内容">
      <div className="grid-toolbar">
        <label>
          缩略图大小
          <select value={gridSize} onChange={(event) => setGridSize(event.target.value as GridSize)}>
            <option value="small">小</option>
            <option value="medium">中</option>
            <option value="large">大</option>
          </select>
        </label>
        <span>{workspace.images.length} 张图片</span>
      </div>
      <VirtualGrid
        items={workspace.images}
        cellWidth={cellPixels}
        cellHeight={cellPixels + 54}
        viewportHeight={viewportHeight}
        getKey={(file) => file.entityId}
        ariaLabel="图片文件"
        activeDescendant={activeId ? `file-${activeId}` : undefined}
        onKeyDown={handleKeyboard}
        renderItem={(file) => (
          <ImageCell
            file={file}
            selected={selected.has(file.entityId)}
            active={activeId === file.entityId}
            maxPixels={cellPixels}
            scaleMilli={scaleMilli}
            loadThumbnail={loadThumbnail}
            onClick={selectFile}
            onPreview={(selectedFile) => onPreview?.(selectedFile)}
          />
        )}
      />
      <h2>文本文件</h2>
      <div
        className="text-file-list"
        role="listbox"
        aria-label="文本文件"
        tabIndex={0}
        onKeyDown={handleKeyboard}
      >
        {workspace.textFiles.map((file) => (
          <button
            type="button"
            role="option"
            id={`file-${file.entityId}`}
            aria-label={file.name}
            aria-selected={selected.has(file.entityId)}
            key={file.entityId}
            onClick={(event) => selectFile(file, event)}
            onDoubleClick={() => onPreview?.(file)}
          >
            <span className="text-file-name">{file.name}</span>
            <span className="text-file-path">{file.relativePath}</span>
            <span className="file-marker">{markerLabel(file.marker)}</span>
          </button>
        ))}
      </div>
    </section>
  )
}

function ImageCell({
  file,
  selected,
  active,
  maxPixels,
  scaleMilli,
  loadThumbnail,
  onClick,
  onPreview,
}: {
  file: BrowserFile
  selected: boolean
  active: boolean
  maxPixels: number
  scaleMilli: number
  loadThumbnail: (file: BrowserFile, maxPixels: number, scaleMilli: number) => Promise<string>
  onClick: (file: BrowserFile, event: MouseEvent) => void
  onPreview: (file: BrowserFile) => void
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
      data-active={active || undefined}
      className="image-cell"
      onClick={(event) => onClick(file, event)}
      onDoubleClick={() => onPreview(file)}
    >
      <div className="image-cell-preview">
        {url ? (
          <img src={url} alt="" />
        ) : (
          <span aria-label={failed ? '缩略图不可用' : '缩略图加载中'} />
        )}
      </div>
      <span>{file.name}</span>
      <span className="file-marker">{markerLabel(file.marker)}</span>
    </div>
  )
}

function markerLabel(marker: BrowserFile['marker']): string {
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
