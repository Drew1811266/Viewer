import { render, screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { BrowserFile } from '../api/types'
import InfoOverlay from './InfoOverlay'

const image: BrowserFile = {
  entityId: '1',
  relativePath: 'catalog/id-1/front.jpg',
  name: 'front.jpg',
  kind: 'jpeg',
  size: 1_024,
  modifiedNs: '1000000',
  marker: { reviewState: null, favorite: false },
  imageMetadata: null,
  imageUrl: null,
}

describe('InfoOverlay', () => {
  it('shows on-demand single-file metadata including known dimensions', () => {
    render(
      <InfoOverlay
        files={[image]}
        dimensions={{ '1': { width: 1200, height: 800 } }}
        onClose={vi.fn()}
      />,
    )

    const inspector = screen.getByRole('complementary', { name: '文件信息' })
    expect(inspector).toHaveClass('info-overlay')
    expect(within(inspector).getByRole('group', { name: '身份与位置' })).toBeVisible()
    expect(within(inspector).getByRole('group', { name: '审阅信息' })).toBeVisible()
    expect(within(inspector).getByRole('group', { name: '技术信息' })).toBeVisible()
    expect(inspector).not.toHaveAttribute('aria-modal')
    expect(within(inspector).getByText('catalog/id-1/front.jpg')).toBeVisible()
    expect(within(inspector).getByText('1200 × 800')).toBeVisible()
    expect(within(inspector).getByText('1 KiB')).toBeVisible()
  })

  it('summarizes count total size and kind distribution for multiple files', () => {
    render(
      <InfoOverlay
        files={[image, { ...image, entityId: '2', kind: 'png', size: 2_048 }]}
        dimensions={{}}
        onClose={vi.fn()}
      />,
    )

    expect(screen.getByText('2 个文件')).toBeVisible()
    expect(screen.getByText('3 KiB')).toBeVisible()
    expect(screen.getByText('JPEG 1 · PNG 1')).toBeVisible()
  })

  it.each([
    ['unsupported_image', '其它图片'],
    ['other', '其它文件'],
  ] as const)('uses a Chinese single-file label for %s', (kind, label) => {
    render(
      <InfoOverlay
        files={[{ ...image, entityId: kind, kind }]}
        dimensions={{}}
        onClose={vi.fn()}
      />,
    )

    expect(screen.getByText(label)).toBeVisible()
  })

  it('uses Chinese labels for appended kinds in the multi-file fallback summary', () => {
    render(
      <InfoOverlay
        files={[
          { ...image, entityId: 'unsupported', kind: 'unsupported_image' },
          { ...image, entityId: 'other', kind: 'other' },
        ]}
        dimensions={{}}
        onClose={vi.fn()}
      />,
    )

    expect(screen.getByText('其它图片 1 · 其它文件 1')).toBeVisible()
  })

  it('uses aggregate selection information for folders/files and common markers', () => {
    render(
      <InfoOverlay
        files={[image, { ...image, entityId: '2', kind: 'png', size: 2_048 }]}
        selectionInfo={{
          relativePaths: ['catalog/id-1', 'catalog/id-1/front.jpg'],
          totalSize: 3_072,
          types: { folders: 1, images: 1, otherFiles: 0 },
          commonReview: { state: 'common', value: 'keep' },
          commonFavorite: { state: 'mixed' },
        }}
        dimensions={{}}
        onClose={vi.fn()}
      />,
    )
    expect(screen.getByText('2 个项目')).toBeVisible()
    expect(screen.getByText('文件夹 1 · 图片 1 · 其它文件 0')).toBeVisible()
    expect(screen.getByText('保留')).toBeVisible()
    expect(screen.getByText('混合')).toBeVisible()
    expect(screen.queryByText(/\/Users\//)).not.toBeInTheDocument()
  })
})
