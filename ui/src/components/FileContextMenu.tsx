import type { KeyboardEvent } from 'react'
import { useEffect, useMemo, useRef, useState } from 'react'
import type { Point, Viewport } from './radialMenuGeometry'
import type { RadialLeafAction, RadialMenuItem } from './radialMenuModel'

export interface FileContextMenuProps {
  origin: Point
  selectionCount: number
  readOnly?: boolean
  returnFocusTarget?: HTMLElement | null
  model: RadialMenuItem[]
  viewport?: Viewport
  onAction(action: RadialLeafAction): void
  onClose(returnFocusTarget: HTMLElement | null): void
}

export default function FileContextMenu({
  origin,
  selectionCount,
  readOnly = false,
  returnFocusTarget = null,
  model,
  viewport = { width: window.innerWidth, height: window.innerHeight },
  onAction,
  onClose,
}: FileContextMenuProps) {
  const rootRef = useRef<HTMLDivElement>(null)
  const secondaryFocusRequested = useRef<number | null>(null)
  const [primaryIndex, setPrimaryIndex] = useState(firstEnabledIndex(model))
  const [expandedIndex, setExpandedIndex] = useState<number | null>(null)
  const [secondaryIndex, setSecondaryIndex] = useState(0)
  const expandedItem = expandedIndex === null ? null : (model[expandedIndex] ?? null)
  const left = Math.min(Math.max(8, origin.x), Math.max(8, viewport.width - 260))
  const top = Math.min(Math.max(8, origin.y), Math.max(8, viewport.height - 360))

  const primaryItems = useMemo(() => model, [model])
  const secondaryItems = expandedItem?.children ?? []

  function requestClose() {
    onClose(returnFocusTarget)
  }

  useEffect(() => {
    rootRef.current
      ?.querySelector<HTMLButtonElement>('[data-menu-level="primary"][tabindex="0"]')
      ?.focus()
  }, [])

  useEffect(() => {
    const outside = (event: PointerEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) requestClose()
    }
    window.addEventListener('pointerdown', outside)
    return () => window.removeEventListener('pointerdown', outside)
  })

  useEffect(() => {
    if (secondaryFocusRequested.current === null || expandedIndex === null) return
    const index = secondaryFocusRequested.current
    secondaryFocusRequested.current = null
    focusItem('secondary', index)
  }, [expandedIndex])

  function focusItem(level: 'primary' | 'secondary', index: number) {
    rootRef.current
      ?.querySelector<HTMLButtonElement>(`[data-menu-level="${level}"][data-menu-index="${index}"]`)
      ?.focus()
  }

  function openChildren(index: number, focusChild: boolean) {
    const item = model[index]
    if (item === undefined || item.disabled || item.children === undefined) return
    const childIndex = firstEnabledIndex(item.children)
    setExpandedIndex(index)
    setSecondaryIndex(childIndex)
    if (focusChild) secondaryFocusRequested.current = childIndex
  }

  function activate(item: RadialMenuItem, index: number) {
    if (item.disabled) return
    if (item.children !== undefined) {
      openChildren(index, true)
      return
    }
    onAction(item.id as RadialLeafAction)
  }

  function handleMenuKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key === 'Escape') {
      event.preventDefault()
      requestClose()
      return
    }
    const activeSecondary = document.activeElement?.getAttribute('data-menu-level') === 'secondary'
    const level = activeSecondary ? 'secondary' : 'primary'
    const items = level === 'primary' ? primaryItems : secondaryItems
    const current = level === 'primary' ? primaryIndex : secondaryIndex
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault()
      const next = nextEnabled(items, current, event.key === 'ArrowDown' ? 1 : -1)
      if (level === 'primary') setPrimaryIndex(next)
      else setSecondaryIndex(next)
      focusItem(level, next)
      return
    }
    if (event.key === 'ArrowRight' && level === 'primary') {
      event.preventDefault()
      openChildren(primaryIndex, true)
      return
    }
    if (event.key === 'ArrowLeft' && level === 'secondary') {
      event.preventDefault()
      setExpandedIndex(null)
      focusItem('primary', primaryIndex)
      return
    }
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault()
      const item = items[current]
      if (item !== undefined) activate(item, level === 'primary' ? current : (expandedIndex ?? 0))
    }
  }

  function renderItem(item: RadialMenuItem, index: number, level: 'primary' | 'secondary') {
    const checkbox = item.checked !== undefined
    const children = item.children !== undefined
    const isExpanded = level === 'primary' && expandedIndex === index
    const disabledReasonId = item.disabledReason
      ? `context-menu-reason-${level}-${index}`
      : undefined
    const isCurrent = level === 'primary' ? primaryIndex === index : secondaryIndex === index
    return (
      <div className="file-context-menu-entry" key={item.id}>
        <button
          type="button"
          role={checkbox ? 'menuitemcheckbox' : 'menuitem'}
          className="file-context-menu-item"
          data-menu-level={level}
          data-menu-index={index}
          data-tone={item.tone}
          tabIndex={isCurrent ? 0 : -1}
          aria-checked={checkbox ? item.checked : undefined}
          aria-disabled={item.disabled}
          aria-haspopup={children ? 'menu' : undefined}
          aria-expanded={children ? isExpanded : undefined}
          aria-describedby={disabledReasonId}
          title={item.disabledReason}
          onFocus={() => {
            if (level === 'primary') {
              setPrimaryIndex(index)
              setExpandedIndex(null)
            } else {
              setSecondaryIndex(index)
            }
          }}
          onClick={() => activate(item, level === 'primary' ? index : (expandedIndex ?? 0))}
        >
          <span aria-hidden="true">{item.symbol}</span>
          <span>{item.label}</span>
          {children && (
            <span className="file-context-menu-disclosure" aria-hidden="true">
              ›
            </span>
          )}
        </button>
        {item.disabledReason && (
          <small id={disabledReasonId} className="file-context-menu-reason">
            {item.disabledReason}
          </small>
        )}
      </div>
    )
  }

  return (
    <div
      ref={rootRef}
      className="file-context-menu"
      style={{ left, top }}
      onKeyDown={handleMenuKeyDown}
    >
      <div role="menu" aria-label="文件操作">
        {primaryItems.map((item, index) => renderItem(item, index, 'primary'))}
      </div>
      {expandedItem?.children && (
        <div className="file-context-submenu" role="menu" aria-label={expandedItem.label}>
          {expandedItem.children.map((item, index) => renderItem(item, index, 'secondary'))}
        </div>
      )}
      <p className="file-context-summary">
        {selectionCount} 个文件{readOnly ? ' · 只读' : ''}
      </p>
    </div>
  )
}

function firstEnabledIndex(items: RadialMenuItem[]): number {
  const first = items.findIndex((item) => !item.disabled)
  return first === -1 ? 0 : first
}

function nextEnabled(items: RadialMenuItem[], current: number, direction: 1 | -1): number {
  for (let offset = 1; offset <= items.length; offset += 1) {
    const index = (current + direction * offset + items.length) % items.length
    if (!items[index]?.disabled) return index
  }
  return current
}
