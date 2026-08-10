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
    private node: Element | null = null
    private readonly callback: ResizeObserverCallback

    constructor(callback: ResizeObserverCallback) {
      this.callback = callback
    }

    observe(node: Element) {
      this.node = node
      callbacks.set(node, this.callback)
    }

    disconnect() {
      if (this.node === null) return
      callbacks.delete(this.node)
      disconnects.set(this.node, (disconnects.get(this.node) ?? 0) + 1)
      this.node = null
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
