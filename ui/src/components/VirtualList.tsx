import { useMemo, useState } from 'react'
import type { ReactNode, UIEvent } from 'react'

interface VirtualListProps<T> {
  items: readonly T[]
  rowHeight: number
  height?: number
  overscan?: number
  getKey: (item: T) => string
  renderItem: (item: T, index: number) => ReactNode
  className?: string
}

export default function VirtualList<T>({
  items,
  rowHeight,
  height = 420,
  overscan = 6,
  getKey,
  renderItem,
  className,
}: VirtualListProps<T>) {
  const [scrollTop, setScrollTop] = useState(0)
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
