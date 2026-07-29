import {
  type DragEvent,
  type KeyboardEvent,
  type MouseEvent,
  type PointerEvent,
  useLayoutEffect,
  useRef,
} from 'react'
import type { BrowserFile } from '../../api/types'
import { OrganizationDragHandle } from './OrganizationDragHandle'

const TEXT_LIST_ID = 'content-text-file-list'
const TEXT_LIST_LABEL_ID = 'content-text-file-list-label'

export interface TextFilePanelProps {
  mode: 'mixed_collapsed' | 'mixed_expanded' | 'text_only'
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

export default function TextFilePanel({
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
}: TextFilePanelProps) {
  const disclosureRef = useRef<HTMLButtonElement>(null)
  const restoreDisclosureFocus = useRef(false)
  const selectedCount = files.filter((file) => selectedIds.has(file.entityId)).length
  const disclosureLabel =
    selectedCount > 0
      ? `文本文件 · ${files.length} · 已选 ${selectedCount}`
      : `文本文件 · ${files.length}`

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
    <section className={`text-file-panel text-file-panel--${mode}`}>
      {mode === 'text_only' ? (
        <h2 id="content-text-file-heading">
          <span id={TEXT_LIST_LABEL_ID}>文本文件</span> · {files.length}
        </h2>
      ) : (
        <button
          ref={disclosureRef}
          type="button"
          className="text-file-disclosure"
          aria-expanded={mode === 'mixed_expanded'}
          aria-controls={TEXT_LIST_ID}
          onClick={() => onExpandedChange(mode !== 'mixed_expanded')}
        >
          <span aria-hidden="true">{mode === 'mixed_expanded' ? '⌄' : '›'}</span>
          {disclosureLabel}
        </button>
      )}
      {mode !== 'mixed_collapsed' && (
        <div
          id={TEXT_LIST_ID}
          role="listbox"
          aria-label={mode === 'text_only' ? undefined : '文本文件'}
          aria-labelledby={mode === 'text_only' ? TEXT_LIST_LABEL_ID : undefined}
          aria-activedescendant={
            activeId !== null && files.some(({ entityId }) => entityId === activeId)
              ? `file-${activeId}`
              : undefined
          }
          tabIndex={0}
          className="text-file-list"
          onKeyDown={handleListKeyDown}
        >
          {files.map((file) => (
            <div
              role="option"
              id={`file-${file.entityId}`}
              aria-label={file.name}
              aria-selected={selectedIds.has(file.entityId)}
              tabIndex={-1}
              key={file.entityId}
              className="text-file-row"
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
          ))}
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
