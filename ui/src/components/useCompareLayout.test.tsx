import { act, render, renderHook, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { BrowserFile } from '../api/types'
import { visibleCompareIndexes } from './compareLayoutEngine'
import {
  compareSourceRevision,
  type RecoveredCompareDimensions,
  useCompareLayout,
} from './useCompareLayout'

afterEach(() => {
  vi.unstubAllGlobals()
  document.body.replaceChildren()
})

describe('useCompareLayout', () => {
  it('uses metadata and quarter rotation to solve the layout', () => {
    const frames = installAnimationFrameQueue()
    const resize = installResizeObserver()
    const hook = renderHook(() =>
      useCompareLayout({
        files: portraitFiles(4),
        rotations: { a: 0, b: 0, c: 0, d: 0 },
        recoveredDimensions: {},
      }),
    )
    attachRef(hook.result.current.containerRef)
    hook.rerender()

    act(() => {
      resize(1_700, 900)
      frames.flush()
    })

    expect(hook.result.current.plan.kind).toBe('fit-row')
  })

  it('coalesces multiple ResizeObserver notifications into one frame', () => {
    const frames = installAnimationFrameQueue()
    const resize = installResizeObserver()
    const hook = renderHook(() =>
      useCompareLayout({ files: portraitFiles(4), rotations: {}, recoveredDimensions: {} }),
    )
    attachRef(hook.result.current.containerRef)
    hook.rerender()

    act(() => {
      resize(1_600, 900)
      resize(1_650, 900)
      resize(1_700, 900)
    })

    expect(frames.pending()).toBe(1)
    act(() => frames.flush())
    expect(hook.result.current.plan.totalWidth).toBe(1_700)
  })

  it('commits a narrower viewport when clamped strip rectangles stay unchanged', () => {
    const frames = installAnimationFrameQueue()
    const resize = installResizeObserver()
    const hook = renderHook(() =>
      useCompareLayout({ files: portraitFiles(4), rotations: {}, recoveredDimensions: {} }),
    )
    attachRef(hook.result.current.containerRef)
    hook.rerender()

    act(() => {
      resize(426, 900)
      frames.flush()
    })
    const widerPlan = hook.result.current.plan
    expect(widerPlan.viewportWidth).toBe(426)
    expect(visibleCompareIndexes(widerPlan, 0, 0)).toEqual([0, 1, 2])

    act(() => {
      resize(320, 900)
      frames.flush()
    })

    expect(hook.result.current.plan).not.toBe(widerPlan)
    expect(hook.result.current.plan.viewportWidth).toBe(320)
    expect(visibleCompareIndexes(hook.result.current.plan, 0, 0)).toEqual([0, 1])
  })

  it('uses a square fallback when metadata and recovered dimensions are unavailable', () => {
    const frames = installAnimationFrameQueue()
    const resize = installResizeObserver()
    const files = [image('a', null), image('b', null)]
    const hook = renderHook(() =>
      useCompareLayout({ files, rotations: {}, recoveredDimensions: {} }),
    )
    attachRef(hook.result.current.containerRef)
    hook.rerender()

    act(() => {
      resize(800, 600)
      frames.flush()
    })

    expect(hook.result.current.plan.score).toBeCloseTo(0.5308, 3)
  })

  it('uses matching recovered dimensions when metadata is unavailable', () => {
    const frames = installAnimationFrameQueue()
    const resize = installResizeObserver()
    const first = image('a', null)
    const second = image('b', null)
    const files = [first, second]
    const recoveredDimensions = {
      ...recoveredFor(first, 400, 800),
      ...recoveredFor(second, 400, 800),
    }
    const hook = renderHook(() => useCompareLayout({ files, rotations: {}, recoveredDimensions }))
    attachRef(hook.result.current.containerRef)
    hook.rerender()

    act(() => {
      resize(800, 600)
      frames.flush()
    })

    expect(hook.result.current.plan.score).toBeCloseTo(0.8158, 3)
  })

  it('ignores recovered dimensions from a stale source revision', () => {
    const frames = installAnimationFrameQueue()
    const resize = installResizeObserver()
    const first = image('a', null)
    const files = [first, image('b', null)]
    const recoveredDimensions = {
      [compareSourceRevision(first)]: { sourceRevision: 'stale', width: 400, height: 800 },
    }
    const hook = renderHook(() => useCompareLayout({ files, rotations: {}, recoveredDimensions }))
    attachRef(hook.result.current.containerRef)
    hook.rerender()

    act(() => {
      resize(800, 600)
      frames.flush()
    })

    expect(hook.result.current.plan.score).toBeCloseTo(0.5308, 3)
  })

  it('inverts metadata and recovered ratios for quarter rotations', () => {
    const frames = installAnimationFrameQueue()
    const resize = installResizeObserver()
    const metadataFiles = [
      image('a', { width: 800, height: 400 }),
      image('b', { width: 800, height: 400 }),
    ]
    const metadata = renderHook(() =>
      useCompareLayout({
        files: metadataFiles,
        rotations: { a: 90, b: 270 },
        recoveredDimensions: {},
      }),
    )
    attachRef(metadata.result.current.containerRef)
    metadata.rerender()

    act(() => {
      resize(800, 600)
      frames.flush()
    })
    expect(metadata.result.current.plan.score).toBeCloseTo(0.8158, 3)

    const recoveredFirst = image('c', null)
    const recoveredSecond = image('d', null)
    const recoveredFiles = [recoveredFirst, recoveredSecond]
    const recovered = renderHook(() =>
      useCompareLayout({
        files: recoveredFiles,
        rotations: { c: 90, d: 270 },
        recoveredDimensions: {
          ...recoveredFor(recoveredFirst, 800, 400),
          ...recoveredFor(recoveredSecond, 800, 400),
        },
      }),
    )
    attachRef(recovered.result.current.containerRef)
    recovered.rerender()

    act(() => {
      resize(800, 600)
      frames.flush()
    })
    expect(recovered.result.current.plan.score).toBeCloseTo(0.8158, 3)
  })

  it('performs one controlled re-solve when matching recovered dimensions replace the fallback', () => {
    const frames = installAnimationFrameQueue()
    const resize = installResizeObserver()
    const first = image('a', null)
    const files = [first, image('b', null)]
    const hook = renderHook(
      ({
        recoveredDimensions,
      }: {
        recoveredDimensions: Record<string, RecoveredCompareDimensions>
      }) => useCompareLayout({ files, rotations: {}, recoveredDimensions }),
      { initialProps: { recoveredDimensions: {} } },
    )
    attachRef(hook.result.current.containerRef)
    hook.rerender({ recoveredDimensions: {} })

    act(() => {
      resize(800, 600)
      frames.flush()
    })
    const fallbackPlan = hook.result.current.plan

    hook.rerender({ recoveredDimensions: recoveredFor(first, 400, 800) })
    expect(frames.pending()).toBe(1)
    act(() => frames.flush())
    expect(hook.result.current.plan).not.toBe(fallbackPlan)

    hook.rerender({ recoveredDimensions: recoveredFor(first, 400, 800) })
    expect(frames.pending()).toBe(0)
  })

  it('observes an outer region attached after the hook first renders', () => {
    const resize = installNodeResizeObserver()
    const rendered = render(<CompareLayoutHarness attached={false} files={[image('a', null)]} />)

    rendered.rerender(<CompareLayoutHarness attached files={[image('a', null)]} />)

    expect(resize.observed(screen.getByTestId('compare-region'))).toBe(true)
  })

  it('disconnects the replaced outer region and observes its replacement', () => {
    const frames = installAnimationFrameQueue()
    const resize = installNodeResizeObserver()
    const rendered = render(
      <CompareLayoutHarness attached nodeKey="first" files={[image('a', null)]} />,
    )
    const first = screen.getByTestId('compare-region')

    act(() => {
      resize.trigger(first, 800, 600)
      frames.flush()
    })

    rendered.rerender(<CompareLayoutHarness attached nodeKey="second" files={[image('a', null)]} />)
    const second = screen.getByTestId('compare-region')

    expect(resize.disconnected(first)).toBe(1)
    expect(resize.observed(second)).toBe(true)
  })

  it('replaces a zero-size safe plan when the source order changes', () => {
    const frames = installAnimationFrameQueue()
    const rendered = render(<CompareLayoutHarness attached={false} files={[image('a', null)]} />)

    expect(screen.getByTestId('planned-entity')).toHaveTextContent('a')
    rendered.rerender(<CompareLayoutHarness attached={false} files={[image('b', null)]} />)

    expect(frames.pending()).toBe(1)
    act(() => frames.flush())
    expect(screen.getByTestId('planned-entity')).toHaveTextContent('b')
  })
})

function CompareLayoutHarness({
  attached,
  files,
  nodeKey = 'region',
}: {
  attached: boolean
  files: readonly BrowserFile[]
  nodeKey?: string
}) {
  const { containerRef, plan } = useCompareLayout({
    files,
    rotations: {},
    recoveredDimensions: {},
  })
  return (
    <>
      <output data-testid="planned-entity">{plan.rects[0]?.entityId}</output>
      {attached ? <div data-testid="compare-region" key={nodeKey} ref={containerRef} /> : null}
    </>
  )
}

function image(entityId: string, imageMetadata: BrowserFile['imageMetadata']): BrowserFile {
  return {
    entityId,
    relativePath: `${entityId}.jpg`,
    name: `${entityId}.jpg`,
    kind: 'jpeg',
    size: 100,
    modifiedNs: '1',
    marker: { reviewState: null, favorite: false },
    imageMetadata,
    imageUrl: null,
    videoMetadata: null,
  }
}

function portraitFiles(count: number): BrowserFile[] {
  return Array.from({ length: count }, (_, index) =>
    image(String.fromCharCode('a'.charCodeAt(0) + index), { width: 3, height: 4 }),
  )
}

function recoveredFor(
  file: BrowserFile,
  width: number,
  height: number,
): Record<string, RecoveredCompareDimensions> {
  return {
    [compareSourceRevision(file)]: { sourceRevision: compareSourceRevision(file), width, height },
  }
}

function attachRef(ref: { current: HTMLDivElement | null }) {
  const element = document.createElement('div')
  document.body.append(element)
  ref.current = element
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
  return (width: number, height: number) => {
    callback?.([{ contentRect: { width, height } } as ResizeObserverEntry], {} as ResizeObserver)
  }
}

function installNodeResizeObserver() {
  const callbacks = new Map<HTMLElement, ResizeObserverCallback>()
  const disconnects = new Map<HTMLElement, number>()
  class Observer {
    private node: HTMLElement | null = null
    private readonly callback: ResizeObserverCallback

    constructor(callback: ResizeObserverCallback) {
      this.callback = callback
    }

    observe(node: HTMLElement) {
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
    observed: (node: HTMLElement) => callbacks.has(node),
    disconnected: (node: HTMLElement) => disconnects.get(node) ?? 0,
    trigger: (node: HTMLElement, width: number, height: number) => {
      callbacks.get(node)?.(
        [{ contentRect: { width, height } } as ResizeObserverEntry],
        {} as ResizeObserver,
      )
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
