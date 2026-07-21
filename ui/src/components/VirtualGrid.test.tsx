import { act, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import VirtualGrid, { type MarqueeSelectionChange } from './VirtualGrid'

afterEach(() => vi.unstubAllGlobals())

function renderGrid(changed: (change: MarqueeSelectionChange) => void) {
  const items = Array.from({ length: 6 }, (_, index) => `item-${index}`)
  return render(
    <VirtualGrid
      items={items}
      cellWidth={100}
      cellHeight={80}
      viewportHeight={200}
      gap={12}
      getKey={(item) => item}
      renderItem={(item) => <span>{item}</span>}
      ariaLabel="files"
      onMarqueeSelectionChange={changed}
    />,
  )
}

function installPointerSurface(grid: HTMLElement) {
  vi.spyOn(grid, 'getBoundingClientRect').mockReturnValue({
    left: 10, top: 20, right: 410, bottom: 220,
    width: 400, height: 200, x: 10, y: 20, toJSON: () => undefined,
  })
  Object.defineProperty(grid, 'setPointerCapture', { value: vi.fn(), configurable: true })
  Object.defineProperty(grid, 'releasePointerCapture', { value: vi.fn(), configurable: true })
}

describe('VirtualGrid marquee selection', () => {
  it('starts only on background, crosses the threshold, and emits ordered keys with an overlay', () => {
    const changed = vi.fn()
    renderGrid(changed)
    const grid = screen.getByRole('listbox', { name: 'files' })
    installPointerSurface(grid)
    fireEvent.pointerDown(grid, { pointerId: 7, button: 0, clientX: 122, clientY: 110 })
    expect(grid.setPointerCapture).toHaveBeenCalledWith(7)
    fireEvent.pointerMove(grid, { pointerId: 7, clientX: 12, clientY: 22 })
    expect(changed).toHaveBeenNthCalledWith(1, { phase: 'start', keys: [], metaKey: false })
    expect(changed).toHaveBeenLastCalledWith({ phase: 'change', keys: ['item-0', 'item-1'], metaKey: false })
    expect(screen.getByTestId('marquee-selection')).toBeVisible()
    fireEvent.pointerUp(grid, { pointerId: 7, clientX: 12, clientY: 22 })
    expect(grid.releasePointerCapture).toHaveBeenCalledWith(7)
    expect(changed).toHaveBeenLastCalledWith({ phase: 'end', keys: ['item-0', 'item-1'], metaKey: false })
    expect(screen.queryByTestId('marquee-selection')).not.toBeInTheDocument()
  })

  it('does not start from an item wrapper or a non-primary button', () => {
    const changed = vi.fn()
    renderGrid(changed)
    fireEvent.pointerDown(screen.getByText('item-0'), { pointerId: 1, button: 0 })
    fireEvent.pointerDown(screen.getByRole('listbox', { name: 'files' }), { pointerId: 2, button: 2 })
    expect(changed).not.toHaveBeenCalled()
  })

  it('freezes Command at pointer-down and reports an empty end for a background click', () => {
    const changed = vi.fn()
    renderGrid(changed)
    const grid = screen.getByRole('listbox', { name: 'files' })
    installPointerSurface(grid)
    fireEvent.pointerDown(grid, { pointerId: 3, button: 0, metaKey: true, clientX: 200, clientY: 100 })
    fireEvent.pointerUp(grid, { pointerId: 3, metaKey: false, clientX: 200, clientY: 100 })
    expect(changed).toHaveBeenLastCalledWith({ phase: 'end', keys: [], metaKey: true })
  })

  it('emits cancel and cleans the overlay on Escape and pointercancel', () => {
    const changed = vi.fn()
    renderGrid(changed)
    const grid = screen.getByRole('listbox', { name: 'files' })
    installPointerSurface(grid)

    fireEvent.pointerDown(grid, { pointerId: 4, button: 0, clientX: 122, clientY: 110 })
    fireEvent.pointerMove(grid, { pointerId: 4, clientX: 12, clientY: 22 })
    fireEvent.keyDown(grid, { key: 'Escape' })
    expect(changed).toHaveBeenLastCalledWith({ phase: 'cancel', keys: [], metaKey: false })
    expect(screen.queryByTestId('marquee-selection')).not.toBeInTheDocument()
    expect(grid).not.toHaveAttribute('data-marquee-active')

    changed.mockClear()
    fireEvent.pointerDown(grid, { pointerId: 5, button: 0, clientX: 122, clientY: 110 })
    fireEvent.pointerMove(grid, { pointerId: 5, clientX: 12, clientY: 22 })
    fireEvent.pointerCancel(grid, { pointerId: 5 })
    expect(grid.releasePointerCapture).toHaveBeenCalledWith(5)
    expect(changed).toHaveBeenLastCalledWith({ phase: 'cancel', keys: [], metaKey: false })
    expect(screen.queryByTestId('marquee-selection')).not.toBeInTheDocument()
    expect(grid).not.toHaveAttribute('data-marquee-active')
  })

  it('auto-scrolls at the edge, recomputes hits, and stops after pointerup', () => {
    let frame: FrameRequestCallback | null = null
    const requestFrame = vi.fn((callback: FrameRequestCallback) => {
      frame = callback
      return requestFrame.mock.calls.length
    })
    const cancelFrame = vi.fn()
    vi.stubGlobal('requestAnimationFrame', requestFrame)
    vi.stubGlobal('cancelAnimationFrame', cancelFrame)
    const changed = vi.fn()
    renderGrid(changed)
    const grid = screen.getByRole('listbox', { name: 'files' })
    installPointerSurface(grid)
    Object.defineProperty(grid, 'scrollHeight', { value: 600, configurable: true })
    Object.defineProperty(grid, 'clientHeight', { value: 200, configurable: true })

    fireEvent.pointerDown(grid, { pointerId: 6, button: 0, clientX: 200, clientY: 180 })
    fireEvent.pointerMove(grid, { pointerId: 6, clientX: 200, clientY: 220 })
    expect(requestFrame).toHaveBeenCalledTimes(1)
    const firstFrame = frame
    act(() => firstFrame?.(0))
    expect(grid.scrollTop).toBeGreaterThan(0)
    expect(grid.scrollTop).toBeLessThanOrEqual(18)
    expect(changed).toHaveBeenLastCalledWith(expect.objectContaining({ phase: 'change' }))

    const staleFrame = frame
    fireEvent.pointerUp(grid, { pointerId: 6, clientX: 200, clientY: 220 })
    const requestCountAtEnd = requestFrame.mock.calls.length
    act(() => staleFrame?.(16))
    expect(requestFrame).toHaveBeenCalledTimes(requestCountAtEnd)
    expect(cancelFrame).toHaveBeenCalled()
  })

  it('stops a queued auto-scroll frame when the pointer returns to the center', () => {
    const requestFrame = vi.fn(() => 1)
    const cancelFrame = vi.fn()
    vi.stubGlobal('requestAnimationFrame', requestFrame)
    vi.stubGlobal('cancelAnimationFrame', cancelFrame)
    const changed = vi.fn()
    renderGrid(changed)
    const grid = screen.getByRole('listbox', { name: 'files' })
    installPointerSurface(grid)

    fireEvent.pointerDown(grid, { pointerId: 8, button: 0, clientX: 200, clientY: 180 })
    fireEvent.pointerMove(grid, { pointerId: 8, clientX: 200, clientY: 220 })
    fireEvent.pointerMove(grid, { pointerId: 8, clientX: 200, clientY: 120 })

    expect(cancelFrame).toHaveBeenCalledWith(1)
  })

  it('uses the resized column layout when a queued frame recomputes marquee hits', () => {
    const resize = installResizeObserver()
    let frame: FrameRequestCallback | null = null
    const requestFrame = vi.fn((callback: FrameRequestCallback) => {
      frame = callback
      return 1
    })
    vi.stubGlobal('requestAnimationFrame', requestFrame)
    const changed = vi.fn()
    const items = Array.from({ length: 12 }, (_, index) => `item-${index}`)
    render(
      <VirtualGrid
        items={items}
        cellWidth={100}
        cellHeight={80}
        viewportHeight={200}
        gap={12}
        getKey={(item) => item}
        renderItem={(item) => <span>{item}</span>}
        ariaLabel="files"
        onMarqueeSelectionChange={changed}
      />,
    )
    const grid = screen.getByRole('listbox', { name: 'files' })
    installPointerSurface(grid)
    Object.defineProperty(grid, 'scrollHeight', { value: 800, configurable: true })
    Object.defineProperty(grid, 'clientHeight', { value: 200, configurable: true })

    fireEvent.pointerDown(grid, { pointerId: 9, button: 0, clientX: 230, clientY: 200 })
    fireEvent.pointerMove(grid, { pointerId: 9, clientX: 12, clientY: 220 })
    act(() => resize(300))
    act(() => frame?.(0))

    expect(changed).toHaveBeenLastCalledWith({ phase: 'change', keys: ['item-4', 'item-5'], metaKey: false })
  })

  it('silently clears a queued session when items are replaced before its frame runs', () => {
    let frame: FrameRequestCallback | null = null
    const requestFrame = vi.fn((callback: FrameRequestCallback) => {
      frame = callback
      return 1
    })
    const cancelFrame = vi.fn()
    vi.stubGlobal('requestAnimationFrame', requestFrame)
    vi.stubGlobal('cancelAnimationFrame', cancelFrame)
    const changed = vi.fn()
    const initialItems = Array.from({ length: 6 }, (_, index) => `old-${index}`)
    const replacementItems = Array.from({ length: 6 }, (_, index) => `new-${index}`)
    const { rerender } = render(
      <VirtualGrid
        items={initialItems}
        cellWidth={100}
        cellHeight={80}
        viewportHeight={200}
        gap={12}
        getKey={(item) => item}
        renderItem={(item) => <span>{item}</span>}
        ariaLabel="files"
        onMarqueeSelectionChange={changed}
      />,
    )
    const grid = screen.getByRole('listbox', { name: 'files' })
    installPointerSurface(grid)
    fireEvent.pointerDown(grid, { pointerId: 10, button: 0, clientX: 200, clientY: 180 })
    fireEvent.pointerMove(grid, { pointerId: 10, clientX: 200, clientY: 220 })
    changed.mockClear()

    rerender(
      <VirtualGrid
        items={replacementItems}
        cellWidth={100}
        cellHeight={80}
        viewportHeight={200}
        gap={12}
        getKey={(item) => item}
        renderItem={(item) => <span>{item}</span>}
        ariaLabel="files"
        onMarqueeSelectionChange={changed}
      />,
    )
    act(() => frame?.(0))

    expect(changed).not.toHaveBeenCalled()
    expect(grid.releasePointerCapture).toHaveBeenCalledWith(10)
    expect(cancelFrame).toHaveBeenCalledWith(1)
  })

  it('silently cleans capture and a queued frame on unmount', () => {
    let frame: FrameRequestCallback | null = null
    const requestFrame = vi.fn((callback: FrameRequestCallback) => {
      frame = callback
      return 1
    })
    const cancelFrame = vi.fn()
    vi.stubGlobal('requestAnimationFrame', requestFrame)
    vi.stubGlobal('cancelAnimationFrame', cancelFrame)
    const changed = vi.fn()
    const rendered = renderGrid(changed)
    const grid = screen.getByRole('listbox', { name: 'files' })
    installPointerSurface(grid)
    fireEvent.pointerDown(grid, { pointerId: 11, button: 0, clientX: 200, clientY: 180 })
    fireEvent.pointerMove(grid, { pointerId: 11, clientX: 200, clientY: 220 })
    changed.mockClear()

    rendered.unmount()
    act(() => frame?.(0))

    expect(grid.releasePointerCapture).toHaveBeenCalledWith(11)
    expect(cancelFrame).toHaveBeenCalledWith(1)
    expect(changed).not.toHaveBeenCalled()
  })
})

function installResizeObserver() {
  let callback: ResizeObserverCallback | undefined
  class Observer {
    constructor(next: ResizeObserverCallback) {
      callback = next
    }

    observe() {}
    disconnect() {}
  }
  vi.stubGlobal('ResizeObserver', Observer)
  return (width: number, height = 200) => {
    callback?.(
      [{ contentRect: { width, height } } as ResizeObserverEntry],
      {} as ResizeObserver,
    )
  }
}
