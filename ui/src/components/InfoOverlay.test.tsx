import { render, screen } from '@testing-library/react'
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

    expect(screen.getByText('catalog/id-1/front.jpg')).toBeVisible()
    expect(screen.getByText('1200 × 800')).toBeVisible()
    expect(screen.getByText('1 KiB')).toBeVisible()
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
})
