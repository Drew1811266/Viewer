import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { AspectRect } from '../layout/aspectLayout'
import AspectVirtualGrid from './AspectVirtualGrid'

interface GridItem {
  key: string
  width: number
  height: number
}

const getKey = (item: GridItem) => item.key
const getDimensions = (item: GridItem) => ({ width: item.width, height: item.height })
const renderItem = (item: GridItem, index: number, rect: AspectRect) => (
  <button
    type="button"
    aria-label={item.key}
    data-source-index={index}
    data-testid={`surface-${item.key}`}
    style={{ width: rect.imageWidth, height: rect.imageHeight }}
  />
)

afterEach(() => vi.unstubAllGlobals())

describe('AspectVirtualGrid', () => {
  it('rebuilds rows from ResizeObserver width changes without changing source order', () => {
    const resize = installResizeObserver()
    const items = mixedItems()
    renderGrid({ items, imageHeight: 60, captionHeight: 0, gap: 10 })

    resize(160)
    expect(mountedKeys()).toEqual(['portrait', 'square', 'landscape', 'last'])
    expect(itemWrapper('landscape')).toHaveStyle({ left: '0px', top: '70px' })
    expect(itemWrapper('last')).toHaveStyle({ left: '100px', top: '70px' })

    resize(110)
    expect(mountedKeys()).toEqual(['portrait', 'square', 'landscape', 'last'])
    expect(itemWrapper('landscape')).toHaveStyle({ left: '0px', top: '70px' })
    expect(itemWrapper('last')).toHaveStyle({ left: '0px', top: '140px' })
  })

  it('uses one image height, proportional widths, and a left-aligned final row', () => {
    const resize = installResizeObserver()
    renderGrid({ items: mixedItems(), imageHeight: 60, captionHeight: 8, gap: 10 })
    resize(160)

    expect(surface('portrait')).toHaveStyle({ width: '40px', height: '60px' })
    expect(surface('square')).toHaveStyle({ width: '60px', height: '60px' })
    expect(surface('landscape')).toHaveStyle({ width: '90px', height: '60px' })
    expect(surface('last')).toHaveStyle({ width: '60px', height: '60px' })
    expect(itemWrapper('last')).toHaveStyle({ left: '100px', top: '78px' })

    resize(120)
    expect(itemWrapper('last')).toHaveStyle({ left: '0px', top: '156px' })
  })

  it('keeps a lone over-wide item at natural width with horizontal access', () => {
    const resize = installResizeObserver()
    renderGrid({
      items: [{ key: 'panorama', width: 10, height: 1 }],
      imageHeight: 20,
      captionHeight: 0,
      viewportHeight: 40,
    })
    resize(100)

    const grid = screen.getByRole('listbox', { name: 'images' })
    const track = within(grid).getByTestId('aspect-virtual-grid-track')
    expect(grid).toHaveStyle({ overflow: 'auto' })
    expect(track).toHaveStyle({ width: '200px' })
    expect(itemWrapper('panorama')).toHaveStyle({ width: '200px', left: '0px' })
  })

  it('mounts only visible rows plus two overscan rows for 1,000 items', () => {
    const resize = installResizeObserver()
    const items = squareItems(1_000)
    renderGrid({
      items,
      imageHeight: 20,
      captionHeight: 0,
      viewportHeight: 40,
      gap: 0,
    })
    resize(100)

    const grid = screen.getByRole('listbox', { name: 'images' })
    expect(grid.querySelectorAll('[data-virtual-grid-item]')).toHaveLength(25)
    expect(screen.queryByRole('button', { name: 'item-100' })).not.toBeInTheDocument()
  })

  it('applies layout defaults and forwards active-descendant and multiselectable ARIA', () => {
    const resize = installResizeObserver()
    render(
      <AspectVirtualGrid
        items={squareItems(1_000)}
        imageHeight={20}
        getKey={getKey}
        getDimensions={getDimensions}
        renderItem={renderItem}
        ariaLabel="default images"
        activeDescendant="image-active"
        ariaMultiselectable
      />,
    )
    resize(100)
    const grid = screen.getByRole('listbox', { name: 'default images' })

    expect(grid).toHaveStyle({ height: '520px' })
    expect(grid).toHaveAttribute('aria-activedescendant', 'image-active')
    expect(grid).toHaveAttribute('aria-multiselectable', 'true')
    expect(itemWrapper('item-1')).toHaveStyle({ left: '32px', height: '68px' })
    expect(itemWrapper('item-3')).toHaveStyle({ left: '0px', top: '80px' })
    expect(grid.querySelectorAll('[data-virtual-grid-item]')).toHaveLength(27)
  })

  it('retains actual descendant focus independently of activeKey until focus moves', async () => {
    const resize = installResizeObserver()
    const items = squareItems(1_000)
    renderGrid({
      items,
      imageHeight: 20,
      captionHeight: 0,
      viewportHeight: 40,
      gap: 0,
      activeKey: 'item-1',
    })
    resize(100)
    const grid = screen.getByRole('listbox', { name: 'images' })
    const focused = screen.getByRole('button', { name: 'item-0' })
    focused.focus()

    grid.scrollTop = 2_000
    fireEvent.scroll(grid)

    await screen.findByRole('button', { name: 'item-500' })
    expect(focused).toBeInTheDocument()
    expect(focused).toHaveFocus()
    expect(screen.getByRole('button', { name: 'item-1' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'item-100' })).not.toBeInTheDocument()
    expect(grid.querySelectorAll('[data-virtual-grid-item]').length).toBeLessThanOrEqual(42)

    screen.getByRole('button', { name: 'item-500' }).focus()

    await waitFor(() => expect(focused).not.toBeInTheDocument())
    expect(screen.getByRole('button', { name: 'item-500' })).toHaveFocus()
    expect(screen.getByRole('button', { name: 'item-1' })).toBeInTheDocument()
  })

  it('marquee hit tests complete geometry with horizontal and vertical scroll offsets', () => {
    const resize = installResizeObserver()
    const changed = vi.fn()
    const requestFrame = vi.fn(() => 1)
    const cancelFrame = vi.fn()
    vi.stubGlobal('requestAnimationFrame', requestFrame)
    vi.stubGlobal('cancelAnimationFrame', cancelFrame)
    renderGrid({
      items: squareItems(50),
      imageHeight: 10,
      captionHeight: 0,
      viewportHeight: 20,
      gap: 0,
      overscanRows: 0,
      onMarqueeSelectionChange: changed,
    })
    resize(40)
    const grid = screen.getByRole('listbox', { name: 'images' })
    installPointerSurface(grid, { left: 10, top: 20, width: 40, height: 20 })
    grid.scrollLeft = 5
    grid.scrollTop = 50

    expect(screen.queryByRole('button', { name: 'item-40' })).not.toBeInTheDocument()
    fireEvent.pointerDown(grid, { pointerId: 7, button: 0, clientX: 5, clientY: 20 })
    fireEvent.pointerMove(grid, { pointerId: 7, clientX: 15, clientY: 80 })

    expect(changed).toHaveBeenLastCalledWith({
      phase: 'change',
      keys: expect.arrayContaining(['item-40']),
      metaKey: false,
    })
    expect(screen.queryByRole('button', { name: 'item-40' })).not.toBeInTheDocument()
    fireEvent.pointerUp(grid, { pointerId: 7, clientX: 15, clientY: 80 })
  })

  it('preserves the first visible key visual offset across density and width reflow', async () => {
    const resize = installResizeObserver()
    const items = squareItems(30)
    const props = {
      items,
      captionHeight: 0,
      viewportHeight: 20,
      gap: 0,
      getKey,
      getDimensions,
      renderItem,
      ariaLabel: 'images',
    }
    const rendered = render(<AspectVirtualGrid {...props} imageHeight={10} />)
    resize(100)
    const grid = screen.getByRole('listbox', { name: 'images' })
    grid.scrollTop = 12
    fireEvent.scroll(grid)

    rendered.rerender(<AspectVirtualGrid {...props} imageHeight={20} />)
    await waitFor(() => expect(grid.scrollTop).toBe(42))
    expect(itemWrapper('item-10')).toHaveStyle({ top: '40px' })

    resize(60)
    await waitFor(() => expect(grid.scrollTop).toBe(62))
    expect(itemWrapper('item-10')).toHaveStyle({ top: '60px' })
  })

  it('anchors the next row at an exact half-open row boundary', async () => {
    const resize = installResizeObserver()
    const items = squareItems(30)
    const props = {
      items,
      captionHeight: 0,
      viewportHeight: 20,
      gap: 0,
      getKey,
      getDimensions,
      renderItem,
      ariaLabel: 'images',
    }
    const rendered = render(<AspectVirtualGrid {...props} imageHeight={10} />)
    resize(100)
    const grid = screen.getByRole('listbox', { name: 'images' })
    grid.scrollTop = 10
    fireEvent.scroll(grid)

    rendered.rerender(<AspectVirtualGrid {...props} imageHeight={20} />)

    await waitFor(() => expect(grid.scrollTop).toBe(40))
    expect(itemWrapper('item-10')).toHaveStyle({ top: '40px' })
  })

  it('synchronizes virtualization to browser-clamped scroll after shrinking near the bottom', async () => {
    const resize = installResizeObserver()
    const items = squareItems(30)
    const props = {
      items,
      captionHeight: 0,
      viewportHeight: 40,
      gap: 0,
      overscanRows: 0,
      getKey,
      getDimensions,
      renderItem,
      ariaLabel: 'images',
    }
    const rendered = render(<AspectVirtualGrid {...props} imageHeight={20} />)
    resize(60)
    const grid = screen.getByRole('listbox', { name: 'images' })
    installClampedScrollTop(grid, 40)
    grid.scrollTop = 160
    fireEvent.scroll(grid)

    expect(screen.queryByRole('button', { name: 'item-6' })).not.toBeInTheDocument()
    rendered.rerender(<AspectVirtualGrid {...props} imageHeight={10} />)

    await waitFor(() => expect(grid.scrollTop).toBe(10))
    expect(screen.getByRole('button', { name: 'item-6' })).toBeInTheDocument()
  })

  it('synchronizes the first visible entity after width expansion clamps a near-bottom anchor', async () => {
    const resize = installResizeObserver()
    renderGrid({
      items: squareItems(60),
      imageHeight: 10,
      captionHeight: 0,
      viewportHeight: 25,
      gap: 0,
      overscanRows: 0,
    })
    resize(30)
    const grid = screen.getByRole('listbox', { name: 'images' })
    installClampedScrollTop(grid, 25)
    grid.scrollTop = 165
    fireEvent.scroll(grid)

    expect(screen.queryByRole('button', { name: 'item-30' })).not.toBeInTheDocument()
    resize(100)

    await waitFor(() => expect(grid.scrollTop).toBe(35))
    expect(itemWrapper('item-30')).toHaveStyle({ top: '30px' })
    expect(screen.queryByRole('button', { name: 'item-20' })).not.toBeInTheDocument()
  })

  it('uses geometry for directional navigation and leaves selection state with the caller', () => {
    const resize = installResizeObserver()
    const navigate = vi.fn()
    const keyDown = vi.fn()
    renderGrid({
      items: [
        { key: 'wide', width: 3, height: 2 },
        { key: 'narrow', width: 1, height: 2 },
        { key: 'upper-right', width: 1, height: 1 },
        { key: 'lower-left', width: 1, height: 1 },
        { key: 'lower-right', width: 3, height: 2 },
      ],
      imageHeight: 10,
      captionHeight: 0,
      viewportHeight: 40,
      gap: 2,
      activeKey: 'upper-right',
      onNavigate: navigate,
      onKeyDown: keyDown,
    })
    resize(22)
    const grid = screen.getByRole('listbox', { name: 'images' })

    fireEvent.keyDown(grid, { key: 'ArrowDown', shiftKey: true })

    expect(navigate).toHaveBeenCalledWith(4, true)
    expect(keyDown).toHaveBeenCalledTimes(1)
    expect(grid).not.toHaveAttribute('aria-selected')
  })

  it('cancels marquee on Escape and prevents a queued auto-scroll frame from reviving it', () => {
    const resize = installResizeObserver()
    let frame: FrameRequestCallback | null = null
    const requestFrame = vi.fn((callback: FrameRequestCallback) => {
      frame = callback
      return 9
    })
    const cancelFrame = vi.fn()
    vi.stubGlobal('requestAnimationFrame', requestFrame)
    vi.stubGlobal('cancelAnimationFrame', cancelFrame)
    const changed = vi.fn()
    renderGrid({
      items: squareItems(100),
      imageHeight: 10,
      captionHeight: 0,
      viewportHeight: 20,
      gap: 0,
      onMarqueeSelectionChange: changed,
    })
    resize(40)
    const grid = screen.getByRole('listbox', { name: 'images' })
    installPointerSurface(grid, { left: 10, top: 20, width: 40, height: 20 })
    Object.defineProperty(grid, 'scrollHeight', { configurable: true, value: 250 })
    Object.defineProperty(grid, 'clientHeight', { configurable: true, value: 20 })

    fireEvent.pointerDown(grid, { pointerId: 8, button: 0, clientX: 15, clientY: 25 })
    fireEvent.pointerMove(grid, { pointerId: 8, clientX: 25, clientY: 45 })
    expect(requestFrame).toHaveBeenCalledTimes(1)

    fireEvent.keyDown(grid, { key: 'Escape' })
    expect(changed).toHaveBeenLastCalledWith({ phase: 'cancel', keys: [], metaKey: false })
    expect(cancelFrame).toHaveBeenCalledWith(9)
    expect(screen.queryByTestId('marquee-selection')).not.toBeInTheDocument()
    expect(grid).not.toHaveAttribute('data-marquee-active')

    runFrame(frame, 0)
    expect(requestFrame).toHaveBeenCalledTimes(1)
    expect(grid.scrollTop).toBe(0)
  })
})

function renderGrid({
  items,
  imageHeight,
  captionHeight = 48,
  viewportHeight = 520,
  gap = 12,
  overscanRows = 2,
  activeKey,
  onNavigate,
  onKeyDown,
  onMarqueeSelectionChange,
}: {
  items: GridItem[]
  imageHeight: number
  captionHeight?: number
  viewportHeight?: number
  gap?: number
  overscanRows?: number
  activeKey?: string
  onNavigate?: (index: number, extendSelection: boolean) => void
  onKeyDown?: React.KeyboardEventHandler<HTMLDivElement>
  onMarqueeSelectionChange?: React.ComponentProps<
    typeof AspectVirtualGrid<GridItem>
  >['onMarqueeSelectionChange']
}) {
  return render(
    <AspectVirtualGrid
      items={items}
      imageHeight={imageHeight}
      captionHeight={captionHeight}
      viewportHeight={viewportHeight}
      gap={gap}
      overscanRows={overscanRows}
      getKey={getKey}
      getDimensions={getDimensions}
      renderItem={renderItem}
      ariaLabel="images"
      activeKey={activeKey}
      onNavigate={onNavigate}
      onKeyDown={onKeyDown}
      onMarqueeSelectionChange={onMarqueeSelectionChange}
    />,
  )
}

function mixedItems(): GridItem[] {
  return [
    { key: 'portrait', width: 2, height: 3 },
    { key: 'square', width: 1, height: 1 },
    { key: 'landscape', width: 3, height: 2 },
    { key: 'last', width: 1, height: 1 },
  ]
}

function squareItems(count: number): GridItem[] {
  return Array.from({ length: count }, (_, index) => ({
    key: `item-${index}`,
    width: 1,
    height: 1,
  }))
}

function mountedKeys(): string[] {
  return [...document.querySelectorAll<HTMLElement>('[data-virtual-grid-item]')].map(
    (item) => item.dataset.key ?? '',
  )
}

function itemWrapper(key: string): HTMLElement {
  const item = document.querySelector<HTMLElement>(`[data-key="${key}"]`)
  if (item === null) throw new Error(`Expected mounted wrapper for ${key}`)
  return item
}

function surface(key: string): HTMLElement {
  return screen.getByTestId(`surface-${key}`)
}

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
    act(() => {
      callback?.([{ contentRect: { width, height } } as ResizeObserverEntry], {} as ResizeObserver)
    })
  }
}

function installPointerSurface(
  grid: HTMLElement,
  bounds: { left: number; top: number; width: number; height: number },
) {
  vi.spyOn(grid, 'getBoundingClientRect').mockReturnValue({
    ...bounds,
    right: bounds.left + bounds.width,
    bottom: bounds.top + bounds.height,
    x: bounds.left,
    y: bounds.top,
    toJSON: () => undefined,
  })
  Object.defineProperty(grid, 'setPointerCapture', { value: vi.fn(), configurable: true })
  Object.defineProperty(grid, 'releasePointerCapture', { value: vi.fn(), configurable: true })
}

function installClampedScrollTop(grid: HTMLElement, clientHeight: number) {
  let scrollTop = grid.scrollTop
  Object.defineProperty(grid, 'clientHeight', { configurable: true, value: clientHeight })
  Object.defineProperty(grid, 'scrollTop', {
    configurable: true,
    get: () => scrollTop,
    set: (next: number) => {
      const track = within(grid).getByTestId('aspect-virtual-grid-track')
      const maximum = Math.max(0, Number.parseFloat(track.style.height) - clientHeight)
      scrollTop = Math.max(0, Math.min(maximum, next))
    },
  })
}

function runFrame(frame: FrameRequestCallback | null, timestamp: number) {
  if (frame === null) throw new Error('Expected a queued animation frame')
  act(() => frame(timestamp))
}
