import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import type { DragEvent, KeyboardEvent, MouseEvent } from 'react'
import type { BrowserFile, FolderWorkspace } from '../api/types'
import VirtualGrid from './VirtualGrid'
import type { MarqueeSelectionChange } from './VirtualGrid'
import type { TaskFeedback } from './TaskBar'

type ContentWorkspace = Extract<FolderWorkspace, { workspace: 'content' }>
type GridSize = 'small' | 'medium' | 'large'
type InternalDragMode = 'move' | 'copy'

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
  organizationDragDisabled?: boolean
  onFinderDragStart?: (entityIds: string[]) => void
  onOrganizationDragStart?: (entityIds: string[], mode: InternalDragMode) => void
  onOrganizationDragEnd?: () => void
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

export const VIEWER_SELECTION_MIME = 'application/x-viewer-selection'

export default function ContentBrowser({
  workspace,
  viewportHeight = 520,
  requestThumbnail,
  onPreview,
  onSelectionChange,
  onThumbnailTaskChange,
  organizationDragDisabled = false,
  onFinderDragStart,
  onOrganizationDragStart,
  onOrganizationDragEnd,
}: ContentBrowserProps) {
  const [gridSize, setGridSize] = useState<GridSize>('medium')
  const [selected, setSelected] = useState<Set<string>>(() => new Set())
  const [activeId, setActiveId] = useState<string | null>(null)
  const anchorId = useRef<string | null>(null)
  const marqueeSelection = useRef<{
    baseline: Set<string>
    metaKey: boolean
  } | null>(null)
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
    marqueeSelection.current = null
  }, [allFiles])

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

  function updateMarqueeSelection(change: MarqueeSelectionChange) {
    if (change.phase === 'start') {
      marqueeSelection.current = { baseline: new Set(selected), metaKey: change.metaKey }
      return
    }
    const session = marqueeSelection.current
    if (session === null) return
    if (change.phase === 'cancel') {
      commitSelection(new Set(session.baseline))
      marqueeSelection.current = null
      return
    }
    const hits = new Set(change.keys)
    const next = session.metaKey ? new Set(session.baseline) : new Set<string>()
    if (session.metaKey) {
      for (const id of hits) {
        if (next.has(id)) next.delete(id)
        else next.add(id)
      }
    } else {
      for (const id of hits) next.add(id)
    }
    commitSelection(next)
    if (change.phase === 'end') {
      const first = workspace.images.find((file) => hits.has(file.entityId))
      setActiveId(first?.entityId ?? null)
      anchorId.current = first?.entityId ?? null
      marqueeSelection.current = null
    }
  }

  function selectAllFiles() {
    const first = allFiles[0]
    if (first && activeId === null) {
      setActiveId(first.entityId)
      anchorId.current = first.entityId
    }
    commitSelection(new Set(allFiles.map((file) => file.entityId)))
  }

  function selectFile(file: BrowserFile, event: MouseEvent) {
    event.currentTarget.closest<HTMLElement>('[role="listbox"]')?.focus()
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

  function freezeDragSelection(file: BrowserFile): string[] {
    const dragSelection = selected.has(file.entityId) ? selected : new Set([file.entityId])
    if (!selected.has(file.entityId)) {
      anchorId.current = file.entityId
      commitSelection(dragSelection)
    }
    return allFiles
      .filter((candidate) => dragSelection.has(candidate.entityId))
      .map((candidate) => candidate.entityId)
  }

  function startFinderDrag(file: BrowserFile, event: DragEvent<HTMLElement>) {
    const entityIds = freezeDragSelection(file)
    event.preventDefault()
    event.stopPropagation()
    if (entityIds.length > 0) onFinderDragStart?.(entityIds)
  }

  function startOrganizationDrag(file: BrowserFile, event: DragEvent<HTMLElement>) {
    event.stopPropagation()
    if (organizationDragDisabled) {
      event.preventDefault()
      return
    }
    const entityIds = freezeDragSelection(file)
    if (entityIds.length === 0) {
      event.preventDefault()
      return
    }
    event.dataTransfer.effectAllowed = 'copyMove'
    event.dataTransfer.setData(VIEWER_SELECTION_MIME, 'viewer-selection')
    onOrganizationDragStart?.(entityIds, event.altKey ? 'copy' : 'move')
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
    if (event.metaKey && event.key.toLowerCase() === 'a') {
      event.preventDefault()
      selectAllFiles()
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
        if (event.shiftKey && anchorId.current !== null) {
          const anchor = allFiles.findIndex(
            (candidate) => candidate.entityId === anchorId.current,
          )
          const target = allFiles.findIndex(
            (candidate) => candidate.entityId === file.entityId,
          )
          const next = new Set(selected)
          for (const candidate of allFiles.slice(Math.min(anchor, target), Math.max(anchor, target) + 1)) {
            next.add(candidate.entityId)
          }
          commitSelection(next)
        } else {
          anchorId.current = file.entityId
          commitSelection(new Set([file.entityId]))
        }
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
        <button type="button" onClick={selectAllFiles} disabled={allFiles.length === 0}>
          全选当前文件夹
        </button>
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
        ariaMultiselectable
        onMarqueeSelectionChange={updateMarqueeSelection}
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
            organizationDragDisabled={organizationDragDisabled}
            onFinderDragStart={startFinderDrag}
            onOrganizationDragStart={startOrganizationDrag}
            onOrganizationDragEnd={onOrganizationDragEnd}
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
          <div
            role="option"
            id={`file-${file.entityId}`}
            aria-label={file.name}
            aria-selected={selected.has(file.entityId)}
            tabIndex={-1}
            key={file.entityId}
            className="text-file-row"
            draggable
            onClick={(event) => selectFile(file, event)}
            onDoubleClick={() => onPreview?.(file)}
            onDragStart={(event) => startFinderDrag(file, event)}
          >
            <span className="text-file-name">{file.name}</span>
            <span className="text-file-path">{file.relativePath}</span>
            <span className="file-marker">{markerLabel(file.marker)}</span>
            <OrganizationDragHandle
              file={file}
              disabled={organizationDragDisabled}
              onDragStart={startOrganizationDrag}
              onDragEnd={onOrganizationDragEnd}
            />
          </div>
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
  organizationDragDisabled,
  onFinderDragStart,
  onOrganizationDragStart,
  onOrganizationDragEnd,
}: {
  file: BrowserFile
  selected: boolean
  active: boolean
  maxPixels: number
  scaleMilli: number
  loadThumbnail: (file: BrowserFile, maxPixels: number, scaleMilli: number) => Promise<string>
  onClick: (file: BrowserFile, event: MouseEvent) => void
  onPreview: (file: BrowserFile) => void
  organizationDragDisabled: boolean
  onFinderDragStart: (file: BrowserFile, event: DragEvent<HTMLElement>) => void
  onOrganizationDragStart: (file: BrowserFile, event: DragEvent<HTMLElement>) => void
  onOrganizationDragEnd?: () => void
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
      draggable
      onClick={(event) => onClick(file, event)}
      onDoubleClick={() => onPreview(file)}
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
      <span className="file-marker">{markerLabel(file.marker)}</span>
      <OrganizationDragHandle
        file={file}
        disabled={organizationDragDisabled}
        onDragStart={onOrganizationDragStart}
        onDragEnd={onOrganizationDragEnd}
      />
    </div>
  )
}

function OrganizationDragHandle({
  file,
  disabled,
  onDragStart,
  onDragEnd,
}: {
  file: BrowserFile
  disabled: boolean
  onDragStart: (file: BrowserFile, event: DragEvent<HTMLElement>) => void
  onDragEnd?: () => void
}) {
  return (
    <button
      type="button"
      className="organization-drag-handle"
      aria-label={`整理 ${file.name}`}
      title="拖到左侧文件夹，按住 Option 复制"
      disabled={disabled}
      draggable={!disabled}
      onClick={(event) => event.stopPropagation()}
      onDragStart={(event) => onDragStart(file, event)}
      onDragEnd={() => onDragEnd?.()}
    >
      ⋮⋮
    </button>
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
