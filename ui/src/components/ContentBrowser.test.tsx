import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { BrowserFile, FolderWorkspace } from '../api/types'
import ContentBrowser from './ContentBrowser'

function image(index: number): BrowserFile {
  return {
    entityId: `image-${index}`,
    relativePath: `id-001/${index}.jpg`,
    name: `${index}.jpg`,
    kind: 'jpeg',
    size: index * 10,
    modifiedNs: String(index),
    marker: { reviewState: null, favorite: false },
    imageMetadata: null,
    imageUrl: null,
  }
}

function workspace(count = 10): Extract<FolderWorkspace, { workspace: 'content' }> {
  return {
    workspace: 'content',
    images: Array.from({ length: count }, (_, index) => image(index + 1)),
    textFiles: [
      {
        entityId: 'text-1',
        relativePath: 'id-001/prompt.md',
        name: 'prompt.md',
        kind: 'markdown',
        size: 40,
        modifiedNs: '11',
        marker: { reviewState: null, favorite: false },
        imageMetadata: null,
        imageUrl: null,
      },
    ],
  }
}

describe('ContentBrowser', () => {
  it('supports click command-toggle shift-range arrows and Space preview', () => {
    const preview = vi.fn()
    render(<ContentBrowser workspace={workspace()} onPreview={preview} />)

    fireEvent.click(screen.getByRole('option', { name: '1.jpg' }))
    fireEvent.click(screen.getByRole('option', { name: '3.jpg' }), { metaKey: true })
    fireEvent.click(screen.getByRole('option', { name: '6.jpg' }), { shiftKey: true })
    const selected = screen
      .getAllByRole('option')
      .filter((item) => item.getAttribute('aria-selected') === 'true')
      .map((item) => item.getAttribute('aria-label'))
    expect(selected).toEqual(['1.jpg', '3.jpg', '4.jpg', '5.jpg', '6.jpg'])

    const grid = screen.getByRole('listbox', { name: '图片文件' })
    fireEvent.keyDown(grid, { key: 'ArrowRight' })
    fireEvent.keyDown(grid, { key: ' ' })
    expect(preview).toHaveBeenCalledWith(expect.objectContaining({ entityId: 'image-7' }))
  })

  it('keeps Markdown and TXT in an independent labelled list', () => {
    render(<ContentBrowser workspace={workspace()} />)

    expect(screen.getByText('文本文件')).toBeVisible()
    expect(screen.getByRole('option', { name: 'prompt.md' })).toBeVisible()
  })

  it('shows text marker labels and preserves selection when marker projections refresh', () => {
    const changed = vi.fn()
    const rendered = render(
      <ContentBrowser workspace={workspace(2)} onSelectionChange={changed} />,
    )
    fireEvent.click(screen.getByRole('option', { name: '1.jpg' }))
    const updated = workspace(2)
    updated.images[0] = {
      ...updated.images[0]!,
      marker: { reviewState: 'keep', favorite: true },
    }
    rendered.rerender(
      <ContentBrowser workspace={updated} onSelectionChange={changed} />,
    )
    expect(screen.getByRole('option', { name: '1.jpg' })).toHaveAttribute(
      'aria-selected',
      'true',
    )
    expect(screen.getByText('保留 · 收藏')).toBeVisible()
  })

  it('mounts and requests thumbnails only for a bounded visible window', async () => {
    const requestThumbnail = vi.fn(() => new Promise<string>(() => undefined))
    render(
      <ContentBrowser
        workspace={workspace(1_000)}
        viewportHeight={420}
        requestThumbnail={requestThumbnail}
      />,
    )

    const imageGrid = screen.getByRole('listbox', { name: '图片文件' })
    expect(imageGrid.querySelectorAll('[role="option"]').length).toBeLessThan(50)
    await waitFor(() => expect(requestThumbnail).toHaveBeenCalled())
    expect(requestThumbnail.mock.calls.length).toBeLessThan(50)
  })
})
