import { fireEvent, render, screen } from '@testing-library/react'
import { useState } from 'react'
import { describe, expect, it, vi } from 'vitest'
import WorkspaceViewMenuView, { type WorkspaceViewContext } from './WorkspaceViewMenu'

function WorkspaceViewMenu({ context }: { context: WorkspaceViewContext }) {
  const [open, setOpen] = useState(false)
  return <WorkspaceViewMenuView context={context} open={open} onOpenChange={setOpen} />
}

describe('WorkspaceViewMenu', () => {
  it.each([
    [1024, 720],
    [720, 450],
  ])('keeps the controlled menu commands mounted at %d×%d CSS pixels', (width, height) => {
    vi.stubGlobal('innerWidth', width)
    vi.stubGlobal('innerHeight', height)
    render(
      <WorkspaceViewMenuView
        open
        onOpenChange={vi.fn()}
        context={{ kind: 'search', layout: 'grouped', onLayoutChange: vi.fn() }}
      />,
    )
    expect(screen.getByRole('button', { name: /按文件夹分组/ })).toBeVisible()
    expect(screen.getByRole('button', { name: '展平结果' })).toBeVisible()
  })

  it('shows search layouts in the contextual view menu', () => {
    const onLayoutChange = vi.fn()
    render(<WorkspaceViewMenu context={{ kind: 'search', layout: 'grouped', onLayoutChange }} />)
    fireEvent.click(screen.getByRole('button', { name: '视图' }))
    const current = screen.getByRole('button', { name: /按文件夹分组/ })
    expect(current).toHaveAttribute('aria-pressed', 'true')
    expect(current).toHaveTextContent('当前')
    fireEvent.click(screen.getByRole('button', { name: '展平结果' }))
    expect(onLayoutChange).toHaveBeenCalledWith('flat')
  })

  it('offers all contextual content select-all scopes', () => {
    const onSelectAll = vi.fn()
    render(
      <WorkspaceViewMenu
        context={{
          kind: 'content',
          showingAggregate: false,
          selectAllRequest: { kind: 'choice', scopes: ['images', 'videos', 'otherFiles'] },
          onSelectAll,
          onShowAllDescendants: vi.fn(),
          onReturnToFolder: vi.fn(),
        }}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: '视图' }))
    expect(screen.getByRole('button', { name: '显示全部后代文件' })).toHaveClass(
      'workspace-menu-item',
    )
    fireEvent.click(screen.getByRole('button', { name: '全选全部文件' }))
    expect(onSelectAll).toHaveBeenCalledWith('all')
  })

  it('opens when its controlled owner requests the menu', () => {
    render(
      <WorkspaceViewMenuView
        open
        onOpenChange={vi.fn()}
        context={{
          kind: 'content',
          showingAggregate: false,
          selectAllRequest: { kind: 'choice', scopes: ['images', 'videos', 'otherFiles'] },
          onSelectAll: vi.fn(),
          onShowAllDescendants: vi.fn(),
          onReturnToFolder: vi.fn(),
        }}
      />,
    )

    expect(screen.getByRole('button', { name: '全选图片' })).toBeVisible()
  })

  it('closes on Escape and restores focus to its trigger', () => {
    render(<WorkspaceViewMenu context={{ kind: 'none' }} />)
    const trigger = screen.getByRole('button', { name: '视图' })
    fireEvent.click(trigger)
    fireEvent.keyDown(trigger, { key: 'Escape' })
    expect(trigger).toHaveAttribute('aria-expanded', 'false')
    expect(trigger).toHaveFocus()
  })
})
