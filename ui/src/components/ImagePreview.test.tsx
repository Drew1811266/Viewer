import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { BrowserFile, ImageRepresentationRequest } from '../api/types'
import { defined } from '../defined'
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
  it('groups image controls in the toolbar and floats navigation over the stage', () => {
    const front = {
      ...image(1),
      relativePath: 'id-1/front.jpg',
      name: 'front.jpg',
    }

    render(
      <ImagePreview
        file={front}
        files={[front]}
        requestImage={vi.fn(() => new Promise<never>(() => undefined))}
        onNavigate={vi.fn()}
        onClose={vi.fn()}
      />,
    )

    const dialog = screen.getByRole('dialog', { name: /front\.jpg/ })
    expect(within(dialog).getByRole('toolbar', { name: '图片显示控制' })).toBeVisible()
    expect(within(dialog).getByRole('navigation', { name: '图片导航' })).toHaveClass(
      'preview-navigation-float',
    )
    expect(within(dialog).queryByText('front.jpg')?.closest('footer')).toBeNull()
  })

  it('renders an unsupported current image without issuing image requests', () => {
    const unsupported = {
      ...image(1),
      entityId: 'raw',
      relativePath: 'id-1/raw.cr2',
      name: 'raw.cr2',
      kind: 'unsupported_image' as const,
    }
    const request = vi.fn(() => new Promise<never>(() => undefined))

    render(
      <ImagePreview
        file={unsupported}
        files={[unsupported]}
        requestImage={request}
        onNavigate={vi.fn()}
        onClose={vi.fn()}
      />,
    )

    expect(screen.getByLabelText('raw.cr2 .CR2 暂不支持预览')).toBeVisible()
    expect(screen.getByRole('button', { name: '适应窗口' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '按 100% 显示' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '缩小' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '放大' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '顺时针旋转' })).toBeDisabled()
    expect(request).not.toHaveBeenCalled()
  })

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
        file={defined(files[1], 'Expected second preview image')}
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
        defined(files[1], 'Expected second preview image'),
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
        file={defined(files[1], 'Expected second preview image')}
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

  it('hides a watcher-removed current image without new requests and preserves navigation', async () => {
    const files = [image(1), image(2), image(3)]
    const navigate = vi.fn()
    const request = vi.fn(async (file: BrowserFile) => ({
      cacheKey: file.entityId,
      url: `viewer-image://localhost/session/${file.entityId}`,
      width: 800,
      height: 600,
      backend: 'quick_look' as const,
    }))
    const rendered = render(
      <ImagePreview
        file={defined(files[1], 'Expected second preview image')}
        files={files}
        requestImage={request}
        onNavigate={navigate}
        onClose={vi.fn()}
      />,
    )
    await screen.findByRole('img', { name: '2.jpg' })
    const requestsBeforeRemoval = request.mock.calls.length

    rendered.rerender(
      <ImagePreview
        file={defined(files[1], 'Expected second preview image')}
        files={files}
        unavailableEntityIds={new Set(['image-2'])}
        requestImage={request}
        onNavigate={navigate}
        onClose={vi.fn()}
      />,
    )

    expect(screen.getByLabelText('2.jpg .JPG 文件已不可用')).toBeVisible()
    expect(screen.queryByRole('img', { name: '2.jpg' })).not.toBeInTheDocument()
    expect(request).toHaveBeenCalledTimes(requestsBeforeRemoval)
    expect(screen.getByRole('button', { name: '上一张' })).toBeEnabled()
    expect(screen.getByRole('button', { name: '下一张' })).toBeEnabled()
    fireEvent.click(screen.getByRole('button', { name: '上一张' }))
    fireEvent.click(screen.getByRole('button', { name: '下一张' }))
    expect(navigate.mock.calls.map(([file]) => file.entityId)).toEqual(['image-1', 'image-3'])
  })
})
