import { fireEvent, render, screen, waitFor } from '@testing-library/react'
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
    keep: 1,
    pending: 1,
    reject: 1,
    unmarked: 2,
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
  it('renders one horizontal filmstrip row per folder with the project summary', async () => {
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
        projectName="测试图"
        requestFolderImages={vi.fn().mockResolvedValue(card.representativeImages)}
        onPreview={vi.fn()}
        onSelect={vi.fn()}
      />,
    )

    expect(screen.getByRole('heading', { level: 1, name: '测试图' })).toBeVisible()
    expect(screen.getByText('2 个文件夹 · 10 个素材')).toBeVisible()
    const rows = screen.getAllByRole('article')
    expect(rows).toHaveLength(2)
    expect(rows[0]).toHaveClass('folder-filmstrip-row')
    expect(rows[1]).toHaveClass('folder-filmstrip-row')
    expect(rows[0]).toHaveTextContent('id-001')
    expect(rows[1]).toHaveTextContent('B02')
    await waitFor(() => expect(rows[0]).toHaveTextContent('4 张图片 · 1 个其它文件'))
    expect(rows[0]).toHaveTextContent('已审阅 3 / 5')
    expect(screen.getByText('横向滚动查看文件夹内容；纵向滚动浏览全部素材。')).toBeVisible()
  })

  it('opens the folder from the row identity', () => {
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

  it('previews a filmstrip image on double click without navigating', async () => {
    const preview = vi.fn()
    render(
      <FolderOverview
        folders={[card]}
        density="standard"
        requestFolderImages={vi.fn().mockResolvedValue(card.representativeImages)}
        onPreview={preview}
        onSelect={vi.fn()}
      />,
    )

    const image = await screen.findByRole('button', { name: '预览 1.jpg' })
    fireEvent.click(image)
    expect(preview).not.toHaveBeenCalled()
    fireEvent.doubleClick(image)
    expect(preview).toHaveBeenCalledWith(card.representativeImages[0], card.representativeImages)
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

  it('passes the selected density through every folder filmstrip', async () => {
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
    expect(screen.getByText(/返工/)).toHaveTextContent('返工 · 2')
  })

  it('shows review state and an empty folder without inventing activity', async () => {
    const emptyCard: ContentFolderCard = {
      ...card,
      entityId: 'folder-empty',
      name: '空目录',
      marker: { reviewState: 'pending', favorite: false },
      imageCount: 0,
      videoCount: 0,
      otherFileCount: 0,
      reviewProgress: {
        total: 0,
        keep: 0,
        pending: 0,
        reject: 0,
        unmarked: 0,
        favorite: 0,
      },
      representativeImages: [],
    }
    render(
      <FolderOverview
        folders={[emptyCard]}
        density="standard"
        requestFolderImages={vi.fn().mockResolvedValue([])}
        onPreview={vi.fn()}
        onSelect={vi.fn()}
      />,
    )

    expect(screen.getByText('待定')).toBeVisible()
    expect(screen.getByText('0 个文件')).toBeVisible()
    expect(screen.getByText('暂无审阅')).toBeVisible()
    expect(await screen.findByText('暂无图片')).toBeVisible()
    expect(screen.getByText('把图片放进这个文件夹后会显示在这里')).toBeVisible()
  })
})
