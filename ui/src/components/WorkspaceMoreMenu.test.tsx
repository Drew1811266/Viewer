import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import WorkspaceMoreMenu from './WorkspaceMoreMenu'

describe('WorkspaceMoreMenu', () => {
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
    expect(close).toHaveAttribute('data-tone', 'destructive')
    fireEvent.click(settings)
    expect(openSettings).toHaveBeenCalledOnce()
    fireEvent.click(screen.getByRole('button', { name: '更多' }))
    fireEvent.click(screen.getByRole('button', { name: '关闭项目' }))
    expect(closeProject).toHaveBeenCalledOnce()
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
