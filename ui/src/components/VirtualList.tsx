import type { ReactNode, UIEvent } from 'react'
import { useEffect, useMemo, useRef, useState } from 'react'

interface VirtualListProps<T> {
  items: readonly T[]
  rowHeight: number
  height?: number
  overscan?: number
  getKey: (item: T) => string
  renderItem: (item: T, index: number) => ReactNode
  className?: string
  scrollToIndex?: number
  viewportProps?: {
    'data-organization-drop-surface'?: string
  }
}

export default function VirtualList<T>({
  items,
  rowHeight,
  height,
  overscan = 6,
  getKey,
  renderItem,
  className,
  scrollToIndex,
  viewportProps,
}: VirtualListProps<T>) {
  const initialScrollTop = scrollOffset(scrollToIndex, items.length, rowHeight)
  const [scrollTop, setScrollTop] = useState(initialScrollTop)
  const [measuredHeight, setMeasuredHeight] = useState(height ?? 0)
  const viewportRef = useRef<HTMLDivElement>(null)
  const viewportHeight = height ?? measuredHeight

  useEffect(() => {
    if (height !== undefined) return
    const node = viewportRef.current
    if (node === null || typeof ResizeObserver === 'undefined') return

    const updateHeight = (nextHeight: number) => {
      if (Number.isFinite(nextHeight) && nextHeight > 0) {
        setMeasuredHeight(Math.floor(nextHeight))
      }
    }
    updateHeight(node.getBoundingClientRect().height)
    const observer = new ResizeObserver((entries) => {
      const nextHeight = entries[0]?.contentRect.height
      if (nextHeight !== undefined) updateHeight(nextHeight)
    })
    observer.observe(node)
    return () => observer.disconnect()
  }, [height])

  useEffect(() => {
    if (scrollToIndex === undefined) return
    const next = scrollOffset(scrollToIndex, items.length, rowHeight)
    setScrollTop(next)
    if (viewportRef.current) viewportRef.current.scrollTop = next
  }, [items.length, rowHeight, scrollToIndex])
  const window = useMemo(() => {
    const firstVisible = Math.floor(scrollTop / rowHeight)
    const visibleCount = Math.ceil(viewportHeight / rowHeight)
    const start = Math.max(0, firstVisible - overscan)
    const end = Math.min(items.length, firstVisible + visibleCount + overscan)
    return { start, end }
  }, [items.length, overscan, rowHeight, scrollTop, viewportHeight])

  function scrolled(event: UIEvent<HTMLDivElement>) {
    setScrollTop(event.currentTarget.scrollTop)
  }

  return (
    <div
      {...viewportProps}
      ref={viewportRef}
      className={className}
      style={{ height: height ?? '100%', overflowY: 'auto', position: 'relative' }}
      onScroll={scrolled}
    >
      <div style={{ height: items.length * rowHeight, position: 'relative' }}>
        {items.slice(window.start, window.end).map((item, offset) => {
          const index = window.start + offset
          return (
            <div
              key={getKey(item)}
              style={{
                height: rowHeight,
                left: 0,
                position: 'absolute',
                right: 0,
                top: index * rowHeight,
              }}
            >
              {renderItem(item, index)}
            </div>
          )
        })}
      </div>
    </div>
  )
}

function scrollOffset(index: number | undefined, itemCount: number, rowHeight: number): number {
  if (index === undefined || itemCount === 0) return 0
  return Math.max(0, Math.min(itemCount - 1, index)) * rowHeight
}
