import { act, render, screen } from '@testing-library/react'
import { createRef } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { ImageRepresentation } from '../../api/types'
import ImageMagnifier, { type ImageMagnifierHandle } from './ImageMagnifier'
import type { CurrentOriginalState } from './useCurrentOriginal'

const ORIGINAL: ImageRepresentation = {
  cacheKey: 'original',
  url: 'viewer-image://localhost/session/original',
  width: 6000,
  height: 4000,
  backend: 'image_io',
}

function currentOriginal(
  status: CurrentOriginalState['status'],
  representation: ImageRepresentation | null = null,
): CurrentOriginalState {
  return { status, entityId: 'image-one', representation }
}

describe('ImageMagnifier', () => {
  it.each([
    ['circle', 'small', 200, 200],
    ['circle', 'medium', 280, 280],
    ['circle', 'large', 380, 380],
    ['rounded_rectangle', 'small', 230, 150],
    ['rounded_rectangle', 'medium', 300, 200],
    ['rounded_rectangle', 'large', 420, 280],
  ] as const)('renders %s %s with the approved fixed area', (shape, area, width, height) => {
    render(
      <ImageMagnifier
        ref={createRef<ImageMagnifierHandle>()}
        shape={shape}
        area={area}
        magnification={2}
        sourceScale={0.25}
        stageSize={{ width: 640, height: 480 }}
        rotation={0}
        fileName="detail.jpg"
        original={currentOriginal('loading')}
      />,
    )

    expect(screen.getByTestId('image-magnifier')).toHaveAttribute('data-shape', shape)
    expect(screen.getByTestId('image-magnifier')).toHaveStyle({
      width: `${width}px`,
      height: `${height}px`,
    })
  })

  it('renders one non-announced duplicate of the ready original', () => {
    render(
      <ImageMagnifier
        ref={createRef<ImageMagnifierHandle>()}
        shape="circle"
        area="small"
        magnification={2}
        sourceScale={0.25}
        stageSize={{ width: 640, height: 480 }}
        rotation={90}
        fileName="detail.jpg"
        original={currentOriginal('ready', ORIGINAL)}
      />,
    )

    const source = screen.getByTestId('image-magnifier-source')
    expect(source).toHaveAttribute('src', ORIGINAL.url)
    expect(source).toHaveAttribute('alt', '')
    expect(source).toHaveAttribute('aria-hidden', 'true')
    expect(source).toHaveAttribute('data-source-name', 'detail.jpg')
    expect(source).toHaveAttribute('draggable', 'false')
  })

  it('keeps the compound overlay visual-only and outside the interaction tree', () => {
    render(
      <ImageMagnifier
        ref={createRef<ImageMagnifierHandle>()}
        shape="circle"
        area="small"
        magnification={2}
        sourceScale={0.25}
        stageSize={{ width: 640, height: 480 }}
        rotation={0}
        fileName="detail.jpg"
        original={currentOriginal('ready', ORIGINAL)}
        overlayPainter={() => 3}
      />,
    )

    expect(screen.getByTestId('image-magnifier')).toHaveAttribute('aria-hidden', 'true')
    expect(screen.getByTestId('image-magnifier-overlay')).not.toHaveAttribute('tabindex')
    expect(screen.queryByRole('button')).not.toBeInTheDocument()
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument()
  })

  it.each([
    ['idle', '正在载入原图'],
    ['loading', '正在载入原图'],
    ['budget_error', '原图超出安全预览限制'],
    ['error', '无法载入原图'],
  ] as const)('contains %s status copy inside the moving visual', (status, copy) => {
    render(
      <ImageMagnifier
        ref={createRef<ImageMagnifierHandle>()}
        shape="circle"
        area="small"
        magnification={2}
        sourceScale={0.25}
        stageSize={{ width: 640, height: 480 }}
        rotation={0}
        fileName="detail.jpg"
        original={currentOriginal(status)}
      />,
    )

    expect(screen.getByText(copy)).toBeInTheDocument()
    expect(screen.getByTestId('image-magnifier')).toHaveAttribute('aria-hidden', 'true')
  })
})

describe('ImageMagnifier imperative placement', () => {
  let frames: FrameRequestCallback[]
  let canceled: number[]
  let nextFrame: number

  beforeEach(() => {
    frames = []
    canceled = []
    nextFrame = 0
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
      frames.push(callback)
      nextFrame += 1
      return nextFrame
    })
    vi.stubGlobal('cancelAnimationFrame', (frame: number) => canceled.push(frame))
  })

  afterEach(() => {
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
  })

  it('paints a high-DPI overlay through the same scheduled placement frame', () => {
    vi.stubGlobal('devicePixelRatio', 2)
    const context = overlayContext()
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(context.value)
    const painter = vi.fn(() => 2)
    const ref = createRef<ImageMagnifierHandle>()
    render(
      <ImageMagnifier
        ref={ref}
        shape="circle"
        area="small"
        magnification={2}
        sourceScale={0.25}
        stageSize={{ width: 640, height: 480 }}
        rotation={0}
        fileName="detail.jpg"
        original={currentOriginal('ready', ORIGINAL)}
        overlayPainter={painter}
      />,
    )

    act(() => {
      ref.current?.place({ stagePoint: { x: 200, y: 150 }, sourcePoint: { x: 1600, y: 1140 } })
      frames.shift()?.(0)
    })

    const overlay = screen.getByTestId('image-magnifier-overlay')
    expect(overlay).toHaveAttribute('width', '400')
    expect(overlay).toHaveAttribute('height', '400')
    expect(overlay).toHaveAttribute('data-has-content', 'true')
    expect(painter).toHaveBeenCalledWith(
      context.value,
      expect.objectContaining({ magnification: 2, pixelRatio: 2 }),
    )
    expect(screen.getByTestId('image-magnifier')).toHaveAttribute('data-visible', 'true')
  })

  it('keeps the image lens visible when the optional overlay painter fails', () => {
    const context = overlayContext()
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(context.value)
    const ref = createRef<ImageMagnifierHandle>()
    render(
      <ImageMagnifier
        ref={ref}
        shape="circle"
        area="small"
        magnification={2}
        sourceScale={0.25}
        stageSize={{ width: 640, height: 480 }}
        rotation={0}
        fileName="detail.jpg"
        original={currentOriginal('ready', ORIGINAL)}
        overlayPainter={() => {
          throw new Error('painter failed')
        }}
      />,
    )

    expect(() => {
      act(() => {
        ref.current?.place({ stagePoint: { x: 200, y: 150 }, sourcePoint: { x: 1600, y: 1140 } })
        frames.shift()?.(0)
      })
    }).not.toThrow()
    expect(screen.getByTestId('image-magnifier')).toHaveAttribute('data-visible', 'true')
    expect(screen.getByTestId('image-magnifier-overlay')).not.toHaveAttribute('data-has-content')
  })

  it('clears old overlay content as soon as the original identity becomes unavailable', () => {
    const context = overlayContext()
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(context.value)
    const ref = createRef<ImageMagnifierHandle>()
    const view = render(
      <ImageMagnifier
        ref={ref}
        shape="circle"
        area="small"
        magnification={2}
        sourceScale={0.25}
        stageSize={{ width: 640, height: 480 }}
        rotation={0}
        fileName="detail.jpg"
        original={currentOriginal('ready', ORIGINAL)}
        overlayPainter={() => 1}
      />,
    )
    act(() => {
      ref.current?.place({ stagePoint: { x: 200, y: 150 }, sourcePoint: { x: 1600, y: 1140 } })
      frames.shift()?.(0)
    })
    expect(screen.getByTestId('image-magnifier-overlay')).toHaveAttribute(
      'data-has-content',
      'true',
    )

    view.rerender(
      <ImageMagnifier
        ref={ref}
        shape="circle"
        area="small"
        magnification={2}
        sourceScale={0.25}
        stageSize={{ width: 640, height: 480 }}
        rotation={0}
        fileName="next.jpg"
        original={{ status: 'loading', entityId: 'image-two', representation: null }}
        overlayPainter={() => 1}
      />,
    )

    expect(screen.getByTestId('image-magnifier-overlay')).not.toHaveAttribute('data-has-content')
    expect(context.clearRect).toHaveBeenCalled()
  })

  it('applies only the latest placement per animation frame', () => {
    const ref = createRef<ImageMagnifierHandle>()
    render(
      <ImageMagnifier
        ref={ref}
        shape="circle"
        area="small"
        magnification={2}
        sourceScale={0.25}
        stageSize={{ width: 640, height: 480 }}
        rotation={90}
        fileName="detail.jpg"
        original={currentOriginal('ready', ORIGINAL)}
      />,
    )

    act(() => {
      ref.current?.place({ stagePoint: { x: 10, y: 20 }, sourcePoint: { x: 30, y: 40 } })
      ref.current?.place({
        stagePoint: { x: 200, y: 150 },
        sourcePoint: { x: 1600, y: 1140 },
      })
    })
    expect(frames).toHaveLength(1)
    expect(screen.getByTestId('image-magnifier')).not.toHaveAttribute('data-visible')

    act(() => frames.shift()?.(0))
    expect(screen.getByTestId('image-magnifier')).toHaveAttribute('data-visible', 'true')
    expect(screen.getByTestId('image-magnifier')).toHaveStyle({
      '--magnifier-x': '318px',
      '--magnifier-y': '268px',
      '--magnifier-shell-origin-x': '0px',
      '--magnifier-shell-origin-y': '0px',
      '--magnifier-scale': '0.5',
      '--magnifier-rotation': '90deg',
    })
    expect(screen.getByTestId('image-magnifier-source')).toHaveStyle({
      '--magnifier-source-left': '-1500px',
      '--magnifier-source-top': '-1040px',
    })

    act(() => {
      ref.current?.place({
        stagePoint: { x: 600, y: 440 },
        sourcePoint: { x: 1600, y: 1140 },
      })
      frames.shift()?.(16)
    })
    expect(screen.getByTestId('image-magnifier')).toHaveStyle({
      '--magnifier-x': '482px',
      '--magnifier-y': '322px',
      '--magnifier-shell-origin-x': '200px',
      '--magnifier-shell-origin-y': '200px',
    })
    expect(screen.getByTestId('image-magnifier-source')).toHaveStyle({
      '--magnifier-source-left': '-1500px',
      '--magnifier-source-top': '-1040px',
    })
  })

  it('hides immediately and cancels queued placement on hide or unmount', () => {
    const ref = createRef<ImageMagnifierHandle>()
    const view = render(
      <ImageMagnifier
        ref={ref}
        shape="circle"
        area="small"
        magnification={2}
        sourceScale={0.25}
        stageSize={{ width: 640, height: 480 }}
        rotation={0}
        fileName="detail.jpg"
        original={currentOriginal('loading')}
      />,
    )
    const lens = screen.getByTestId('image-magnifier')

    act(() => {
      ref.current?.place({ stagePoint: { x: 1, y: 2 }, sourcePoint: { x: 3, y: 4 } })
      ref.current?.hide()
    })
    expect(lens).not.toHaveAttribute('data-visible')
    expect(canceled).toEqual([1])
    frames.shift()?.(0)
    expect(lens).not.toHaveAttribute('data-visible')

    act(() => {
      ref.current?.place({ stagePoint: { x: 5, y: 6 }, sourcePoint: { x: 7, y: 8 } })
    })
    view.unmount()
    expect(canceled).toEqual([1, 2])
  })
})

function overlayContext() {
  const clearRect = vi.fn()
  return {
    clearRect,
    value: {
      canvas: document.createElement('canvas'),
      clearRect,
      setTransform: vi.fn(),
    } as unknown as CanvasRenderingContext2D,
  }
}
