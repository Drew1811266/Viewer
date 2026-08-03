import type { KeyboardEvent } from 'react'
import { forwardRef, useImperativeHandle, useRef } from 'react'
import type { ProjectAccess } from '../api/types'
import ViewerMenuRow from './ui/ViewerMenuRow'
import ViewerPopover from './ui/ViewerPopover'
import useViewportPopoverMaxWidth from './useViewportPopoverMaxWidth'

export interface WorkspaceMoreMenuProps {
  open: boolean
  onOpenChange(open: boolean): void
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
      open,
      onOpenChange,
      access,
      closing,
      onOpenSettings,
      onOpenPermissionSettings,
      onReselectProject,
      onCloseProject,
    },
    ref,
  ) {
    const summaryRef = useRef<HTMLElement>(null)
    const popoverMaxWidth = useViewportPopoverMaxWidth()
    useImperativeHandle(ref, () => summaryRef.current as HTMLElement)

    function toggle() {
      onOpenChange(!open)
    }

    function handleSummaryKeyDown(event: KeyboardEvent<HTMLElement>) {
      if (event.key !== 'Enter' && event.key !== ' ' && event.key !== 'Spacebar') return
      event.preventDefault()
      toggle()
    }

    function run(action: () => void) {
      onOpenChange(false)
      action()
    }

    return (
      <details className="workspace-more-menu" open={open}>
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
        <ViewerPopover
          className="workspace-menu-popover"
          open={open}
          label="更多操作"
          triggerRef={summaryRef}
          onOpenChange={onOpenChange}
          style={{ maxWidth: popoverMaxWidth }}
        >
          <ViewerMenuRow className="workspace-menu-item" onSelect={() => run(onOpenSettings)}>
            软件设置
          </ViewerMenuRow>
          {access === 'read_only' && <p className="project-access-status">访问权限：只读</p>}
          {access === 'read_only' && (
            <ViewerMenuRow
              className="workspace-menu-item"
              onSelect={() => run(onOpenPermissionSettings)}
            >
              权限设置
            </ViewerMenuRow>
          )}
          {access === 'read_only' && (
            <ViewerMenuRow className="workspace-menu-item" onSelect={() => run(onReselectProject)}>
              重新选择目录
            </ViewerMenuRow>
          )}
          <hr className="workspace-menu-separator" />
          <ViewerMenuRow
            className="workspace-menu-item"
            data-tone="destructive"
            tone="danger"
            disabled={closing}
            onSelect={() => run(onCloseProject)}
          >
            {closing ? '正在关闭…' : '关闭项目'}
          </ViewerMenuRow>
        </ViewerPopover>
      </details>
    )
  },
)

export default WorkspaceMoreMenu
