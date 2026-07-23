/// <reference types="node" />

import { readFileSync } from 'node:fs'
import { act, fireEvent, render, screen } from '@testing-library/react'
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

function sequentialTabStops(container: HTMLElement): HTMLElement[] {
  return Array.from(
    container.querySelectorAll<HTMLElement>(
      'button, [href], input, select, textarea, [tabindex]',
    ),
  ).filter((element) => element.tabIndex >= 0)
}

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

  it('keeps exactly one sequential tab stop across the whole component', () => {
    const { container } = render(
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
    const center = screen.getByRole('button', { name: '关闭文件操作' })
    expect(preview).toHaveFocus()
    expect(preview).toHaveAttribute('tabindex', '0')
    expect(primaryItems.filter((item) => item.tabIndex === 0)).toEqual([preview])
    expect(center).toHaveAttribute('tabindex', '-1')
    expect(sequentialTabStops(container)).toEqual([preview])

    fireEvent.keyDown(screen.getByRole('menu', { name: '文件操作' }), {
      key: 'ArrowRight',
    })
    const mark = screen.getByRole('menuitem', { name: '标记' })
    expect(mark).toHaveAttribute('tabindex', '0')
    expect(preview).toHaveAttribute('tabindex', '-1')

    fireEvent.click(mark)
    const keep = screen.getByRole('menuitemcheckbox', { name: '保留' })
    expect(sequentialTabStops(container)).toEqual([keep])
  })

  it('uses legal sibling menus and only emits resolvable aria-controls references', () => {
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
    const primaryMenu = screen.getByRole('menu', { name: '文件操作' })
    const center = screen.getByRole('button', { name: '关闭文件操作' })
    expect(mark).toHaveAttribute('aria-expanded', 'false')
    expect(organize).toHaveAttribute('aria-expanded', 'false')
    expect(mark).not.toHaveAttribute('aria-controls')
    expect(organize).not.toHaveAttribute('aria-controls')
    expect(primaryMenu.parentElement).not.toHaveAttribute('role')
    expect(center.parentElement).toBe(primaryMenu.parentElement)
    expect(primaryMenu.querySelectorAll(':scope > [role^="menuitem"]')).toHaveLength(6)

    fireEvent.click(mark)

    const submenuId = mark.getAttribute('aria-controls')
    const submenu = document.getElementById(submenuId ?? '')
    const keep = screen.getByRole('menuitemcheckbox', { name: '保留' })
    const secondaryItems = screen.getAllByRole('menuitemcheckbox')
    expect(mark).toHaveAttribute('aria-expanded', 'true')
    expect(submenu).toHaveAttribute('role', 'menu')
    expect(submenu).toHaveAttribute('aria-labelledby', mark.id)
    expect(submenu?.parentElement).toBe(primaryMenu.parentElement)
    expect(primaryMenu.contains(submenu)).toBe(false)
    expect(primaryMenu.contains(center)).toBe(false)
    document.querySelectorAll<HTMLElement>('[aria-controls]').forEach((controller) => {
      expect(document.getElementById(controller.getAttribute('aria-controls') ?? '')).not.toBeNull()
    })
    expect(keep).toHaveFocus()
    expect(keep).toHaveAttribute('tabindex', '0')
    expect(secondaryItems.filter((item) => item.tabIndex === 0)).toEqual([keep])
    expect(screen.getAllByRole('menuitem').every((item) => item.tabIndex === -1)).toBe(true)
  })

  it('keeps a focused disabled primary item as the sole roving current item', () => {
    const action = vi.fn()
    const { container } = render(
      <RadialFileMenu
        origin={{ x: 320, y: 240 }}
        pointerId={null}
        selectionCount={1}
        model={model}
        onAction={action}
        onClose={vi.fn()}
      />,
    )

    const compare = screen.getByRole('menuitem', { name: '并排对比' })
    act(() => compare.focus())

    expect(compare).toHaveFocus()
    expect(compare).toHaveAttribute('tabindex', '0')
    expect(sequentialTabStops(container)).toEqual([compare])
    fireEvent.click(compare)
    expect(action).not.toHaveBeenCalled()

    fireEvent.keyDown(screen.getByRole('menu', { name: '文件操作' }), {
      key: 'ArrowRight',
    })
    const info = screen.getByRole('menuitem', { name: '信息' })
    expect(info).toHaveFocus()
    expect(sequentialTabStops(container)).toEqual([info])
  })

  it('keeps a focused disabled secondary item as the sole roving current item', () => {
    const action = vi.fn()
    const modelWithDisabledChild = model.map((item) =>
      item.id === 'mark'
        ? {
            ...item,
            children: item.children?.map((child) =>
              child.id === 'mark.pending' ? { ...child, disabled: true } : child,
            ),
          }
        : item,
    )
    const { container } = render(
      <RadialFileMenu
        origin={{ x: 320, y: 240 }}
        pointerId={null}
        selectionCount={1}
        model={modelWithDisabledChild}
        onAction={action}
        onClose={vi.fn()}
      />,
    )

    fireEvent.click(screen.getByRole('menuitem', { name: '标记' }))
    const pending = screen.getByRole('menuitemcheckbox', { name: '待定' })
    act(() => pending.focus())

    expect(pending).toHaveFocus()
    expect(pending).toHaveAttribute('tabindex', '0')
    expect(sequentialTabStops(container)).toEqual([pending])
    fireEvent.click(pending)
    expect(action).not.toHaveBeenCalled()

    fireEvent.keyDown(screen.getByRole('menu', { name: '标记' }), {
      key: 'ArrowRight',
    })
    const reject = screen.getByRole('menuitemcheckbox', { name: '淘汰' })
    expect(reject).toHaveFocus()
    expect(sequentialTabStops(container)).toEqual([reject])
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

  it('rotates only the local fan at the right edge and keeps accessible labels unchanged', () => {
    render(
      <RadialFileMenu
        origin={{ x: 1260, y: 400 }}
        pointerId={null}
        selectionCount={1}
        viewport={{ width: 1280, height: 800 }}
        model={model}
        onAction={vi.fn()}
        onClose={vi.fn()}
      />,
    )
    fireEvent.click(screen.getByRole('menuitem', { name: '标记' }))
    expect(screen.getByRole('menuitemcheckbox', { name: '保留' })).toBeVisible()
    expect(
      screen.queryByRole('menuitemcheckbox', { name: '取消收藏' }),
    ).not.toBeInTheDocument()
    const menu = screen.getByRole('menu', { name: '文件操作' })
    expect(menu.parentElement).toHaveStyle({ '--radial-origin-x': '1100px' })
  })

  it('provides dark appearance and reduced-motion rules for every new surface', () => {
    expect(appCss).toMatch(
      /@media \(prefers-color-scheme: dark\)[\s\S]*\.workspace-header,[\s\S]*\.radial-menu-center\s*\{[\s\S]*background:\s*#24282f;/,
    )
    expect(appCss).toMatch(
      /@media \(prefers-color-scheme: dark\)[\s\S]*\.radial-primary-shape\s*\{[\s\S]*fill:\s*#2f343d;/,
    )
    expect(appCss).toMatch(
      /@media \(prefers-reduced-motion: reduce\)[\s\S]*\.organization-drag-handle,[\s\S]*\.radial-secondary-shape\s*\{[\s\S]*transition:\s*none;/,
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
