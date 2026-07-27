import type { KeyboardEventHandler, PointerEventHandler, ReactNode, UIEvent } from 'react'
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import {
  type AspectGeometry,
  type AspectRect,
  anchoredScrollOffset,
  buildFlowGeometry,
  directionalNeighbor,
  type ImageDimensions,
  intersectingAspectIndexes,
  verticalVisibleRows,
} from '../layout/aspectLayout'
import {
  type MarqueePoint,
  type MarqueeRect,
  type MarqueeSelectionChange,
  marqueeContentPoint,
  marqueeDistance,
  normalizeMarquee,
  verticalAutoScrollDelta,
} from './marqueeSelection'

export interface AspectVirtualGridProps<T> {
  items: readonly T[]
  imageHeight: number
  captionHeight?: number
  viewportHeight?: number
  gap?: number
  overscanRows?: number
  getKey: (item: T) => string
  getDimensions: (item: T) => ImageDimensions | null
  renderItem: (item: T, index: number, rect: AspectRect) => ReactNode
  ariaLabel: string
  activeKey?: string
  activeDescendant?: string
  onNavigate?: (index: number, extendSelection: boolean) => void
  onKeyDown?: KeyboardEventHandler<HTMLDivElement>
  ariaMultiselectable?: boolean
  onMarqueeSelectionChange?: (change: MarqueeSelectionChange) => void
}

interface MarqueeSession {
  pointerId: number
  start: MarqueePoint
  current: MarqueePoint
  lastClientX: number
  lastClientY: number
  metaKey: boolean
  activated: boolean
  keys: string[]
}

interface MarqueeLayout<T> {
  items: readonly T[]
  geometry: AspectGeometry
  getKey: (item: T) => string
  onChange?: (change: MarqueeSelectionChange) => void
}

const DIRECTIONS = {
  ArrowLeft: 'left',
  ArrowRight: 'right',
  ArrowUp: 'up',
  ArrowDown: 'down',
} as const

export default function AspectVirtualGrid<T>({
  items,
  imageHeight,
  captionHeight = 48,
  viewportHeight = 520,
  gap = 12,
  overscanRows = 2,
  getKey,
  getDimensions,
  renderItem,
  ariaLabel,
  activeKey,
  activeDescendant,
  onNavigate,
  onKeyDown,
  ariaMultiselectable,
  onMarqueeSelectionChange,
}: AspectVirtualGridProps<T>) {
  const container = useRef<HTMLDivElement>(null)
  const previousGeometry = useRef<AspectGeometry | null>(null)
  const marqueeSession = useRef<MarqueeSession | null>(null)
  const captureOwner = useRef<HTMLDivElement | null>(null)
  const animationFrame = useRef<number | null>(null)
  const marqueeLayout = useRef<MarqueeLayout<T> | null>(null)
  const [width, setWidth] = useState(0)
  const [scrollTop, setScrollTop] = useState(0)
  const [marqueeRect, setMarqueeRect] = useState<MarqueeRect | null>(null)

  useEffect(() => {
    const node = container.current
    if (node === null || typeof ResizeObserver === 'undefined') return
    const observer = new ResizeObserver((entries) => {
      const nextWidth = entries[0]?.contentRect.width
      if (nextWidth !== undefined && Number.isFinite(nextWidth) && nextWidth > 0) {
        setWidth(nextWidth)
      }
    })
    observer.observe(node)
    return () => observer.disconnect()
  }, [])

  const geometry = useMemo(
    () =>
      buildFlowGeometry(
        items.map((item) => ({ key: getKey(item), dimensions: getDimensions(item) })),
        width,
        imageHeight,
        captionHeight,
        gap,
      ),
    [captionHeight, gap, getDimensions, getKey, imageHeight, items, width],
  )

  useLayoutEffect(() => {
    const node = container.current
    const previous = previousGeometry.current
    if (
      node !== null &&
      previous !== null &&
      previous !== geometry &&
      previous.items.length > 0 &&
      geometry.items.length > 0
    ) {
      const visibleRows = verticalVisibleRows(previous, node.scrollTop, viewportHeight, 0)
      const anchorRow = previous.rows[visibleRows.start]
      const anchor = anchorRow === undefined ? undefined : previous.items[anchorRow.start]
      if (anchor !== undefined) {
        const nextScrollTop = anchoredScrollOffset(
          previous,
          geometry,
          anchor.key,
          node.scrollTop,
          'vertical',
        )
        node.scrollTop = nextScrollTop
        setScrollTop(nextScrollTop)
      }
    }
    previousGeometry.current = geometry
  }, [geometry, viewportHeight])

  const visibleRows = useMemo(
    () => verticalVisibleRows(geometry, scrollTop, viewportHeight, overscanRows),
    [geometry, overscanRows, scrollTop, viewportHeight],
  )
  const mountedIndexes = useMemo(
    () => indexesForRows(geometry, visibleRows, activeKey),
    [activeKey, geometry, visibleRows],
  )

  function cancelAutoScroll() {
    if (animationFrame.current !== null) {
      cancelAnimationFrame(animationFrame.current)
      animationFrame.current = null
    }
  }

  function clearMarqueeSession() {
    const session = marqueeSession.current
    const node = container.current ?? captureOwner.current
    marqueeSession.current = null
    captureOwner.current = null
    cancelAutoScroll()
    setMarqueeRect(null)
    if (session !== null && node?.releasePointerCapture !== undefined) {
      try {
        node.releasePointerCapture(session.pointerId)
      } catch {
        // Pointer capture may already be revoked. Session cleanup must still finish.
      }
    }
  }

  useLayoutEffect(() => {
    marqueeLayout.current = {
      items,
      geometry,
      getKey,
      onChange: onMarqueeSelectionChange,
    }
  })

  useLayoutEffect(() => () => clearMarqueeSession(), [items])

  function contentPoint(clientX: number, clientY: number): MarqueePoint {
    const node = container.current
    if (node === null) throw new Error('Aspect virtual grid pointer event requires a container')
    return marqueeContentPoint(
      clientX,
      clientY,
      node.getBoundingClientRect(),
      node.scrollLeft,
      node.scrollTop,
    )
  }

  function updateMarquee(session: MarqueeSession) {
    const layout = marqueeLayout.current
    if (layout === null) return
    const rect = normalizeMarquee(session.start, session.current)
    const indexes = intersectingAspectIndexes(layout.geometry, rect)
    session.keys = indexes.map((index) => {
      const item = layout.items[index]
      if (item === undefined) {
        throw new Error(`Aspect grid intersection returned missing item index ${index}`)
      }
      return layout.getKey(item)
    })
    setMarqueeRect(rect)
    layout.onChange?.({ phase: 'change', keys: session.keys, metaKey: session.metaKey })
  }

  function queueAutoScroll() {
    if (animationFrame.current !== null) return
    const node = container.current
    const session = marqueeSession.current
    if (node === null || session === null || !session.activated) return
    const bounds = node.getBoundingClientRect()
    if (verticalAutoScrollDelta(session.lastClientY, bounds.top, bounds.bottom) === 0) return

    animationFrame.current = requestAnimationFrame(() => {
      animationFrame.current = null
      const activeSession = marqueeSession.current
      const activeNode = container.current
      if (activeSession === null || activeNode === null || !activeSession.activated) return

      const activeBounds = activeNode.getBoundingClientRect()
      const delta = verticalAutoScrollDelta(
        activeSession.lastClientY,
        activeBounds.top,
        activeBounds.bottom,
      )
      const nextScrollTop = Math.max(
        0,
        Math.min(activeNode.scrollHeight - activeNode.clientHeight, activeNode.scrollTop + delta),
      )
      const didScroll = nextScrollTop !== activeNode.scrollTop
      if (didScroll) {
        activeNode.scrollTop = nextScrollTop
        setScrollTop(nextScrollTop)
        activeSession.current = contentPoint(activeSession.lastClientX, activeSession.lastClientY)
        updateMarquee(activeSession)
      }
      if (didScroll && delta !== 0) queueAutoScroll()
    })
  }

  const pointerDown: PointerEventHandler<HTMLDivElement> = (event) => {
    const target = event.target
    if (
      event.button !== 0 ||
      onMarqueeSelectionChange === undefined ||
      (target instanceof Element && target.closest('[data-virtual-grid-item]') !== null)
    ) {
      return
    }

    const node = container.current
    if (node === null) return
    node.focus()
    const point = contentPoint(event.clientX, event.clientY)
    try {
      node.setPointerCapture?.(event.pointerId)
    } catch {
      return
    }
    marqueeSession.current = {
      pointerId: event.pointerId,
      start: point,
      current: point,
      lastClientX: event.clientX,
      lastClientY: event.clientY,
      metaKey: event.metaKey,
      activated: false,
      keys: [],
    }
    captureOwner.current = node
    onMarqueeSelectionChange({ phase: 'start', keys: [], metaKey: event.metaKey })
    event.preventDefault()
  }

  const pointerMove: PointerEventHandler<HTMLDivElement> = (event) => {
    const session = marqueeSession.current
    if (session === null || session.pointerId !== event.pointerId) return
    session.lastClientX = event.clientX
    session.lastClientY = event.clientY
    session.current = contentPoint(event.clientX, event.clientY)
    if (!session.activated && marqueeDistance(session.start, session.current) < 4) return
    session.activated = true
    updateMarquee(session)
    const bounds = container.current?.getBoundingClientRect()
    if (
      bounds === undefined ||
      verticalAutoScrollDelta(event.clientY, bounds.top, bounds.bottom) === 0
    ) {
      cancelAutoScroll()
    } else {
      queueAutoScroll()
    }
    event.preventDefault()
  }

  const pointerUp: PointerEventHandler<HTMLDivElement> = (event) => {
    const session = marqueeSession.current
    if (session === null || session.pointerId !== event.pointerId) return
    onMarqueeSelectionChange?.({ phase: 'end', keys: session.keys, metaKey: session.metaKey })
    clearMarqueeSession()
  }

  const pointerCancel: PointerEventHandler<HTMLDivElement> = (event) => {
    const session = marqueeSession.current
    if (session === null || session.pointerId !== event.pointerId) return
    onMarqueeSelectionChange?.({ phase: 'cancel', keys: [], metaKey: session.metaKey })
    clearMarqueeSession()
  }

  const keyDown: KeyboardEventHandler<HTMLDivElement> = (event) => {
    if (event.key === 'Escape' && marqueeSession.current !== null) {
      const session = marqueeSession.current
      onMarqueeSelectionChange?.({ phase: 'cancel', keys: [], metaKey: session.metaKey })
      clearMarqueeSession()
    }

    const direction = DIRECTIONS[event.key as keyof typeof DIRECTIONS]
    const activeIndex = activeKey === undefined ? undefined : geometry.indexByKey.get(activeKey)
    if (direction !== undefined && activeIndex !== undefined && onNavigate !== undefined) {
      event.preventDefault()
      onNavigate(directionalNeighbor(geometry, activeIndex, direction), event.shiftKey)
    }
    onKeyDown?.(event)
  }

  function scrolled(event: UIEvent<HTMLDivElement>) {
    setScrollTop(event.currentTarget.scrollTop)
  }

  return (
    <div
      ref={container}
      role="listbox"
      aria-label={ariaLabel}
      aria-activedescendant={activeDescendant}
      aria-multiselectable={ariaMultiselectable || undefined}
      tabIndex={0}
      className="virtual-grid aspect-virtual-grid"
      data-marquee-active={marqueeRect ? 'true' : undefined}
      style={{ height: viewportHeight, overflow: 'auto', position: 'relative' }}
      onKeyDown={keyDown}
      onPointerDown={pointerDown}
      onPointerMove={pointerMove}
      onPointerUp={pointerUp}
      onPointerCancel={pointerCancel}
      onScroll={scrolled}
    >
      <div
        data-testid="aspect-virtual-grid-track"
        style={{
          width: geometry.totalWidth,
          height: geometry.totalHeight,
          position: 'relative',
        }}
      >
        {mountedIndexes.map((index) => {
          const item = items[index]
          const rect = geometry.items[index]
          if (item === undefined || rect === undefined) {
            throw new Error(`Missing aspect grid item at mounted index ${index}`)
          }
          return (
            <div
              key={rect.key}
              data-key={rect.key}
              data-virtual-grid-item
              style={{
                height: rect.height,
                left: rect.left,
                position: 'absolute',
                top: rect.top,
                width: rect.width,
              }}
            >
              {renderItem(item, index, rect)}
            </div>
          )
        })}
        {marqueeRect && (
          <div
            aria-hidden="true"
            className="virtual-grid-marquee"
            data-testid="marquee-selection"
            style={{
              left: marqueeRect.left,
              top: marqueeRect.top,
              width: marqueeRect.width,
              height: marqueeRect.height,
            }}
          />
        )}
      </div>
    </div>
  )
}

function indexesForRows(
  geometry: AspectGeometry,
  visibleRows: { start: number; end: number },
  activeKey: string | undefined,
): number[] {
  const firstRow = geometry.rows[visibleRows.start]
  const lastRow = geometry.rows[visibleRows.end - 1]
  const start = firstRow?.start ?? 0
  const end = lastRow?.end ?? 0
  const indexes = Array.from({ length: Math.max(0, end - start) }, (_, offset) => start + offset)
  const activeIndex = activeKey === undefined ? undefined : geometry.indexByKey.get(activeKey)
  if (activeIndex !== undefined && (activeIndex < start || activeIndex >= end)) {
    indexes.push(activeIndex)
    indexes.sort((left, right) => left - right)
  }
  return indexes
}
