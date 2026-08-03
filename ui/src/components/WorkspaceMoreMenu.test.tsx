import { fireEvent, render, screen } from '@testing-library/react'
import { useState } from 'react'
import { describe, expect, it, vi } from 'vitest'
import WorkspaceMoreMenuView, { type WorkspaceMoreMenuProps } from './WorkspaceMoreMenu'

function WorkspaceMoreMenu(props: Omit<WorkspaceMoreMenuProps, 'open' | 'onOpenChange'>) {
  const [open, setOpen] = useState(false)
  return <WorkspaceMoreMenuView {...props} open={open} onOpenChange={setOpen} />
}

describe('WorkspaceMoreMenu', () => {
  it.each([
    [1024, 720],
    [720, 450],
  ])('keeps every real project command named at %d×%d CSS pixels', (width, height) => {
    vi.stubGlobal('innerWidth', width)
    vi.stubGlobal('innerHeight', height)
    render(
      <WorkspaceMoreMenuView
        open
        onOpenChange={vi.fn()}
        access="read_only"
        closing={false}
        onOpenSettings={vi.fn()}
        onOpenPermissionSettings={vi.fn()}
        onReselectProject={vi.fn()}
        onCloseProject={vi.fn()}
      />,
    )
    for (const name of ['软件设置', '权限设置', '重新选择目录', '关闭项目']) {
      expect(screen.getByRole('button', { name })).toBeVisible()
    }
  })

  it('contains settings and project commands in one named menu', () => {
    const openSettings = vi.fn()
    const closeProject = vi.fn()
    render(
      <WorkspaceMoreMenu
        access="read_write"
        closing={false}
        onOpenSettings={openSettings}
        onOpenPermissionSettings={vi.fn()}
        onReselectProject={vi.fn()}
        onCloseProject={closeProject}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: '更多' }))
    const settings = screen.getByRole('button', { name: '软件设置' })
    expect(settings).toHaveClass('workspace-menu-item')
    expect(document.querySelector('.workspace-menu-separator')).not.toBeNull()
    const close = screen.getByRole('button', { name: '关闭项目' })
    expect(close).toHaveClass('workspace-menu-item')
    expect(close).toHaveAttribute('data-tone', 'neutral')
    expect(close).toHaveAttribute('data-danger-reveal', 'interaction')
    fireEvent.click(settings)
    expect(openSettings).toHaveBeenCalledOnce()
    fireEvent.click(screen.getByRole('button', { name: '更多' }))
    fireEvent.click(screen.getByRole('button', { name: '关闭项目' }))
    expect(closeProject).toHaveBeenCalledOnce()
  })

  it('shows read-only access as a formal status row with a visible disabled reason', () => {
    render(
      <WorkspaceMoreMenu
        access="read_only"
        closing={false}
        onOpenSettings={vi.fn()}
        onOpenPermissionSettings={vi.fn()}
        onReselectProject={vi.fn()}
        onCloseProject={vi.fn()}
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: '更多' }))
    const access = screen.getByRole('button', { name: /访问权限/ })
    expect(access).toBeDisabled()
    expect(access).toHaveTextContent('只读')
    expect(access).toHaveTextContent('修改命令已停用')
    expect(access.querySelector('.viewer-status-tag')).toHaveAttribute('data-tone', 'warning')
  })

  it('closes on Escape and restores focus to its trigger', () => {
    render(
      <WorkspaceMoreMenu
        access="read_write"
        closing={false}
        onOpenSettings={vi.fn()}
        onOpenPermissionSettings={vi.fn()}
        onReselectProject={vi.fn()}
        onCloseProject={vi.fn()}
      />,
    )
    const trigger = screen.getByRole('button', { name: '更多' })
    fireEvent.click(trigger)
    fireEvent.keyDown(trigger, { key: 'Escape' })
    expect(trigger).toHaveAttribute('aria-expanded', 'false')
    expect(trigger).toHaveFocus()
  })
})
