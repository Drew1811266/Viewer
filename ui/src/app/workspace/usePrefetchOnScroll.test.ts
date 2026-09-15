import { act, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import {
  computePrefetchTarget,
  findVerticalScrollAncestor,
  PREFETCH_REST_MS,
  usePrefetchOnScroll,
} from './usePrefetchOnScroll'

interface ScrollFixture {
  container: HTMLElement
  setScrollTop(value: number): void
  scrollBy(delta: number): void
}

function createScrollFixture(clientHeight = 400): ScrollFixture {
  const container = document.createElement('div')
  let scrollTop = 0
  Object.defineProperty(container, 'scrollTop', {
    configurable: true,
    get: () => scrollTop,
    set: (value: number) => {
      scrollTop = value
    },
  })
  Object.defineProperty(container, 'clientHeight', {
    configurable: true,
    get: () => clientHeight,
  })
  return {
    container,
    setScrollTop(value: number) {
      scrollTop = value
    },
    scrollBy(delta: number) {
      scrollTop += delta
      container.dispatchEvent(new Event('scroll'))
    },
  }
}

describe('computePrefetchTarget', () => {
  it('maps the viewport edges to the adjacent rows', () => {
    // 视口覆盖第 0-3 行（400px / 100px），向下预取第 4 行。
    expect(computePrefetchTarget(0, 400, 100, 6, 'down')).toBe(4)
    // 滚动两行后向下预取第 6 行。
    expect(computePrefetchTarget(200, 400, 100, 8, 'down')).toBe(6)
    // 向上预取视口上边缘外第一行。
    expect(computePrefetchTarget(250, 400, 100, 8, 'up')).toBe(1)
    expect(computePrefetchTarget(300, 400, 100, 8, 'up')).toBe(1)
  })

  it('returns null at the list boundaries and for degenerate inputs', () => {
    expect(computePrefetchTarget(400, 400, 100, 5, 'down')).toBeNull()
    expect(computePrefetchTarget(0, 400, 100, 5, 'up')).toBeNull()
    expect(computePrefetchTarget(0, 400, 0, 5, 'down')).toBeNull()
    expect(computePrefetchTarget(0, 0, 100, 5, 'down')).toBeNull()
    expect(computePrefetchTarget(0, 400, 100, 0, 'down')).toBeNull()
  })
})

describe('findVerticalScrollAncestor', () => {
  it('returns the nearest ancestor with vertical overflow', () => {
    const scroller = document.createElement('div')
    const inner = document.createElement('div')
    const leaf = document.createElement('div')
    inner.appendChild(leaf)
    scroller.appendChild(inner)
    scroller.style.overflowY = 'auto'
    document.body.appendChild(scroller)
    try {
      expect(findVerticalScrollAncestor(leaf)).toBe(scroller)
    } finally {
      scroller.remove()
    }
  })

  it('returns null when no ancestor scrolls vertically', () => {
    const plain = document.createElement('div')
    const leaf = document.createElement('div')
    plain.appendChild(leaf)
    document.body.appendChild(plain)
    try {
      expect(findVerticalScrollAncestor(leaf)).toBeNull()
    } finally {
      plain.remove()
    }
  })
})

describe('usePrefetchOnScroll', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) =>
      setTimeout(() => callback(performance.now()), 0),
    )
    vi.stubGlobal('cancelAnimationFrame', (handle: number) => clearTimeout(handle))
  })

  afterEach(() => {
    vi.unstubAllGlobals()
    vi.useRealTimers()
  })

  function setup(fixture: ScrollFixture, rowCount = 6, rowHeight = 100) {
    const prefetches: Array<{ rowIndex: number; signal: AbortSignal }> = []
    const rendered = renderHook(() =>
      usePrefetchOnScroll({
        getContainer: () => fixture.container,
        rowCount,
        rowHeight,
        onPrefetchRow: (rowIndex, signal) => prefetches.push({ rowIndex, signal }),
      }),
    )
    return { prefetches, unmount: rendered.unmount }
  }

  it('prefetches three rows ahead below the viewport once scrolling rests', () => {
    const fixture = createScrollFixture()
    const { prefetches } = setup(fixture)

    act(() => {
      vi.advanceTimersByTime(100)
      fixture.scrollBy(60)
    })
    act(() => {
      vi.advanceTimersByTime(PREFETCH_REST_MS + 16)
    })

    expect(prefetches.map(({ rowIndex }) => rowIndex)).toEqual([4, 5])
    expect(prefetches.every(({ signal }) => !signal.aborted)).toBe(true)
  })

  it('prefetches three rows ahead above when scrolling up', () => {
    const fixture = createScrollFixture()
    fixture.setScrollTop(450)
    const { prefetches } = setup(fixture)

    act(() => {
      vi.advanceTimersByTime(100)
      fixture.scrollBy(-80)
    })
    act(() => {
      vi.advanceTimersByTime(PREFETCH_REST_MS + 16)
    })

    expect(prefetches.map(({ rowIndex }) => rowIndex)).toEqual([2, 1, 0])
  })

  it('skips prefetching while the gesture is a fast flick', () => {
    const fixture = createScrollFixture()
    const { prefetches } = setup(fixture)

    act(() => {
      vi.advanceTimersByTime(100)
      fixture.scrollBy(5)
      vi.advanceTimersByTime(16)
      fixture.scrollBy(805)
      vi.advanceTimersByTime(16)
    })
    act(() => {
      vi.advanceTimersByTime(PREFETCH_REST_MS + 200)
    })

    expect(prefetches).toHaveLength(0)
  })

  it('aborts the active prefetch when the direction reverses', () => {
    const fixture = createScrollFixture()
    const { prefetches } = setup(fixture)

    act(() => {
      vi.advanceTimersByTime(100)
      fixture.scrollBy(60)
      vi.advanceTimersByTime(PREFETCH_REST_MS + 16)
    })
    expect(prefetches).toHaveLength(2)

    act(() => {
      fixture.scrollBy(-20)
      vi.advanceTimersByTime(16)
    })
    expect(prefetches.every(({ signal }) => signal.aborted)).toBe(true)
  })

  it('does not repeat covered targets until the direction changes', () => {
    const fixture = createScrollFixture()
    const { prefetches } = setup(fixture)

    act(() => {
      vi.advanceTimersByTime(100)
      fixture.scrollBy(60)
      vi.advanceTimersByTime(PREFETCH_REST_MS + 16)
      fixture.scrollBy(10)
      vi.advanceTimersByTime(PREFETCH_REST_MS + 16)
    })
    expect(prefetches.map(({ rowIndex }) => rowIndex)).toEqual([4, 5])
    expect(prefetches.every(({ signal }) => !signal.aborted)).toBe(true)

    // 反向（abort 旧预取）之后再向下，覆盖集清空，会以新信号重新发起。
    act(() => {
      fixture.scrollBy(-10)
      vi.advanceTimersByTime(32)
      fixture.scrollBy(10)
      vi.advanceTimersByTime(PREFETCH_REST_MS + 16)
    })
    expect(prefetches.map(({ rowIndex }) => rowIndex)).toEqual([4, 5, 4, 5])
    expect(prefetches[0]?.signal.aborted).toBe(true)
    expect(prefetches[2]?.signal.aborted).toBe(false)
  })

  it('aborts the active prefetch when the hook unmounts', () => {
    const fixture = createScrollFixture()
    const { prefetches, unmount } = setup(fixture)

    act(() => {
      vi.advanceTimersByTime(100)
      fixture.scrollBy(60)
      vi.advanceTimersByTime(PREFETCH_REST_MS + 16)
    })
    expect(prefetches).toHaveLength(2)
    expect(prefetches.every(({ signal }) => !signal.aborted)).toBe(true)

    unmount()
    expect(prefetches.every(({ signal }) => signal.aborted)).toBe(true)
  })
})
