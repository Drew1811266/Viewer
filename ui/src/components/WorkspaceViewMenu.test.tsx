import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import WorkspaceViewMenu from './WorkspaceViewMenu'

describe('WorkspaceViewMenu', () => {
  it('shows search layouts in the contextual view menu', () => {
    const onLayoutChange = vi.fn()
    render(<WorkspaceViewMenu context={{ kind: 'search', layout: 'grouped', onLayoutChange }} />)
    fireEvent.click(screen.getByRole('button', { name: '视图' }))
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
          selectAllRequest: { kind: 'choice' },
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

  it('opens when a content keyboard command requests the global menu', () => {
    render(
      <WorkspaceViewMenu
        openRequest={1}
        context={{
          kind: 'content',
          showingAggregate: false,
          selectAllRequest: { kind: 'choice' },
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
