/// <reference types="node" />

import { readFileSync } from 'node:fs'
import { act, fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import RadialFileMenu from './RadialFileMenu'
import { buildRadialMenuModel } from './radialMenuModel'
import { fitMenuOrigin, polarPoint } from './radialMenuGeometry'

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

function primaryPaths(container: HTMLElement): string[] {
  return Array.from(container.querySelectorAll<SVGPathElement>('.radial-primary-shape')).map(
    (path) => path.getAttribute('d') ?? '',
  )
}

function primaryOffsets(container: HTMLElement): string[] {
  return Array.from(
    container.querySelectorAll<HTMLButtonElement>('.radial-menu-button[data-level="primary"]'),
  ).map((button) => button.getAttribute('style') ?? '')
}

function primarySectors(container: HTMLElement): Array<{ start: string | null; end: string | null }> {
  return Array.from(container.querySelectorAll<SVGPathElement>('.radial-primary-shape')).map(
    (path) => ({
      start: path.getAttribute('data-sector-start'),
      end: path.getAttribute('data-sector-end'),
    }),
  )
}

function expectVisibleLabelsUpright(container: HTMLElement) {
  const buttons = Array.from(
    container.querySelectorAll<HTMLButtonElement>('.radial-menu-button'),
  )
  expect(buttons.length).toBeGreaterThan(0)
  buttons.forEach((button) => {
    expect(button).toHaveAttribute('data-label-orientation', 'upright')
    expect(button.getAttribute('style')).not.toMatch(/rotate/i)
  })
  expect(appCss).toMatch(
    /\.radial-menu-button\s*\{(?=[^}]*transform:\s*translate\(-50%,\s*-50%\);)(?![^}]*rotate)[^}]*\}/s,
  )
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

  it('shows read-only context in the center summary', () => {
    const readOnlyModel = buildRadialMenuModel({
      selectedCount: 1,
      selectedImageCount: 1,
      readOnly: true,
      busy: false,
      compareContextAvailable: true,
      commonReview: null,
      commonFavorite: false,
    })
    render(
      <RadialFileMenu
        origin={{ x: 320, y: 240 }}
        pointerId={null}
        selectionCount={1}
        readOnly
        model={readOnlyModel}
        onAction={vi.fn()}
        onClose={vi.fn()}
      />,
    )

    expect(screen.getByText('只读')).toBeVisible()
  })

  it('renders visible non-color cues for checked and mixed marker states', () => {
    const mixedModel = buildRadialMenuModel({
      selectedCount: 2,
      selectedImageCount: 2,
      readOnly: false,
      busy: false,
      compareContextAvailable: true,
      commonReview: 'keep',
      commonFavorite: 'mixed',
    })
    render(
      <RadialFileMenu
        origin={{ x: 320, y: 240 }}
        pointerId={null}
        selectionCount={2}
        model={mixedModel}
        onAction={vi.fn()}
        onClose={vi.fn()}
      />,
    )
    fireEvent.click(screen.getByRole('menuitem', { name: '标记' }))

    expect(
      screen.getByRole('menuitemcheckbox', { name: '保留' }).querySelector('.radial-state-cue'),
    ).toHaveTextContent('✓')
    expect(
      screen
        .getByRole('menuitemcheckbox', { name: '切换收藏' })
        .querySelector('.radial-state-cue'),
    ).toHaveTextContent('±')
  })

  it.each([
    ['标记', { x: 398, y: 195 }, 5],
    ['整理', { x: 398, y: 285 }, 3],
  ])('expands the %s fan after the pointer dwell', (label, point, expectedChildren) => {
    vi.useFakeTimers()
    try {
      render(
        <RadialFileMenu
          origin={{ x: 320, y: 240 }}
          pointerId={7}
          selectionCount={1}
          model={model}
          onAction={vi.fn()}
          onClose={vi.fn()}
        />,
      )

      fireEvent.pointerMove(window, { pointerId: 7, clientX: point.x, clientY: point.y })
      act(() => vi.advanceTimersByTime(120))

      expect(screen.getByRole('menu', { name: label }).children).toHaveLength(
        expectedChildren,
      )
    } finally {
      vi.useRealTimers()
    }
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

  it('expands a primary fan after a short release switches to click-mode hover', () => {
    vi.useFakeTimers()
    try {
      render(
        <RadialFileMenu
          origin={{ x: 320, y: 240 }}
          pointerId={7}
          selectionCount={1}
          model={model}
          onAction={vi.fn()}
          onClose={vi.fn()}
        />,
      )
      fireEvent.pointerUp(window, { pointerId: 7, clientX: 324, clientY: 243 })

      fireEvent.pointerEnter(screen.getByRole('menuitem', { name: '标记' }))
      act(() => vi.advanceTimersByTime(119))
      expect(screen.queryByRole('menu', { name: '标记' })).not.toBeInTheDocument()
      act(() => vi.advanceTimersByTime(1))

      expect(screen.getByRole('menu', { name: '标记' })).toBeVisible()
    } finally {
      vi.useRealTimers()
    }
  })

  it('closes a held-pointer session on pointercancel without executing an action', () => {
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
    fireEvent.pointerCancel(window, { pointerId: 7 })
    fireEvent.pointerUp(window, { pointerId: 7, clientX: 320, clientY: 150 })

    expect(action).not.toHaveBeenCalled()
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

  it('keeps the six-sector primary ring and preferred local fan direction stable at an edge', () => {
    const center = render(
      <RadialFileMenu
        origin={{ x: 640, y: 400 }}
        pointerId={null}
        selectionCount={1}
        viewport={{ width: 1280, height: 800 }}
        model={model}
        onAction={vi.fn()}
        onClose={vi.fn()}
      />,
    )
    const centerPrimaryPaths = primaryPaths(center.container)
    const centerPrimaryOffsets = primaryOffsets(center.container)
    fireEvent.click(screen.getByRole('menuitem', { name: '标记' }))
    const centerAnchor = screen.getByRole('menu', { name: '标记' })
    const centerSecondaryPaths = Array.from(
      center.container.querySelectorAll<SVGPathElement>('.radial-secondary-shape'),
    ).map((path) => path.getAttribute('d'))
    expect(centerAnchor).toHaveAttribute('data-anchor-degrees', '-30')
    expect(primaryPaths(center.container)).toEqual(centerPrimaryPaths)
    expect(primaryOffsets(center.container)).toEqual(centerPrimaryOffsets)
    expectVisibleLabelsUpright(center.container)
    center.unmount()

    const edge = render(
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
    expect(primaryPaths(edge.container)).toEqual(centerPrimaryPaths)
    expect(primaryOffsets(edge.container)).toEqual(centerPrimaryOffsets)
    fireEvent.click(screen.getByRole('menuitem', { name: '标记' }))
    const edgeFan = screen.getByRole('menu', { name: '标记' })
    const edgeSecondaryPaths = Array.from(
      edge.container.querySelectorAll<SVGPathElement>('.radial-secondary-shape'),
    ).map((path) => path.getAttribute('d'))
    expect(edgeFan).toHaveAttribute('data-anchor-degrees', '-30')
    expect(edgeSecondaryPaths).toEqual(centerSecondaryPaths)
    expect(primaryPaths(edge.container)).toEqual(centerPrimaryPaths)
    expect(primaryOffsets(edge.container)).toEqual(centerPrimaryOffsets)
    expectVisibleLabelsUpright(edge.container)
  })

  it.each([
    ['left', { x: 4, y: 400 }],
    ['top', { x: 640, y: 4 }],
    ['bottom', { x: 640, y: 796 }],
    ['right', { x: 1276, y: 400 }],
  ])(
    'keeps a held Mark stroke connected through the %s edge and executes its leaf',
    (_edge, origin) => {
      vi.useFakeTimers()
      try {
        const action = vi.fn()
        const viewport = { width: 1280, height: 800 }
        const fittedOrigin = fitMenuOrigin(origin, viewport)
        const primaryPoint = polarPoint(fittedOrigin, 90, -30)
        const leafPoint = polarPoint(fittedOrigin, 140, -30)
        const { container } = render(
          <RadialFileMenu
            origin={origin}
            pointerId={77}
            selectionCount={1}
            viewport={viewport}
            model={model}
            onAction={action}
            onClose={vi.fn()}
          />,
        )

        fireEvent.pointerMove(window, {
          pointerId: 77,
          clientX: primaryPoint.x,
          clientY: primaryPoint.y,
        })
        act(() => vi.advanceTimersByTime(120))
        expect(screen.getByRole('menu', { name: '标记' })).toHaveAttribute(
          'data-anchor-degrees',
          '-30',
        )
        fireEvent.pointerMove(window, {
          pointerId: 77,
          clientX: leafPoint.x,
          clientY: leafPoint.y,
        })
        fireEvent.pointerUp(window, {
          pointerId: 77,
          clientX: leafPoint.x,
          clientY: leafPoint.y,
        })

        expect(action).toHaveBeenCalledOnce()
        expect(action).toHaveBeenCalledWith('mark.reject')
        expect(primarySectors(container)).toEqual([
          { start: '-120', end: '-60' },
          { start: '-60', end: '0' },
          { start: '0', end: '60' },
          { start: '60', end: '120' },
          { start: '120', end: '180' },
          { start: '180', end: '240' },
        ])
        expectVisibleLabelsUpright(container)
      } finally {
        vi.useRealTimers()
      }
    },
  )

  it.each([
    ['left', { x: 4, y: 400 }, { x: '180px', y: '400px' }],
    ['top', { x: 640, y: 4 }, { x: '640px', y: '180px' }],
    ['bottom', { x: 640, y: 796 }, { x: '640px', y: '620px' }],
    ['right', { x: 1276, y: 400 }, { x: '1100px', y: '400px' }],
  ])(
    'clamps the origin at the %s edge without rotating the primary ring or labels',
    (_edge, origin, expected) => {
      const { container } = render(
        <RadialFileMenu
          origin={origin}
          pointerId={null}
          selectionCount={1}
          viewport={{ width: 1280, height: 800 }}
          model={model}
          onAction={vi.fn()}
          onClose={vi.fn()}
        />,
      )

      const root = screen.getByRole('menu', { name: '文件操作' }).parentElement
      expect(root).toHaveStyle({
        '--radial-origin-x': expected.x,
        '--radial-origin-y': expected.y,
      })
      expect(primaryPaths(container)).toHaveLength(6)
      expect(primarySectors(container)).toEqual([
        { start: '-120', end: '-60' },
        { start: '-60', end: '0' },
        { start: '0', end: '60' },
        { start: '60', end: '120' },
        { start: '120', end: '180' },
        { start: '180', end: '240' },
      ])
      expectVisibleLabelsUpright(container)
    },
  )

  it('matches the complete dark and reduced-motion selector groups', () => {
    expect(appCss).toMatch(
      /@media \(prefers-color-scheme: dark\)\s*\{\s*\.workspace-header,\s*\.search-field,\s*\.search-options-popover,\s*\.search-view-popover,\s*\.project-menu > div,\s*\.content-view-menu > div,\s*\.radial-menu-center\s*\{\s*background:\s*#24282f;\s*border-color:\s*#4d5663;\s*color:\s*#f3f5f7;/s,
    )
    expect(appCss).toMatch(
      /@media \(prefers-color-scheme: dark\)[\s\S]*?\.radial-primary-shape\s*\{\s*fill:\s*#2f343d;\s*stroke:\s*#596474;\s*\}/,
    )
    expect(appCss).toMatch(
      /@media \(prefers-color-scheme: dark\)[\s\S]*?\.radial-secondary-shape,\s*\.radial-primary-shape\[data-active="true"\],\s*\.radial-secondary-shape\[data-active="true"\]\s*\{\s*fill:\s*#244d7d;\s*stroke:\s*#5d9ee8;\s*\}/,
    )
    expect(appCss).toMatch(
      /@media \(prefers-color-scheme: dark\)[\s\S]*?\.radial-menu-button,\s*\.radial-menu-center strong\s*\{\s*color:\s*#f3f5f7;\s*\}/,
    )
    expect(appCss).toMatch(
      /@media \(prefers-reduced-motion: reduce\)\s*\{\s*\.organization-drag-handle,\s*\.radial-primary-shape,\s*\.radial-secondary-shape\s*\{\s*transition:\s*none;\s*\}\s*\}/,
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
