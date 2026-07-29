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
    expect(frames.cancelled()).toBe(1)
  })

  it('observes a measured node that mounts after the hook', () => {
    const observer = installResizeObserver()
    const frames = installAnimationFrameQueue()
    const rendered = render(<HeightHarness fallbackHeight={520} attached={false} />)

    rendered.rerender(<HeightHarness fallbackHeight={520} attached />)
    const slot = screen.getByTestId('measured-slot')
    expect(observer.observed(slot)).toBe(true)

    observer.trigger(slot, 900, 430)
    act(() => frames.flush())

    expect(screen.getByRole('status')).toHaveTextContent('430')
  })

  it('cleans up a removed node before observing and publishing from its replacement', () => {
    const observer = installResizeObserver()
    const frames = installAnimationFrameQueue()
    const rendered = render(<HeightHarness fallbackHeight={520} attached nodeKey="first" />)
    const first = screen.getByTestId('measured-slot')

    observer.trigger(first, 900, 480)
    expect(frames.pending()).toBe(1)

    rendered.rerender(<HeightHarness fallbackHeight={520} attached={false} />)
    expect(observer.disconnected(first)).toBe(1)
    expect(frames.pending()).toBe(0)
    expect(frames.cancelled()).toBe(1)

    rendered.rerender(<HeightHarness fallbackHeight={520} attached nodeKey="second" />)
    const second = screen.getByTestId('measured-slot')
    expect(second).not.toBe(first)
    expect(observer.observed(second)).toBe(true)

    observer.trigger(second, 900, 440)
    act(() => frames.flush())

    expect(screen.getByRole('status')).toHaveTextContent('440')
  })
})

function HeightHarness({
  fallbackHeight,
  attached = true,
  nodeKey = 'slot',
}: {
  fallbackHeight: number
  attached?: boolean
  nodeKey?: string
}) {
  const measured = useMeasuredElementHeight(fallbackHeight)
  return (
    <>
      {attached ? <div ref={measured.ref} data-testid="measured-slot" key={nodeKey} /> : null}
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
    observed: (node: Element) => callbacks.has(node),
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
  let cancelled = 0
  const callbacks = new Map<number, FrameRequestCallback>()
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
    const id = nextId++
    callbacks.set(id, callback)
    return id
  })
  vi.stubGlobal('cancelAnimationFrame', (id: number) => {
    if (callbacks.delete(id)) cancelled += 1
  })
  return {
    cancelled: () => cancelled,
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
