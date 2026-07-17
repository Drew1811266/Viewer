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
  it('shows metadata before four representative thumbnails resolve', () => {
    const requestImage = vi.fn(() => new Promise<string>(() => undefined))
    render(
      <FolderOverview
        folders={[card]}
        currentPath="catalog/shoes"
        requestThumbnail={requestImage}
        onSelect={vi.fn()}
        onShowAll={vi.fn()}
      />,
    )

    expect(screen.getByText('4 张图片')).toBeVisible()
    expect(screen.getByText('1 个文本')).toBeVisible()
    expect(screen.getAllByLabelText('缩略图加载中')).toHaveLength(4)
  })

  it('offers an explicit aggregate view without changing the displayed category path', () => {
    const showAll = vi.fn()
    const select = vi.fn()
    render(
      <FolderOverview
        folders={[card]}
        currentPath="catalog/shoes"
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
})
