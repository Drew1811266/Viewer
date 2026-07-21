import { useEffect, useMemo, useRef, useState } from 'react'
import type { KeyboardEventHandler, PointerEventHandler, ReactNode, UIEvent } from 'react'
import {
  intersectingGridIndexes,
  marqueeDistance,
  normalizeMarquee,
  verticalAutoScrollDelta,
  type MarqueePoint,
  type MarqueeRect,
} from './marqueeSelection'

export type MarqueePhase = 'start' | 'change' | 'end' | 'cancel'

export interface MarqueeSelectionChange {
  phase: MarqueePhase
  keys: string[]
  metaKey: boolean
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

interface VirtualGridProps<T> {
  items: readonly T[]
  cellWidth: number
  cellHeight: number
  viewportHeight?: number
  gap?: number
  overscanRows?: number
  getKey: (item: T) => string
  renderItem: (item: T, index: number) => ReactNode
  ariaLabel: string
  activeDescendant?: string
  onKeyDown?: KeyboardEventHandler<HTMLDivElement>
  ariaMultiselectable?: boolean
  onMarqueeSelectionChange?: (change: MarqueeSelectionChange) => void
}

export default function VirtualGrid<T>({
  items,
  cellWidth,
  cellHeight,
  viewportHeight = 520,
  gap = 12,
  overscanRows = 2,
  getKey,
  renderItem,
  ariaLabel,
  activeDescendant,
  onKeyDown,
  ariaMultiselectable,
  onMarqueeSelectionChange,
}: VirtualGridProps<T>) {
  const container = useRef<HTMLDivElement>(null)
  const marqueeSession = useRef<MarqueeSession | null>(null)
  const animationFrame = useRef<number | null>(null)
  const [width, setWidth] = useState(900)
  const [scrollTop, setScrollTop] = useState(0)
  const [marqueeRect, setMarqueeRect] = useState<MarqueeRect | null>(null)

  useEffect(() => {
    if (container.current === null || typeof ResizeObserver === 'undefined') return
    const observer = new ResizeObserver((entries) => {
      const nextWidth = entries[0]?.contentRect.width
      if (nextWidth && nextWidth > 0) setWidth(nextWidth)
    })
    observer.observe(container.current)
    return () => observer.disconnect()
  }, [])

  const columns = Math.max(1, Math.floor((width + gap) / (cellWidth + gap)))
  const rowStride = cellHeight + gap
  const rowCount = Math.ceil(items.length / columns)
  const window = useMemo(() => {
    const firstVisible = Math.floor(scrollTop / rowStride)
    const visibleRows = Math.ceil(viewportHeight / rowStride)
    const startRow = Math.max(0, firstVisible - overscanRows)
    const endRow = Math.min(rowCount, firstVisible + visibleRows + overscanRows)
    return {
      start: startRow * columns,
      end: Math.min(items.length, endRow * columns),
    }
  }, [columns, items.length, overscanRows, rowCount, rowStride, scrollTop, viewportHeight])

  function cancelAutoScroll() {
    if (animationFrame.current !== null) {
      cancelAnimationFrame(animationFrame.current)
      animationFrame.current = null
    }
  }

  function clearMarqueeSession() {
    const session = marqueeSession.current
    const node = container.current
    if (session !== null && node?.releasePointerCapture !== undefined) {
      node.releasePointerCapture(session.pointerId)
    }
    cancelAutoScroll()
    marqueeSession.current = null
    setMarqueeRect(null)
  }

  useEffect(() => () => clearMarqueeSession(), [items])

  function contentPoint(clientX: number, clientY: number): MarqueePoint {
    const node = container.current!
    const bounds = node.getBoundingClientRect()
    return {
      x: clientX - bounds.left + node.scrollLeft,
      y: clientY - bounds.top + node.scrollTop,
    }
  }

  function updateMarquee(session: MarqueeSession) {
    const rect = normalizeMarquee(session.start, session.current)
    const indexes = intersectingGridIndexes(rect, {
      itemCount: items.length,
      columns,
      cellWidth,
      cellHeight,
      columnStride: cellWidth + gap,
      rowStride,
    })
    session.keys = indexes.map((index) => getKey(items[index]!))
    setMarqueeRect(rect)
    onMarqueeSelectionChange?.({ phase: 'change', keys: session.keys, metaKey: session.metaKey })
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
    ) return

    const node = container.current
    if (node === null) return
    node.focus()
    const point = contentPoint(event.clientX, event.clientY)
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
    node.setPointerCapture?.(event.pointerId)
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
    if (bounds === undefined || verticalAutoScrollDelta(event.clientY, bounds.top, bounds.bottom) === 0) {
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
      className="virtual-grid"
      data-marquee-active={marqueeRect ? 'true' : undefined}
      style={{ height: viewportHeight, overflowY: 'auto', position: 'relative' }}
      onKeyDown={keyDown}
      onPointerDown={pointerDown}
      onPointerMove={pointerMove}
      onPointerUp={pointerUp}
      onPointerCancel={pointerCancel}
      onScroll={scrolled}
    >
      <div style={{ height: rowCount * rowStride, position: 'relative' }}>
        {items.slice(window.start, window.end).map((item, offset) => {
          const index = window.start + offset
          const row = Math.floor(index / columns)
          const column = index % columns
          return (
            <div
              key={getKey(item)}
              data-virtual-grid-item
              style={{
                height: cellHeight,
                left: column * (cellWidth + gap),
                position: 'absolute',
                top: row * rowStride,
                width: cellWidth,
              }}
            >
              {renderItem(item, index)}
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
