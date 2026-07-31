import { fireEvent, render, screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import FileContextMenu from './FileContextMenu'
import { buildRadialMenuModel } from './radialMenuModel'

const model = buildRadialMenuModel({
  selectedCount: 2,
  selectedImageCount: 2,
  previewEnabled: true,
  readOnly: false,
  busy: false,
  compareContextAvailable: true,
  commonReview: null,
  commonFavorite: false,
})

describe('FileContextMenu', () => {
  it('presents the radial action model as one conventional menu', () => {
    const action = vi.fn()
    render(
      <FileContextMenu
        origin={{ x: 320, y: 240 }}
        selectionCount={2}
        model={model}
        onAction={action}
        onClose={vi.fn()}
      />,
    )

    const menu = screen.getByRole('menu', { name: '文件操作' })
    expect(within(menu).getAllByRole('menuitem')).toHaveLength(6)
    fireEvent.click(within(menu).getByRole('menuitem', { name: '标记' }))
    expect(screen.getByRole('menuitemcheckbox', { name: '保留' })).toBeVisible()
    expect(screen.getByRole('menuitem', { name: '移到废纸篓' })).toHaveAttribute(
      'data-tone',
      'destructive',
    )
  })

  it('closes on Escape and returns focus to its invoker', () => {
    const trigger = document.createElement('button')
    document.body.append(trigger)
    const close = vi.fn()
    render(
      <FileContextMenu
        origin={{ x: 320, y: 240 }}
        selectionCount={2}
        returnFocusTarget={trigger}
        model={model}
        onAction={vi.fn()}
        onClose={close}
      />,
    )

    fireEvent.keyDown(screen.getByRole('menu', { name: '文件操作' }), { key: 'Escape' })
    expect(close).toHaveBeenCalledWith(trigger)
    trigger.remove()
  })

  it('uses roving focus to open and return from a child menu', () => {
    render(
      <FileContextMenu
        origin={{ x: 320, y: 240 }}
        selectionCount={2}
        model={model}
        onAction={vi.fn()}
        onClose={vi.fn()}
      />,
    )

    const menu = screen.getByRole('menu', { name: '文件操作' })
    fireEvent.keyDown(menu, { key: 'ArrowDown' })
    expect(screen.getByRole('menuitem', { name: '标记' })).toHaveFocus()
    fireEvent.keyDown(menu, { key: 'ArrowRight' })
    expect(screen.getByRole('menuitemcheckbox', { name: '保留' })).toHaveFocus()
    fireEvent.keyDown(screen.getByRole('menu', { name: '标记' }), { key: 'ArrowLeft' })
    expect(screen.getByRole('menuitem', { name: '标记' })).toHaveFocus()
  })
})
