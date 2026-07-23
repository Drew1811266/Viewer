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
  it('applies an external repair target to selection, active item, and range anchor', () => {
    const selection = vi.fn()
    const rendered = render(
      <ContentBrowser
        workspace={workspace(4)}
        repairSelectionId={null}
        onSelectionChange={selection}
      />,
    )
    fireEvent.click(screen.getByRole('option', { name: '1.jpg' }))
    rendered.rerender(
      <ContentBrowser
        workspace={workspace(4)}
        repairSelectionId="image-3"
        onSelectionChange={selection}
      />,
    )

    expect(selectedLabels()).toEqual(['3.jpg'])
    expect(screen.getByRole('listbox', { name: '图片文件' })).toHaveAttribute(
      'aria-activedescendant',
      'file-image-3',
    )
    fireEvent.click(screen.getByRole('option', { name: '4.jpg' }), { shiftKey: true })
    expect(selectedLabels()).toEqual(['3.jpg', '4.jpg'])
  })

  it('consumes an external repair once and never reselects it after later projection updates', () => {
    const selection = vi.fn()
    const rendered = render(
      <ContentBrowser
        workspace={workspace(4)}
        repairSelectionId="image-3"
        onSelectionChange={selection}
      />,
    )
    expect(selectedLabels()).toEqual(['3.jpg'])
    fireEvent.click(screen.getByRole('option', { name: '2.jpg' }))
    expect(selectedLabels()).toEqual(['2.jpg'])
    const updated = workspace(4)
    updated.images[0] = {
      ...updated.images[0],
      marker: { reviewState: 'keep', favorite: true },
    }

    rendered.rerender(
      <ContentBrowser
        workspace={updated}
        repairSelectionId="image-3"
        onSelectionChange={selection}
      />,
    )

    expect(selectedLabels()).toEqual(['2.jpg'])
    expect(screen.getByRole('listbox', { name: '图片文件' })).toHaveAttribute(
      'aria-activedescendant',
      'file-image-2',
    )
  })

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

  it('keeps background marquee, image Finder export, and pointer organization isolated', () => {
    const exportFiles = vi.fn()
    const organize = vi.fn()
    render(
      <ContentBrowser
        workspace={workspace(3)}
        onFinderDragStart={exportFiles}
        onOrganizationPointerInput={organize}
      />,
    )

    const option = screen.getByRole('option', { name: '1.jpg' })
    const exportSurface = option.querySelector<HTMLElement>('.file-export-surface')!
    const handle = screen.getByRole('button', { name: '整理 2.jpg' })
    expect(option).not.toHaveAttribute('draggable')
    expect(exportSurface).toHaveAttribute('draggable', 'true')
    expect(handle).toHaveAttribute('draggable', 'false')

    fireEvent.dragStart(exportSurface)
    expect(exportFiles).toHaveBeenCalledTimes(1)
    fireEvent.pointerDown(handle, {
      pointerId: 10,
      button: 0,
      clientX: 12,
      clientY: 14,
    })
    fireEvent.dragStart(handle)
    expect(organize).toHaveBeenCalledTimes(1)
    expect(exportFiles).toHaveBeenCalledTimes(1)
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

  it.each([
    ['image cell', '2.jpg'],
    ['text row', 'prompt.md'],
  ])('splits the %s body export surface from its pointer handle', (_kind, name) => {
    const exportFiles = vi.fn()
    const setData = vi.fn()
    render(<ContentBrowser workspace={workspace(4)} onFinderDragStart={exportFiles} />)
    const option = screen.getByRole('option', { name })
    const exportSurface = option.querySelector<HTMLElement>('.file-export-surface')!
    const handle = screen.getByRole('button', { name: `整理 ${name}` })

    expect(option).not.toHaveAttribute('draggable')
    expect(exportSurface).toHaveAttribute('draggable', 'true')
    expect(handle).toHaveAttribute('draggable', 'false')

    const event = createEvent.dragStart(exportSurface, {
      dataTransfer: { setData, effectAllowed: 'none' },
    })
    fireEvent(exportSurface, event)

    expect(event.defaultPrevented).toBe(true)
    expect(exportFiles).toHaveBeenCalledWith([
      name === 'prompt.md' ? 'text-1' : 'image-2',
    ])
    expect(setData).not.toHaveBeenCalled()

    const handleDrag = createEvent.dragStart(handle)
    fireEvent(handle, handleDrag)
    expect(handleDrag.defaultPrevented).toBe(true)
    expect(exportFiles).toHaveBeenCalledTimes(1)
  })

  it.each([
    ['image cell', '2.jpg', 'image-2'],
    ['text row', 'prompt.md', 'text-1'],
  ])('normalizes the captured %s pointer session and freezes Option-copy', (_kind, name, id) => {
    const inputs = vi.fn()
    const exportFiles = vi.fn()
    render(
      <ContentBrowser
        workspace={workspace(2)}
        onFinderDragStart={exportFiles}
        onOrganizationPointerInput={inputs}
      />,
    )
    fireEvent.click(screen.getByRole('option', { name: '1.jpg' }))
    if (name === '2.jpg') {
      fireEvent.click(screen.getByRole('option', { name: '2.jpg' }), { metaKey: true })
    }
    const handle = screen.getByRole('button', { name: `整理 ${name}` })

    const down = createEvent.pointerDown(handle, {
      pointerId: 17,
      button: 0,
      altKey: true,
      clientX: 21,
      clientY: 34,
    })
    fireEvent(handle, down)
    fireEvent.pointerMove(handle, {
      pointerId: 17,
      altKey: false,
      clientX: 28,
      clientY: 39,
    })
    fireEvent.pointerUp(handle, {
      pointerId: 17,
      altKey: false,
      clientX: 31,
      clientY: 42,
    })

    expect(down.defaultPrevented).toBe(true)
    expect(inputs).toHaveBeenNthCalledWith(1, {
      type: 'start',
      pointerId: 17,
      entityIds: name === '2.jpg' ? ['image-1', 'image-2'] : [id],
      mode: 'copy',
      clientX: 21,
      clientY: 34,
      captureNode: handle,
    })
    expect(inputs).toHaveBeenNthCalledWith(2, {
      type: 'move',
      pointerId: 17,
      clientX: 28,
      clientY: 39,
    })
    expect(inputs).toHaveBeenNthCalledWith(3, {
      type: 'end',
      pointerId: 17,
      clientX: 31,
      clientY: 42,
    })
    expect(exportFiles).not.toHaveBeenCalled()
  })

  it('emits cancel but ignores nonprimary and disabled pointer starts', () => {
    const inputs = vi.fn()
    const rendered = render(
      <ContentBrowser workspace={workspace(1)} onOrganizationPointerInput={inputs} />,
    )
    const handle = screen.getByRole('button', { name: '整理 1.jpg' })
    fireEvent.pointerDown(handle, {
      pointerId: 18,
      button: 1,
      clientX: 1,
      clientY: 2,
    })
    expect(inputs).not.toHaveBeenCalled()

    fireEvent.pointerDown(handle, {
      pointerId: 19,
      button: 0,
      clientX: 3,
      clientY: 4,
    })
    fireEvent.pointerCancel(handle, { pointerId: 19 })
    expect(inputs).toHaveBeenLastCalledWith({ type: 'cancel', pointerId: 19 })

    inputs.mockClear()
    rendered.rerender(
      <ContentBrowser
        workspace={workspace(1)}
        organizationDragDisabled
        onOrganizationPointerInput={inputs}
      />,
    )
    fireEvent.pointerDown(screen.getByRole('button', { name: '整理 1.jpg' }), {
      pointerId: 20,
      button: 0,
      clientX: 5,
      clientY: 6,
    })
    expect(inputs).not.toHaveBeenCalled()
  })

  it('prevents handle pointer, click, and double-click events from changing selection or preview', () => {
    const preview = vi.fn()
    const inputs = vi.fn()
    render(
      <ContentBrowser
        workspace={workspace(2)}
        onPreview={preview}
        onOrganizationPointerInput={inputs}
      />,
    )
    const handle = screen.getByRole('button', { name: '整理 2.jpg' })
    fireEvent.pointerDown(handle, {
      pointerId: 21,
      button: 0,
      clientX: 7,
      clientY: 8,
    })
    fireEvent.click(handle)
    fireEvent.doubleClick(handle)

    expect(selectedLabels()).toEqual(['2.jpg'])
    expect(preview).not.toHaveBeenCalled()
    expect(inputs).toHaveBeenCalledOnce()
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
    const option = screen.getByRole('option', { name: '1.jpg' })
    fireEvent.dragStart(option.querySelector('.file-export-surface')!)
    expect(exportFiles).toHaveBeenCalledWith(['image-1'])
  })

  it('suppresses stale selected IDs when a refreshed workspace starts a drag', async () => {
    const start = vi.fn()
    const rendered = render(
      <ContentBrowser workspace={workspace(3)} onOrganizationPointerInput={start} />,
    )
    fireEvent.click(screen.getByRole('option', { name: '1.jpg' }))
    fireEvent.click(screen.getByRole('option', { name: '2.jpg' }), { metaKey: true })
    const refreshed = workspace(3)
    refreshed.images = refreshed.images.filter((file) => file.entityId !== 'image-1')
    rendered.rerender(
      <ContentBrowser workspace={refreshed} onOrganizationPointerInput={start} />,
    )

    const handle = screen.getByRole('button', { name: '整理 2.jpg' })
    fireEvent.pointerDown(handle, {
      pointerId: 22,
      button: 0,
      clientX: 9,
      clientY: 10,
    })
    await waitFor(() =>
      expect(start).toHaveBeenLastCalledWith({
        type: 'start',
        pointerId: 22,
        entityIds: ['image-2'],
        mode: 'move',
        clientX: 9,
        clientY: 10,
        captureNode: handle,
      }),
    )
  })

  it('opens the radial request on an unselected image and replaces selection first', () => {
    const request = vi.fn()
    render(<ContentBrowser workspace={workspace(3)} onRadialMenuRequest={request} />)
    fireEvent.click(screen.getByRole('option', { name: '1.jpg' }))
    fireEvent.pointerDown(screen.getByRole('option', { name: '2.jpg' }), {
      pointerId: 70,
      button: 2,
      clientX: 210,
      clientY: 160,
    })
    expect(selectedLabels()).toEqual(['2.jpg'])
    expect(request).toHaveBeenCalledWith({
      files: [expect.objectContaining({ entityId: 'image-2' })],
      origin: { x: 210, y: 160 },
      pointerId: 70,
    })
  })

  it('preserves a multi-selection when right-clicking one of its files', () => {
    const request = vi.fn()
    render(<ContentBrowser workspace={workspace(3)} onRadialMenuRequest={request} />)
    fireEvent.click(screen.getByRole('option', { name: '1.jpg' }))
    fireEvent.click(screen.getByRole('option', { name: '2.jpg' }), { metaKey: true })
    fireEvent.pointerDown(screen.getByRole('option', { name: '2.jpg' }), {
      pointerId: 71,
      button: 2,
      clientX: 220,
      clientY: 170,
    })
    expect(selectedLabels()).toEqual(['1.jpg', '2.jpg'])
    expect(request.mock.calls[0]?.[0].files.map((file: BrowserFile) => file.entityId)).toEqual([
      'image-1',
      'image-2',
    ])
  })

  it('snapshots a text-row right-click request after replacing an unrelated selection', () => {
    const request = vi.fn()
    render(<ContentBrowser workspace={workspace(3)} onRadialMenuRequest={request} />)
    fireEvent.click(screen.getByRole('option', { name: '1.jpg' }))
    fireEvent.click(screen.getByRole('option', { name: '2.jpg' }), { metaKey: true })

    fireEvent.pointerDown(screen.getByRole('option', { name: 'prompt.md' }), {
      pointerId: 72,
      button: 2,
      clientX: 230,
      clientY: 180,
    })

    expect(selectedLabels()).toEqual(['prompt.md'])
    expect(request).toHaveBeenCalledWith({
      files: [expect.objectContaining({ entityId: 'text-1' })],
      origin: { x: 230, y: 180 },
      pointerId: 72,
    })
  })

  it('suppresses the native context menu for image and text options', () => {
    render(<ContentBrowser workspace={workspace(1)} onRadialMenuRequest={vi.fn()} />)
    for (const name of ['1.jpg', 'prompt.md']) {
      const event = createEvent.contextMenu(screen.getByRole('option', { name }))
      fireEvent(screen.getByRole('option', { name }), event)
      expect(event.defaultPrevented).toBe(true)
    }
  })

  it('hides unmarked copy and keeps marked badges plus select-all in the view menu', () => {
    const data = workspace(2)
    data.images[1] = {
      ...data.images[1]!,
      marker: { reviewState: 'keep', favorite: true },
    }
    render(<ContentBrowser workspace={data} currentPath="项目根目录" />)
    expect(screen.queryByText('未标记')).not.toBeInTheDocument()
    expect(screen.getByText('保留 · 收藏')).toBeVisible()
    expect(screen.getByText('项目根目录')).toBeVisible()
    fireEvent.click(screen.getByText('视图'))
    fireEvent.click(screen.getByRole('button', { name: '全选当前文件夹' }))
    expect(selectedLabels()).toHaveLength(3)
  })
})
