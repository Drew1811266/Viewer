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
const originalIntersectionObserver = globalThis.IntersectionObserver

afterEach(() => {
  intersectionCallback = null
  globalThis.IntersectionObserver = originalIntersectionObserver
})

function installIntersectionObserver() {
  class TestIntersectionObserver {
    readonly root = null
    readonly rootMargin = '240px 0px'
    readonly thresholds = [0]

    constructor(callback: IntersectionObserverCallback) {
      intersectionCallback = callback
    }

    disconnect() {}
    observe() {}
    takeRecords(): IntersectionObserverEntry[] {
      return []
    }
    unobserve() {}
  }
  globalThis.IntersectionObserver =
    TestIntersectionObserver as unknown as typeof IntersectionObserver
}

function revealRow() {
  intersectionCallback?.(
    [{ isIntersecting: true } as IntersectionObserverEntry],
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
    expect(screen.getByRole('region', { name: 'B01 图片' })).toHaveAttribute(
      'data-state',
      'idle',
    )

    act(revealRow)

    const filmstrip = await screen.findByRole('region', { name: 'B01 图片' })
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
})
