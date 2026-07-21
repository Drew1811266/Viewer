import { createEvent, fireEvent, render, screen, waitFor } from '@testing-library/react'
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

function selectedLabels(): string[] {
  return screen
    .getAllByRole('option')
    .filter((item) => item.getAttribute('aria-selected') === 'true')
    .map((item) => item.getAttribute('aria-label')!)
}

function imageGrid(): HTMLElement {
  const grid = screen.getByRole('listbox', { name: '图片文件' })
  vi.spyOn(grid, 'getBoundingClientRect').mockReturnValue({
    left: 0,
    top: 0,
    right: 900,
    bottom: 520,
    width: 900,
    height: 520,
    x: 0,
    y: 0,
    toJSON: () => undefined,
  })
  Object.defineProperty(grid, 'setPointerCapture', { value: vi.fn(), configurable: true })
  Object.defineProperty(grid, 'releasePointerCapture', { value: vi.fn(), configurable: true })
  return grid
}

function marqueeImages({
  start,
  end,
  metaKey = false,
}: {
  start: [number, number]
  end: [number, number]
  metaKey?: boolean
}) {
  const grid = imageGrid()
  fireEvent.pointerDown(grid, {
    pointerId: 41,
    button: 0,
    metaKey,
    clientX: start[0],
    clientY: start[1],
  })
  fireEvent.pointerMove(grid, {
    pointerId: 41,
    metaKey: false,
    clientX: end[0],
    clientY: end[1],
  })
}

function finishMarquee(end: [number, number]) {
  fireEvent.pointerUp(screen.getByRole('listbox', { name: '图片文件' }), {
    pointerId: 41,
    clientX: end[0],
    clientY: end[1],
  })
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

  it('extends a contiguous selection with Shift plus an arrow key', () => {
    render(<ContentBrowser workspace={workspace(5)} />)

    fireEvent.click(screen.getByRole('option', { name: '1.jpg' }))
    const grid = screen.getByRole('listbox', { name: '图片文件' })
    fireEvent.keyDown(grid, { key: 'ArrowRight', shiftKey: true })
    fireEvent.keyDown(grid, { key: 'ArrowRight', shiftKey: true })

    const selected = screen
      .getAllByRole('option')
      .filter((item) => item.getAttribute('aria-selected') === 'true')
      .map((item) => item.getAttribute('aria-label'))
    expect(selected).toEqual(['1.jpg', '2.jpg', '3.jpg'])
  })

  it('selects every file in the current folder with Command-A', () => {
    render(<ContentBrowser workspace={workspace(3)} />)

    const grid = screen.getByRole('listbox', { name: '图片文件' })
    fireEvent.keyDown(grid, { key: 'a', metaKey: true })

    expect(
      screen
        .getAllByRole('option')
        .filter((item) => item.getAttribute('aria-selected') === 'true'),
    ).toHaveLength(4)
  })

  it('offers an explicit select-all action for mouse and assistive users', () => {
    render(<ContentBrowser workspace={workspace(3)} />)

    fireEvent.click(screen.getByRole('button', { name: '全选当前文件夹' }))

    expect(
      screen
        .getAllByRole('option')
        .filter((item) => item.getAttribute('aria-selected') === 'true'),
    ).toHaveLength(4)
  })

  it('moves keyboard focus to the owning listbox when a file is clicked', () => {
    render(<ContentBrowser workspace={workspace(2)} />)

    const grid = screen.getByRole('listbox', { name: '图片文件' })
    fireEvent.click(screen.getByRole('option', { name: '1.jpg' }))

    expect(grid).toHaveFocus()
  })

  it('replaces image selection live with a visible background marquee', () => {
    render(<ContentBrowser workspace={workspace(4)} />)
    fireEvent.click(screen.getByRole('option', { name: '4.jpg' }))
    marqueeImages({ start: [378, 100], end: [0, 0] })
    expect(selectedLabels()).toEqual(['1.jpg', '2.jpg'])
    expect(screen.getByTestId('marquee-selection')).toBeVisible()
    finishMarquee([0, 0])
    expect(screen.queryByTestId('marquee-selection')).not.toBeInTheDocument()
  })

  it('toggles marquee hits against the frozen Command selection snapshot', () => {
    render(<ContentBrowser workspace={workspace(4)} />)
    fireEvent.click(screen.getByRole('option', { name: '1.jpg' }))
    fireEvent.click(screen.getByRole('option', { name: '3.jpg' }), { metaKey: true })
    marqueeImages({ start: [378, 100], end: [0, 0], metaKey: true })
    finishMarquee([0, 0])
    expect(selectedLabels()).toEqual(['2.jpg', '3.jpg'])
  })

  it('clears on a background click and restores the start snapshot on Escape', () => {
    render(<ContentBrowser workspace={workspace(4)} />)
    fireEvent.click(screen.getByRole('option', { name: '1.jpg' }))
    const grid = imageGrid()
    fireEvent.pointerDown(grid, { pointerId: 41, button: 0, clientX: 500, clientY: 300 })
    fireEvent.pointerUp(grid, { pointerId: 41, clientX: 500, clientY: 300 })
    expect(selectedLabels()).toEqual([])

    fireEvent.click(screen.getByRole('option', { name: '1.jpg' }))
    marqueeImages({ start: [378, 100], end: [0, 0] })
    expect(selectedLabels()).toEqual(['1.jpg', '2.jpg'])
    fireEvent.keyDown(screen.getByRole('listbox', { name: '图片文件' }), { key: 'Escape' })
    expect(selectedLabels()).toEqual(['1.jpg'])
    expect(screen.queryByTestId('marquee-selection')).not.toBeInTheDocument()
  })

  it('keeps background marquee, card Finder export, and handle organization drag isolated', () => {
    const exportFiles = vi.fn()
    const organize = vi.fn()
    render(
      <ContentBrowser
        workspace={workspace(3)}
        onFinderDragStart={exportFiles}
        onOrganizationDragStart={organize}
      />,
    )
    fireEvent.pointerDown(screen.getByRole('option', { name: '1.jpg' }), {
      pointerId: 9,
      button: 0,
    })
    fireEvent.dragStart(screen.getByRole('option', { name: '1.jpg' }))
    expect(exportFiles).toHaveBeenCalledTimes(1)
    fireEvent.pointerDown(screen.getByRole('button', { name: '整理 2.jpg' }), {
      pointerId: 10,
      button: 0,
    })
    fireEvent.dragStart(screen.getByRole('button', { name: '整理 2.jpg' }), {
      dataTransfer: { setData: vi.fn(), effectAllowed: 'none' },
    })
    expect(organize).toHaveBeenCalledTimes(1)
    expect(screen.queryByTestId('marquee-selection')).not.toBeInTheDocument()
  })

  it('keeps Markdown and TXT in an independent labelled list', () => {
    render(<ContentBrowser workspace={workspace()} />)

    expect(screen.getByText('文本文件')).toBeVisible()
    expect(screen.getByRole('option', { name: 'prompt.md' })).toHaveAttribute('tabindex', '-1')
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

  it('starts Finder export immediately from the file body without an HTML payload', () => {
    const exportFiles = vi.fn()
    const setData = vi.fn()
    render(<ContentBrowser workspace={workspace(4)} onFinderDragStart={exportFiles} />)
    marqueeImages({ start: [378, 100], end: [0, 0] })
    finishMarquee([0, 0])

    const event = createEvent.dragStart(screen.getByRole('option', { name: '2.jpg' }), {
      dataTransfer: { setData, effectAllowed: 'none' },
    })
    fireEvent(screen.getByRole('option', { name: '2.jpg' }), event)

    expect(event.defaultPrevented).toBe(true)
    expect(exportFiles).toHaveBeenCalledWith(['image-1', 'image-2'])
    expect(setData).not.toHaveBeenCalled()
  })

  it('starts entity-only internal drag from the organization handle and freezes Option-copy', () => {
    const organize = vi.fn()
    const exportFiles = vi.fn()
    const setData = vi.fn()
    render(
      <ContentBrowser
        workspace={workspace(2)}
        onFinderDragStart={exportFiles}
        onOrganizationDragStart={organize}
      />,
    )

    const event = createEvent.dragStart(screen.getByRole('button', { name: '整理 1.jpg' }), {
      dataTransfer: { setData, effectAllowed: 'none' },
    })
    Object.defineProperty(event, 'altKey', { value: true })
    fireEvent(screen.getByRole('button', { name: '整理 1.jpg' }), event)

    expect(organize).toHaveBeenCalledWith(['image-1'], 'copy')
    expect(exportFiles).not.toHaveBeenCalled()
    expect(setData).toHaveBeenCalledWith('application/x-viewer-selection', 'viewer-selection')
    expect(setData).not.toHaveBeenCalledWith(expect.anything(), expect.stringContaining('image-'))
    expect(setData).not.toHaveBeenCalledWith(expect.anything(), expect.stringContaining('/'))
  })

  it('keeps Finder export available but disables the organization handle in read-only mode', () => {
    const exportFiles = vi.fn()
    render(
      <ContentBrowser
        workspace={workspace(1)}
        organizationDragDisabled
        onFinderDragStart={exportFiles}
      />,
    )

    expect(screen.getByRole('button', { name: '整理 1.jpg' })).toBeDisabled()
    fireEvent.dragStart(screen.getByRole('option', { name: '1.jpg' }))
    expect(exportFiles).toHaveBeenCalledWith(['image-1'])
  })

  it('suppresses stale selected IDs when a refreshed workspace starts a drag', async () => {
    const start = vi.fn()
    const rendered = render(
      <ContentBrowser workspace={workspace(3)} onOrganizationDragStart={start} />,
    )
    fireEvent.click(screen.getByRole('option', { name: '1.jpg' }))
    fireEvent.click(screen.getByRole('option', { name: '2.jpg' }), { metaKey: true })
    const refreshed = workspace(3)
    refreshed.images = refreshed.images.filter((file) => file.entityId !== 'image-1')
    rendered.rerender(
      <ContentBrowser workspace={refreshed} onOrganizationDragStart={start} />,
    )

    const event = createEvent.dragStart(screen.getByRole('button', { name: '整理 2.jpg' }), {
      dataTransfer: { setData: vi.fn(), effectAllowed: 'none' },
    })
    fireEvent(screen.getByRole('button', { name: '整理 2.jpg' }), event)
    await waitFor(() => expect(start).toHaveBeenLastCalledWith(['image-2'], 'move'))
  })
})
