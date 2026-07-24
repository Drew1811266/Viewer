import type { DragEvent } from 'react'
import { useState } from 'react'
import type { ViewerBridge } from '../api/viewer'
import { safeUserMessage } from '../api/viewer'

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
  const disabled = busy || working

  async function openPath(path: string) {
    setLocalError(null)
    setWorking(true)
    try {
      await (onOpenProject ? onOpenProject(path) : bridge.openProject(path))
    } catch (error) {
      setLocalError(safeUserMessage(error))
    } finally {
      setWorking(false)
    }
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
    if (disabled) return
    const items = Array.from(event.dataTransfer.items ?? [])
    const entries = items.map((item) => item.webkitGetAsEntry?.()).filter(Boolean)
    if (entries.length !== 1 || !entries[0]?.isDirectory) {
      setLocalError('请选择项目文件夹，不能导入单个文件。')
      return
    }
    const file = event.dataTransfer.files.item(0) as (File & { path?: string }) | null
    if (!file?.path) {
      setLocalError('请点击“选择项目文件夹”完成导入。')
      return
    }
    void openPath(file.path)
  }

  return (
    <main
      className="empty-project"
      data-testid="project-drop-zone"
      onDragOver={(event) => event.preventDefault()}
      onDrop={dropProject}
    >
      <h1>Viewer</h1>
      <p>拖入或选择一个项目文件夹</p>
      <p>支持 JPG、PNG、Markdown 和 TXT，本次关闭后不会记住目录。</p>
      <button type="button" disabled={disabled} onClick={() => void chooseProject()}>
        {disabled ? '正在打开…' : '选择项目文件夹'}
      </button>
      {(localError ?? errorMessage) && <p role="alert">{localError ?? errorMessage}</p>}
    </main>
  )
}
