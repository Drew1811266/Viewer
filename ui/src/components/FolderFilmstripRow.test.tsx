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
  videoCount: 0,
  otherFileCount: 1,
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
    videoMetadata: null,
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
  feedbackCountByEntityId = new Map(),
}: {
  files?: BrowserFile[]
  density?: ThumbnailDensity
  requestThumbnail?: (file: BrowserFile, maxPixels: number, scaleMilli: number) => Promise<string>
  loadImages?: (entityId: string, retry?: boolean) => Promise<BrowserFile[]>
  onPreview?: (file: BrowserFile, files: BrowserFile[]) => void
  feedbackCountByEntityId?: ReadonlyMap<string, number>
} = {}) {
  const rendered = render(
    <FolderFilmstripRow
      folder={{ ...folder, imageCount: files.length }}
      density={density}
      loadImages={loadImages}
      requestThumbnail={requestThumbnail}
      onSelect={vi.fn()}
      onPreview={onPreview}
      feedbackCountByEntityId={feedbackCountByEntityId}
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
  it('shows feedback counts only on affected filmstrip images', async () => {
    const filmstrip = renderRow({
      feedbackCountByEntityId: new Map([
        ['image-1', 4],
        ['image-2', 0],
      ]),
    })

    await filmstrip.findByRole('button', { name: '预览 image-1.jpg' })
    expect(screen.getByText('返工 · 4 条')).toBeVisible()
    expect(screen.getAllByText(/返工/)).toHaveLength(1)
  })

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
    expect(screen.getByText('1 个其它文件')).toBeVisible()
    expect(screen.getByText('文件夹：未标记')).toBeVisible()
    expect(screen.getByText('已审阅 1 / 4')).toBeVisible()

    act(revealRow)

    const buttons = await within(filmstrip).findAllByRole('button', { name: /^预览 / })
    expect(buttons.map((button) => button.getAttribute('aria-label'))).toEqual([
      '预览 image-1.jpg',
      '预览 image-2.jpg',
      '预览 image-3.jpg',
    ])
    const secondImage = defined(buttons[1], 'Expected second filmstrip image button')
    fireEvent.click(secondImage)
    expect(preview).not.toHaveBeenCalled()
    fireEvent.doubleClick(secondImage)
    fireEvent.click(screen.getByRole('button', { name: '打开 B01' }))

    expect(loadImages).toHaveBeenCalledWith('folder-b01', false)
    expect(preview).toHaveBeenCalledWith(mixedImages[1], mixedImages)
    expect(select).toHaveBeenCalledWith('folder-b01')
  })

  it('keeps Enter and Space as accessible preview actions', async () => {
    const preview = vi.fn()
    renderRow({ onPreview: preview })
    const imageButton = await screen.findByRole('button', { name: '预览 image-1.jpg' })

    fireEvent.keyDown(imageButton, { key: 'Enter' })
    fireEvent.keyDown(imageButton, { key: ' ' })

    expect(preview).toHaveBeenNthCalledWith(1, mixedImages[0], mixedImages)
    expect(preview).toHaveBeenNthCalledWith(2, mixedImages[0], mixedImages)
  })

  it('renders empty rows and retries only the failed row request', async () => {
    const loadImages = vi
      .fn<(entityId: string, retry?: boolean) => Promise<BrowserFile[]>>()
      .mockRejectedValueOnce(new Error('offline'))
      .mockResolvedValueOnce([])
    renderRow({ files: [], loadImages })

    const rowError = await screen.findByRole('alert')
    expect(rowError).toHaveTextContent('无法加载图片')
    expect(rowError).toHaveClass('viewer-local-feedback')
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

  it('renders a deterministic overlay scrollbar and tracks horizontal progress', async () => {
    const files = Array.from({ length: 10 }, (_, index) => image(index))
    renderRow({ files })
    const filmstrip = await sizeViewport(200)

    const scrollbar = await screen.findByTestId('folder-filmstrip-scrollbar')
    const thumb = within(scrollbar).getByTestId('folder-filmstrip-scrollbar-thumb')
    expect(scrollbar).toHaveAttribute('aria-hidden', 'true')
    expect(thumb).toHaveStyle({ width: '36px', transform: 'translateX(0px)' })

    filmstrip.scrollLeft = 608
    fireEvent.scroll(filmstrip)

    await waitFor(() => expect(thumb).toHaveStyle({ transform: 'translateX(78px)' }))
  })

  it('renders unsupported representatives without requesting their thumbnails', async () => {
    const unsupported = {
      ...image(1, { width: 1, height: 1 }),
      entityId: 'raw',
      relativePath: '角色/B01/raw.cr2',
      name: 'raw.cr2',
      kind: 'unsupported_image' as const,
    }
    const requestThumbnail = vi.fn().mockResolvedValue('viewer-image://thumbnail')
    const supported = image(0, { width: 1, height: 1 })
    const { onPreview } = renderRow({
      files: [supported, unsupported],
      requestThumbnail,
    })
    const filmstrip = await sizeViewport(1_000)

    await waitFor(() => expect(requestThumbnail).toHaveBeenCalled())
    expect(requestThumbnail.mock.calls.map(([file]) => file.entityId)).toEqual([supported.entityId])
    expect(within(filmstrip).getByLabelText('raw.cr2 .CR2 暂不支持预览')).toBeVisible()
    fireEvent.doubleClick(within(filmstrip).getByRole('button', { name: '预览 raw.cr2' }))
    expect(onPreview).toHaveBeenCalledWith(unsupported, [supported, unsupported])
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

    fireEvent.doubleClick(within(filmstrip).getByRole('button', { name: '预览 image-501.jpg' }))
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

  it('keeps the near-edge anchor when large-to-compact reflow clamps horizontal scroll', async () => {
    const files = Array.from({ length: 10 }, (_, index) => image(index))
    const props = {
      folder: { ...folder, imageCount: files.length },
      loadImages: vi.fn().mockResolvedValue(files),
      requestThumbnail: vi.fn().mockResolvedValue('viewer-image://thumbnail'),
      onSelect: vi.fn(),
      onPreview: vi.fn(),
    }
    const rendered = render(<FolderFilmstripRow {...props} density="large" />)
    const filmstrip = await sizeViewport(200)
    installClampedScrollLeft(filmstrip, 200)
    filmstrip.scrollLeft = 1_500
    fireEvent.scroll(filmstrip)

    rendered.rerender(<FolderFilmstripRow {...props} density="compact" />)

    // Large geometry anchors image-9 at 1420 - 1500 = -80px. Compact
    // geometry would place it at 844px, so 924px clamps to the 856px maximum.
    await waitFor(() => expect(filmstrip.scrollLeft).toBe(856))
    expect(
      within(filmstrip)
        .getByRole('button', { name: '预览 image-9.jpg' })
        .closest<HTMLElement>('[role="listitem"]'),
    ).toHaveStyle({ left: '844px', width: '96px', height: '96px' })
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

  it('prefers valid source metadata over recovered dimensions for the same identity', async () => {
    const initiallyUnknown = image(0, null)
    const props = {
      folder: { ...folder, imageCount: 1 },
      loadImages: vi.fn().mockResolvedValue([initiallyUnknown]),
      requestThumbnail: vi.fn().mockResolvedValue('viewer-image://thumbnail'),
      onSelect: vi.fn(),
      onPreview: vi.fn(),
    }
    const rendered = render(<FolderFilmstripRow {...props} density="standard" />)
    const filmstrip = await sizeViewport(1_000)
    const thumbnail = await waitFor(() => {
      const candidate = filmstrip.querySelector('img')
      expect(candidate).not.toBeNull()
      return candidate as HTMLImageElement
    })
    Object.defineProperties(thumbnail, {
      naturalWidth: { configurable: true, value: 4_000 },
      naturalHeight: { configurable: true, value: 1_000 },
    })
    fireEvent.load(thumbnail)
    await waitFor(() =>
      expect(filmstrip.querySelector('.folder-filmstrip-track')).toHaveStyle({ width: '552px' }),
    )

    initiallyUnknown.imageMetadata = { width: 1, height: 1 }
    rendered.rerender(<FolderFilmstripRow {...props} density="compact" />)

    await waitFor(() =>
      expect(filmstrip.querySelector('.folder-filmstrip-track')).toHaveStyle({ width: '120px' }),
    )
    expect(within(filmstrip).getByRole('listitem')).toHaveStyle({
      left: '12px',
      width: '96px',
      height: '96px',
    })
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

function installClampedScrollLeft(filmstrip: HTMLElement, clientWidth: number) {
  let scrollLeft = filmstrip.scrollLeft
  Object.defineProperty(filmstrip, 'clientWidth', { configurable: true, value: clientWidth })
  Object.defineProperty(filmstrip, 'scrollLeft', {
    configurable: true,
    get: () => {
      const track = filmstrip.querySelector<HTMLElement>('.folder-filmstrip-track')
      const maximum = Math.max(0, Number.parseFloat(track?.style.width ?? '0') - clientWidth)
      return Math.max(0, Math.min(maximum, scrollLeft))
    },
    set: (next: number) => {
      scrollLeft = next
    },
  })
}
