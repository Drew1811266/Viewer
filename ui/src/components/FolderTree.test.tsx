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
    marker: { reviewState: null, favorite: false },
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
})
