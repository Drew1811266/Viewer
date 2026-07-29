import {
  type DragEvent,
  type KeyboardEvent,
  type MouseEvent,
  type PointerEvent,
  useLayoutEffect,
  useRef,
} from 'react'
import type { BrowserFile } from '../../api/types'
import VirtualList from '../VirtualList'
import { OrganizationDragHandle } from './OrganizationDragHandle'

const OTHER_LIST_ID = 'content-other-file-list'
const OTHER_LIST_LABEL_ID = 'content-other-file-list-label'
export const OTHER_FILE_ROW_HEIGHT = 56

export interface OtherFilePanelProps {
  mode: 'mixed_collapsed' | 'mixed_expanded' | 'other_only'
  files: readonly BrowserFile[]
  selectedIds: ReadonlySet<string>
  activeId: string | null
  organizationDragDisabled: boolean
  onExpandedChange(expanded: boolean): void
  onListKeyDown(event: KeyboardEvent<HTMLElement>): void
  onSelect(file: BrowserFile, event: MouseEvent<HTMLElement>): void
  onPreview(file: BrowserFile): void
  onRadialMenuPointerDown(file: BrowserFile, event: PointerEvent<HTMLElement>): void
  onRadialMenuContextMenu(file: BrowserFile, event: MouseEvent<HTMLElement>): void
  onFinderDragStart(file: BrowserFile, event: DragEvent<HTMLElement>): void
  onOrganizationPointerDown(file: BrowserFile, event: PointerEvent<HTMLElement>): void
  onOrganizationPointerMove(event: PointerEvent<HTMLElement>): void
  onOrganizationPointerUp(event: PointerEvent<HTMLElement>): void
  onOrganizationPointerCancel(event: PointerEvent<HTMLElement>): void
}

export default function OtherFilePanel({
  mode,
  files,
  selectedIds,
  activeId,
  organizationDragDisabled,
  onExpandedChange,
  onListKeyDown,
  onSelect,
  onPreview,
  onRadialMenuPointerDown,
  onRadialMenuContextMenu,
  onFinderDragStart,
  onOrganizationPointerDown,
  onOrganizationPointerMove,
  onOrganizationPointerUp,
  onOrganizationPointerCancel,
}: OtherFilePanelProps) {
  const disclosureRef = useRef<HTMLButtonElement>(null)
  const restoreDisclosureFocus = useRef(false)
  const selectedCount = files.filter((file) => selectedIds.has(file.entityId)).length
  const activeIndex =
    activeId === null ? -1 : files.findIndex(({ entityId }) => entityId === activeId)
  const activeOtherId = activeIndex >= 0 ? activeId : null
  const disclosureLabel =
    selectedCount > 0
      ? `其它文件 · ${files.length} · 已选 ${selectedCount}`
      : `其它文件 · ${files.length}`

  useLayoutEffect(() => {
    if (mode !== 'mixed_collapsed' || !restoreDisclosureFocus.current) return
    restoreDisclosureFocus.current = false
    disclosureRef.current?.focus()
  }, [mode])

  function handleListKeyDown(event: KeyboardEvent<HTMLElement>) {
    if (event.key === 'Escape' && mode === 'mixed_expanded') {
      event.preventDefault()
      restoreDisclosureFocus.current = true
      onExpandedChange(false)
      return
    }
    onListKeyDown(event)
  }

  return (
    <section className={`other-file-panel other-file-panel--${mode}`}>
      {mode === 'other_only' ? (
        <h2 id="content-other-file-heading">
          <span id={OTHER_LIST_LABEL_ID}>其它文件</span> · {files.length}
        </h2>
      ) : (
        <button
          ref={disclosureRef}
          type="button"
          className="other-file-disclosure"
          aria-expanded={mode === 'mixed_expanded'}
          aria-controls={OTHER_LIST_ID}
          onClick={() => onExpandedChange(mode !== 'mixed_expanded')}
        >
          <span aria-hidden="true">{mode === 'mixed_expanded' ? '⌄' : '›'}</span>
          {disclosureLabel}
        </button>
      )}
      {mode !== 'mixed_collapsed' && (
        <div
          id={OTHER_LIST_ID}
          role="listbox"
          aria-label={mode === 'other_only' ? undefined : '其它文件'}
          aria-labelledby={mode === 'other_only' ? OTHER_LIST_LABEL_ID : undefined}
          aria-activedescendant={activeOtherId ? `file-${activeOtherId}` : undefined}
          tabIndex={0}
          className="other-file-listbox"
          style={
            mode === 'mixed_expanded' ? { height: files.length * OTHER_FILE_ROW_HEIGHT } : undefined
          }
          onKeyDown={handleListKeyDown}
        >
          <VirtualList
            items={files}
            rowHeight={OTHER_FILE_ROW_HEIGHT}
            getKey={(file) => file.entityId}
            scrollToIndex={activeIndex >= 0 ? activeIndex : undefined}
            className="other-file-virtual-list"
            renderItem={(file) => (
              <div
                role="option"
                id={`file-${file.entityId}`}
                aria-label={file.name}
                aria-selected={selectedIds.has(file.entityId)}
                tabIndex={-1}
                className="text-file-row other-file-row"
                onPointerDown={(event) => onRadialMenuPointerDown(file, event)}
                onContextMenu={(event) => onRadialMenuContextMenu(file, event)}
                onClick={(event) => onSelect(file, event)}
                onDoubleClick={() => onPreview(file)}
              >
                <div
                  className="file-export-surface"
                  draggable
                  title="拖到 Finder"
                  onDragStart={(event) => onFinderDragStart(file, event)}
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
                  onPointerDown={onOrganizationPointerDown}
                  onPointerMove={onOrganizationPointerMove}
                  onPointerUp={onOrganizationPointerUp}
                  onPointerCancel={onOrganizationPointerCancel}
                />
              </div>
            )}
          />
        </div>
      )}
    </section>
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
