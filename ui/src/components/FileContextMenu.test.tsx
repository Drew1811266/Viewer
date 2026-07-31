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

  it.each([
    ['top left', { x: 0, y: 0 }, 'right'],
    ['top right', { x: 1_023, y: 0 }, 'left'],
    ['bottom left', { x: 0, y: 719 }, 'right'],
    ['bottom right', { x: 1_023, y: 719 }, 'left'],
  ] as const)(
    'keeps child commands reachable at the %s edge of a 1024×720 viewport',
    (_edge, origin, expectedSide) => {
      const action = vi.fn()
      render(
        <FileContextMenu
          origin={origin}
          selectionCount={2}
          viewport={{ width: 1_024, height: 720 }}
          model={model}
          onAction={action}
          onClose={vi.fn()}
        />,
      )

      const root = screen.getByRole('menu', { name: '文件操作' }).closest('.file-context-menu')
      expect(root).toHaveAttribute('data-submenu-side', expectedSide)
      fireEvent.click(screen.getByRole('menuitem', { name: '标记' }))
      const submenu = screen.getByRole('menu', { name: '标记' })
      expect(submenu).toHaveAttribute('data-side', expectedSide)

      const rootTop = Number.parseFloat((root as HTMLElement).style.top)
      const submenuTop = rootTop + 6
      const submenuMaxHeight = Number.parseFloat(submenu.style.maxHeight)
      expect(submenuTop).toBeGreaterThanOrEqual(8)
      expect(submenuTop + submenuMaxHeight).toBeLessThanOrEqual(712)

      fireEvent.click(within(submenu).getByRole('menuitemcheckbox', { name: '保留' }))
      expect(action).toHaveBeenCalledWith('mark.keep')
    },
  )
})
