import type { KeyboardEvent } from 'react'
import { forwardRef, useEffect, useImperativeHandle, useRef, useState } from 'react'
import type { ProjectAccess } from '../api/types'
import useViewportPopoverMaxWidth from './useViewportPopoverMaxWidth'

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
    const popoverMaxWidth = useViewportPopoverMaxWidth()
    useImperativeHandle(ref, () => summaryRef.current as HTMLElement)

    useEffect(() => {
      const closeForPeer = (event: Event) => {
        if ((event as CustomEvent<string>).detail !== 'more') setOpen(false)
      }
      window.addEventListener('viewer-toolbar-popover', closeForPeer)
      return () => window.removeEventListener('viewer-toolbar-popover', closeForPeer)
    }, [])

    function toggle() {
      setOpen((current) => {
        const next = !current
        if (next)
          window.dispatchEvent(new CustomEvent('viewer-toolbar-popover', { detail: 'more' }))
        return next
      })
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
        <div
          className="workspace-menu-popover"
          hidden={!open}
          style={{ maxWidth: popoverMaxWidth }}
        >
          <button type="button" className="workspace-menu-item" onClick={() => run(onOpenSettings)}>
            软件设置
          </button>
          {access === 'read_only' && <p className="project-access-status">访问权限：只读</p>}
          {access === 'read_only' && (
            <button
              type="button"
              className="workspace-menu-item"
              onClick={() => run(onOpenPermissionSettings)}
            >
              权限设置
            </button>
          )}
          {access === 'read_only' && (
            <button
              type="button"
              className="workspace-menu-item"
              onClick={() => run(onReselectProject)}
            >
              重新选择目录
            </button>
          )}
          <hr className="workspace-menu-separator" aria-hidden="true" />
          <button
            type="button"
            className="workspace-menu-item"
            data-tone="destructive"
            disabled={closing}
            onClick={() => run(onCloseProject)}
          >
            {closing ? '正在关闭…' : '关闭项目'}
          </button>
        </div>
      </details>
    )
  },
)

export default WorkspaceMoreMenu
