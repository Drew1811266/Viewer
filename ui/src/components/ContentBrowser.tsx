import type { DragEvent, KeyboardEvent, MouseEvent, PointerEvent } from 'react'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { flushSync } from 'react-dom'
import type { BrowserFile, FolderWorkspace, ThumbnailDensity } from '../api/types'
import { type AspectRect, type ImageDimensions, validDimensions } from '../layout/aspectLayout'
import { THUMBNAIL_HEIGHT } from '../settings/thumbnailDensity'
import type { OrganizationPointerInput } from '../state/useOrganizationPointerDrag'
import AspectVirtualGrid from './AspectVirtualGrid'
import {
  filesForSelectAllScope,
  resolveAdaptiveContentMode,
  resolveSelectAllRequest,
  type SelectAllScope,
} from './contentBrowser/adaptiveTextPanelModel'
import { rangeSelection, toggleSelection } from './contentBrowser/contentSelection'
import { ImageCell } from './contentBrowser/ImageCell'
import SelectAllChoicePanel from './contentBrowser/SelectAllChoicePanel'
import TextFilePanel from './contentBrowser/TextFilePanel'
import { useMeasuredElementHeight } from './contentBrowser/useMeasuredElementHeight'
import type { MarqueeSelectionChange } from './marqueeSelection'
import type { RadialMenuRequest } from './RadialFileMenu'
import type { TaskFeedback } from './TaskBar'

type ContentWorkspace = Extract<FolderWorkspace, { workspace: 'content' }>

interface ContentBrowserProps {
  workspace: ContentWorkspace
  density: ThumbnailDensity
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
  textPanelExpanded: boolean
  onTextPanelExpandedChange(expanded: boolean): void
}

interface ThumbnailWork {
  requested: number
  completed: number
  failed: number
}

export default function ContentBrowser({
  workspace,
  density,
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
  textPanelExpanded,
  onTextPanelExpandedChange,
}: ContentBrowserProps) {
  const [selected, setSelected] = useState<Set<string>>(() => new Set())
  const [activeId, setActiveId] = useState<string | null>(null)
  const [recoveredDimensions, setRecoveredDimensions] = useState(
    () => new Map<string, ImageDimensions>(),
  )
  const [selectAllChoiceOpen, setSelectAllChoiceOpen] = useState(false)
  const anchorId = useRef<string | null>(null)
  const appliedRepairId = useRef<string | null>(null)
  const viewMenuRef = useRef<HTMLDetailsElement>(null)
  const selectAllButtonRef = useRef<HTMLButtonElement>(null)
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
  const mode = resolveAdaptiveContentMode(
    workspace.images.length,
    workspace.textFiles.length,
    textPanelExpanded,
  )
  const selectAllRequest = resolveSelectAllRequest(
    workspace.images.length,
    workspace.textFiles.length,
  )
  const contentIdentity = useMemo(
    () =>
      JSON.stringify(
        [...workspace.images, ...workspace.textFiles].map(({ entityId, modifiedNs }) => [
          entityId,
          modifiedNs,
        ]),
      ),
    [workspace.images, workspace.textFiles],
  )
  const imageSlot = useMeasuredElementHeight(viewportHeight)
  const fileById = useMemo(() => new Map(allFiles.map((file) => [file.entityId, file])), [allFiles])
  const activeImageId = workspace.images.some(({ entityId }) => entityId === activeId)
    ? activeId
    : null
  const activeTextId = workspace.textFiles.some(({ entityId }) => entityId === activeId)
    ? activeId
    : null

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
    setSelectAllChoiceOpen(false)
  }, [contentIdentity, currentPath])

  useEffect(() => {
    if (organizationDragDisabled) setSelectAllChoiceOpen(false)
  }, [organizationDragDisabled])

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
  const dimensionsForImage = useCallback(
    (file: BrowserFile) => dimensionsFor(file, recoveredDimensions),
    [recoveredDimensions],
  )
  const rememberNaturalDimensions = useCallback(
    (identity: { entityId: string; modifiedNs: string }, dimensions: ImageDimensions) => {
      if (!validDimensions(dimensions)) return
      const key = `${identity.entityId}:${identity.modifiedNs}`
      setRecoveredDimensions((current) => {
        const existing = current.get(key)
        if (existing?.width === dimensions.width && existing.height === dimensions.height) {
          return current
        }
        const next = new Map(current)
        next.set(key, dimensions)
        return next
      })
    },
    [],
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

  function commitSelectAll(scope: SelectAllScope) {
    setSelectAllChoiceOpen(false)
    const files = filesForSelectAllScope(workspace, scope)
    const first = files[0] ?? null
    setActiveId(first?.entityId ?? null)
    anchorId.current = first?.entityId ?? null
    commitSelection(new Set(files.map(({ entityId }) => entityId)))
  }

  function requestSelectAll() {
    if (selectAllRequest.kind === 'none') return
    if (selectAllRequest.kind === 'direct') {
      commitSelectAll(selectAllRequest.scope)
      return
    }
    if (viewMenuRef.current !== null) viewMenuRef.current.open = true
    setSelectAllChoiceOpen(true)
  }

  function previewFile(file: BrowserFile) {
    closeSelectAllChoiceBeforeCallback()
    onPreview?.(file)
  }

  function closeSelectAllChoiceBeforeCallback() {
    if (!selectAllChoiceOpen) return
    flushSync(() => setSelectAllChoiceOpen(false))
  }

  function navigateToIndex(nextIndex: number, extendSelection: boolean) {
    const file = workspace.images[nextIndex]
    if (file === undefined) return
    setActiveId(file.entityId)
    if (extendSelection && anchorId.current !== null) {
      const range = rangeSelection(
        workspace.images.map((candidate) => candidate.entityId),
        anchorId.current,
        file.entityId,
      )
      commitSelection(new Set([...selected, ...range]))
      return
    }
    anchorId.current = file.entityId
    commitSelection(new Set([file.entityId]))
  }

  function selectFile(file: BrowserFile, event: MouseEvent) {
    event.currentTarget.closest<HTMLElement>('[role="listbox"]')?.focus()
    setActiveId(file.entityId)
    if (event.shiftKey && anchorId.current !== null) {
      const orderedEntityIds = allFiles.map((candidate) => candidate.entityId)
      const anchorExists = orderedEntityIds.includes(anchorId.current)
      const range = rangeSelection(orderedEntityIds, anchorId.current, file.entityId)
      commitSelection(new Set([...selected, ...range]))
      if (!anchorExists) anchorId.current = file.entityId
      return
    }
    anchorId.current = file.entityId
    if (event.metaKey) {
      commitSelection(new Set(toggleSelection([...selected], file.entityId)))
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
    closeSelectAllChoiceBeforeCallback()
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
    if (isEditableKeyboardTarget(target)) return
    if ((event.key === ' ' || event.key === 'Spacebar') && activeId) {
      event.preventDefault()
      const file = fileById.get(activeId)
      if (file) previewFile(file)
    }
  }

  function handleContentBrowserKeyboard(event: KeyboardEvent<HTMLElement>) {
    const target = event.target as HTMLElement
    if (isEditableKeyboardTarget(target)) return
    if (event.metaKey && event.key.toLowerCase() === 'a') {
      event.preventDefault()
      requestSelectAll()
    }
  }

  function handleTextListKeyboard(event: KeyboardEvent<HTMLElement>) {
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
        if (event.shiftKey && anchorId.current !== null) {
          const range = rangeSelection(
            allFiles.map((candidate) => candidate.entityId),
            anchorId.current,
            file.entityId,
          )
          commitSelection(new Set([...selected, ...range]))
        } else {
          anchorId.current = file.entityId
          commitSelection(new Set([file.entityId]))
        }
      }
      return
    }
    handleKeyboard(event)
  }

  return (
    <section
      className="content-browser"
      data-content-mode={mode}
      aria-label="文件内容"
      onKeyDownCapture={handleContentBrowserKeyboard}
    >
      <div className="content-toolbar">
        <div>
          <strong>{currentPath ?? '当前文件夹'}</strong>
          <span>· {workspace.images.length} 张图片</span>
          {workspace.textFiles.length > 0 && <span>· {workspace.textFiles.length} 个文本文件</span>}
        </div>
        <details
          ref={viewMenuRef}
          className="content-view-menu"
          onToggle={(event) => {
            if (!event.currentTarget.open) setSelectAllChoiceOpen(false)
          }}
        >
          <summary>视图</summary>
          <div>
            <div className="select-all-control">
              <button
                ref={selectAllButtonRef}
                type="button"
                aria-haspopup={selectAllRequest.kind === 'choice' ? 'menu' : undefined}
                aria-expanded={selectAllChoiceOpen || undefined}
                onClick={() => {
                  if (selectAllChoiceOpen) {
                    setSelectAllChoiceOpen(false)
                    selectAllButtonRef.current?.focus()
                  } else {
                    requestSelectAll()
                  }
                }}
                disabled={selectAllRequest.kind === 'none'}
              >
                全选当前文件夹
              </button>
              <SelectAllChoicePanel
                open={selectAllChoiceOpen}
                anchorRef={selectAllButtonRef}
                onChoose={commitSelectAll}
                onCancel={() => setSelectAllChoiceOpen(false)}
              />
            </div>
          </div>
        </details>
      </div>
      <div className="content-browser-body">
        {workspace.images.length > 0 && (
          <div
            ref={imageSlot.ref}
            className="content-browser-image-slot"
            data-testid="content-image-slot"
          >
            <AspectVirtualGrid
              items={workspace.images}
              imageHeight={THUMBNAIL_HEIGHT[density]}
              viewportHeight={imageSlot.height}
              getKey={imageEntityId}
              getDimensions={dimensionsForImage}
              ariaLabel="图片文件"
              activeKey={activeImageId ?? undefined}
              activeDescendant={activeImageId ? `file-${activeImageId}` : undefined}
              onNavigate={navigateToIndex}
              onKeyDown={handleKeyboard}
              ariaMultiselectable
              onMarqueeSelectionChange={updateMarqueeSelection}
              renderItem={(file, _index, rect: AspectRect) => (
                <ImageCell
                  file={file}
                  rect={rect}
                  dimensionsKnown={validDimensions(dimensionsForImage(file))}
                  selected={selected.has(file.entityId)}
                  active={activeId === file.entityId}
                  loadThumbnail={loadThumbnail}
                  onNaturalDimensions={rememberNaturalDimensions}
                  markerLabel={markerLabel(file.marker)}
                  onClick={selectFile}
                  onPreview={previewFile}
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
          </div>
        )}
        {mode !== 'image_only' && mode !== 'empty' && (
          <TextFilePanel
            mode={mode}
            files={workspace.textFiles}
            selectedIds={selected}
            activeId={activeTextId}
            organizationDragDisabled={organizationDragDisabled}
            onExpandedChange={onTextPanelExpandedChange}
            onListKeyDown={handleTextListKeyboard}
            onSelect={selectFile}
            onPreview={previewFile}
            onRadialMenuPointerDown={(file, event) => openRadialMenuFromPointer(file, event)}
            onRadialMenuContextMenu={(file, event) => openRadialMenuFromContext(file, event)}
            onFinderDragStart={startFinderDrag}
            onOrganizationPointerDown={startPointerOrganization}
            onOrganizationPointerMove={movePointerOrganization}
            onOrganizationPointerUp={endPointerOrganization}
            onOrganizationPointerCancel={cancelPointerOrganization}
          />
        )}
      </div>
    </section>
  )
}

function isEditableKeyboardTarget(target: HTMLElement): boolean {
  return (
    target instanceof HTMLInputElement ||
    target instanceof HTMLTextAreaElement ||
    target instanceof HTMLSelectElement ||
    target.isContentEditable
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

function imageIdentity(file: BrowserFile): string {
  return `${file.entityId}:${file.modifiedNs}`
}

function imageEntityId(file: BrowserFile): string {
  return file.entityId
}

function dimensionsFor(
  file: BrowserFile,
  recoveredDimensions: ReadonlyMap<string, ImageDimensions>,
): ImageDimensions | null {
  if (validDimensions(file.imageMetadata)) return file.imageMetadata
  const recovered = recoveredDimensions.get(imageIdentity(file))
  if (recovered !== undefined && validDimensions(recovered)) return recovered
  return null
}
