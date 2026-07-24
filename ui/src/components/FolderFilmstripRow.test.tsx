import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { BrowserFile, ContentFolderCard } from '../api/types'
import FolderFilmstripRow from './FolderFilmstripRow'

const folder: ContentFolderCard = {
  entityId: 'folder-b01',
  relativePath: '角色/B01',
  name: 'B01',
  marker: { reviewState: null, favorite: false },
  imageCount: 2,
  textCount: 1,
  reviewProgress: {
    total: 3,
    keep: 1,
    pending: 0,
    reject: 0,
    unmarked: 2,
    favorite: 0,
  },
  representativeImages: [],
}

const images: BrowserFile[] = ['front', 'side'].map((name, index) => ({
  entityId: `image-${index + 1}`,
  relativePath: `角色/B01/${name}.jpg`,
  name: `${name}.jpg`,
  kind: 'jpeg',
  size: 100,
  modifiedNs: String(index + 1),
  marker: { reviewState: null, favorite: false },
  imageMetadata: null,
  imageUrl: null,
}))

let intersectionCallback: IntersectionObserverCallback | null = null
let intersectionOptions: IntersectionObserverInit | undefined
let observedTargets = new Set<Element>()
const originalIntersectionObserver = globalThis.IntersectionObserver

afterEach(() => {
  intersectionCallback = null
  intersectionOptions = undefined
  observedTargets = new Set()
  globalThis.IntersectionObserver = originalIntersectionObserver
})

function installIntersectionObserver() {
  class TestIntersectionObserver {
    readonly root = null
    readonly rootMargin = '240px 0px'
    readonly thresholds = [0]

    constructor(callback: IntersectionObserverCallback, options?: IntersectionObserverInit) {
      intersectionCallback = callback
      intersectionOptions = options
    }

    disconnect() {
      observedTargets.clear()
    }
    observe(target: Element) {
      observedTargets.add(target)
    }
    takeRecords(): IntersectionObserverEntry[] {
      return []
    }
    unobserve(target: Element) {
      observedTargets.delete(target)
    }
  }
  globalThis.IntersectionObserver =
    TestIntersectionObserver as unknown as typeof IntersectionObserver
}

function revealRow() {
  const target = observedTargets.values().next().value
  if (target === undefined) throw new Error('No observed row is registered')
  intersectionCallback?.(
    [{ isIntersecting: true, target } as IntersectionObserverEntry],
    {} as IntersectionObserver,
  )
}

describe('FolderFilmstripRow', () => {
  it('loads only after approaching the viewport and previews in returned image order', async () => {
    installIntersectionObserver()
    const loadImages = vi.fn().mockResolvedValue(images)
    const preview = vi.fn()
    render(
      <FolderFilmstripRow
        folder={folder}
        loadImages={loadImages}
        requestThumbnail={vi.fn().mockResolvedValue('viewer-image://thumbnail')}
        onSelect={vi.fn()}
        onPreview={preview}
      />,
    )

    expect(loadImages).not.toHaveBeenCalled()
    const filmstrip = screen.getByRole('region', { name: 'B01 图片' })
    const row = filmstrip.closest('article')
    expect(intersectionOptions).toEqual({ rootMargin: '240px 0px' })
    expect(observedTargets).toEqual(new Set([row]))
    expect(filmstrip).toHaveAttribute('data-state', 'idle')

    act(revealRow)

    await within(filmstrip).findByRole('button', { name: '预览 front.jpg' })
    const imageButtons = within(filmstrip).getAllByRole('button', { name: /^预览 / })
    expect(imageButtons.map((button) => button.getAttribute('aria-label'))).toEqual([
      '预览 front.jpg',
      '预览 side.jpg',
    ])
    fireEvent.click(imageButtons[1]!)

    expect(loadImages).toHaveBeenCalledWith('folder-b01', false)
    expect(preview).toHaveBeenCalledWith(images[1], images)
  })

  it('keeps metadata available while loading and opens the folder from its identity button', () => {
    const select = vi.fn()
    render(
      <FolderFilmstripRow
        folder={folder}
        loadImages={() => new Promise<BrowserFile[]>(() => undefined)}
        onSelect={select}
        onPreview={vi.fn()}
      />,
    )

    expect(screen.getByText('2 张图片')).toBeVisible()
    expect(screen.getByText('1 个文本')).toBeVisible()
    expect(screen.getByText('文件夹：未标记')).toBeVisible()
    expect(screen.getByText('已审阅 1 / 3')).toBeVisible()
    fireEvent.click(screen.getByRole('button', { name: '打开 B01' }))
    expect(select).toHaveBeenCalledWith('folder-b01')
  })

  it('renders empty rows and retries only the failed row request', async () => {
    const loadImages = vi
      .fn<(entityId: string, retry?: boolean) => Promise<BrowserFile[]>>()
      .mockRejectedValueOnce(new Error('offline'))
      .mockResolvedValueOnce([])
    render(
      <FolderFilmstripRow
        folder={folder}
        loadImages={loadImages}
        onSelect={vi.fn()}
        onPreview={vi.fn()}
      />,
    )

    expect(await screen.findByRole('alert')).toHaveTextContent('无法加载图片')
    fireEvent.click(screen.getByRole('button', { name: '重试 B01' }))

    await waitFor(() => expect(screen.getByText('无图片')).toBeVisible())
    expect(loadImages).toHaveBeenNthCalledWith(1, 'folder-b01', false)
    expect(loadImages).toHaveBeenNthCalledWith(2, 'folder-b01', true)
  })

  it('windows a high-cardinality row while preserving width, order, and preview context', async () => {
    const manyImages: BrowserFile[] = Array.from({ length: 100 }, (_, index) => ({
      ...images[0]!,
      entityId: `image-${index + 1}`,
      relativePath: `角色/B01/image-${index + 1}.jpg`,
      name: `image-${index + 1}.jpg`,
      modifiedNs: String(index + 1),
    }))
    const preview = vi.fn()
    const requestThumbnail = vi.fn().mockResolvedValue('viewer-image://thumbnail')
    render(
      <FolderFilmstripRow
        folder={{ ...folder, imageCount: manyImages.length }}
        loadImages={vi.fn().mockResolvedValue(manyImages)}
        requestThumbnail={requestThumbnail}
        onSelect={vi.fn()}
        onPreview={preview}
      />,
    )

    const filmstrip = await screen.findByRole('region', { name: 'B01 图片' })
    Object.defineProperty(filmstrip, 'clientWidth', {
      configurable: true,
      value: 420,
    })
    Object.defineProperty(filmstrip, 'scrollLeft', {
      configurable: true,
      value: 0,
      writable: true,
    })
    fireEvent.scroll(filmstrip)

    const track = filmstrip.querySelector<HTMLElement>('.folder-filmstrip-track')
    expect(track).toHaveStyle({ width: '13992px' })
    const list = within(filmstrip).getByRole('list')
    const initialItems = within(list).getAllByRole('listitem')
    expect(initialItems).toHaveLength(5)
    expect(initialItems[0]).toHaveAttribute('aria-posinset', '1')
    expect(initialItems[0]).toHaveAttribute('aria-setsize', '100')
    expect(
      within(initialItems[0]!).getByRole('button', { name: '预览 image-1.jpg' }),
    ).not.toHaveAttribute('aria-posinset')
    expect(within(filmstrip).getAllByRole('button', { name: /^预览 / })).toHaveLength(5)
    expect(requestThumbnail).toHaveBeenCalledTimes(5)

    filmstrip.scrollLeft = 7000
    fireEvent.scroll(filmstrip)

    await waitFor(() =>
      expect(within(filmstrip).getByRole('button', { name: '预览 image-51.jpg' })).toBeVisible(),
    )
    const advancedButtons = within(filmstrip).getAllByRole('button', {
      name: /^预览 /,
    })
    expect(advancedButtons).toHaveLength(8)
    expect(
      within(filmstrip).queryByRole('button', { name: '预览 image-1.jpg' }),
    ).not.toBeInTheDocument()

    fireEvent.click(within(filmstrip).getByRole('button', { name: '预览 image-51.jpg' }))
    expect(preview).toHaveBeenCalledWith(manyImages[50], manyImages)
  })

  it('retains only the focused out-of-window thumbnail until it blurs', async () => {
    const manyImages: BrowserFile[] = Array.from({ length: 100 }, (_, index) => ({
      ...images[0]!,
      entityId: `image-${index + 1}`,
      relativePath: `角色/B01/image-${index + 1}.jpg`,
      name: `image-${index + 1}.jpg`,
      modifiedNs: String(index + 1),
    }))
    render(
      <FolderFilmstripRow
        folder={{ ...folder, imageCount: manyImages.length }}
        loadImages={vi.fn().mockResolvedValue(manyImages)}
        requestThumbnail={vi.fn().mockResolvedValue('viewer-image://thumbnail')}
        onSelect={vi.fn()}
        onPreview={vi.fn()}
      />,
    )

    const filmstrip = await screen.findByRole('region', { name: 'B01 图片' })
    Object.defineProperty(filmstrip, 'clientWidth', {
      configurable: true,
      value: 420,
    })
    Object.defineProperty(filmstrip, 'scrollLeft', {
      configurable: true,
      value: 0,
      writable: true,
    })
    fireEvent.scroll(filmstrip)
    const focused = within(filmstrip).getByRole('button', {
      name: '预览 image-3.jpg',
    })
    focused.focus()
    expect(focused).toHaveFocus()

    filmstrip.scrollLeft = 7000
    fireEvent.scroll(filmstrip)

    await within(filmstrip).findByRole('button', { name: '预览 image-51.jpg' })
    expect(focused).toBeInTheDocument()
    expect(focused).toHaveFocus()
    expect(within(filmstrip).getAllByRole('button', { name: /^预览 / })).toHaveLength(9)
    expect(
      within(filmstrip).queryByRole('button', { name: '预览 image-20.jpg' }),
    ).not.toBeInTheDocument()
    const focusedItem = focused.closest('[role="listitem"]')
    expect(focusedItem).toHaveAttribute('aria-posinset', '3')
    expect(focusedItem).toHaveAttribute('aria-setsize', '100')

    fireEvent.blur(focused)

    await waitFor(() =>
      expect(
        within(filmstrip).queryByRole('button', { name: '预览 image-3.jpg' }),
      ).not.toBeInTheDocument(),
    )
    expect(within(filmstrip).getAllByRole('button', { name: /^预览 / })).toHaveLength(8)
  })
})
