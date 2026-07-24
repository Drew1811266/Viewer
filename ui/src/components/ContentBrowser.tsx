import type { DragEvent, KeyboardEvent, MouseEvent, PointerEvent } from 'react'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import type { BrowserFile, FolderWorkspace } from '../api/types'
import type { OrganizationPointerInput } from '../state/useOrganizationPointerDrag'
import type { RadialMenuRequest } from './RadialFileMenu'
import type { TaskFeedback } from './TaskBar'
import type { MarqueeSelectionChange } from './VirtualGrid'
import VirtualGrid from './VirtualGrid'

type ContentWorkspace = Extract<FolderWorkspace, { workspace: 'content' }>
type GridSize = 'small' | 'medium' | 'large'

interface ContentBrowserProps {
  workspace: ContentWorkspace
  currentPath?: string
  viewportHeight?: number
  requestThumbnail?: (file: BrowserFile, maxPixels: number, scaleMilli: number) => Promise<string>
  onPreview?: (file: BrowserFile) => void
  onSelectionChange?: (files: BrowserFile[]) => void
  onThumbnailTaskChange?: (task: TaskFeedback | null) => void
  organizationDragDisabled?: boolean
  onFinderDragStart?: (entityIds: string[]) => void
  onOrganizationPointerInput?: (input: OrganizationPointerInput) => void
  repairSelectionId?: string | null
  onRepairSelectionApplied?: () => void
  onRadialMenuRequest?: (request: RadialMenuRequest) => void
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
  currentPath,
  viewportHeight = 520,
  requestThumbnail,
  onPreview,
  onSelectionChange,
  onThumbnailTaskChange,
  organizationDragDisabled = false,
  onFinderDragStart,
  onOrganizationPointerInput,
  repairSelectionId = null,
  onRepairSelectionApplied,
  onRadialMenuRequest,
}: ContentBrowserProps) {
  const [gridSize, setGridSize] = useState<GridSize>('medium')
  const [selected, setSelected] = useState<Set<string>>(() => new Set())
  const [activeId, setActiveId] = useState<string | null>(null)
  const anchorId = useRef<string | null>(null)
  const appliedRepairId = useRef<string | null>(null)
  const marqueeSelection = useRef<{
    baseline: Set<string>
    metaKey: boolean
  } | null>(null)
  const radialContextDeduplication = useRef<{
    entityId: string
    timeoutId: number
  } | null>(null)
  const cache = useRef(new Map<string, string>())
  const pending = useRef(new Map<string, Promise<string>>())
  const [work, setWork] = useState<ThumbnailWork>({ requested: 0, completed: 0, failed: 0 })
  const allFiles = useMemo(
    () => [...workspace.images, ...workspace.textFiles],
    [workspace.images, workspace.textFiles],
  )
  const fileById = useMemo(() => new Map(allFiles.map((file) => [file.entityId, file])), [allFiles])

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

  useEffect(
    () => () => {
      if (radialContextDeduplication.current !== null) {
        window.clearTimeout(radialContextDeduplication.current.timeoutId)
      }
    },
    [],
  )

  useEffect(() => {
    if (repairSelectionId === null) {
      appliedRepairId.current = null
      return
    }
    if (appliedRepairId.current === repairSelectionId || !fileById.has(repairSelectionId)) {
      return
    }
    appliedRepairId.current = repairSelectionId
    const repaired = new Set([repairSelectionId])
    setSelected(repaired)
    setActiveId(repairSelectionId)
    anchorId.current = repairSelectionId
    onSelectionChange?.(allFiles.filter((file) => repaired.has(file.entityId)))
    onRepairSelectionApplied?.()
  }, [allFiles, fileById, onRepairSelectionApplied, onSelectionChange, repairSelectionId])

  useEffect(() => {
    if (onThumbnailTaskChange === undefined || work.requested === 0) {
      onThumbnailTaskChange?.(null)
      return
    }
    const settled = work.completed + work.failed
    onThumbnailTaskChange({
      id: 'visible-thumbnails',
      label: '加载可见缩略图',
      status: settled < work.requested ? 'running' : work.failed > 0 ? 'failed' : 'complete',
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
    onSelectionChange?.(allFiles.filter((file) => next.has(file.entityId)))
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
        for (const candidate of allFiles.slice(
          Math.min(anchor, target),
          Math.max(anchor, target) + 1,
        )) {
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

  function requestRadialMenu(
    file: BrowserFile,
    eventTarget: HTMLElement,
    origin: { x: number; y: number },
    pointerId: number | null,
  ) {
    if (onRadialMenuRequest === undefined) return
    const returnFocusTarget = eventTarget.closest<HTMLElement>('[role="listbox"]') ?? eventTarget
    const contextSelection = selected.has(file.entityId) ? selected : new Set([file.entityId])
    if (!selected.has(file.entityId)) {
      anchorId.current = file.entityId
      setActiveId(file.entityId)
      commitSelection(contextSelection)
    }
    onRadialMenuRequest({
      files: allFiles.filter((candidate) => contextSelection.has(candidate.entityId)),
      origin,
      pointerId,
      returnFocusTarget,
    })
  }

  function clearRadialContextDeduplication() {
    if (radialContextDeduplication.current === null) return
    window.clearTimeout(radialContextDeduplication.current.timeoutId)
    radialContextDeduplication.current = null
  }

  function openRadialMenuFromPointer(file: BrowserFile, event: PointerEvent<HTMLElement>) {
    if (event.button !== 2 || onRadialMenuRequest === undefined) return
    event.preventDefault()
    event.stopPropagation()
    clearRadialContextDeduplication()
    const timeoutId = window.setTimeout(() => {
      if (radialContextDeduplication.current?.timeoutId === timeoutId) {
        radialContextDeduplication.current = null
      }
    }, 1_000)
    radialContextDeduplication.current = { entityId: file.entityId, timeoutId }
    requestRadialMenu(
      file,
      event.currentTarget,
      { x: event.clientX, y: event.clientY },
      event.pointerId,
    )
  }

  function openRadialMenuFromContext(file: BrowserFile, event: MouseEvent<HTMLElement>) {
    event.preventDefault()
    event.stopPropagation()
    if (onRadialMenuRequest === undefined) return
    const alreadyHandled = radialContextDeduplication.current?.entityId === file.entityId
    clearRadialContextDeduplication()
    if (alreadyHandled) return
    requestRadialMenu(file, event.currentTarget, { x: event.clientX, y: event.clientY }, null)
  }

  function startFinderDrag(file: BrowserFile, event: DragEvent<HTMLElement>) {
    const entityIds = freezeDragSelection(file)
    event.preventDefault()
    event.stopPropagation()
    if (entityIds.length > 0) onFinderDragStart?.(entityIds)
  }

  function startPointerOrganization(file: BrowserFile, event: PointerEvent<HTMLElement>) {
    if (event.button !== 0 || event.ctrlKey) return
    event.preventDefault()
    event.stopPropagation()
    if (organizationDragDisabled) return
    const entityIds = freezeDragSelection(file)
    if (entityIds.length === 0) return
    onOrganizationPointerInput?.({
      type: 'start',
      pointerId: event.pointerId,
      entityIds,
      mode: event.altKey ? 'copy' : 'move',
      clientX: event.clientX,
      clientY: event.clientY,
      captureNode: event.currentTarget,
    })
  }

  function movePointerOrganization(event: PointerEvent<HTMLElement>) {
    event.preventDefault()
    event.stopPropagation()
    onOrganizationPointerInput?.({
      type: 'move',
      pointerId: event.pointerId,
      clientX: event.clientX,
      clientY: event.clientY,
    })
  }

  function endPointerOrganization(event: PointerEvent<HTMLElement>) {
    event.preventDefault()
    event.stopPropagation()
    onOrganizationPointerInput?.({
      type: 'end',
      pointerId: event.pointerId,
      clientX: event.clientX,
      clientY: event.clientY,
    })
  }

  function cancelPointerOrganization(event: PointerEvent<HTMLElement>) {
    event.preventDefault()
    event.stopPropagation()
    onOrganizationPointerInput?.({ type: 'cancel', pointerId: event.pointerId })
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
          const anchor = allFiles.findIndex((candidate) => candidate.entityId === anchorId.current)
          const target = allFiles.findIndex((candidate) => candidate.entityId === file.entityId)
          const next = new Set(selected)
          for (const candidate of allFiles.slice(
            Math.min(anchor, target),
            Math.max(anchor, target) + 1,
          )) {
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
      <div className="content-toolbar">
        <div>
          <strong>{currentPath ?? '当前文件夹'}</strong>
          <span>· {workspace.images.length} 张图片</span>
          {workspace.textFiles.length > 0 && <span>· {workspace.textFiles.length} 个文本文件</span>}
        </div>
        <details className="content-view-menu">
          <summary>视图</summary>
          <div>
            <label>
              缩略图大小
              <select
                value={gridSize}
                onChange={(event) => setGridSize(event.target.value as GridSize)}
              >
                <option value="small">小</option>
                <option value="medium">中</option>
                <option value="large">大</option>
              </select>
            </label>
            <button type="button" onClick={selectAllFiles} disabled={allFiles.length === 0}>
              全选当前文件夹
            </button>
          </div>
        </details>
      </div>
      <VirtualGrid
        items={workspace.images}
        cellWidth={cellPixels}
        cellHeight={cellPixels + 42}
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
            onRadialMenuPointerDown={openRadialMenuFromPointer}
            onRadialMenuContextMenu={openRadialMenuFromContext}
            organizationDragDisabled={organizationDragDisabled}
            onFinderDragStart={startFinderDrag}
            onPointerDown={startPointerOrganization}
            onPointerMove={movePointerOrganization}
            onPointerUp={endPointerOrganization}
            onPointerCancel={cancelPointerOrganization}
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
            onPointerDown={(event) => openRadialMenuFromPointer(file, event)}
            onContextMenu={(event) => openRadialMenuFromContext(file, event)}
            onClick={(event) => selectFile(file, event)}
            onDoubleClick={() => onPreview?.(file)}
          >
            <div
              className="file-export-surface"
              draggable
              title="拖到 Finder"
              onDragStart={(event) => startFinderDrag(file, event)}
            >
              <span className="text-file-name">{file.name}</span>
              <span className="text-file-path">{file.relativePath}</span>
              {markerLabel(file.marker) && (
                <span className="file-marker">{markerLabel(file.marker)}</span>
              )}
            </div>
            <OrganizationDragHandle
              file={file}
              disabled={organizationDragDisabled}
              onPointerDown={startPointerOrganization}
              onPointerMove={movePointerOrganization}
              onPointerUp={endPointerOrganization}
              onPointerCancel={cancelPointerOrganization}
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
        {markerLabel(file.marker) && (
          <span className="file-marker">{markerLabel(file.marker)}</span>
        )}
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

function OrganizationDragHandle({
  file,
  disabled,
  onPointerDown,
  onPointerMove,
  onPointerUp,
  onPointerCancel,
}: {
  file: BrowserFile
  disabled: boolean
  onPointerDown: (file: BrowserFile, event: PointerEvent<HTMLElement>) => void
  onPointerMove: (event: PointerEvent<HTMLElement>) => void
  onPointerUp: (event: PointerEvent<HTMLElement>) => void
  onPointerCancel: (event: PointerEvent<HTMLElement>) => void
}) {
  const organizationPointerId = useRef<number | null>(null)

  return (
    <button
      type="button"
      className="organization-drag-handle"
      aria-label={`整理 ${file.name}`}
      title="拖到左侧文件夹，按住 Option 复制"
      disabled={disabled}
      draggable={false}
      onClick={(event) => {
        event.preventDefault()
        event.stopPropagation()
      }}
      onDoubleClick={(event) => {
        event.preventDefault()
        event.stopPropagation()
      }}
      onDragStart={(event) => {
        event.preventDefault()
        event.stopPropagation()
      }}
      onPointerDown={(event) => {
        if (event.button === 0 && !event.ctrlKey) {
          organizationPointerId.current = event.pointerId
        }
        onPointerDown(file, event)
      }}
      onPointerMove={(event) => {
        if (organizationPointerId.current === event.pointerId) onPointerMove(event)
      }}
      onPointerUp={(event) => {
        if (organizationPointerId.current !== event.pointerId) return
        organizationPointerId.current = null
        onPointerUp(event)
      }}
      onPointerCancel={(event) => {
        if (organizationPointerId.current !== event.pointerId) return
        organizationPointerId.current = null
        onPointerCancel(event)
      }}
    >
      ⋮⋮
    </button>
  )
}

function markerLabel(marker: BrowserFile['marker']): string | null {
  const review =
    marker.reviewState === 'keep'
      ? '保留'
      : marker.reviewState === 'pending'
        ? '待定'
        : marker.reviewState === 'reject'
          ? '淘汰'
          : null
  if (review === null && !marker.favorite) return null
  if (review === null) return '收藏'
  return marker.favorite ? `${review} · 收藏` : review
}
