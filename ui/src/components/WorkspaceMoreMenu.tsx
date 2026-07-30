import type { KeyboardEvent } from 'react'
import { forwardRef, useImperativeHandle, useRef, useState } from 'react'
import type { ProjectAccess } from '../api/types'

export interface WorkspaceMoreMenuProps {
  access: ProjectAccess
  closing: boolean
  onOpenSettings(): void
  onOpenPermissionSettings(): void
  onReselectProject(): void
  onCloseProject(): void
}

const WorkspaceMoreMenu = forwardRef<HTMLElement, WorkspaceMoreMenuProps>(
  function WorkspaceMoreMenu(
    {
      access,
      closing,
      onOpenSettings,
      onOpenPermissionSettings,
      onReselectProject,
      onCloseProject,
    },
    ref,
  ) {
    const [open, setOpen] = useState(false)
    const summaryRef = useRef<HTMLElement>(null)
    useImperativeHandle(ref, () => summaryRef.current as HTMLElement)

    function toggle() {
      setOpen((current) => !current)
    }

    function handleSummaryKeyDown(event: KeyboardEvent<HTMLElement>) {
      if (event.key !== 'Enter' && event.key !== ' ' && event.key !== 'Spacebar') return
      event.preventDefault()
      toggle()
    }

    function closeFromEscape(event: KeyboardEvent<HTMLDetailsElement>) {
      if (event.key !== 'Escape') return
      event.preventDefault()
      setOpen(false)
      summaryRef.current?.focus()
    }

    function run(action: () => void) {
      setOpen(false)
      action()
    }

    return (
      <details className="workspace-more-menu" open={open} onKeyDown={closeFromEscape}>
        <summary
          ref={summaryRef}
          role="button"
          aria-label="更多"
          aria-expanded={open}
          onClick={(event) => {
            event.preventDefault()
            toggle()
          }}
          onKeyDown={handleSummaryKeyDown}
        >
          更多
        </summary>
        <div className="workspace-menu-popover" hidden={!open}>
          <button type="button" onClick={() => run(onOpenSettings)}>
            软件设置
          </button>
          {access === 'read_only' && <p className="project-access-status">访问权限：只读</p>}
          {access === 'read_only' && (
            <button type="button" onClick={() => run(onOpenPermissionSettings)}>
              权限设置
            </button>
          )}
          {access === 'read_only' && (
            <button type="button" onClick={() => run(onReselectProject)}>
              重新选择目录
            </button>
          )}
          <hr />
          <button type="button" disabled={closing} onClick={() => run(onCloseProject)}>
            {closing ? '正在关闭…' : '关闭项目'}
          </button>
        </div>
      </details>
    )
  },
)

export default WorkspaceMoreMenu
