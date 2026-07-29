import { fireEvent, render, screen } from '@testing-library/react'
import { useState } from 'react'
import { describe, expect, it, vi } from 'vitest'
import CompareVirtualViewport from './CompareVirtualViewport'
import type { CompareLayoutPlan } from './compareLayoutEngine'

describe('CompareVirtualViewport', () => {
  it('mounts only the horizontal visible window, overscan, and active item', () => {
    const plan = planWithTwentyItems('horizontal')
    const renderItem = vi.fn((entityId: string) => <article tabIndex={0}>{entityId}</article>)

    render(
      <CompareVirtualViewport
        plan={plan}
        activeEntityId="image-19"
        renderItem={renderItem}
        onActivate={vi.fn()}
      />,
    )

    expect(renderItem.mock.calls.length).toBeLessThan(20)
    expect(
      screen
        .getAllByRole('listitem')
        .map((item) => [item.getAttribute('aria-posinset'), item.getAttribute('aria-setsize')]),
    ).toEqual([
      ['1', '20'],
      ['2', '20'],
      ['20', '20'],
    ])
  })

  it('retains an offscreen active pane as at most one item beyond the visible window', () => {
    const plan = planWithTwentyItems('horizontal')
    const props = {
      plan,
      renderItem: (entityId: string) => <article tabIndex={0}>{entityId}</article>,
      onActivate: vi.fn(),
    }
    const rendered = render(<CompareVirtualViewport {...props} activeEntityId="missing" />)
    const visibleCount = screen.getAllByRole('listitem').length

    rendered.rerender(<CompareVirtualViewport {...props} activeEntityId="image-19" />)

    expect(screen.getByRole('listitem', { name: 'image-19' })).toBeInTheDocument()
    expect(screen.getAllByRole('listitem').length).toBeLessThanOrEqual(visibleCount + 1)
  })

  it('mounts only the vertical visible window, overscan, and active item', () => {
    const plan = planWithTwentyItems('vertical')
    const renderItem = vi.fn((entityId: string) => <article tabIndex={0}>{entityId}</article>)

    render(
      <CompareVirtualViewport
        plan={plan}
        activeEntityId="image-19"
        renderItem={renderItem}
        onActivate={vi.fn()}
      />,
    )

    expect(renderItem.mock.calls.length).toBeLessThan(20)
    expect(screen.getByRole('listitem', { name: 'image-19' })).toHaveAttribute(
      'aria-posinset',
      '20',
    )
    expect(screen.getByRole('list', { name: '滚动图片对比' })).toHaveAttribute(
      'data-axis',
      'vertical',
    )
  })

  it('unmounts an item after it leaves the vertical render window', () => {
    const plan = planWithTwentyItems('vertical')
    render(
      <CompareVirtualViewport
        plan={plan}
        activeEntityId="image-19"
        renderItem={(entityId) => <article tabIndex={0}>{entityId}</article>}
        onActivate={vi.fn()}
      />,
    )
    const viewport = screen.getByRole('list', { name: '滚动图片对比' })
    expect(screen.getByRole('listitem', { name: 'image-0' })).toBeInTheDocument()

    viewport.scrollTop = 1_200
    fireEvent.scroll(viewport)

    expect(screen.queryByRole('listitem', { name: 'image-0' })).not.toBeInTheDocument()
    expect(screen.getAllByRole('listitem').length).toBeLessThan(20)
  })

  it('translates a plain vertical wheel gesture into horizontal movement', () => {
    render(
      <CompareVirtualViewport
        plan={planWithTwentyItems('horizontal')}
        activeEntityId="image-0"
        renderItem={(entityId) => <article tabIndex={0}>{entityId}</article>}
        onActivate={vi.fn()}
      />,
    )
    const viewport = screen.getByRole('list', { name: '滚动图片对比' })
    installHorizontalScrollMetrics(viewport, 100, 2_380)
    viewport.scrollLeft = 120

    const { preventDefault } = wheel(viewport, { deltaY: 40 })

    expect(viewport.scrollLeft).toBe(160)
    expect(preventDefault).toHaveBeenCalledOnce()
  })

  it('actively cancels the native wheel default while horizontal movement is possible', () => {
    render(
      <CompareVirtualViewport
        plan={planWithTwentyItems('horizontal')}
        activeEntityId="image-0"
        renderItem={(entityId) => <article tabIndex={0}>{entityId}</article>}
        onActivate={vi.fn()}
      />,
    )
    const viewport = screen.getByRole('list', { name: '滚动图片对比' })
    installHorizontalScrollMetrics(viewport, 100, 2_380)
    viewport.scrollLeft = 120

    const { event } = wheel(viewport, { deltaY: 40 })

    expect(event.defaultPrevented).toBe(true)
  })

  it('prefers a horizontal wheel delta when the gesture supplies both axes', () => {
    render(
      <CompareVirtualViewport
        plan={planWithTwentyItems('horizontal')}
        activeEntityId="image-0"
        renderItem={(entityId) => <article tabIndex={0}>{entityId}</article>}
        onActivate={vi.fn()}
      />,
    )
    const viewport = screen.getByRole('list', { name: '滚动图片对比' })
    installHorizontalScrollMetrics(viewport, 100, 2_380)
    viewport.scrollLeft = 120

    wheel(viewport, { deltaX: 15, deltaY: 40 })

    expect(viewport.scrollLeft).toBe(135)
  })

  it('releases horizontal wheel gestures at the start and end edges', () => {
    render(
      <CompareVirtualViewport
        plan={planWithTwentyItems('horizontal')}
        activeEntityId="image-0"
        renderItem={(entityId) => <article tabIndex={0}>{entityId}</article>}
        onActivate={vi.fn()}
      />,
    )
    const viewport = screen.getByRole('list', { name: '滚动图片对比' })
    installHorizontalScrollMetrics(viewport, 100, 2_380)

    viewport.scrollLeft = 0
    const start = wheel(viewport, { deltaY: -40 })
    expect(viewport.scrollLeft).toBe(0)
    expect(start.preventDefault).not.toHaveBeenCalled()

    viewport.scrollLeft = 2_280
    const end = wheel(viewport, { deltaY: 40 })
    expect(viewport.scrollLeft).toBe(2_280)
    expect(end.preventDefault).not.toHaveBeenCalled()
  })

  it('leaves vertical-plan wheel scrolling native', () => {
    render(
      <CompareVirtualViewport
        plan={planWithTwentyItems('vertical')}
        activeEntityId="image-0"
        renderItem={(entityId) => <article tabIndex={0}>{entityId}</article>}
        onActivate={vi.fn()}
      />,
    )
    const viewport = screen.getByRole('list', { name: '滚动图片对比' })
    installHorizontalScrollMetrics(viewport, 100, 500)

    const { preventDefault } = wheel(viewport, { deltaY: 40 })

    expect(viewport.scrollLeft).toBe(0)
    expect(preventDefault).not.toHaveBeenCalled()
  })

  it('removes horizontal wheel translation when the plan changes to vertical', () => {
    const props = {
      activeEntityId: 'image-0',
      renderItem: (entityId: string) => <article tabIndex={0}>{entityId}</article>,
      onActivate: vi.fn(),
    }
    const rendered = render(
      <CompareVirtualViewport {...props} plan={planWithTwentyItems('horizontal')} />,
    )
    const viewport = screen.getByRole('list', { name: '滚动图片对比' })
    installHorizontalScrollMetrics(viewport, 100, 500)

    rendered.rerender(<CompareVirtualViewport {...props} plan={planWithTwentyItems('vertical')} />)
    viewport.scrollLeft = 0
    const { preventDefault } = wheel(viewport, { deltaY: 40 })

    expect(viewport.scrollLeft).toBe(0)
    expect(preventDefault).not.toHaveBeenCalled()
  })

  it('activates and focuses horizontal strip neighbors without interpolating entity selectors', () => {
    const targetEntityId = 'image-"6]'
    const plan = {
      ...planWithTwentyItems('horizontal'),
      rects: planWithTwentyItems('horizontal').rects.map((rect) =>
        rect.index === 6 ? { ...rect, entityId: targetEntityId } : rect,
      ),
    }
    const activated = vi.fn()
    renderActivatingViewport(plan, 'image-5', activated)
    const viewport = screen.getByRole('list', { name: '滚动图片对比' })

    fireEvent.keyDown(viewport, { key: 'ArrowRight' })

    expect(activated).toHaveBeenNthCalledWith(1, targetEntityId)
    expect(screen.getByRole('article', { name: targetEntityId })).toHaveFocus()

    fireEvent.keyDown(viewport, { key: 'ArrowLeft' })

    expect(activated).toHaveBeenCalledWith('image-5')
    expect(screen.getByRole('article', { name: 'image-5' })).toHaveFocus()
  })

  it('moves vertically by the plan column count and focuses the logical neighbor', () => {
    const plan = planWithTwentyItems('vertical', 2)
    const activated = vi.fn()
    renderActivatingViewport(plan, 'image-4', activated)
    const viewport = screen.getByRole('list', { name: '滚动图片对比' })

    fireEvent.keyDown(viewport, { key: 'ArrowDown' })

    expect(activated).toHaveBeenNthCalledWith(1, 'image-6')
    expect(screen.getByRole('article', { name: 'image-6' })).toHaveFocus()

    fireEvent.keyDown(viewport, { key: 'ArrowUp' })

    expect(activated).toHaveBeenCalledWith('image-4')
    expect(screen.getByRole('article', { name: 'image-4' })).toHaveFocus()
  })

  it.each([
    ['horizontal', 'image-5', 550, 750],
    ['vertical', 'image-6', 310, 430],
  ] as const)(
    'keeps the active entity at its prior visual offset after a %s plan change',
    (axis, activeEntityId, previousOffset, expectedOffset) => {
      const previous = planWithTwentyItems(axis, 2)
      const next = respacePlan(previous, 160)
      const props = {
        activeEntityId,
        renderItem: (entityId: string) => (
          <article tabIndex={0} aria-label={entityId}>
            {entityId}
          </article>
        ),
        onActivate: vi.fn(),
      }
      const rendered = render(<CompareVirtualViewport {...props} plan={previous} />)
      const viewport = screen.getByRole('list', { name: '滚动图片对比' })
      if (axis === 'horizontal') viewport.scrollLeft = previousOffset
      else viewport.scrollTop = previousOffset
      fireEvent.scroll(viewport)

      rendered.rerender(<CompareVirtualViewport {...props} plan={next} />)

      expect(axis === 'horizontal' ? viewport.scrollLeft : viewport.scrollTop).toBe(expectedOffset)
    },
  )

  it('anchors from the captured offset when a shrinking track clamps the DOM before layout effects', () => {
    const previous = respacePlan(planWithTwentyItems('horizontal'), 160)
    const next = respacePlan(planWithTwentyItems('horizontal'), 80)
    const props = {
      activeEntityId: 'image-15',
      renderItem: (entityId: string) => <article tabIndex={0}>{entityId}</article>,
      onActivate: vi.fn(),
    }
    const rendered = render(<CompareVirtualViewport {...props} plan={previous} />)
    const viewport = screen.getByRole('list', { name: '滚动图片对比' })
    installTrackClampedScrollLeft(viewport, 100)
    viewport.scrollLeft = 2_300
    fireEvent.scroll(viewport)

    rendered.rerender(<CompareVirtualViewport {...props} plan={next} />)

    expect(viewport.scrollLeft).toBe(1_100)
  })

  it('preserves the active visual delta when the DOM resets during a horizontal-to-vertical transition', () => {
    const previous = planWithTwentyItems('horizontal')
    const next = planWithTwentyItems('vertical', 2)
    const props = {
      activeEntityId: 'image-5',
      renderItem: (entityId: string) => <article tabIndex={0}>{entityId}</article>,
      onActivate: vi.fn(),
    }
    const rendered = render(<CompareVirtualViewport {...props} plan={previous} />)
    const viewport = screen.getByRole('list', { name: '滚动图片对比' })
    installAxisResetScrollOffsets(viewport)
    viewport.scrollLeft = 550
    fireEvent.scroll(viewport)

    rendered.rerender(<CompareVirtualViewport {...props} plan={next} />)

    expect(viewport.scrollTop).toBe(190)
  })
})

function planWithTwentyItems(
  axis: 'horizontal' | 'vertical',
  verticalColumns = 1,
): CompareLayoutPlan {
  const horizontal = axis === 'horizontal'
  const columns = horizontal ? 20 : verticalColumns
  const rows = horizontal ? 1 : Math.ceil(20 / columns)
  return {
    key: `${axis}-twenty`,
    kind: horizontal ? 'horizontal-strip' : 'vertical-flow',
    score: 0,
    eligible: true,
    scrollAxis: axis,
    columns,
    rows,
    rects: Array.from({ length: 20 }, (_, index) => ({
      entityId: `image-${index}`,
      index,
      left: horizontal ? index * 120 : (index % columns) * 120,
      top: horizontal ? 0 : Math.floor(index / columns) * 120,
      width: 100,
      height: 100,
      stageWidth: 100,
      stageHeight: 80,
    })),
    viewportWidth: 100,
    viewportHeight: 100,
    totalWidth: horizontal ? 2_380 : columns * 120 - 20,
    totalHeight: horizontal ? 100 : rows * 120 - 20,
    candidateCount: 1,
    retainedPrevious: false,
  }
}

function renderActivatingViewport(
  plan: CompareLayoutPlan,
  initialActiveEntityId: string,
  activated: (entityId: string) => void,
) {
  function Harness() {
    const [activeEntityId, setActiveEntityId] = useState(initialActiveEntityId)
    return (
      <CompareVirtualViewport
        plan={plan}
        activeEntityId={activeEntityId}
        renderItem={(entityId) => (
          <article tabIndex={0} aria-label={entityId}>
            {entityId}
          </article>
        )}
        onActivate={(entityId) => {
          activated(entityId)
          setActiveEntityId(entityId)
        }}
      />
    )
  }

  return render(<Harness />)
}

function respacePlan(plan: CompareLayoutPlan, stride: number): CompareLayoutPlan {
  const horizontal = plan.scrollAxis === 'horizontal'
  const rects = plan.rects.map((rect) => ({
    ...rect,
    left: horizontal ? rect.index * stride : (rect.index % plan.columns) * stride,
    top: horizontal ? 0 : Math.floor(rect.index / plan.columns) * stride,
  }))
  return {
    ...plan,
    key: `${plan.key}-respace-${stride}`,
    rects,
    totalWidth: horizontal
      ? Math.max(...rects.map((rect) => rect.left + rect.width))
      : plan.totalWidth,
    totalHeight: horizontal
      ? plan.totalHeight
      : Math.max(...rects.map((rect) => rect.top + rect.height)),
  }
}

function installHorizontalScrollMetrics(
  viewport: HTMLElement,
  clientWidth: number,
  scrollWidth: number,
) {
  Object.defineProperty(viewport, 'clientWidth', { configurable: true, value: clientWidth })
  Object.defineProperty(viewport, 'scrollWidth', { configurable: true, value: scrollWidth })
}

function installTrackClampedScrollLeft(viewport: HTMLElement, clientWidth: number) {
  let scrollLeft = viewport.scrollLeft
  Object.defineProperty(viewport, 'clientWidth', { configurable: true, value: clientWidth })
  Object.defineProperty(viewport, 'scrollLeft', {
    configurable: true,
    get: () => {
      const track = viewport.querySelector<HTMLElement>('.compare-scroll-track')
      if (track === null) throw new Error('Expected compare scroll track')
      const maximum = Math.max(0, Number.parseFloat(track.style.width) - clientWidth)
      return Math.max(0, Math.min(maximum, scrollLeft))
    },
    set: (next: number) => {
      scrollLeft = next
    },
  })
}

function installAxisResetScrollOffsets(viewport: HTMLElement) {
  let scrollLeft = viewport.scrollLeft
  let scrollTop = viewport.scrollTop
  Object.defineProperty(viewport, 'scrollLeft', {
    configurable: true,
    get: () => (viewport.dataset.axis === 'horizontal' ? scrollLeft : 0),
    set: (next: number) => {
      scrollLeft = next
    },
  })
  Object.defineProperty(viewport, 'scrollTop', {
    configurable: true,
    get: () => (viewport.dataset.axis === 'vertical' ? scrollTop : 0),
    set: (next: number) => {
      scrollTop = next
    },
  })
}

function wheel(viewport: HTMLElement, init: WheelEventInit) {
  const event = new WheelEvent('wheel', { bubbles: true, cancelable: true, ...init })
  const preventDefault = vi.spyOn(event, 'preventDefault')
  fireEvent(viewport, event)
  return { event, preventDefault }
}
