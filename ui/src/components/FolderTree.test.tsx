import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { FolderTreeItem } from '../api/types'
import FolderTree from './FolderTree'

const folders: FolderTreeItem[] = [
  {
    entityId: '1',
    parentEntityId: null,
    relativePath: 'catalog',
    name: 'catalog',
    marker: { reviewState: null, favorite: false },
  },
  {
    entityId: '2',
    parentEntityId: '1',
    relativePath: 'catalog/shoes',
    name: 'shoes',
    marker: { reviewState: null, favorite: false },
  },
  {
    entityId: '3',
    parentEntityId: '2',
    relativePath: 'catalog/shoes/id-001',
    name: 'id-001',
    marker: { reviewState: null, favorite: false },
  },
  {
    entityId: '4',
    parentEntityId: null,
    relativePath: 'empty',
    name: 'empty',
    marker: { reviewState: 'keep', favorite: true },
  },
]

describe('FolderTree', () => {
  it('renders only folders at arbitrary depth with full relative-path labels', () => {
    render(<FolderTree folders={folders} selectedId="3" onSelect={vi.fn()} />)

    expect(screen.getByText('catalog/shoes/id-001')).toBeVisible()
    expect(screen.getByText('empty')).toBeVisible()
    expect(screen.queryByText('front.jpg')).not.toBeInTheDocument()
    expect(screen.queryByText('.viewer')).not.toBeInTheDocument()
    expect(screen.getByRole('treeitem', { name: 'catalog/shoes/id-001' })).toHaveAttribute(
      'aria-selected',
      'true',
    )
  })

  it('collapses descendants and calls selection without losing the selected id', () => {
    const onSelect = vi.fn()
    render(<FolderTree folders={folders} selectedId="3" onSelect={onSelect} />)

    fireEvent.click(screen.getByRole('button', { name: '折叠 catalog' }))
    expect(screen.queryByText('catalog/shoes/id-001')).not.toBeInTheDocument()

    fireEvent.click(screen.getByRole('treeitem', { name: 'empty' }))
    expect(onSelect).toHaveBeenCalledWith('4')
    expect(screen.getByLabelText('empty：保留，已收藏')).toBeVisible()
  })

  it('keeps a large flat hierarchy bounded to the visible virtual window', () => {
    const many = Array.from({ length: 1_000 }, (_, index) => ({
      entityId: String(index),
      parentEntityId: null,
      relativePath: `folder-${String(index).padStart(4, '0')}`,
      name: `folder-${index}`,
      marker: { reviewState: null, favorite: false },
    }))
    render(<FolderTree folders={many} selectedId={null} onSelect={vi.fn()} height={280} />)

    expect(screen.getAllByRole('treeitem').length).toBeLessThanOrEqual(20)
  })

  it('marks the scrolling viewport and visible rows with opaque organization ids', () => {
    const { container } = render(
      <FolderTree folders={folders} selectedId={null} onSelect={vi.fn()} />,
    )

    expect(container.querySelector('[data-organization-drop-surface]')).toHaveAttribute(
      'data-organization-drop-surface',
      '',
    )
    expect(
      screen.getAllByRole('treeitem').map((row) => row.getAttribute('data-organization-folder-id')),
    ).toEqual(['1', '2', '3', '4'])
  })

  it('renders only the matching controlled valid drop target', () => {
    render(
      <FolderTree
        folders={folders}
        selectedId={null}
        onSelect={vi.fn()}
        organizationDropTarget={{ entityId: '4', mode: 'copy', valid: true }}
      />,
    )
    const target = screen.getByRole('treeitem', { name: 'empty' })
    expect(target).toHaveAttribute('data-drop-mode', 'copy')
    expect(screen.getByRole('treeitem', { name: 'catalog' })).not.toHaveAttribute('data-drop-mode')
  })

  it('renders only the matching controlled invalid drop target and clears declaratively', () => {
    const rendered = render(
      <FolderTree
        folders={folders}
        selectedId={null}
        onSelect={vi.fn()}
        organizationDropTarget={{ entityId: '4', mode: 'move', valid: false }}
      />,
    )
    const target = screen.getByRole('treeitem', { name: 'empty' })
    expect(target).toHaveAttribute('data-drop-invalid', 'true')
    expect(screen.getByRole('treeitem', { name: 'catalog' })).not.toHaveAttribute(
      'data-drop-invalid',
    )

    rendered.rerender(
      <FolderTree
        folders={folders}
        selectedId={null}
        onSelect={vi.fn()}
        organizationDropTarget={null}
      />,
    )
    expect(target).not.toHaveAttribute('data-drop-mode')
    expect(target).not.toHaveAttribute('data-drop-invalid')
  })
})
