import {
  act,
  createEvent,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from '@testing-library/react'
import type { ComponentProps } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { BrowserFile, FolderWorkspace, ImageMetadata, ThumbnailDensity } from '../api/types'
import { defined } from '../defined'
import ContentBrowserComponent from './ContentBrowser'

type ContentBrowserTestProps = Omit<ComponentProps<typeof ContentBrowserComponent>, 'density'> & {
  density?: ThumbnailDensity
}

function ContentBrowser({ density = 'standard', ...props }: ContentBrowserTestProps) {
  return <ContentBrowserComponent {...props} density={density} />
}

const resizeCallbacks = new Map<Element, ResizeObserverCallback>()
const frameCallbacks = new Map<number, FrameRequestCallback>()
let nextFrameId = 1
const originalDevicePixelRatio = window.devicePixelRatio

beforeEach(() => {
  resizeCallbacks.clear()
  frameCallbacks.clear()
  nextFrameId = 1
  class Observer {
    private readonly callback: ResizeObserverCallback
    private node: Element | null = null

    constructor(next: ResizeObserverCallback) {
      this.callback = next
    }

    observe(node: Element) {
      this.node = node
      resizeCallbacks.set(node, this.callback)
      this.callback(
        [{ target: node, contentRect: { width: 900, height: 520 } } as ResizeObserverEntry],
        this as unknown as ResizeObserver,
      )
    }

    disconnect() {
      if (this.node !== null) resizeCallbacks.delete(this.node)
      this.node = null
    }
  }
  vi.stubGlobal('ResizeObserver', Observer)
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
    const id = nextFrameId++
    frameCallbacks.set(id, callback)
    return id
  })
  vi.stubGlobal('cancelAnimationFrame', (id: number) => frameCallbacks.delete(id))
})

afterEach(() => {
  vi.unstubAllGlobals()
  Object.defineProperty(window, 'devicePixelRatio', {
    configurable: true,
    value: originalDevicePixelRatio,
  })
})

function image(index: number, imageMetadata: ImageMetadata | null = null): BrowserFile {
  return {
    entityId: `image-${index}`,
    relativePath: `id-001/${index}.jpg`,
    name: `${index}.jpg`,
    kind: 'jpeg',
    size: index * 10,
    modifiedNs: String(index),
    marker: { reviewState: null, favorite: false },
    imageMetadata,
    imageUrl: null,
  }
}

function ratioWorkspace(
  dimensions: (ImageMetadata | null)[],
): Extract<FolderWorkspace, { workspace: 'content' }> {
  return {
    workspace: 'content',
    images: dimensions.map((metadata, index) => image(index + 1, metadata)),
    textFiles: [],
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

function workspaceWithTextFiles(
  imageCount = 2,
  textCount = 6,
): Extract<FolderWorkspace, { workspace: 'content' }> {
  return {
    workspace: 'content',
    images: Array.from({ length: imageCount }, (_, index) => image(index + 1)),
    textFiles: Array.from({ length: textCount }, (_, index) => ({
      entityId: `text-${index + 1}`,
      relativePath: `id-001/note-${index + 1}.txt`,
      name: `note-${index + 1}.txt`,
      kind: 'text' as const,
      size: (index + 1) * 10,
      modifiedNs: String(100 + index),
      marker: { reviewState: null, favorite: false },
      imageMetadata: null,
      imageUrl: null,
    })),
  }
}

function selectedLabels(): string[] {
  return screen
    .getAllByRole('option')
    .filter((item) => item.getAttribute('aria-selected') === 'true')
    .map((item) =>
      defined(item.getAttribute('aria-label'), 'Selected option is missing its aria-label'),
    )
}

function triggerResize(node: Element, width: number, height: number) {
  resizeCallbacks.get(node)?.(
    [{ target: node, contentRect: { width, height } } as ResizeObserverEntry],
    {} as ResizeObserver,
  )
}

function resizeGrid(width: number, height = 520) {
  act(() => {
    triggerResize(screen.getByRole('listbox', { name: '图片文件' }), width, height)
  })
}

function flushAnimationFrames() {
  act(() => {
    while (frameCallbacks.size > 0) {
      const queued = [...frameCallbacks.values()]
      frameCallbacks.clear()
      queued.forEach((callback) => {
        callback(0)
      })
    }
  })
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
  it('removes the entire text surface and measures all available image height in image-only mode', () => {
    render(<ContentBrowser workspace={ratioWorkspace([{ width: 1, height: 1 }])} />)
    expect(screen.queryByText('文本文件')).not.toBeInTheDocument()
    expect(screen.queryByRole('listbox', { name: '文本文件' })).not.toBeInTheDocument()

    const slot = screen.getByTestId('content-image-slot')
    triggerResize(slot, 900, 688)
    flushAnimationFrames()
    expect(screen.getByRole('listbox', { name: '图片文件' })).toHaveStyle({
      height: '688px',
    })
  })

  it('keeps the same image grid node and scroll offset when its measured height changes', () => {
    render(<ContentBrowser workspace={ratioWorkspace(Array(40).fill({ width: 1, height: 1 }))} />)
    const slot = screen.getByTestId('content-image-slot')
    const grid = screen.getByRole('listbox', { name: '图片文件' })
    grid.scrollTop = 180
    fireEvent.scroll(grid)

    triggerResize(slot, 900, 420)
    flushAnimationFrames()
    triggerResize(slot, 900, 650)
    flushAnimationFrames()

    expect(screen.getByRole('listbox', { name: '图片文件' })).toBe(grid)
    expect(grid.scrollTop).toBe(180)
  })

  it.each([
    ['compact', 96],
    ['standard', 132],
    ['large', 168],
  ] satisfies [ThumbnailDensity, number][])(
    'uses the %s global density height with proportional source-order rows',
    (density, expectedHeight) => {
      render(
        <ContentBrowser
          workspace={ratioWorkspace([
            { width: 2, height: 3 },
            { width: 1, height: 1 },
            { width: 3, height: 2 },
            { width: 1, height: 1 },
          ])}
          density={density}
        />,
      )
      resizeGrid(320)

      const options = within(screen.getByRole('listbox', { name: '图片文件' })).getAllByRole(
        'option',
      )
      expect(options.map((option) => option.getAttribute('aria-label'))).toEqual([
        '1.jpg',
        '2.jpg',
        '3.jpg',
        '4.jpg',
      ])
      expect(thumbnailSurface('1.jpg')).toHaveStyle({
        width: `${(expectedHeight * 2) / 3}px`,
        height: `${expectedHeight}px`,
      })
      expect(thumbnailSurface('2.jpg')).toHaveStyle({
        width: `${expectedHeight}px`,
        height: `${expectedHeight}px`,
      })
      expect(thumbnailSurface('3.jpg')).toHaveStyle({
        width: `${expectedHeight * 1.5}px`,
        height: `${expectedHeight}px`,
      })
      expect(itemWrapper('image-1')).toHaveStyle({ left: '0px', top: '0px' })
      expect(itemWrapper('image-3')).toHaveStyle({
        left: '0px',
        top: `${expectedHeight + 60}px`,
      })
    },
  )

  it('keeps select-all in the view menu without a local thumbnail-size control', () => {
    render(<ContentBrowser workspace={workspace(2)} density="standard" />)

    fireEvent.click(screen.getByText('视图'))

    expect(screen.getByRole('button', { name: '全选当前文件夹' })).toBeVisible()
    expect(screen.queryByText('缩略图大小')).not.toBeInTheDocument()
    expect(screen.queryByRole('combobox')).not.toBeInTheDocument()
  })

  it('requests each rendered long edge at DPR and leaves successful images ratio-sized', async () => {
    Object.defineProperty(window, 'devicePixelRatio', {
      configurable: true,
      value: 2.5,
    })
    const requestThumbnail = vi.fn().mockImplementation(async (file: BrowserFile) => {
      return `viewer-image://thumbnail/${file.entityId}`
    })
    render(
      <ContentBrowser
        workspace={ratioWorkspace([
          { width: 2, height: 3 },
          { width: 3, height: 2 },
        ])}
        density="standard"
        requestThumbnail={requestThumbnail}
      />,
    )
    resizeGrid(500)

    await waitFor(() =>
      expect(document.querySelectorAll('.aspect-thumbnail > img')).toHaveLength(2),
    )
    expect(requestThumbnail).toHaveBeenCalledWith(
      expect.objectContaining({ entityId: 'image-1' }),
      330,
      2_500,
    )
    expect(requestThumbnail).toHaveBeenCalledWith(
      expect.objectContaining({ entityId: 'image-2' }),
      495,
      2_500,
    )
    for (const name of ['1.jpg', '2.jpg']) {
      const surface = thumbnailSurface(name)
      const thumbnail = defined(
        screen.getByRole('option', { name }).querySelector<HTMLImageElement>('img'),
        `Expected successful thumbnail for ${name}`,
      )
      expect(thumbnail).toHaveStyle({
        width: surface.style.width,
        height: surface.style.height,
        visibility: 'visible',
      })
      expect(
        screen.getByRole('option', { name }).querySelector('.aspect-thumbnail-placeholder'),
      ).toBeNull()
    }
  })

  it('uses source order horizontally and closest adjacent-row centers vertically', () => {
    const selection = vi.fn()
    render(
      <ContentBrowser
        workspace={ratioWorkspace([
          { width: 3, height: 2 },
          { width: 1, height: 2 },
          { width: 1, height: 1 },
          { width: 3, height: 2 },
          { width: 1, height: 1 },
          { width: 1, height: 1 },
        ])}
        density="compact"
        onSelectionChange={selection}
      />,
    )
    resizeGrid(260)
    const grid = screen.getByRole('listbox', { name: '图片文件' })

    fireEvent.click(screen.getByRole('option', { name: '2.jpg' }))
    fireEvent.keyDown(grid, { key: 'ArrowRight' })
    expect(selectedLabels()).toEqual(['3.jpg'])
    fireEvent.keyDown(grid, { key: 'ArrowLeft' })
    expect(selectedLabels()).toEqual(['2.jpg'])
    fireEvent.keyDown(grid, { key: 'ArrowDown' })
    expect(selectedLabels()).toEqual(['4.jpg'])

    fireEvent.click(screen.getByRole('option', { name: '3.jpg' }))
    fireEvent.keyDown(grid, { key: 'ArrowUp' })
    expect(selectedLabels()).toEqual(['1.jpg'])
    expect(selection).toHaveBeenLastCalledWith([expect.objectContaining({ entityId: 'image-1' })])
  })

  it('preserves selection, active ID, and preview entity across density changes', () => {
    const preview = vi.fn()
    const data = ratioWorkspace([
      { width: 2, height: 3 },
      { width: 3, height: 2 },
    ])
    const rendered = render(
      <ContentBrowser workspace={data} density="standard" onPreview={preview} />,
    )
    const second = screen.getByRole('option', { name: '2.jpg' })
    fireEvent.click(second)
    fireEvent.doubleClick(second)

    rendered.rerender(<ContentBrowser workspace={data} density="compact" onPreview={preview} />)

    expect(screen.getByRole('option', { name: '2.jpg' })).toHaveAttribute('aria-selected', 'true')
    expect(screen.getByRole('listbox', { name: '图片文件' })).toHaveAttribute(
      'aria-activedescendant',
      'file-image-2',
    )
    expect(thumbnailSurface('2.jpg')).toHaveStyle({ width: '144px', height: '96px' })
    fireEvent.keyDown(screen.getByRole('listbox', { name: '图片文件' }), { key: ' ' })
    expect(preview).toHaveBeenNthCalledWith(1, expect.objectContaining({ entityId: 'image-2' }))
    expect(preview).toHaveBeenNthCalledWith(2, expect.objectContaining({ entityId: 'image-2' }))
  })

  it('recovers missing metadata by entity and modification identity before revealing the ratio', async () => {
    const requestThumbnail = vi.fn().mockResolvedValue('viewer-image://thumbnail')
    const data = ratioWorkspace([null, { width: 1, height: 1 }])
    const rendered = render(
      <ContentBrowser workspace={data} density="standard" requestThumbnail={requestThumbnail} />,
    )
    resizeGrid(300)

    const firstImage = await thumbnailImage('1.jpg')
    expect(thumbnailSurface('1.jpg')).toHaveStyle({ width: '132px', height: '132px' })
    expect(firstImage).toHaveStyle({ visibility: 'hidden' })
    expect(itemWrapper('image-2')).toHaveStyle({ top: '0px' })
    Object.defineProperties(firstImage, {
      naturalWidth: { configurable: true, value: 3_000 },
      naturalHeight: { configurable: true, value: 1_000 },
    })
    fireEvent.load(firstImage)

    await waitFor(() =>
      expect(thumbnailSurface('1.jpg')).toHaveStyle({ width: '396px', height: '132px' }),
    )
    expect(
      defined(
        screen.getByRole('option', { name: '1.jpg' }).querySelector('img'),
        'Expected recovered image',
      ),
    ).toHaveStyle({
      visibility: 'visible',
    })
    expect(itemWrapper('image-2')).toHaveStyle({ top: '192px' })

    rendered.rerender(
      <ContentBrowser
        workspace={{
          ...data,
          images: [
            { ...defined(data.images[0], 'Expected unknown image'), modifiedNs: 'replacement' },
            defined(data.images[1], 'Expected known image'),
          ],
        }}
        density="standard"
        requestThumbnail={requestThumbnail}
      />,
    )

    await waitFor(() =>
      expect(thumbnailSurface('1.jpg')).toHaveStyle({ width: '132px', height: '132px' }),
    )
    expect(
      defined(
        screen.getByRole('option', { name: '1.jpg' }).querySelector('img'),
        'Expected replacement image',
      ),
    ).toHaveStyle({
      visibility: 'hidden',
    })
    expect(itemWrapper('image-2')).toHaveStyle({ top: '0px' })
  })

  it('prefers later valid metadata over a recovered ratio for the same image identity', async () => {
    const requestThumbnail = vi.fn().mockResolvedValue('viewer-image://thumbnail')
    const data = ratioWorkspace([null])
    const rendered = render(
      <ContentBrowser workspace={data} density="standard" requestThumbnail={requestThumbnail} />,
    )
    resizeGrid(1_000)

    const firstImage = await thumbnailImage('1.jpg')
    Object.defineProperties(firstImage, {
      naturalWidth: { configurable: true, value: 3_000 },
      naturalHeight: { configurable: true, value: 1_000 },
    })
    fireEvent.load(firstImage)
    await waitFor(() =>
      expect(thumbnailSurface('1.jpg')).toHaveStyle({ width: '396px', height: '132px' }),
    )

    rendered.rerender(
      <ContentBrowser
        workspace={{
          ...data,
          images: [
            {
              ...defined(data.images[0], 'Expected recovered image fixture'),
              imageMetadata: { width: 1, height: 2 },
            },
          ],
        }}
        density="standard"
        requestThumbnail={requestThumbnail}
      />,
    )

    await waitFor(() =>
      expect(thumbnailSurface('1.jpg')).toHaveStyle({ width: '66px', height: '132px' }),
    )
    expect(
      defined(
        screen.getByRole('option', { name: '1.jpg' }).querySelector('img'),
        'Expected metadata-sized image',
      ),
    ).toHaveStyle({ visibility: 'visible' })
  })

  it('keeps an over-wide panorama horizontally reachable and uncropped', async () => {
    render(
      <ContentBrowser
        workspace={ratioWorkspace([{ width: 10, height: 1 }])}
        density="standard"
        requestThumbnail={vi.fn().mockResolvedValue('viewer-image://panorama')}
      />,
    )
    resizeGrid(300)

    const grid = screen.getByRole('listbox', { name: '图片文件' })
    expect(grid).toHaveStyle({ overflow: 'auto' })
    expect(screen.getByTestId('aspect-virtual-grid-track')).toHaveStyle({ width: '1320px' })
    expect(itemWrapper('image-1')).toHaveStyle({ width: '1320px', left: '0px' })
    expect(await thumbnailImage('1.jpg')).toHaveStyle({
      width: '1320px',
      height: '132px',
      objectFit: 'contain',
    })
  })

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
      ...defined(updated.images[0], 'Expected first refreshed image'),
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

  it('restores text-list arrows, active ID, and Space preview with legacy all-file offsets', () => {
    const preview = vi.fn()
    render(<ContentBrowser workspace={workspaceWithTextFiles()} onPreview={preview} />)
    const textList = screen.getByRole('listbox', { name: '文本文件' })

    fireEvent.click(screen.getByRole('option', { name: 'note-2.txt' }))
    fireEvent.keyDown(textList, { key: 'ArrowRight' })
    expect(selectedLabels()).toEqual(['note-3.txt'])
    expect(screen.getByRole('listbox', { name: '图片文件' })).toHaveAttribute(
      'aria-activedescendant',
      'file-text-3',
    )
    fireEvent.keyDown(textList, { key: ' ' })
    expect(preview).toHaveBeenCalledWith(expect.objectContaining({ entityId: 'text-3' }))

    fireEvent.keyDown(textList, { key: 'ArrowLeft' })
    expect(selectedLabels()).toEqual(['note-2.txt'])
    fireEvent.keyDown(textList, { key: 'ArrowDown' })
    expect(selectedLabels()).toEqual(['note-6.txt'])
    fireEvent.keyDown(textList, { key: 'ArrowUp' })
    expect(selectedLabels()).toEqual(['note-2.txt'])
  })

  it('extends text-list Shift arrows from the source anchor with legacy all-file offsets', () => {
    render(<ContentBrowser workspace={workspaceWithTextFiles()} />)
    const textList = screen.getByRole('listbox', { name: '文本文件' })

    fireEvent.click(screen.getByRole('option', { name: 'note-1.txt' }))
    fireEvent.keyDown(textList, { key: 'ArrowRight', shiftKey: true })
    expect(selectedLabels()).toEqual(['note-1.txt', 'note-2.txt'])

    fireEvent.keyDown(textList, { key: 'ArrowDown', shiftKey: true })
    expect(selectedLabels()).toEqual([
      'note-1.txt',
      'note-2.txt',
      'note-3.txt',
      'note-4.txt',
      'note-5.txt',
      'note-6.txt',
    ])
  })

  it('keeps reverse Shift ranges additive in display order and Command-click isolated', () => {
    render(<ContentBrowser workspace={workspace(5)} />)

    fireEvent.click(screen.getByRole('option', { name: '1.jpg' }))
    fireEvent.click(screen.getByRole('option', { name: '4.jpg' }), { metaKey: true })
    fireEvent.click(screen.getByRole('option', { name: '2.jpg' }), { shiftKey: true })
    expect(selectedLabels()).toEqual(['1.jpg', '2.jpg', '3.jpg', '4.jpg'])

    fireEvent.click(screen.getByRole('option', { name: '3.jpg' }), { metaKey: true })
    expect(selectedLabels()).toEqual(['1.jpg', '2.jpg', '4.jpg'])
  })

  it('repairs a removed range anchor after a Shift-click fallback', async () => {
    const rendered = render(<ContentBrowser workspace={workspace(5)} />)
    fireEvent.click(screen.getByRole('option', { name: '3.jpg' }))
    fireEvent.click(screen.getByRole('option', { name: '1.jpg' }), { metaKey: true })

    const refreshed = workspace(5)
    refreshed.images = refreshed.images.slice(1)
    rendered.rerender(<ContentBrowser workspace={refreshed} />)
    await waitFor(() => expect(selectedLabels()).toEqual(['3.jpg']))

    fireEvent.click(screen.getByRole('option', { name: '2.jpg' }), { shiftKey: true })
    expect(selectedLabels()).toEqual(['2.jpg', '3.jpg'])

    fireEvent.click(screen.getByRole('option', { name: '5.jpg' }), { shiftKey: true })
    expect(selectedLabels()).toEqual(['2.jpg', '3.jpg', '4.jpg', '5.jpg'])
  })

  it('selects every file in the current folder with Command-A', () => {
    render(<ContentBrowser workspace={workspace(3)} />)

    const grid = screen.getByRole('listbox', { name: '图片文件' })
    fireEvent.keyDown(grid, { key: 'a', metaKey: true })

    expect(
      screen.getAllByRole('option').filter((item) => item.getAttribute('aria-selected') === 'true'),
    ).toHaveLength(4)
  })

  it('offers an explicit select-all action for mouse and assistive users', () => {
    render(<ContentBrowser workspace={workspace(3)} />)

    fireEvent.click(screen.getByRole('button', { name: '全选当前文件夹' }))

    expect(
      screen.getAllByRole('option').filter((item) => item.getAttribute('aria-selected') === 'true'),
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
    marqueeImages({ start: [280, 100], end: [0, 0] })
    expect(selectedLabels()).toEqual(['1.jpg', '2.jpg'])
    expect(screen.getByTestId('marquee-selection')).toBeVisible()
    finishMarquee([0, 0])
    expect(screen.queryByTestId('marquee-selection')).not.toBeInTheDocument()
  })

  it('toggles marquee hits against the frozen Command selection snapshot', () => {
    render(<ContentBrowser workspace={workspace(4)} />)
    fireEvent.click(screen.getByRole('option', { name: '1.jpg' }))
    fireEvent.click(screen.getByRole('option', { name: '3.jpg' }), { metaKey: true })
    marqueeImages({ start: [280, 100], end: [0, 0], metaKey: true })
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
    marqueeImages({ start: [280, 100], end: [0, 0] })
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
    const exportSurface = defined(
      option.querySelector<HTMLElement>('.file-export-surface'),
      'Expected image export surface',
    )
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
    expect(screen.getByRole('option', { name: '1.jpg' })).not.toHaveAttribute('tabindex')
  })

  it('shows text marker labels and preserves selection when marker projections refresh', () => {
    const changed = vi.fn()
    const rendered = render(<ContentBrowser workspace={workspace(2)} onSelectionChange={changed} />)
    fireEvent.click(screen.getByRole('option', { name: '1.jpg' }))
    const updated = workspace(2)
    updated.images[0] = {
      ...defined(updated.images[0], 'Expected first refreshed image'),
      marker: { reviewState: 'keep', favorite: true },
    }
    rendered.rerender(<ContentBrowser workspace={updated} onSelectionChange={changed} />)
    expect(screen.getByRole('option', { name: '1.jpg' })).toHaveAttribute('aria-selected', 'true')
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
    const exportSurface = defined(
      option.querySelector<HTMLElement>('.file-export-surface'),
      'Expected file export surface',
    )
    const handle = screen.getByRole('button', { name: `整理 ${name}` })

    expect(option).not.toHaveAttribute('draggable')
    expect(exportSurface).toHaveAttribute('draggable', 'true')
    expect(handle).toHaveAttribute('draggable', 'false')

    const event = createEvent.dragStart(exportSurface, {
      dataTransfer: { setData, effectAllowed: 'none' },
    })
    fireEvent(exportSurface, event)

    expect(event.defaultPrevented).toBe(true)
    expect(exportFiles).toHaveBeenCalledWith([name === 'prompt.md' ? 'text-1' : 'image-2'])
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
    fireEvent.dragStart(
      defined(option.querySelector('.file-export-surface'), 'Expected file export surface'),
    )
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
    rendered.rerender(<ContentBrowser workspace={refreshed} onOrganizationPointerInput={start} />)

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
    const grid = screen.getByRole('listbox', { name: '图片文件' })
    fireEvent.click(screen.getByRole('option', { name: '1.jpg' }))
    const second = screen.getByRole('option', { name: '2.jpg' })
    fireEvent.pointerDown(second, {
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
      returnFocusTarget: grid,
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
    const textList = screen.getByRole('listbox', { name: '文本文件' })
    fireEvent.click(screen.getByRole('option', { name: '1.jpg' }))
    fireEvent.click(screen.getByRole('option', { name: '2.jpg' }), { metaKey: true })

    const prompt = screen.getByRole('option', { name: 'prompt.md' })
    fireEvent.pointerDown(prompt, {
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
      returnFocusTarget: textList,
    })
  })

  it('falls back to click-mode radial requests for image and text context-menu events', () => {
    const request = vi.fn()
    render(<ContentBrowser workspace={workspace(1)} onRadialMenuRequest={request} />)

    for (const [name, entityId, listName] of [
      ['1.jpg', 'image-1', '图片文件'],
      ['prompt.md', 'text-1', '文本文件'],
    ] as const) {
      const option = screen.getByRole('option', { name })
      const event = createEvent.contextMenu(option, {
        button: 0,
        ctrlKey: true,
        clientX: 240,
        clientY: 180,
      })
      fireEvent(option, event)

      expect(event.defaultPrevented).toBe(true)
      expect(request).toHaveBeenLastCalledWith({
        files: [expect.objectContaining({ entityId })],
        origin: { x: 240, y: 180 },
        pointerId: null,
        returnFocusTarget: screen.getByRole('listbox', { name: listName }),
      })
    }
    expect(request).toHaveBeenCalledTimes(2)
  })

  it('deduplicates a secondary pointerdown followed by contextmenu', () => {
    const request = vi.fn()
    render(<ContentBrowser workspace={workspace(1)} onRadialMenuRequest={request} />)
    const option = screen.getByRole('option', { name: '1.jpg' })

    fireEvent.pointerDown(option, {
      pointerId: 73,
      button: 2,
      clientX: 210,
      clientY: 160,
    })
    fireEvent.contextMenu(option, {
      button: 2,
      clientX: 210,
      clientY: 160,
    })

    expect(request).toHaveBeenCalledTimes(1)
    expect(request.mock.calls[0]?.[0]).toEqual(expect.objectContaining({ pointerId: 73 }))
  })

  it('routes secondary and Control-click input on the organization handle to the radial menu', () => {
    const request = vi.fn()
    const organize = vi.fn()
    render(
      <ContentBrowser
        workspace={workspace(1)}
        onRadialMenuRequest={request}
        onOrganizationPointerInput={organize}
      />,
    )
    const handle = screen.getByRole('button', { name: '整理 1.jpg' })

    fireEvent.pointerDown(handle, {
      pointerId: 74,
      button: 2,
      clientX: 220,
      clientY: 170,
    })
    fireEvent.pointerMove(handle, {
      pointerId: 74,
      buttons: 2,
      clientX: 230,
      clientY: 180,
    })
    fireEvent.pointerUp(handle, {
      pointerId: 74,
      button: 2,
      clientX: 230,
      clientY: 180,
    })
    fireEvent.contextMenu(handle, {
      button: 2,
      clientX: 220,
      clientY: 170,
    })
    fireEvent.pointerDown(handle, {
      pointerId: 75,
      button: 0,
      ctrlKey: true,
      clientX: 225,
      clientY: 175,
    })
    fireEvent.contextMenu(handle, {
      button: 0,
      ctrlKey: true,
      clientX: 225,
      clientY: 175,
    })

    expect(request).toHaveBeenCalledTimes(2)
    expect(request.mock.calls.map(([item]) => item.pointerId)).toEqual([74, null])
    expect(organize).not.toHaveBeenCalled()
  })

  it('hides unmarked copy and keeps marked badges plus select-all in the view menu', () => {
    const data = workspace(2)
    data.images[1] = {
      ...defined(data.images[1], 'Expected second image fixture'),
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

function itemWrapper(entityId: string): HTMLElement {
  const wrapper = document.querySelector<HTMLElement>(`[data-key="${entityId}"]`)
  if (wrapper === null) throw new Error(`Expected mounted image wrapper for ${entityId}`)
  return wrapper
}

function thumbnailSurface(name: string): HTMLElement {
  const surface = screen
    .getByRole('option', { name })
    .querySelector<HTMLElement>('.aspect-thumbnail')
  if (surface === null) throw new Error(`Expected aspect thumbnail surface for ${name}`)
  return surface
}

async function thumbnailImage(name: string): Promise<HTMLImageElement> {
  let image: HTMLImageElement | null = null
  await waitFor(() => {
    image = screen.getByRole('option', { name }).querySelector('img')
    expect(image).not.toBeNull()
  })
  if (image === null) throw new Error(`Expected thumbnail image for ${name}`)
  return image
}
