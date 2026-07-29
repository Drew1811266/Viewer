import type { KeyboardEventHandler, ReactNode, UIEvent, WheelEventHandler } from 'react'
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import {
  anchoredCompareScrollOffset,
  type CompareLayoutPlan,
  visibleCompareIndexes,
} from './compareLayoutEngine'

export interface CompareVirtualViewportProps {
  plan: CompareLayoutPlan
  activeEntityId: string
  renderItem: (entityId: string, index: number) => ReactNode
  onActivate: (entityId: string) => void
}

export default function CompareVirtualViewport({
  plan,
  activeEntityId,
  renderItem,
  onActivate,
}: CompareVirtualViewportProps) {
  const viewportRef = useRef<HTMLDivElement>(null)
  const pendingFocusEntityId = useRef<string | null>(null)
  const previousPlan = useRef<CompareLayoutPlan | null>(null)
  const scrollOffsets = useRef({ left: 0, top: 0 })
  const [scrollLeft, setScrollLeft] = useState(0)
  const [scrollTop, setScrollTop] = useState(0)
  const mountedIndexes = useMemo(
    () => visibleCompareIndexes(plan, scrollLeft, scrollTop, activeEntityId),
    [activeEntityId, plan, scrollLeft, scrollTop],
  )

  useLayoutEffect(() => {
    const entityId = pendingFocusEntityId.current
    const viewport = viewportRef.current
    if (entityId === null || entityId !== activeEntityId || viewport === null) return
    const item = [...viewport.querySelectorAll<HTMLElement>('[data-compare-entity-id]')].find(
      (candidate) => candidate.dataset.compareEntityId === entityId,
    )
    pendingFocusEntityId.current = null
    item?.querySelector<HTMLElement>('article')?.focus()
  }, [activeEntityId])

  useLayoutEffect(() => {
    const node = viewportRef.current
    const previous = previousPlan.current
    if (node !== null && previous !== plan) {
      const nextOffset =
        previous === null
          ? scrollOffsetIncludingActive(plan, activeEntityId, 0)
          : anchoredCompareScrollOffset(
              previous,
              plan,
              activeEntityId,
              previous.scrollAxis === 'horizontal'
                ? scrollOffsets.current.left
                : scrollOffsets.current.top,
            )
      if (plan.scrollAxis === 'horizontal') node.scrollLeft = nextOffset
      else node.scrollTop = nextOffset
      const actualScrollLeft = node.scrollLeft
      const actualScrollTop = node.scrollTop
      scrollOffsets.current = { left: actualScrollLeft, top: actualScrollTop }
      setScrollLeft(actualScrollLeft)
      setScrollTop(actualScrollTop)
    }
    previousPlan.current = plan
  }, [activeEntityId, plan])

  useEffect(() => {
    const node = viewportRef.current
    if (node === null || plan.scrollAxis !== 'horizontal') return
    const nativeWheel = (event: WheelEvent) => {
      translateHorizontalWheel(
        node,
        event.deltaX,
        event.deltaY,
        () => event.preventDefault(),
        (nextScrollLeft) => {
          scrollOffsets.current.left = nextScrollLeft
          setScrollLeft(nextScrollLeft)
        },
      )
    }
    node.addEventListener('wheel', nativeWheel, { passive: false })
    return () => node.removeEventListener('wheel', nativeWheel)
  }, [plan.scrollAxis])

  function scrolled(event: UIEvent<HTMLDivElement>) {
    const actualScrollLeft = event.currentTarget.scrollLeft
    const actualScrollTop = event.currentTarget.scrollTop
    scrollOffsets.current = { left: actualScrollLeft, top: actualScrollTop }
    setScrollLeft(actualScrollLeft)
    setScrollTop(actualScrollTop)
  }

  const wheel: WheelEventHandler<HTMLDivElement> = (event) => {
    if (plan.scrollAxis !== 'horizontal' || event.nativeEvent.defaultPrevented) return
    translateHorizontalWheel(
      event.currentTarget,
      event.deltaX,
      event.deltaY,
      () => event.preventDefault(),
      (nextScrollLeft) => {
        scrollOffsets.current.left = nextScrollLeft
        setScrollLeft(nextScrollLeft)
      },
    )
  }

  const keyDown: KeyboardEventHandler<HTMLDivElement> = (event) => {
    const activeIndex = plan.rects.findIndex(({ entityId }) => entityId === activeEntityId)
    if (activeIndex < 0) return

    let nextIndex: number | undefined
    if (plan.scrollAxis === 'horizontal') {
      if (event.key === 'ArrowLeft') nextIndex = activeIndex - 1
      if (event.key === 'ArrowRight') nextIndex = activeIndex + 1
    } else if (plan.scrollAxis === 'vertical') {
      if (event.key === 'ArrowUp') nextIndex = activeIndex - plan.columns
      if (event.key === 'ArrowDown') nextIndex = activeIndex + plan.columns
    }
    const nextRect =
      nextIndex === undefined || nextIndex < 0 || nextIndex >= plan.rects.length
        ? undefined
        : plan.rects[nextIndex]
    if (nextRect === undefined) return

    event.preventDefault()
    pendingFocusEntityId.current = nextRect.entityId
    onActivate(nextRect.entityId)
  }

  return (
    <div
      ref={viewportRef}
      role="list"
      aria-label="滚动图片对比"
      className="compare-scroll-viewport"
      data-axis={plan.scrollAxis}
      onScroll={scrolled}
      onWheel={wheel}
      onKeyDown={keyDown}
    >
      <div
        className="compare-scroll-track"
        style={{ width: plan.totalWidth, height: plan.totalHeight }}
      >
        {mountedIndexes.map((index) => {
          const rect = plan.rects[index]
          if (rect === undefined) throw new Error(`Missing compare rect ${index}`)
          return (
            <div
              key={rect.entityId}
              role="listitem"
              aria-label={rect.entityId}
              aria-posinset={index + 1}
              aria-setsize={plan.rects.length}
              data-compare-entity-id={rect.entityId}
              className="compare-layout-item"
              style={{
                left: rect.left,
                top: rect.top,
                width: rect.width,
                height: rect.height,
              }}
              onFocusCapture={() => onActivate(rect.entityId)}
            >
              {renderItem(rect.entityId, index)}
            </div>
          )
        })}
      </div>
    </div>
  )
}

function scrollOffsetIncludingActive(
  plan: CompareLayoutPlan,
  activeEntityId: string,
  currentOffset: number,
) {
  const active = plan.rects.find(({ entityId }) => entityId === activeEntityId)
  if (active === undefined || plan.scrollAxis === 'none') return currentOffset
  const viewportExtent = plan.scrollAxis === 'horizontal' ? plan.viewportWidth : plan.viewportHeight
  const totalExtent = plan.scrollAxis === 'horizontal' ? plan.totalWidth : plan.totalHeight
  const activeStart = plan.scrollAxis === 'horizontal' ? active.left : active.top
  const activeExtent = plan.scrollAxis === 'horizontal' ? active.width : active.height
  const activeEnd = activeStart + activeExtent
  const maximumOffset = Math.max(0, totalExtent - viewportExtent)
  const desired =
    activeStart < currentOffset
      ? activeStart
      : activeEnd > currentOffset + viewportExtent
        ? activeEnd - viewportExtent
        : currentOffset
  return Math.max(0, Math.min(maximumOffset, desired))
}

function translateHorizontalWheel(
  node: HTMLElement,
  deltaX: number,
  deltaY: number,
  preventDefault: () => void,
  scrolled: (scrollLeft: number) => void,
) {
  const desired = deltaX !== 0 ? deltaX : deltaY
  const maximum = Math.max(0, node.scrollWidth - node.clientWidth)
  const next = Math.max(0, Math.min(maximum, node.scrollLeft + desired))
  if (next !== node.scrollLeft) {
    preventDefault()
    node.scrollLeft = next
    scrolled(next)
  }
}
