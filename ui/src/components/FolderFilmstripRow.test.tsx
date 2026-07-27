import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { BrowserFile, ContentFolderCard, ThumbnailDensity } from '../api/types'
import { defined } from '../defined'
import FolderFilmstripRow from './FolderFilmstripRow'

const folder: ContentFolderCard = {
  entityId: 'folder-b01',
  relativePath: '角色/B01',
  name: 'B01',
  marker: { reviewState: null, favorite: false },
  imageCount: 3,
  textCount: 1,
  reviewProgress: {
    total: 4,
    keep: 1,
    pending: 0,
    reject: 0,
    unmarked: 3,
    favorite: 0,
  },
  representativeImages: [],
}

function image(
  index: number,
  dimensions: BrowserFile['imageMetadata'] = { width: 1, height: 1 },
): BrowserFile {
  return {
    entityId: `image-${index + 1}`,
    relativePath: `角色/B01/image-${index + 1}.jpg`,
    name: `image-${index + 1}.jpg`,
    kind: 'jpeg',
    size: 100,
    modifiedNs: String(index + 1),
    marker: { reviewState: null, favorite: false },
    imageMetadata: dimensions,
    imageUrl: null,
  }
}

const mixedImages = [
  image(0, { width: 2, height: 3 }),
  image(1, { width: 1, height: 1 }),
  image(2, { width: 3, height: 2 }),
]

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

function renderRow({
  files = mixedImages,
  density = 'standard',
  requestThumbnail = vi.fn().mockResolvedValue('viewer-image://thumbnail'),
  loadImages = vi.fn().mockResolvedValue(files),
  onPreview = vi.fn(),
}: {
  files?: BrowserFile[]
  density?: ThumbnailDensity
  requestThumbnail?: (file: BrowserFile, maxPixels: number, scaleMilli: number) => Promise<string>
  loadImages?: (entityId: string, retry?: boolean) => Promise<BrowserFile[]>
  onPreview?: (file: BrowserFile, files: BrowserFile[]) => void
} = {}) {
  const rendered = render(
    <FolderFilmstripRow
      folder={{ ...folder, imageCount: files.length }}
      density={density}
      loadImages={loadImages}
      requestThumbnail={requestThumbnail}
      onSelect={vi.fn()}
      onPreview={onPreview}
    />,
  )
  return { ...rendered, loadImages, onPreview, requestThumbnail }
}

async function sizeViewport(width: number, scrollLeft = 0) {
  const filmstrip = await screen.findByRole('region', { name: 'B01 图片' })
  Object.defineProperty(filmstrip, 'clientWidth', {
    configurable: true,
    value: width,
  })
  Object.defineProperty(filmstrip, 'scrollLeft', {
    configurable: true,
    value: scrollLeft,
    writable: true,
  })
  fireEvent.scroll(filmstrip)
  return filmstrip
}

describe('FolderFilmstripRow', () => {
  it('loads near the viewport and preserves folder navigation, source order, and preview context', async () => {
    installIntersectionObserver()
    const loadImages = vi.fn().mockResolvedValue(mixedImages)
    const preview = vi.fn()
    const select = vi.fn()
    render(
      <FolderFilmstripRow
        folder={folder}
        density="standard"
        loadImages={loadImages}
        requestThumbnail={vi.fn().mockResolvedValue('viewer-image://thumbnail')}
        onSelect={select}
        onPreview={preview}
      />,
    )

    expect(loadImages).not.toHaveBeenCalled()
    const filmstrip = screen.getByRole('region', { name: 'B01 图片' })
    expect(intersectionOptions).toEqual({ rootMargin: '240px 0px' })
    expect(observedTargets).toEqual(new Set([filmstrip.closest('article')]))
    expect(filmstrip).toHaveAttribute('data-state', 'idle')
    expect(screen.getByText('3 张图片')).toBeVisible()
    expect(screen.getByText('1 个文本')).toBeVisible()
    expect(screen.getByText('文件夹：未标记')).toBeVisible()
    expect(screen.getByText('已审阅 1 / 4')).toBeVisible()

    act(revealRow)

    const buttons = await within(filmstrip).findAllByRole('button', { name: /^预览 / })
    expect(buttons.map((button) => button.getAttribute('aria-label'))).toEqual([
      '预览 image-1.jpg',
      '预览 image-2.jpg',
      '预览 image-3.jpg',
    ])
    fireEvent.click(defined(buttons[1], 'Expected second filmstrip image button'))
    fireEvent.click(screen.getByRole('button', { name: '打开 B01' }))

    expect(loadImages).toHaveBeenCalledWith('folder-b01', false)
    expect(preview).toHaveBeenCalledWith(mixedImages[1], mixedImages)
    expect(select).toHaveBeenCalledWith('folder-b01')
  })

  it('renders empty rows and retries only the failed row request', async () => {
    const loadImages = vi
      .fn<(entityId: string, retry?: boolean) => Promise<BrowserFile[]>>()
      .mockRejectedValueOnce(new Error('offline'))
      .mockResolvedValueOnce([])
    renderRow({ files: [], loadImages })

    expect(await screen.findByRole('alert')).toHaveTextContent('无法加载图片')
    fireEvent.click(screen.getByRole('button', { name: '重试 B01' }))

    await waitFor(() => expect(screen.getByText('无图片')).toBeVisible())
    expect(loadImages).toHaveBeenNthCalledWith(1, 'folder-b01', false)
    expect(loadImages).toHaveBeenNthCalledWith(2, 'folder-b01', true)
  })

  it('uses proportional mixed-ratio geometry at the selected density', async () => {
    renderRow()
    const filmstrip = await sizeViewport(1_000)
    const track = filmstrip.querySelector<HTMLElement>('.folder-filmstrip-track')
    const items = within(filmstrip).getAllByRole('listitem')

    expect(track).toHaveStyle({ width: '458px', height: '132px' })
    expect(items).toHaveLength(3)
    expect(items[0]).toHaveStyle({ left: '12px', width: '88px', height: '132px' })
    expect(items[1]).toHaveStyle({ left: '108px', width: '132px', height: '132px' })
    expect(items[2]).toHaveStyle({ left: '248px', width: '198px', height: '132px' })
    await waitFor(() => expect(filmstrip.querySelectorAll('img')).toHaveLength(3))
    expect(
      [...filmstrip.querySelectorAll<HTMLImageElement>('img')].map((thumbnail) => ({
        width: thumbnail.style.width,
        height: thumbnail.style.height,
      })),
    ).toEqual([
      { width: '88px', height: '132px' },
      { width: '132px', height: '132px' },
      { width: '198px', height: '132px' },
    ])
  })

  it('keeps a bounded binary-search window for 1,000 mixed ratios and unions focused content', async () => {
    const files = Array.from({ length: 1_000 }, (_, index) =>
      image(
        index,
        [
          { width: 2, height: 3 },
          { width: 1, height: 1 },
          { width: 3, height: 2 },
        ][index % 3],
      ),
    )
    const preview = vi.fn()
    renderRow({ files, onPreview: preview })
    const filmstrip = await sizeViewport(420)
    const track = filmstrip.querySelector<HTMLElement>('.folder-filmstrip-track')

    expect(track).toHaveStyle({ width: '147298px' })
    expect(within(filmstrip).getAllByRole('listitem')).toHaveLength(7)
    const focused = within(filmstrip).getByRole('button', { name: '预览 image-3.jpg' })
    focused.focus()

    filmstrip.scrollLeft = 73_620
    fireEvent.scroll(filmstrip)

    await within(filmstrip).findByRole('button', { name: '预览 image-501.jpg' })
    expect(within(filmstrip).getAllByRole('listitem')).toHaveLength(13)
    expect(focused).toBeInTheDocument()
    expect(focused).toHaveFocus()
    expect(
      within(filmstrip).queryByRole('button', { name: '预览 image-100.jpg' }),
    ).not.toBeInTheDocument()

    fireEvent.click(within(filmstrip).getByRole('button', { name: '预览 image-501.jpg' }))
    expect(preview).toHaveBeenCalledWith(files[500], files)

    fireEvent.blur(focused)
    await waitFor(() => expect(focused).not.toBeInTheDocument())
    expect(within(filmstrip).getAllByRole('listitem')).toHaveLength(12)
  })

  it('preserves the first visible key and inline offset after density changes', async () => {
    const fourth = image(3, { width: 2, height: 3 })
    const files = [...mixedImages, fourth]
    const props = {
      folder: { ...folder, imageCount: files.length },
      loadImages: vi.fn().mockResolvedValue(files),
      requestThumbnail: vi.fn().mockResolvedValue('viewer-image://thumbnail'),
      onSelect: vi.fn(),
      onPreview: vi.fn(),
    }
    const rendered = render(<FolderFilmstripRow {...props} density="standard" />)
    const filmstrip = await sizeViewport(200, 280)

    rendered.rerender(<FolderFilmstripRow {...props} density="large" />)

    await waitFor(() => expect(filmstrip.scrollLeft).toBe(340))
    const landscape = within(filmstrip)
      .getByRole('button', { name: '预览 image-3.jpg' })
      .closest<HTMLElement>('[role="listitem"]')
    expect(landscape).toHaveStyle({ left: '308px', width: '252px', height: '168px' })
  })

  it('recovers missing metadata before reveal and isolates another thumbnail failure', async () => {
    const unknown = image(0, null)
    const failed = image(1, { width: 1, height: 1 })
    const requestThumbnail = vi.fn((file: BrowserFile) =>
      file.entityId === failed.entityId
        ? Promise.reject(new Error('offline'))
        : Promise.resolve('viewer-image://thumbnail'),
    )
    renderRow({ files: [unknown, failed], requestThumbnail })
    const filmstrip = await sizeViewport(1_000)
    const unknownButton = within(filmstrip).getByRole('button', {
      name: '预览 image-1.jpg',
    })
    const thumbnail = await waitFor(() => {
      const candidate = unknownButton.querySelector('img')
      expect(candidate).not.toBeNull()
      return candidate as HTMLImageElement
    })

    expect(filmstrip.querySelector('.folder-filmstrip-track')).toHaveStyle({ width: '296px' })
    expect(thumbnail).toHaveStyle({ visibility: 'hidden' })
    expect(within(filmstrip).getByLabelText('缩略图不可用')).toBeVisible()

    Object.defineProperties(thumbnail, {
      naturalWidth: { configurable: true, value: 4_000 },
      naturalHeight: { configurable: true, value: 1_000 },
    })
    fireEvent.load(thumbnail)

    await waitFor(() =>
      expect(filmstrip.querySelector('.folder-filmstrip-track')).toHaveStyle({ width: '692px' }),
    )
    expect(thumbnail).toHaveStyle({ visibility: 'visible', width: '528px', height: '132px' })
    expect(within(filmstrip).getByLabelText('缩略图不可用')).toBeVisible()
  })

  it('keeps an extreme panorama unrestricted and reachable by horizontal scrolling', async () => {
    const files = [
      ...Array.from({ length: 20 }, (_, index) => image(index)),
      image(20, { width: 100, height: 1 }),
    ]
    renderRow({ files })
    const filmstrip = await sizeViewport(300)

    expect(
      within(filmstrip).queryByRole('button', { name: '预览 image-21.jpg' }),
    ).not.toBeInTheDocument()
    expect(filmstrip.querySelector('.folder-filmstrip-track')).toHaveStyle({ width: '16024px' })

    filmstrip.scrollLeft = 2_812
    fireEvent.scroll(filmstrip)

    const panorama = await within(filmstrip).findByRole('button', {
      name: '预览 image-21.jpg',
    })
    expect(panorama.closest('[role="listitem"]')).toHaveStyle({
      left: '2812px',
      width: '13200px',
      height: '132px',
    })
  })
})
