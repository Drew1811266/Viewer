import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { BrowserFile } from '../api/types'
import AspectThumbnail from './AspectThumbnail'

const file: BrowserFile = {
  entityId: 'image-1',
  relativePath: 'catalog/image-1.jpg',
  name: 'image-1.jpg',
  kind: 'jpeg',
  size: 100,
  modifiedNs: '42',
  marker: { reviewState: null, favorite: false },
  imageMetadata: { width: 3_000, height: 2_000 },
  imageUrl: null,
  videoMetadata: null,
}

const originalDevicePixelRatio = window.devicePixelRatio

afterEach(() => {
  Object.defineProperty(window, 'devicePixelRatio', {
    configurable: true,
    value: originalDevicePixelRatio,
  })
})

describe('AspectThumbnail', () => {
  it('keeps known thumbnail loading geometry before the image request resolves', () => {
    const loading = deferred<string>()
    const { container } = render(
      <AspectThumbnail
        file={file}
        width={198}
        height={132}
        dimensionsKnown
        loadThumbnail={() => loading.promise}
        onNaturalDimensions={vi.fn()}
      />,
    )

    expect(screen.getByLabelText('缩略图加载中')).toHaveStyle({ width: '198px', height: '132px' })
    expect(container.querySelector('.aspect-thumbnail')).toHaveStyle({
      width: '198px',
      height: '132px',
    })
    expect(container.querySelector('.aspect-thumbnail')).toHaveAttribute(
      'data-thumbnail-state',
      'loading',
    )
  })

  it('renders the complete known-ratio image at the supplied proportional size', async () => {
    const loadThumbnail = vi.fn().mockResolvedValue('viewer-image://thumbnail')
    const { container } = render(
      <AspectThumbnail
        file={file}
        width={198}
        height={132}
        dimensionsKnown
        loadThumbnail={loadThumbnail}
        onNaturalDimensions={vi.fn()}
      />,
    )

    const image = await findImage(container)
    const surface = container.querySelector<HTMLElement>('.aspect-thumbnail')
    expect(surface).toHaveStyle({ width: '198px', height: '132px' })
    expect(image).toHaveStyle({
      width: '198px',
      height: '132px',
      objectFit: 'contain',
      visibility: 'visible',
    })
    expect(image.style.objectFit).not.toBe('cover')
  })

  it('keeps an unknown-ratio image hidden behind a square skeleton until recovery rerenders', async () => {
    const unknownFile = { ...file, imageMetadata: null }
    const onNaturalDimensions = vi.fn()
    const loadThumbnail = vi.fn().mockResolvedValue('viewer-image://thumbnail')
    const rendered = render(
      <AspectThumbnail
        file={unknownFile}
        width={132}
        height={132}
        dimensionsKnown={false}
        loadThumbnail={loadThumbnail}
        onNaturalDimensions={onNaturalDimensions}
      />,
    )

    const image = await findImage(rendered.container)
    expect(screen.getByLabelText('缩略图加载中')).toBeVisible()
    expect(rendered.container.querySelector('.aspect-thumbnail')).toHaveStyle({
      width: '132px',
      height: '132px',
    })
    expect(image).toHaveStyle({
      width: '132px',
      height: '132px',
      visibility: 'hidden',
    })

    Object.defineProperties(image, {
      naturalWidth: { configurable: true, value: 4_000 },
      naturalHeight: { configurable: true, value: 2_000 },
    })
    fireEvent.load(image)

    expect(onNaturalDimensions).toHaveBeenCalledWith(
      { entityId: 'image-1', modifiedNs: '42' },
      { width: 4_000, height: 2_000 },
    )
    expect(image).toHaveStyle({ visibility: 'hidden' })
    expect(screen.getByLabelText('缩略图加载中')).toBeVisible()

    rendered.rerender(
      <AspectThumbnail
        file={unknownFile}
        width={264}
        height={132}
        dimensionsKnown
        loadThumbnail={loadThumbnail}
        onNaturalDimensions={onNaturalDimensions}
      />,
    )

    expect(rendered.container.querySelector('img')).toHaveStyle({
      width: '264px',
      height: '132px',
      visibility: 'visible',
    })
    expect(screen.queryByLabelText('缩略图加载中')).not.toBeInTheDocument()
  })

  it('keeps the placeholder when loaded natural dimensions are invalid', async () => {
    const onNaturalDimensions = vi.fn()
    const { container } = render(
      <AspectThumbnail
        file={{ ...file, imageMetadata: null }}
        width={132}
        height={132}
        dimensionsKnown={false}
        loadThumbnail={vi.fn().mockResolvedValue('viewer-image://thumbnail')}
        onNaturalDimensions={onNaturalDimensions}
      />,
    )

    const image = await findImage(container)
    Object.defineProperties(image, {
      naturalWidth: { configurable: true, value: 0 },
      naturalHeight: { configurable: true, value: Number.NaN },
    })
    fireEvent.load(image)

    expect(onNaturalDimensions).not.toHaveBeenCalled()
    expect(image).toHaveStyle({ visibility: 'hidden' })
    expect(screen.getByLabelText('缩略图加载中')).toBeVisible()
  })

  it('displays a per-thumbnail failure state', async () => {
    const { container } = render(
      <AspectThumbnail
        file={file}
        width={198}
        height={132}
        dimensionsKnown
        loadThumbnail={vi.fn().mockRejectedValue(new Error('offline'))}
        onNaturalDimensions={vi.fn()}
      />,
    )

    const failed = await screen.findByLabelText('缩略图不可用')
    expect(failed).toHaveStyle({ width: '198px', height: '132px' })
    expect(container.querySelector('.aspect-thumbnail')).toHaveStyle({
      width: '198px',
      height: '132px',
    })
    expect(container.querySelector('.aspect-thumbnail')).toHaveAttribute(
      'data-thumbnail-state',
      'failed',
    )
    expect(screen.queryByRole('img')).not.toBeInTheDocument()
  })

  it('sizes requests for DPR and caps the physical edge and scale', async () => {
    Object.defineProperty(window, 'devicePixelRatio', {
      configurable: true,
      value: 5,
    })
    const loadThumbnail = vi.fn().mockResolvedValue('viewer-image://thumbnail')
    render(
      <AspectThumbnail
        file={file}
        width={2_000}
        height={1_000}
        dimensionsKnown
        loadThumbnail={loadThumbnail}
        onNaturalDimensions={vi.fn()}
      />,
    )

    await waitFor(() => expect(loadThumbnail).toHaveBeenCalledWith(file, 4_096, 4_000))
  })

  it('ignores an obsolete thumbnail completion after the file identity changes', async () => {
    const firstRequest = deferred<string>()
    const secondRequest = deferred<string>()
    const changedFile = {
      ...file,
      entityId: 'image-2',
      relativePath: 'catalog/image-2.jpg',
      name: 'image-2.jpg',
      modifiedNs: '43',
    }
    const loadThumbnail = vi
      .fn<(candidate: BrowserFile) => Promise<string>>()
      .mockReturnValueOnce(firstRequest.promise)
      .mockReturnValueOnce(secondRequest.promise)
    const rendered = render(
      <AspectThumbnail
        file={file}
        width={198}
        height={132}
        dimensionsKnown
        loadThumbnail={loadThumbnail}
        onNaturalDimensions={vi.fn()}
      />,
    )

    rendered.rerender(
      <AspectThumbnail
        file={changedFile}
        width={198}
        height={132}
        dimensionsKnown
        loadThumbnail={loadThumbnail}
        onNaturalDimensions={vi.fn()}
      />,
    )
    firstRequest.resolve('viewer-image://obsolete')

    await waitFor(() => expect(loadThumbnail).toHaveBeenCalledTimes(2))
    expect(rendered.container.querySelector('img')).not.toBeInTheDocument()
    expect(screen.getByLabelText('缩略图加载中')).toBeVisible()

    secondRequest.resolve('viewer-image://current')

    await waitFor(() =>
      expect(rendered.container.querySelector('img')).toHaveAttribute(
        'src',
        'viewer-image://current',
      ),
    )
    expect(rendered.container.querySelector('img')).not.toHaveAttribute(
      'src',
      'viewer-image://obsolete',
    )
  })
})

async function findImage(container: HTMLElement): Promise<HTMLImageElement> {
  let image: HTMLImageElement | null = null
  await waitFor(() => {
    image = container.querySelector('img')
    expect(image).not.toBeNull()
  })
  if (image === null) throw new Error('Expected thumbnail image')
  return image
}

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((next) => {
    resolve = next
  })
  return { promise, resolve }
}
