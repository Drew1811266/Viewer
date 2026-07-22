import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import RadialFileMenu from './RadialFileMenu'
import { buildRadialMenuModel } from './radialMenuModel'

const model = buildRadialMenuModel({
  selectedCount: 1,
  selectedImageCount: 1,
  readOnly: false,
  busy: false,
  compareContextAvailable: true,
  commonReview: null,
  commonFavorite: false,
})

describe('RadialFileMenu', () => {
  it('exposes six stable primary menuitems and a selection-count center', () => {
    render(
      <RadialFileMenu
        origin={{ x: 320, y: 240 }}
        pointerId={null}
        selectionCount={1}
        model={model}
        onAction={vi.fn()}
        onClose={vi.fn()}
      />,
    )
    expect(screen.getByRole('menu', { name: '文件操作' })).toBeVisible()
    expect(screen.getAllByRole('menuitem')).toHaveLength(6)
    expect(screen.getByText('1 个文件')).toBeVisible()
    expect(screen.getByRole('menuitem', { name: '并排对比' })).toHaveAttribute(
      'aria-disabled',
      'true',
    )
  })

  it('opens the local marker fan and executes a leaf', () => {
    const action = vi.fn()
    render(
      <RadialFileMenu
        origin={{ x: 320, y: 240 }}
        pointerId={null}
        selectionCount={1}
        model={model}
        onAction={action}
        onClose={vi.fn()}
      />,
    )
    fireEvent.click(screen.getByRole('menuitem', { name: '标记' }))
    expect(screen.getByRole('menuitemcheckbox', { name: '保留' })).toBeVisible()
    fireEvent.click(screen.getByRole('menuitemcheckbox', { name: '保留' }))
    expect(action).toHaveBeenCalledWith('mark.keep')
  })

  it('uses ArrowRight/ArrowLeft and ArrowUp/ArrowDown for radial keyboard navigation', () => {
    const action = vi.fn()
    render(
      <RadialFileMenu
        origin={{ x: 320, y: 240 }}
        pointerId={null}
        selectionCount={1}
        model={model}
        onAction={action}
        onClose={vi.fn()}
      />,
    )
    const menu = screen.getByRole('menu', { name: '文件操作' })
    fireEvent.keyDown(menu, { key: 'ArrowRight' })
    expect(screen.getByRole('menuitem', { name: '标记' })).toHaveFocus()
    fireEvent.keyDown(menu, { key: 'ArrowUp' })
    expect(screen.getByRole('menuitemcheckbox', { name: '保留' })).toHaveFocus()
    fireEvent.keyDown(menu, { key: 'ArrowDown' })
    expect(screen.getByRole('menuitem', { name: '标记' })).toHaveFocus()
  })

  it('does not execute a short right-button release and closes on Escape', () => {
    const action = vi.fn()
    const close = vi.fn()
    render(
      <RadialFileMenu
        origin={{ x: 320, y: 240 }}
        pointerId={7}
        selectionCount={1}
        model={model}
        onAction={action}
        onClose={close}
      />,
    )
    fireEvent.pointerUp(window, { pointerId: 7, clientX: 324, clientY: 243 })
    expect(action).not.toHaveBeenCalled()
    fireEvent.keyDown(screen.getByRole('menu'), { key: 'Escape' })
    expect(close).toHaveBeenCalledOnce()
  })

  it('hands Trash to the confirmation owner instead of mutating directly', () => {
    const action = vi.fn()
    render(
      <RadialFileMenu
        origin={{ x: 320, y: 240 }}
        pointerId={null}
        selectionCount={1}
        model={model}
        onAction={action}
        onClose={vi.fn()}
      />,
    )
    fireEvent.click(screen.getByRole('menuitem', { name: '移到废纸篓' }))
    expect(action).toHaveBeenCalledWith('trash')
  })
})
