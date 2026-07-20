import { useEffect, useMemo, useRef, useState } from 'react'
import type { DragEvent } from 'react'
import type { FolderTreeItem } from '../api/types'
import VirtualList from './VirtualList'

interface FolderTreeProps {
  folders: FolderTreeItem[]
  selectedId: string | null
  onSelect: (entityId: string) => void
  height?: number
  draggedEntityIds?: string[]
  internalDragMode?: 'move' | 'copy' | null
  invalidDropTargetIds?: string[]
  readOnly?: boolean
  isDropTargetValid?: (folderId: string, mode: 'move' | 'copy') => boolean
  onDropFiles?: (entityIds: string[], destinationId: string, mode: 'move' | 'copy') => void
}

interface VisibleFolder {
  folder: FolderTreeItem
  depth: number
  hasChildren: boolean
}

export default function FolderTree({
  folders,
  selectedId,
  onSelect,
  height = 420,
  draggedEntityIds = [],
  internalDragMode = null,
  invalidDropTargetIds = [],
  readOnly = false,
  isDropTargetValid,
  onDropFiles,
}: FolderTreeProps) {
  const safeFolders = useMemo(
    () => folders.filter((folder) => !isReserved(folder.relativePath)),
    [folders],
  )
  const childIds = useMemo(() => parentIds(safeFolders), [safeFolders])
  const [expanded, setExpanded] = useState(() => new Set(childIds))
  const knownExpandable = useRef(new Set(childIds))
  const [dropTarget, setDropTarget] = useState<{
    entityId: string
    mode: 'move' | 'copy'
    valid: boolean
  } | null>(null)
  const invalidTargets = useMemo(() => new Set(invalidDropTargetIds), [invalidDropTargetIds])

  useEffect(() => {
    const added = childIds.filter((id) => !knownExpandable.current.has(id))
    knownExpandable.current = new Set(childIds)
    if (added.length > 0) {
      setExpanded((current) => new Set([...current, ...added]))
    }
  }, [childIds])

  useEffect(() => {
    setDropTarget((current) => {
      if (current === null) return null
      const stillVisible = safeFolders.some((folder) => folder.entityId === current.entityId)
      return draggedEntityIds.length === 0 || internalDragMode === null || readOnly || !stillVisible
        ? null
        : current
    })
  }, [draggedEntityIds.length, internalDragMode, readOnly, safeFolders])

  const visible = useMemo(
    () => flattenFolders(safeFolders, expanded),
    [expanded, safeFolders],
  )

  function toggle(entityId: string) {
    setExpanded((current) => {
      const next = new Set(current)
      if (next.has(entityId)) next.delete(entityId)
      else next.add(entityId)
      return next
    })
  }

  function acceptsViewerDrag(event: DragEvent<HTMLElement>): boolean {
    return (
      internalDragMode !== null &&
      Array.from(event.dataTransfer.types).includes('application/x-viewer-selection')
    )
  }

  function dragOver(folderId: string, event: DragEvent<HTMLElement>) {
    if (!acceptsViewerDrag(event) || draggedEntityIds.length === 0) return
    const mode = internalDragMode
    if (mode === null) return
    const valid =
      !readOnly &&
      !invalidTargets.has(folderId) &&
      (isDropTargetValid?.(folderId, mode) ?? true)
    event.preventDefault()
    event.dataTransfer.dropEffect = valid ? mode : 'none'
    setDropTarget({ entityId: folderId, mode, valid })
  }

  function drop(folderId: string, event: DragEvent<HTMLElement>) {
    if (!acceptsViewerDrag(event) || draggedEntityIds.length === 0) return
    event.preventDefault()
    const mode = internalDragMode
    if (mode === null) return
    const valid =
      !readOnly &&
      !invalidTargets.has(folderId) &&
      (isDropTargetValid?.(folderId, mode) ?? true)
    if (!valid) {
      setDropTarget({ entityId: folderId, mode, valid: false })
      return
    }
    setDropTarget(null)
    onDropFiles?.([...draggedEntityIds], folderId, mode)
  }

  return (
    <div className="folder-tree" role="tree" aria-label="项目文件夹">
      <VirtualList
        items={visible}
        rowHeight={28}
        height={height}
        overscan={6}
        getKey={(item) => item.folder.entityId}
        renderItem={({ folder, depth, hasChildren }) => (
          <div
            className="folder-tree-row"
            role="treeitem"
            aria-label={folder.relativePath}
            aria-level={depth + 1}
            aria-selected={folder.entityId === selectedId}
            aria-expanded={hasChildren ? expanded.has(folder.entityId) : undefined}
            data-drop-mode={
              dropTarget?.entityId === folder.entityId && dropTarget.valid
                ? dropTarget.mode
                : undefined
            }
            data-drop-invalid={
              dropTarget?.entityId === folder.entityId && !dropTarget.valid ? true : undefined
            }
            style={{ paddingInlineStart: depth * 16 }}
            onClick={() => onSelect(folder.entityId)}
            onDragOver={(event) => dragOver(folder.entityId, event)}
            onDragLeave={(event) => {
              if (!event.currentTarget.contains(event.relatedTarget as Node | null)) {
                setDropTarget((current) =>
                  current?.entityId === folder.entityId ? null : current,
                )
              }
            }}
            onDrop={(event) => drop(folder.entityId, event)}
          >
            {hasChildren ? (
              <button
                type="button"
                className="folder-disclosure"
                aria-label={`${expanded.has(folder.entityId) ? '折叠' : '展开'} ${folder.relativePath}`}
                onClick={(event) => {
                  event.stopPropagation()
                  toggle(folder.entityId)
                }}
              >
                {expanded.has(folder.entityId) ? '▾' : '▸'}
              </button>
            ) : (
              <span className="folder-disclosure-placeholder" aria-hidden="true" />
            )}
            <span className="folder-name">{folder.name}</span>
            <span
              className="folder-marker-badge"
              aria-label={folderMarkerAriaLabel(folder)}
            >
              {folderMarkerLabel(folder)}
            </span>
            {folder.relativePath.includes('/') && (
              <span className="folder-path">{folder.relativePath}</span>
            )}
          </div>
        )}
      />
    </div>
  )
}

function parentIds(folders: FolderTreeItem[]): string[] {
  const parents = new Set(
    folders.flatMap((folder) => (folder.parentEntityId ? [folder.parentEntityId] : [])),
  )
  return folders
    .filter((folder) => parents.has(folder.entityId))
    .map((folder) => folder.entityId)
}

function flattenFolders(
  folders: FolderTreeItem[],
  expanded: Set<string>,
): VisibleFolder[] {
  const ids = new Set(folders.map((folder) => folder.entityId))
  const children = new Map<string | null, FolderTreeItem[]>()
  for (const folder of folders) {
    const parent = folder.parentEntityId && ids.has(folder.parentEntityId)
      ? folder.parentEntityId
      : null
    const siblings = children.get(parent) ?? []
    siblings.push(folder)
    children.set(parent, siblings)
  }
  for (const siblings of children.values()) {
    siblings.sort((left, right) => left.relativePath.localeCompare(right.relativePath))
  }
  const visible: VisibleFolder[] = []
  const visited = new Set<string>()
  function append(parent: string | null, depth: number) {
    for (const folder of children.get(parent) ?? []) {
      if (visited.has(folder.entityId)) continue
      visited.add(folder.entityId)
      const hasChildren = (children.get(folder.entityId)?.length ?? 0) > 0
      visible.push({ folder, depth, hasChildren })
      if (hasChildren && expanded.has(folder.entityId)) append(folder.entityId, depth + 1)
    }
  }
  append(null, 0)
  return visible
}

function isReserved(path: string): boolean {
  return path.split('/').some((segment) => segment.toLowerCase() === '.viewer')
}

function folderMarkerLabel(folder: FolderTreeItem): string {
  const review = reviewLabel(folder.marker.reviewState)
  if (folder.marker.favorite) return `${review} ★`
  return folder.marker.reviewState === null ? '' : review
}

function folderMarkerAriaLabel(folder: FolderTreeItem): string {
  const favorite = folder.marker.favorite ? '，已收藏' : ''
  return `${folder.name}：${reviewLabel(folder.marker.reviewState)}${favorite}`
}

function reviewLabel(value: FolderTreeItem['marker']['reviewState']): string {
  if (value === 'keep') return '保留'
  if (value === 'pending') return '待定'
  if (value === 'reject') return '淘汰'
  return '未标记'
}
