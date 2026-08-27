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
  videoCount: 0,
  otherFileCount: 1,
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
    videoMetadata: null,
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
        density="standard"
        requestFolderImages={vi.fn().mockResolvedValue([])}
        onPreview={vi.fn()}
        onSelect={vi.fn()}
      />,
    )

    const rows = screen.getAllByRole('article')
    expect(rows).toHaveLength(2)
    expect(rows[0]).toHaveTextContent('id-001')
    expect(rows[1]).toHaveTextContent('B02')
    expect(screen.queryByText(/保留 0 · 待定/)).not.toBeInTheDocument()
  })

  it('keeps folder navigation actions unchanged', () => {
    const select = vi.fn()
    render(
      <FolderOverview
        folders={[card]}
        density="standard"
        requestFolderImages={vi.fn().mockResolvedValue([])}
        onPreview={vi.fn()}
        onSelect={select}
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: '打开 id-001' }))
    expect(select).toHaveBeenCalledWith('folder-1')
  })

  it('reuses a completed folder request while the overview remains mounted', async () => {
    const requestFolderImages = vi.fn().mockResolvedValue(card.representativeImages)
    const props = {
      density: 'standard' as const,
      requestFolderImages,
      onPreview: vi.fn(),
      onSelect: vi.fn(),
    }
    const rendered = render(<FolderOverview {...props} folders={[card]} />)

    await screen.findByRole('button', { name: '预览 1.jpg' })
    rendered.rerender(<FolderOverview {...props} folders={[]} />)
    rendered.rerender(<FolderOverview {...props} folders={[card]} />)
    await screen.findByRole('button', { name: '预览 1.jpg' })

    expect(requestFolderImages).toHaveBeenCalledOnce()
  })

  it('passes the selected density through every folder row', async () => {
    const secondCard = {
      ...card,
      entityId: 'folder-2',
      relativePath: 'catalog/shoes/B02',
      name: 'B02',
    }
    const square = {
      ...card.representativeImages[0],
      imageMetadata: { width: 1, height: 1 },
    }
    render(
      <FolderOverview
        folders={[card, secondCard]}
        density="compact"
        requestFolderImages={vi.fn().mockResolvedValue([square])}
        requestThumbnail={vi.fn().mockResolvedValue('viewer-image://thumbnail')}
        onPreview={vi.fn()}
        onSelect={vi.fn()}
      />,
    )

    await screen.findAllByRole('button', { name: '预览 1.jpg' })
    const tracks = document.querySelectorAll<HTMLElement>('.folder-filmstrip-track')
    const items = document.querySelectorAll<HTMLElement>('.folder-filmstrip-item')
    expect(tracks).toHaveLength(2)
    expect(items).toHaveLength(2)
    for (const track of tracks) expect(track).toHaveStyle({ height: '96px' })
    for (const item of items) expect(item).toHaveStyle({ width: '96px', height: '96px' })
  })

  it('passes review feedback counts into loaded folder filmstrips', async () => {
    render(
      <FolderOverview
        folders={[card]}
        density="standard"
        requestFolderImages={vi.fn().mockResolvedValue(card.representativeImages)}
        onPreview={vi.fn()}
        onSelect={vi.fn()}
        feedbackCountByEntityId={new Map([['1', 2]])}
      />,
    )

    await screen.findByRole('button', { name: '预览 1.jpg' })
    expect(screen.getByText('返工 · 2 条')).toBeVisible()
  })
})
