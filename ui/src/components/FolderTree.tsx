import { useEffect, useMemo, useRef, useState } from 'react'
import type { FolderTreeItem } from '../api/types'
import type { OrganizationDropTarget } from '../state/useOrganizationPointerDrag'
import VirtualList from './VirtualList'

interface FolderTreeProps {
  folders: FolderTreeItem[]
  selectedId: string | null
  onSelect: (entityId: string) => void
  height?: number
  organizationDropTarget?: OrganizationDropTarget | null
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
  organizationDropTarget = null,
}: FolderTreeProps) {
  const safeFolders = useMemo(
    () => folders.filter((folder) => !isReserved(folder.relativePath)),
    [folders],
  )
  const childIds = useMemo(() => parentIds(safeFolders), [safeFolders])
  const [expanded, setExpanded] = useState(() => new Set(childIds))
  const knownExpandable = useRef(new Set(childIds))

  useEffect(() => {
    const added = childIds.filter((id) => !knownExpandable.current.has(id))
    knownExpandable.current = new Set(childIds)
    if (added.length > 0) {
      setExpanded((current) => new Set([...current, ...added]))
    }
  }, [childIds])

  const visible = useMemo(() => flattenFolders(safeFolders, expanded), [expanded, safeFolders])

  function toggle(entityId: string) {
    setExpanded((current) => {
      const next = new Set(current)
      if (next.has(entityId)) next.delete(entityId)
      else next.add(entityId)
      return next
    })
  }

  return (
    <div className="folder-tree" role="tree" aria-label="项目文件夹">
      <VirtualList
        items={visible}
        rowHeight={28}
        height={height}
        overscan={6}
        viewportProps={{ 'data-organization-drop-surface': '' }}
        getKey={(item) => item.folder.entityId}
        renderItem={({ folder, depth, hasChildren }) => (
          <div
            className="folder-tree-row"
            role="treeitem"
            aria-label={folder.relativePath}
            aria-level={depth + 1}
            aria-selected={folder.entityId === selectedId}
            aria-expanded={hasChildren ? expanded.has(folder.entityId) : undefined}
            tabIndex={-1}
            data-organization-folder-id={folder.entityId}
            data-drop-mode={
              organizationDropTarget?.entityId === folder.entityId && organizationDropTarget.valid
                ? organizationDropTarget.mode
                : undefined
            }
            data-drop-invalid={
              organizationDropTarget?.entityId === folder.entityId && !organizationDropTarget.valid
                ? true
                : undefined
            }
            style={{ paddingInlineStart: depth * 16 }}
            onClick={() => onSelect(folder.entityId)}
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
              role="img"
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
  return folders.filter((folder) => parents.has(folder.entityId)).map((folder) => folder.entityId)
}

function flattenFolders(folders: FolderTreeItem[], expanded: Set<string>): VisibleFolder[] {
  const ids = new Set(folders.map((folder) => folder.entityId))
  const children = new Map<string | null, FolderTreeItem[]>()
  for (const folder of folders) {
    const parent =
      folder.parentEntityId && ids.has(folder.parentEntityId) ? folder.parentEntityId : null
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
