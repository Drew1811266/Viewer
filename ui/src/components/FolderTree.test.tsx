import { createEvent, fireEvent, render, screen } from '@testing-library/react'
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

  it('highlights valid move/copy targets and emits the same entity-only drop intent', () => {
    const drop = vi.fn()
    render(
      <FolderTree
        folders={folders}
        selectedId={null}
        onSelect={vi.fn()}
        draggedEntityIds={['file-1', 'file-2']}
        onDropFiles={drop}
      />,
    )
    const target = screen.getByRole('treeitem', { name: 'empty' })
    const transfer = { types: ['application/x-viewer-selection'], dropEffect: 'none' }
    fireEvent.dragOver(target, { dataTransfer: transfer })
    expect(target).toHaveAttribute('data-drop-mode', 'move')
    const copyOver = createEvent.dragOver(target, { dataTransfer: transfer })
    Object.defineProperty(copyOver, 'altKey', { value: true })
    fireEvent(target, copyOver)
    expect(target).toHaveAttribute('data-drop-mode', 'copy')
    const copyDrop = createEvent.drop(target, { dataTransfer: transfer })
    Object.defineProperty(copyDrop, 'altKey', { value: true })
    fireEvent(target, copyDrop)
    expect(drop).toHaveBeenCalledWith(['file-1', 'file-2'], '4', 'copy')
  })

  it('rejects read-only and explicitly invalid internal drop targets', () => {
    const drop = vi.fn()
    const rendered = render(
      <FolderTree
        folders={folders}
        selectedId={null}
        onSelect={vi.fn()}
        draggedEntityIds={['file-1']}
        readOnly
        onDropFiles={drop}
      />,
    )
    const target = screen.getByRole('treeitem', { name: 'empty' })
    const transfer = { types: ['application/x-viewer-selection'], dropEffect: 'none' }
    fireEvent.dragOver(target, { dataTransfer: transfer })
    fireEvent.drop(target, { dataTransfer: transfer })
    expect(target).not.toHaveAttribute('data-drop-mode')
    expect(drop).not.toHaveBeenCalled()

    rendered.rerender(
      <FolderTree
        folders={folders}
        selectedId={null}
        onSelect={vi.fn()}
        draggedEntityIds={['file-1']}
        invalidDropTargetIds={['4']}
        onDropFiles={drop}
      />,
    )
    fireEvent.dragOver(target, { dataTransfer: transfer })
    fireEvent.drop(target, { dataTransfer: transfer })
    expect(target).toHaveAttribute('data-drop-invalid', 'true')
    expect(drop).not.toHaveBeenCalled()
  })

  it('validates the destination against the current move/copy mode', () => {
    const drop = vi.fn()
    const validate = vi.fn((_folderId: string, mode: 'move' | 'copy') => mode === 'copy')
    render(
      <FolderTree
        folders={folders}
        selectedId={null}
        onSelect={vi.fn()}
        draggedEntityIds={['file-1']}
        isDropTargetValid={validate}
        onDropFiles={drop}
      />,
    )
    const target = screen.getByRole('treeitem', { name: 'empty' })
    const transfer = { types: ['application/x-viewer-selection'], dropEffect: 'none' }

    fireEvent.dragOver(target, { dataTransfer: transfer })
    expect(target).toHaveAttribute('data-drop-invalid', 'true')
    fireEvent.drop(target, { dataTransfer: transfer })
    expect(drop).not.toHaveBeenCalled()

    const copyOver = createEvent.dragOver(target, { dataTransfer: transfer })
    Object.defineProperty(copyOver, 'altKey', { value: true })
    fireEvent(target, copyOver)
    expect(target).toHaveAttribute('data-drop-mode', 'copy')
    const copyDrop = createEvent.drop(target, { dataTransfer: transfer })
    Object.defineProperty(copyDrop, 'altKey', { value: true })
    fireEvent(target, copyDrop)
    expect(drop).toHaveBeenCalledWith(['file-1'], '4', 'copy')
    expect(validate).toHaveBeenCalledWith('4', 'move')
    expect(validate).toHaveBeenCalledWith('4', 'copy')
  })

  it('clears a highlighted target when the drag ends', () => {
    const rendered = render(
      <FolderTree
        folders={folders}
        selectedId={null}
        onSelect={vi.fn()}
        draggedEntityIds={['file-1']}
      />,
    )
    const target = screen.getByRole('treeitem', { name: 'empty' })
    fireEvent.dragOver(target, {
      dataTransfer: { types: ['application/x-viewer-selection'], dropEffect: 'none' },
    })
    expect(target).toHaveAttribute('data-drop-mode', 'move')

    rendered.rerender(
      <FolderTree folders={folders} selectedId={null} onSelect={vi.fn()} draggedEntityIds={[]} />,
    )

    expect(target).not.toHaveAttribute('data-drop-mode')
    expect(target).not.toHaveAttribute('data-drop-invalid')
  })
})
