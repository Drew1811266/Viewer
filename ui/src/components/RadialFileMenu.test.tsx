/// <reference types="node" />

import { readFileSync } from 'node:fs'
import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import RadialFileMenu from './RadialFileMenu'
import { buildRadialMenuModel } from './radialMenuModel'

const appCss = readFileSync('src/styles/app.css', 'utf8')

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

  it('keeps one enabled primary item in the tab order', () => {
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

    const preview = screen.getByRole('menuitem', { name: '预览' })
    const primaryItems = screen.getAllByRole('menuitem')
    expect(preview).toHaveFocus()
    expect(preview).toHaveAttribute('tabindex', '0')
    expect(primaryItems.filter((item) => item.tabIndex === 0)).toEqual([preview])

    fireEvent.keyDown(screen.getByRole('menu', { name: '文件操作' }), {
      key: 'ArrowRight',
    })
    const mark = screen.getByRole('menuitem', { name: '标记' })
    expect(mark).toHaveAttribute('tabindex', '0')
    expect(preview).toHaveAttribute('tabindex', '-1')
  })

  it('links expanded branches to a secondary menu with its own current item', () => {
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

    const mark = screen.getByRole('menuitem', { name: '标记' })
    const organize = screen.getByRole('menuitem', { name: '整理' })
    expect(mark).toHaveAttribute('aria-expanded', 'false')
    expect(organize).toHaveAttribute('aria-expanded', 'false')
    expect(mark).toHaveAttribute('aria-controls')

    fireEvent.click(mark)

    const submenuId = mark.getAttribute('aria-controls')
    const submenu = document.getElementById(submenuId ?? '')
    const keep = screen.getByRole('menuitemcheckbox', { name: '保留' })
    const secondaryItems = screen.getAllByRole('menuitemcheckbox')
    expect(mark).toHaveAttribute('aria-expanded', 'true')
    expect(submenu).toHaveAttribute('role', 'menu')
    expect(submenu).toHaveAttribute('aria-labelledby', mark.id)
    expect(keep).toHaveFocus()
    expect(keep).toHaveAttribute('tabindex', '0')
    expect(secondaryItems.filter((item) => item.tabIndex === 0)).toEqual([keep])
    expect(screen.getAllByRole('menuitem').every((item) => item.tabIndex === -1)).toBe(true)
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

  it('executes a primary action after a real long-distance right-button gesture', () => {
    const action = vi.fn()
    render(
      <RadialFileMenu
        origin={{ x: 320, y: 240 }}
        pointerId={7}
        selectionCount={1}
        model={model}
        onAction={action}
        onClose={vi.fn()}
      />,
    )

    fireEvent.pointerMove(window, { pointerId: 7, clientX: 320, clientY: 150 })
    fireEvent.pointerUp(window, { pointerId: 7, clientX: 320, clientY: 150 })

    expect(action).toHaveBeenCalledOnce()
    expect(action).toHaveBeenCalledWith('preview')
  })

  it('cancels when a long right-button gesture returns to the center before release', () => {
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

    fireEvent.pointerMove(window, { pointerId: 7, clientX: 320, clientY: 150 })
    fireEvent.pointerMove(window, { pointerId: 7, clientX: 320, clientY: 240 })
    fireEvent.pointerUp(window, { pointerId: 7, clientX: 320, clientY: 240 })

    expect(action).not.toHaveBeenCalled()
    expect(close).toHaveBeenCalledOnce()
  })

  it.each(['Enter', ' '])('lets the center cancel %j without executing the current item', (key) => {
    const action = vi.fn()
    const close = vi.fn()
    render(
      <RadialFileMenu
        origin={{ x: 320, y: 240 }}
        pointerId={null}
        selectionCount={1}
        model={model}
        onAction={action}
        onClose={close}
      />,
    )
    const center = screen.getByRole('button', { name: '关闭文件操作' })
    center.focus()

    fireEvent.keyDown(center, { key })

    expect(action).not.toHaveBeenCalled()
    expect(close).toHaveBeenCalledOnce()
  })

  it('restores focus to the previous control when the menu unmounts', () => {
    const opener = document.createElement('button')
    opener.textContent = '打开菜单'
    document.body.append(opener)
    opener.focus()
    const { unmount } = render(
      <RadialFileMenu
        origin={{ x: 320, y: 240 }}
        pointerId={null}
        selectionCount={1}
        model={model}
        onAction={vi.fn()}
        onClose={vi.fn()}
      />,
    )
    expect(screen.getByRole('menuitem', { name: '预览' })).toHaveFocus()

    unmount()

    expect(opener).toHaveFocus()
    opener.remove()
  })

  it('renders a 336-square SVG that fills the radial menu container', () => {
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
    const shapes = document.querySelector('svg.radial-file-menu-shapes')

    expect(shapes).toHaveAttribute('viewBox', '0 0 336 336')
    expect(appCss).toMatch(
      /\.radial-file-menu-shapes\s*\{[^}]*height:\s*100%;[^}]*width:\s*100%;[^}]*\}/s,
    )
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
