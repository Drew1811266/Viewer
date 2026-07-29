import { act, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { useMeasuredElementHeight } from './useMeasuredElementHeight'

afterEach(() => {
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
})

describe('useMeasuredElementHeight', () => {
  it('publishes only the latest valid height once per animation frame', () => {
    const observer = installResizeObserver()
    const frames = installAnimationFrameQueue()
    render(<HeightHarness fallbackHeight={520} />)
    const slot = screen.getByTestId('measured-slot')

    observer.trigger(slot, 900, 480)
    observer.trigger(slot, 900, 460)
    observer.trigger(slot, 900, Number.NaN)
    expect(frames.pending()).toBe(1)
    expect(screen.getByRole('status')).toHaveTextContent('520')

    act(() => frames.flush())
    expect(screen.getByRole('status')).toHaveTextContent('460')

    observer.trigger(slot, 900, 460)
    act(() => frames.flush())
    expect(screen.getByRole('status')).toHaveTextContent('460')

    observer.trigger(slot, 900, 420)
    observer.trigger(slot, 900, 460)
    expect(frames.pending()).toBe(1)
    act(() => frames.flush())
    expect(screen.getByRole('status')).toHaveTextContent('460')
  })

  it('uses the bounding rectangle without ResizeObserver and keeps the last valid height', () => {
    vi.stubGlobal('ResizeObserver', undefined)
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue({
      width: 800,
      height: 410,
      top: 0,
      right: 800,
      bottom: 410,
      left: 0,
      x: 0,
      y: 0,
      toJSON: () => undefined,
    })

    render(<HeightHarness fallbackHeight={520} />)

    expect(screen.getByRole('status')).toHaveTextContent('410')
  })

  it('disconnects its observer and cancels a queued frame on unmount', () => {
    const observer = installResizeObserver()
    const frames = installAnimationFrameQueue()
    const rendered = render(<HeightHarness fallbackHeight={520} />)
    const slot = screen.getByTestId('measured-slot')

    observer.trigger(slot, 900, 480)
    expect(frames.pending()).toBe(1)

    rendered.unmount()

    expect(observer.disconnected(slot)).toBe(1)
    expect(frames.pending()).toBe(0)
  })
})

function HeightHarness({ fallbackHeight }: { fallbackHeight: number }) {
  const measured = useMeasuredElementHeight(fallbackHeight)
  return (
    <>
      <div ref={measured.ref} data-testid="measured-slot" />
      <output>{measured.height}</output>
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

function installAnimationFrameQueue() {
  let nextId = 1
  const callbacks = new Map<number, FrameRequestCallback>()
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
    const id = nextId++
    callbacks.set(id, callback)
    return id
  })
  vi.stubGlobal('cancelAnimationFrame', (id: number) => callbacks.delete(id))
  return {
    pending: () => callbacks.size,
    flush: () => {
      const queued = [...callbacks.values()]
      callbacks.clear()
      queued.forEach((callback) => {
        callback(0)
      })
    },
  }
}
