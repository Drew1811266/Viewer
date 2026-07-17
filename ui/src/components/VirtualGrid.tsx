import { useEffect, useMemo, useRef, useState } from 'react'
import type { KeyboardEventHandler, ReactNode, UIEvent } from 'react'

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
}: VirtualGridProps<T>) {
  const container = useRef<HTMLDivElement>(null)
  const [width, setWidth] = useState(900)
  const [scrollTop, setScrollTop] = useState(0)

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

  function scrolled(event: UIEvent<HTMLDivElement>) {
    setScrollTop(event.currentTarget.scrollTop)
  }

  return (
    <div
      ref={container}
      role="listbox"
      aria-label={ariaLabel}
      aria-activedescendant={activeDescendant}
      tabIndex={0}
      className="virtual-grid"
      style={{ height: viewportHeight, overflowY: 'auto', position: 'relative' }}
      onKeyDown={onKeyDown}
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
      </div>
    </div>
  )
}
