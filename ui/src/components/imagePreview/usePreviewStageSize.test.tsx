import { act, render, screen } from '@testing-library/react'
import { useRef } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { usePreviewStageSize } from './usePreviewStageSize'

afterEach(() => {
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
})

describe('usePreviewStageSize', () => {
  it('keeps zero measurements pending until ResizeObserver publishes a real stage', () => {
    const resize = installResizeObserver()
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue(rect(0, 0))

    render(<StageHarness />)
    const stage = screen.getByTestId('preview-stage')
    expect(screen.getByRole('status')).toHaveTextContent('0×0')

    resize.trigger(stage, Number.NaN, 800)
    expect(screen.getByRole('status')).toHaveTextContent('0×0')

    resize.trigger(stage, 1512.4, 982.6)
    expect(screen.getByRole('status')).toHaveTextContent('1512×983')
  })

  it('uses a real bounding rectangle when ResizeObserver is unavailable', () => {
    vi.stubGlobal('ResizeObserver', undefined)
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue(rect(1280, 720))

    render(<StageHarness />)

    expect(screen.getByRole('status')).toHaveTextContent('1280×720')
  })

  it('remeasures after the mounted stage finishes its next-frame layout', () => {
    installResizeObserver()
    let bounds = rect(640, 480)
    let nextFrame: FrameRequestCallback | undefined
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(() => bounds)
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
      nextFrame = callback
      return 1
    })
    vi.stubGlobal('cancelAnimationFrame', vi.fn())

    render(<StageHarness />)
    expect(screen.getByRole('status')).toHaveTextContent('640×480')

    bounds = rect(1280, 720)
    act(() => nextFrame?.(0))

    expect(screen.getByRole('status')).toHaveTextContent('1280×720')
  })

  it('keeps measuring while the review layout expands across later frames', () => {
    installResizeObserver()
    let bounds = rect(625, 416)
    const frames: FrameRequestCallback[] = []
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(() => bounds)
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
      frames.push(callback)
      return frames.length
    })
    vi.stubGlobal('cancelAnimationFrame', vi.fn())

    render(<StageHarness />)
    expect(screen.getByRole('status')).toHaveTextContent('625×416')

    act(() => frames.shift()?.(0))
    expect(screen.getByRole('status')).toHaveTextContent('625×416')

    bounds = rect(800, 520)
    act(() => frames.shift()?.(16))
    expect(screen.getByRole('status')).toHaveTextContent('800×520')

    bounds = rect(960, 664)
    act(() => frames.shift()?.(32))
    expect(screen.getByRole('status')).toHaveTextContent('960×664')
  })

  it('self-corrects on window resize when the stage observer misses a layout change', () => {
    installResizeObserver()
    let bounds = rect(640, 480)
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(() => bounds)

    render(<StageHarness />)
    expect(screen.getByRole('status')).toHaveTextContent('640×480')

    bounds = rect(1280, 720)
    act(() => window.dispatchEvent(new Event('resize')))

    expect(screen.getByRole('status')).toHaveTextContent('1280×720')
  })

  it('remeasures the stage when its review-layout parent reports a resize', () => {
    const resize = installResizeObserver()
    let bounds = rect(625, 416)
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(() => bounds)
    vi.stubGlobal(
      'requestAnimationFrame',
      vi.fn(() => 1),
    )
    vi.stubGlobal('cancelAnimationFrame', vi.fn())

    render(<StageHarness />)
    const stage = screen.getByTestId('preview-stage')
    const layout = stage.parentElement
    if (layout === null) throw new Error('Expected a review-layout parent')
    expect(screen.getByRole('status')).toHaveTextContent('625×416')

    bounds = rect(960, 664)
    resize.trigger(layout, 1280, 720)

    expect(screen.getByRole('status')).toHaveTextContent('960×664')
  })

  it('never substitutes parent dimensions for a stage that has not completed layout', () => {
    const resize = installResizeObserver()
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue(rect(0, 0))
    vi.stubGlobal(
      'requestAnimationFrame',
      vi.fn(() => 1),
    )
    vi.stubGlobal('cancelAnimationFrame', vi.fn())

    render(<StageHarness />)
    const stage = screen.getByTestId('preview-stage')
    const layout = stage.parentElement
    if (layout === null) throw new Error('Expected a review-layout parent')

    resize.trigger(layout, 1280, 720)

    expect(screen.getByRole('status')).toHaveTextContent('0×0')
  })

  it('disconnects the stage observer on unmount', () => {
    const resize = installResizeObserver()
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue(rect(0, 0))
    const rendered = render(<StageHarness />)
    const stage = screen.getByTestId('preview-stage')

    rendered.unmount()

    expect(resize.disconnected(stage)).toBe(1)
  })
})

function StageHarness() {
  const stage = useRef<HTMLDivElement>(null)
  const size = usePreviewStageSize(stage)
  return (
    <>
      <div ref={stage} data-testid="preview-stage" />
      <output>
        {size.width}×{size.height}
      </output>
    </>
  )
}

function installResizeObserver() {
  const callbacks = new Map<Element, ResizeObserverCallback>()
  const disconnects = new Map<Element, number>()
  class Observer {
    private readonly nodes = new Set<Element>()
    private readonly callback: ResizeObserverCallback

    constructor(callback: ResizeObserverCallback) {
      this.callback = callback
    }

    observe(node: Element) {
      this.nodes.add(node)
      callbacks.set(node, this.callback)
    }

    disconnect() {
      for (const node of this.nodes) {
        callbacks.delete(node)
        disconnects.set(node, (disconnects.get(node) ?? 0) + 1)
      }
      this.nodes.clear()
    }
  }
  vi.stubGlobal('ResizeObserver', Observer)
  return {
    disconnected: (node: Element) => disconnects.get(node) ?? 0,
    trigger: (node: Element, width: number, height: number) => {
      act(() => {
        callbacks.get(node)?.(
          [{ target: node, contentRect: { width, height } } as ResizeObserverEntry],
          {} as ResizeObserver,
        )
      })
    },
  }
}

function rect(width: number, height: number): DOMRect {
  return {
    x: 0,
    y: 0,
    left: 0,
    top: 0,
    right: width,
    bottom: height,
    width,
    height,
    toJSON: () => undefined,
  }
}
