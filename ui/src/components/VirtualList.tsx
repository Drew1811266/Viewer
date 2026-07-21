import { useEffect, useMemo, useRef, useState } from 'react'
import type { ReactNode, UIEvent } from 'react'

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
  height = 420,
  overscan = 6,
  getKey,
  renderItem,
  className,
  scrollToIndex,
  viewportProps,
}: VirtualListProps<T>) {
  const initialScrollTop = scrollOffset(scrollToIndex, items.length, rowHeight)
  const [scrollTop, setScrollTop] = useState(initialScrollTop)
  const viewportRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (scrollToIndex === undefined) return
    const next = scrollOffset(scrollToIndex, items.length, rowHeight)
    setScrollTop(next)
    if (viewportRef.current) viewportRef.current.scrollTop = next
  }, [items.length, rowHeight, scrollToIndex])
  const window = useMemo(() => {
    const firstVisible = Math.floor(scrollTop / rowHeight)
    const visibleCount = Math.ceil(height / rowHeight)
    const start = Math.max(0, firstVisible - overscan)
    const end = Math.min(items.length, firstVisible + visibleCount + overscan)
    return { start, end }
  }, [height, items.length, overscan, rowHeight, scrollTop])

  function scrolled(event: UIEvent<HTMLDivElement>) {
    setScrollTop(event.currentTarget.scrollTop)
  }

  return (
    <div
      {...viewportProps}
      ref={viewportRef}
      className={className}
      style={{ height, overflowY: 'auto', position: 'relative' }}
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
