import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { BrowserFile, ImageRepresentationRequest } from '../api/types'
import ImagePreview from './ImagePreview'

function image(index: number): BrowserFile {
  return {
    entityId: `image-${index}`,
    relativePath: `id-1/${index}.jpg`,
    name: `${index}.jpg`,
    kind: 'jpeg',
    size: index * 100,
    modifiedNs: String(index),
    marker: { reviewState: null, favorite: false },
    imageMetadata: null,
    imageUrl: null,
  }
}

describe('ImagePreview', () => {
  it('starts fitted, rotates for display, and requests original only for explicit 100%', async () => {
    const files = [image(1), image(2), image(3)]
    const request = vi.fn(
      async (file: BrowserFile, representation: ImageRepresentationRequest) => ({
        cacheKey: `${file.entityId}:${representation.kind}`,
        url: `viewer-image://localhost/session/${file.entityId}-${representation.kind}`,
        width: 1600,
        height: 1200,
        backend: 'image_io' as const,
      }),
    )
    render(
      <ImagePreview
        file={files[1]!}
        files={files}
        requestImage={request}
        onNavigate={vi.fn()}
        onClose={vi.fn()}
      />,
    )

    const preview = await screen.findByRole('img', { name: '2.jpg' })
    expect(preview).toHaveAttribute('data-mode', 'fit')
    expect(request.mock.calls.every((call) => call[1].kind === 'fit_preview')).toBe(true)

    fireEvent.click(screen.getByRole('button', { name: '顺时针旋转' }))
    expect(preview).toHaveStyle({ transform: 'rotate(90deg) scale(1)' })

    fireEvent.click(screen.getByRole('button', { name: '按 100% 显示' }))
    await waitFor(() =>
      expect(request).toHaveBeenCalledWith(
        files[1]!,
        expect.objectContaining({ kind: 'original100_percent' }),
      ),
    )
  })

  it('navigates in stable content order and prefetches no more than three fit images', async () => {
    const files = [image(1), image(2), image(3), image(4)]
    const navigate = vi.fn()
    const request = vi.fn(async (file: BrowserFile) => ({
      cacheKey: file.entityId,
      url: `viewer-image://localhost/session/${file.entityId}`,
      width: 800,
      height: 600,
      backend: 'quick_look' as const,
    }))
    render(
      <ImagePreview
        file={files[1]!}
        files={files}
        requestImage={request}
        onNavigate={navigate}
        onClose={vi.fn()}
      />,
    )
    await screen.findByRole('img', { name: '2.jpg' })

    fireEvent.keyDown(screen.getByRole('dialog'), { key: 'ArrowRight' })

    expect(navigate).toHaveBeenCalledWith(files[2])
    expect(request).toHaveBeenCalledTimes(3)
  })
})
