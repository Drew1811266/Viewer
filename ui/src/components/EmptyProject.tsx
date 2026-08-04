import type { DragEvent } from 'react'
import { useRef, useState } from 'react'
import type { ViewerBridge } from '../api/viewer'
import { safeUserMessage } from '../api/viewer'
import ViewerButton from './ui/ViewerButton'
import ViewerLocalFeedback from './ui/ViewerLocalFeedback'

interface EmptyProjectProps {
  bridge: ViewerBridge
  busy?: boolean
  errorMessage?: string | null
  onOpenProject?: (path: string) => Promise<unknown>
}

export default function EmptyProject({
  bridge,
  busy = false,
  errorMessage = null,
  onOpenProject,
}: EmptyProjectProps) {
  const [localError, setLocalError] = useState<string | null>(null)
  const [working, setWorking] = useState(false)
  const [dropState, setDropState] = useState<'idle' | 'valid' | 'invalid'>('idle')
  const [openingName, setOpeningName] = useState('Viewer 项目')
  const dragDepth = useRef(0)
  const disabled = busy || working

  async function openPath(path: string) {
    setLocalError(null)
    setDropState('idle')
    setOpeningName(path.split(/[\\/]/).filter(Boolean).at(-1) ?? 'Viewer 项目')
    setWorking(true)
    try {
      await (onOpenProject ? onOpenProject(path) : bridge.openProject(path))
    } catch (error) {
      setLocalError(safeUserMessage(error))
    } finally {
      setWorking(false)
    }
  }

  function enterProjectDrag(event: DragEvent<HTMLElement>) {
    if (!hasDirectory(event)) return
    dragDepth.current += 1
    setDropState('valid')
  }

  function leaveProjectDrag() {
    if (dropState !== 'valid') return
    dragDepth.current = Math.max(0, dragDepth.current - 1)
    if (dragDepth.current === 0) setDropState('idle')
  }

  async function chooseProject() {
    if (disabled) return
    try {
      const selected = await bridge.chooseProject()
      if (selected !== null) await openPath(selected)
    } catch (error) {
      setLocalError(safeUserMessage(error))
    }
  }

  function dropProject(event: DragEvent<HTMLElement>) {
    event.preventDefault()
    dragDepth.current = 0
    setDropState('idle')
    if (disabled) return
    const items = Array.from(event.dataTransfer.items ?? [])
    const entries = items.map((item) => item.webkitGetAsEntry?.()).filter(Boolean)
    if (entries.length !== 1 || !entries[0]?.isDirectory) {
      setLocalError('请选择项目文件夹，不能导入单个文件。')
      setDropState('invalid')
      return
    }
    const file = event.dataTransfer.files.item(0) as (File & { path?: string }) | null
    if (!file?.path) {
      setLocalError('请点击“选择项目文件夹”完成导入。')
      setDropState('invalid')
      return
    }
    void openPath(file.path)
  }

  if (working) {
    return (
      <main className="project-opening-state" aria-busy="true">
        <h1>{openingName}</h1>
        <p>正在验证项目…</p>
        <div className="project-opening-progress" role="progressbar" aria-label="正在打开项目">
          <span />
        </div>
      </main>
    )
  }

  return (
    <main
      className="empty-project"
      data-drop-state={dropState === 'idle' ? undefined : dropState}
      data-testid="project-drop-zone"
      onDragEnter={enterProjectDrag}
      onDragOver={(event) => event.preventDefault()}
      onDragLeave={leaveProjectDrag}
      onDrop={dropProject}
    >
      <h1>Viewer</h1>
      <p>选择或拖入一个项目文件夹</p>
      <ViewerButton
        className="empty-project-primary-action"
        tone="primary"
        disabled={disabled}
        onClick={() => void chooseProject()}
      >
        选择项目文件夹
      </ViewerButton>
      {dropState !== 'idle' && (
        <div className="project-drop-feedback" data-drop-state={dropState}>
          {dropState === 'valid' ? (
            <strong>松开以打开项目</strong>
          ) : (
            <ViewerLocalFeedback tone="danger" title="无法打开此项目">
              {localError ?? '请选择一个项目文件夹。'}
            </ViewerLocalFeedback>
          )}
        </div>
      )}
      {(localError ?? errorMessage) && dropState !== 'invalid' && (
        <div className="empty-project-feedback">
          <ViewerLocalFeedback tone="danger" title="无法打开此项目">
            {localError ?? errorMessage}
          </ViewerLocalFeedback>
        </div>
      )}
    </main>
  )
}

function hasDirectory(event: DragEvent<HTMLElement>): boolean {
  return Array.from(event.dataTransfer?.items ?? []).some(
    (item) => item.webkitGetAsEntry?.()?.isDirectory,
  )
}
