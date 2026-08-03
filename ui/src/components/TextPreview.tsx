import type { KeyboardEvent } from 'react'
import { useCallback, useEffect, useRef, useState } from 'react'
import type { BrowserFile, TextEncoding, TextPreview as TextPreviewDto } from '../api/types'
import type { TaskFeedback } from './TaskBar'
import TextPreviewPane, { type TextPaneStatus } from './TextPreviewPane'
import ViewerButton from './ui/ViewerButton'
import ViewerToolbar from './ui/ViewerToolbar'

export type TextPreviewFiles = readonly [BrowserFile] | readonly [BrowserFile, BrowserFile]

export interface TextPreviewProps {
  files: TextPreviewFiles
  unavailableEntityIds?: ReadonlySet<string>
  requestPreview(file: BrowserFile, encoding?: TextEncoding): Promise<TextPreviewDto>
  openExternalLink(url: string): Promise<void>
  onClose(): void
  onTaskChange?(task: TaskFeedback | null): void
}

const EMPTY_ENTITY_IDS: ReadonlySet<string> = new Set()
const FOCUSABLE =
  'button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [href], [tabindex]:not([tabindex="-1"])'

export default function TextPreview({
  files,
  unavailableEntityIds = EMPTY_ENTITY_IDS,
  requestPreview,
  openExternalLink,
  onClose,
  onTaskChange,
}: TextPreviewProps) {
  const dialogRef = useRef<HTMLElement>(null)
  const [paneStatuses, setPaneStatuses] = useState<Record<string, TextPaneStatus>>({})
  const taskId = `text-preview-${files.map((file) => file.entityId).join('-')}`
  const statuses = files.map((file) => paneStatuses[file.entityId] ?? 'running')
  const completed = statuses.filter((status) => status === 'complete').length
  const failed = statuses.filter((status) => status === 'failed').length

  const recordStatus = useCallback((entityId: string, status: TextPaneStatus) => {
    setPaneStatuses((current) =>
      current[entityId] === status ? current : { ...current, [entityId]: status },
    )
  }, [])

  const paneNodes = files.map((file) => (
    <TextPreviewPane
      key={`${file.entityId}:${file.modifiedNs}`}
      file={file}
      unavailable={unavailableEntityIds.has(file.entityId)}
      requestPreview={requestPreview}
      openExternalLink={openExternalLink}
      onStatusChange={recordStatus}
    />
  ))

  useEffect(() => {
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null
    dialogRef.current?.querySelector<HTMLElement>(FOCUSABLE)?.focus()
    return () => {
      if (previous?.isConnected) previous.focus()
    }
  }, [])

  useEffect(() => {
    const settled = completed + failed
    onTaskChange?.({
      id: taskId,
      label: '读取文本预览',
      status: settled < files.length ? 'running' : failed > 0 ? 'failed' : 'complete',
      requested: files.length,
      completed,
      failed,
      cancellable: false,
      failures: [],
    })
    return () => onTaskChange?.(null)
  }, [completed, failed, files.length, onTaskChange, taskId])

  function containFocus(event: KeyboardEvent<HTMLElement>) {
    if (event.key === 'Escape') {
      event.preventDefault()
      event.stopPropagation()
      onClose()
      return
    }
    if (event.key !== 'Tab' || dialogRef.current === null) return
    const focusable = [...dialogRef.current.querySelectorAll<HTMLElement>(FOCUSABLE)]
    if (focusable.length === 0) {
      event.preventDefault()
      dialogRef.current.focus()
      return
    }
    const [first] = focusable
    const last = focusable.at(-1)
    if (first === undefined || last === undefined) {
      throw new Error('Non-empty text preview focus list is missing a boundary element')
    }
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault()
      last.focus()
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault()
      first.focus()
    }
  }

  return (
    <section
      ref={dialogRef}
      className="preview-overlay text-preview"
      role="dialog"
      aria-modal="true"
      aria-label={files.map((file) => file.name).join('、')}
      tabIndex={-1}
      onKeyDown={containFocus}
    >
      <ViewerToolbar
        label="文本预览工具"
        leading={
          <div className="text-preview-identity">
            <strong>{files.map((file) => file.name).join(' · ')}</strong>
            <span>{files.map((file) => textFormatLabel(file.kind)).join(' · ')}</span>
          </div>
        }
        actions={
          <ViewerButton
            tone="quiet"
            className="preview-complete-action"
            aria-label="关闭预览"
            onClick={onClose}
          >
            完成
          </ViewerButton>
        }
      />
      <div className="text-preview-stage">
        <div className="text-preview-panes" data-pane-count={files.length}>
          {paneNodes}
        </div>
      </div>
    </section>
  )
}

function textFormatLabel(kind: BrowserFile['kind']): string {
  return kind === 'markdown' ? 'Markdown' : '纯文本'
}
