import { fireEvent, render, screen } from '@testing-library/react'
import type { ComponentProps } from 'react'
import { describe, expect, it, vi } from 'vitest'
import type { BrowserFile } from '../../api/types'
import TextFilePanel from './TextFilePanel'

const textFiles: readonly BrowserFile[] = [
  {
    entityId: 'text-1',
    relativePath: 'notes/first.md',
    name: 'first.md',
    kind: 'markdown',
    size: 10,
    modifiedNs: '1',
    marker: { reviewState: 'keep', favorite: true },
    imageMetadata: null,
    imageUrl: null,
  },
  {
    entityId: 'text-2',
    relativePath: 'notes/second.txt',
    name: 'second.txt',
    kind: 'text',
    size: 20,
    modifiedNs: '2',
    marker: { reviewState: null, favorite: false },
    imageMetadata: null,
    imageUrl: null,
  },
]

function panelProps(
  overrides: Partial<ComponentProps<typeof TextFilePanel>> = {},
): ComponentProps<typeof TextFilePanel> {
  return {
    mode: 'mixed_collapsed',
    files: textFiles,
    selectedIds: new Set(),
    activeId: null,
    organizationDragDisabled: false,
    onExpandedChange: () => undefined,
    onListKeyDown: () => undefined,
    onSelect: () => undefined,
    onPreview: () => undefined,
    onRadialMenuPointerDown: () => undefined,
    onRadialMenuContextMenu: () => undefined,
    onFinderDragStart: () => undefined,
    onOrganizationPointerDown: () => undefined,
    onOrganizationPointerMove: () => undefined,
    onOrganizationPointerUp: () => undefined,
    onOrganizationPointerCancel: () => undefined,
    ...overrides,
  }
}

function renderPanel(overrides: Partial<ComponentProps<typeof TextFilePanel>> = {}) {
  return render(<TextFilePanel {...panelProps(overrides)} />)
}

function renderPanelElement(overrides: Partial<ComponentProps<typeof TextFilePanel>> = {}) {
  return <TextFilePanel {...panelProps(overrides)} />
}

describe('TextFilePanel', () => {
  it('renders one complete collapsed disclosure with selected hidden count', () => {
    renderPanel({
      mode: 'mixed_collapsed',
      selectedIds: new Set(['text-2']),
    })

    const disclosure = screen.getByRole('button', {
      name: '文本文件 · 2 · 已选 1',
    })
    expect(disclosure).toHaveAttribute('aria-expanded', 'false')
    expect(disclosure).toHaveAttribute('aria-controls', 'content-text-file-list')
    expect(screen.queryByRole('listbox', { name: '文本文件' })).not.toBeInTheDocument()
  })

  it('uses a native disclosure button and reports the controlled next state', () => {
    const onExpandedChange = vi.fn()
    renderPanel({ mode: 'mixed_collapsed', onExpandedChange })

    const disclosure = screen.getByRole('button', { name: '文本文件 · 2' })
    expect(disclosure.tagName).toBe('BUTTON')
    fireEvent.click(disclosure)

    expect(onExpandedChange).toHaveBeenCalledOnce()
    expect(onExpandedChange).toHaveBeenCalledWith(true)
  })

  it('collapses an expanded mixed shelf on Escape and restores disclosure focus', () => {
    const onExpandedChange = vi.fn()
    const rendered = renderPanel({ mode: 'mixed_expanded', onExpandedChange })
    const list = screen.getByRole('listbox', { name: '文本文件' })
    list.focus()

    fireEvent.keyDown(list, { key: 'Escape' })

    expect(onExpandedChange).toHaveBeenCalledWith(false)
    rendered.rerender(renderPanelElement({ mode: 'mixed_collapsed', onExpandedChange }))
    expect(screen.getByRole('button', { name: '文本文件 · 2' })).toHaveFocus()
  })

  it('renders text-only content as a non-collapsible labelled primary list', () => {
    renderPanel({ mode: 'text_only' })

    expect(screen.getByRole('heading', { name: '文本文件 · 2' })).toBeVisible()
    expect(screen.queryByRole('button', { name: /文本文件/ })).not.toBeInTheDocument()
    expect(screen.getByRole('listbox', { name: '文本文件' })).toBeVisible()
  })

  it('forwards every existing text-row interaction with the original file', () => {
    const onSelect = vi.fn()
    const onPreview = vi.fn()
    const onRadialMenuPointerDown = vi.fn()
    const onRadialMenuContextMenu = vi.fn()
    const onFinderDragStart = vi.fn()
    const onOrganizationPointerDown = vi.fn()
    const onOrganizationPointerMove = vi.fn()
    const onOrganizationPointerUp = vi.fn()
    const onOrganizationPointerCancel = vi.fn()
    renderPanel({
      mode: 'text_only',
      onSelect,
      onPreview,
      onRadialMenuPointerDown,
      onRadialMenuContextMenu,
      onFinderDragStart,
      onOrganizationPointerDown,
      onOrganizationPointerMove,
      onOrganizationPointerUp,
      onOrganizationPointerCancel,
    })
    const row = screen.getByRole('option', { name: 'first.md' })
    const exportSurface = row.querySelector<HTMLElement>('.file-export-surface')
    if (exportSurface === null) throw new Error('Expected Finder export surface')
    const handle = screen.getByRole('button', { name: '整理 first.md' })

    fireEvent.click(row)
    fireEvent.doubleClick(row)
    fireEvent.pointerDown(row, { pointerId: 11, button: 2 })
    fireEvent.contextMenu(row, { button: 2 })
    fireEvent.dragStart(exportSurface)
    fireEvent.pointerDown(handle, { pointerId: 12, button: 0 })
    fireEvent.pointerMove(handle, { pointerId: 12 })
    fireEvent.pointerUp(handle, { pointerId: 12 })
    fireEvent.pointerDown(handle, { pointerId: 13, button: 0 })
    fireEvent.pointerCancel(handle, { pointerId: 13 })

    const first = textFiles[0]
    expect(onSelect).toHaveBeenCalledWith(first, expect.anything())
    expect(onPreview).toHaveBeenCalledWith(first)
    expect(onRadialMenuPointerDown).toHaveBeenCalledWith(first, expect.anything())
    expect(onRadialMenuContextMenu).toHaveBeenCalledWith(first, expect.anything())
    expect(onFinderDragStart).toHaveBeenCalledWith(first, expect.anything())
    expect(onOrganizationPointerDown).toHaveBeenCalledWith(first, expect.anything())
    expect(onOrganizationPointerMove).toHaveBeenCalledWith(expect.anything())
    expect(onOrganizationPointerUp).toHaveBeenCalledWith(expect.anything())
    expect(onOrganizationPointerCancel).toHaveBeenCalledWith(expect.anything())
  })

  it('owns only text active-descendant state and preserves marker labels', () => {
    renderPanel({ mode: 'text_only', activeId: 'text-1' })

    expect(screen.getByRole('listbox', { name: '文本文件' })).toHaveAttribute(
      'aria-activedescendant',
      'file-text-1',
    )
    expect(screen.getByText('保留 · 收藏')).toBeVisible()
  })
})
