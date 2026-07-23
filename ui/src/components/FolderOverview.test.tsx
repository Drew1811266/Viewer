import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { ContentFolderCard } from '../api/types'
import FolderOverview from './FolderOverview'

const card: ContentFolderCard = {
  entityId: 'folder-1',
  relativePath: 'catalog/shoes/id-001',
  name: 'id-001',
  marker: { reviewState: null, favorite: false },
  imageCount: 4,
  textCount: 1,
  reviewProgress: {
    total: 5,
    keep: 0,
    pending: 0,
    reject: 0,
    unmarked: 5,
    favorite: 0,
  },
  representativeImages: ['1', '2', '3', '4'].map((entityId) => ({
    entityId,
    relativePath: `catalog/shoes/id-001/${entityId}.jpg`,
    name: `${entityId}.jpg`,
    kind: 'jpeg' as const,
    size: 10,
    modifiedNs: '1',
    marker: { reviewState: null, favorite: false },
    imageMetadata: null,
    imageUrl: null,
  })),
}

describe('FolderOverview', () => {
  it('renders one row per folder in source order with compact metadata', async () => {
    const secondCard = {
      ...card,
      entityId: 'folder-2',
      relativePath: 'catalog/shoes/B02',
      name: 'B02',
    }
    render(
      <FolderOverview
        folders={[card, secondCard]}
        currentPath="catalog/shoes"
        requestFolderImages={vi.fn().mockResolvedValue([])}
        onPreview={vi.fn()}
        onSelect={vi.fn()}
        onShowAll={vi.fn()}
      />,
    )

    const rows = screen.getAllByRole('article')
    expect(rows).toHaveLength(2)
    expect(rows[0]).toHaveTextContent('id-001')
    expect(rows[1]).toHaveTextContent('B02')
    expect(screen.queryByText(/保留 0 · 待定/)).not.toBeInTheDocument()
  })

  it('keeps aggregate and folder navigation actions unchanged', () => {
    const showAll = vi.fn()
    const select = vi.fn()
    render(
      <FolderOverview
        folders={[card]}
        currentPath="catalog/shoes"
        requestFolderImages={vi.fn().mockResolvedValue([])}
        onPreview={vi.fn()}
        onSelect={select}
        onShowAll={showAll}
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: '显示全部后代文件' }))
    expect(showAll).toHaveBeenCalledOnce()
    expect(screen.getByText('catalog/shoes')).toBeVisible()

    fireEvent.click(screen.getByRole('button', { name: '打开 id-001' }))
    expect(select).toHaveBeenCalledWith('folder-1')
  })

  it('reuses a completed folder request while the overview remains mounted', async () => {
    const requestFolderImages = vi.fn().mockResolvedValue(card.representativeImages)
    const props = {
      currentPath: 'catalog/shoes',
      requestFolderImages,
      onPreview: vi.fn(),
      onSelect: vi.fn(),
      onShowAll: vi.fn(),
    }
    const rendered = render(<FolderOverview {...props} folders={[card]} />)

    await screen.findByRole('button', { name: '预览 1.jpg' })
    rendered.rerender(<FolderOverview {...props} folders={[]} />)
    rendered.rerender(<FolderOverview {...props} folders={[card]} />)
    await screen.findByRole('button', { name: '预览 1.jpg' })

    expect(requestFolderImages).toHaveBeenCalledOnce()
  })
})
