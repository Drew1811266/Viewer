import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import type { CSSProperties, KeyboardEvent } from 'react'
import type { BrowserFile } from '../api/types'
import {
  MOTION_THRESHOLD,
  PRIMARY_INNER_RADIUS,
  PRIMARY_OUTER_RADIUS,
  SECONDARY_INNER_RADIUS,
  SECONDARY_OUTER_RADIUS,
  annularSectorPath,
  fitMenuOrigin,
  polarPoint,
  primaryCenterAngle,
  primaryIndexAt,
  secondaryIndexAt,
} from './radialMenuGeometry'
import type { Point, Viewport } from './radialMenuGeometry'
import type { RadialLeafAction, RadialMenuItem } from './radialMenuModel'

export interface RadialMenuRequest {
  files: BrowserFile[]
  origin: Point
  pointerId: number | null
  returnFocusTarget: HTMLElement
}

interface RadialFileMenuProps {
  origin: Point
  pointerId: number | null
  selectionCount: number
  readOnly?: boolean
  returnFocusTarget?: HTMLElement | null
  model: RadialMenuItem[]
  viewport?: Viewport
  onAction: (action: RadialLeafAction) => void
  onClose: (returnFocusTarget: HTMLElement | null) => void
}

export default function RadialFileMenu({
  origin,
  pointerId,
  selectionCount,
  readOnly = false,
  returnFocusTarget = null,
  model,
  viewport = { width: window.innerWidth, height: window.innerHeight },
  onAction,
  onClose,
}: RadialFileMenuProps) {
  const fittedOrigin = useMemo(() => fitMenuOrigin(origin, viewport), [origin, viewport])
  const firstEnabledIndex = Math.max(
    0,
    model.findIndex((item) => !item.disabled),
  )
  const [primaryIndex, setPrimaryIndex] = useState(firstEnabledIndex)
  const [pointerPrimaryIndex, setPointerPrimaryIndex] = useState<number | null>(null)
  const [expandedIndex, setExpandedIndex] = useState<number | null>(null)
  const [secondaryIndex, setSecondaryIndex] = useState<number | null>(null)
  const [clickMode, setClickMode] = useState(pointerId === null)
  const expandTimer = useRef<number | null>(null)
  const closeTimer = useRef<number | null>(null)
  const secondaryFocusRequested = useRef<number | null>(null)
  const rootRef = useRef<HTMLDivElement>(null)
  const startPoint = useRef(origin)
  const maximumTravelled = useRef(0)
  const gestureCancelled = useRef(false)
  const expandedItem = expandedIndex === null ? null : (model[expandedIndex] ?? null)
  const secondaryAnchor =
    expandedIndex === null ? 0 : primaryCenterAngle(expandedIndex)
  const displayedPrimaryIndex = clickMode ? primaryIndex : pointerPrimaryIndex
  const requestClose = useCallback(
    () => onClose(returnFocusTarget),
    [onClose, returnFocusTarget],
  )

  useEffect(() => {
    rootRef.current
      ?.querySelector<HTMLButtonElement>('[data-level="primary"]:not([aria-disabled="true"])')
      ?.focus()
    return () => {
      if (expandTimer.current !== null) window.clearTimeout(expandTimer.current)
      if (closeTimer.current !== null) window.clearTimeout(closeTimer.current)
    }
  }, [])

  useEffect(() => {
    if (pointerId === null || clickMode) return
    const move = (event: PointerEvent) => {
      if (event.pointerId !== pointerId || gestureCancelled.current) return
      const point = { x: event.clientX, y: event.clientY }
      maximumTravelled.current = Math.max(
        maximumTravelled.current,
        Math.hypot(point.x - startPoint.current.x, point.y - startPoint.current.y),
      )
      const child =
        expandedItem?.children === undefined
          ? null
          : secondaryIndexAt(
              point,
              fittedOrigin,
              secondaryAnchor,
              expandedItem.children.length,
            )
      if (child !== null) {
        setPointerPrimaryIndex(expandedIndex)
        setSecondaryIndex(child)
        return
      }
      setSecondaryIndex(null)
      const nextPrimary = primaryIndexAt(point, fittedOrigin)
      setPointerPrimaryIndex(nextPrimary)
      if (nextPrimary === null) return
      setPrimaryIndex(nextPrimary)
      scheduleExpansion(nextPrimary)
    }
    const up = (event: PointerEvent) => {
      if (event.pointerId !== pointerId || gestureCancelled.current) return
      maximumTravelled.current = Math.max(
        maximumTravelled.current,
        Math.hypot(
          event.clientX - startPoint.current.x,
          event.clientY - startPoint.current.y,
        ),
      )
      if (maximumTravelled.current < MOTION_THRESHOLD) {
        setClickMode(true)
        return
      }
      const child =
        secondaryIndex === null ? null : (expandedItem?.children?.[secondaryIndex] ?? null)
      if (child !== null && !child.disabled && isLeaf(child)) {
        onAction(child.id)
        return
      }
      const primary = pointerPrimaryIndex === null ? undefined : model[pointerPrimaryIndex]
      if (primary !== undefined && !primary.disabled && isLeaf(primary)) {
        onAction(primary.id)
        return
      }
      requestClose()
    }
    const cancel = (event: PointerEvent) => {
      if (event.pointerId !== pointerId || gestureCancelled.current) return
      gestureCancelled.current = true
      if (expandTimer.current !== null) {
        window.clearTimeout(expandTimer.current)
        expandTimer.current = null
      }
      requestClose()
    }
    window.addEventListener('pointermove', move)
    window.addEventListener('pointerup', up)
    window.addEventListener('pointercancel', cancel)
    return () => {
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerup', up)
      window.removeEventListener('pointercancel', cancel)
    }
  }, [
    clickMode,
    expandedItem,
    fittedOrigin,
    model,
    onAction,
    pointerId,
    pointerPrimaryIndex,
    primaryIndex,
    requestClose,
    secondaryAnchor,
    secondaryIndex,
  ])

  useEffect(() => {
    if (!clickMode) return
    const outside = (event: PointerEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) requestClose()
    }
    window.addEventListener('pointerdown', outside)
    return () => window.removeEventListener('pointerdown', outside)
  }, [clickMode, requestClose])

  useEffect(() => {
    if (secondaryFocusRequested.current === null || expandedIndex === null) return
    const index = secondaryFocusRequested.current
    secondaryFocusRequested.current = null
    focusItem('secondary', index)
  }, [expandedIndex])

  function scheduleExpansion(index: number) {
    if (expandTimer.current !== null) window.clearTimeout(expandTimer.current)
    const item = model[index]
    if (item?.disabled || item?.children === undefined) {
      setExpandedIndex(null)
      return
    }
    expandTimer.current = window.setTimeout(() => {
      expandTimer.current = null
      setExpandedIndex(index)
      setSecondaryIndex(null)
    }, 120)
  }

  function expandImmediately(index: number) {
    const item = model[index]
    if (item?.disabled || item?.children === undefined) return
    const firstEnabledChild = item.children.findIndex((child) => !child.disabled)
    if (firstEnabledChild === -1) return
    secondaryFocusRequested.current = firstEnabledChild
    setExpandedIndex(index)
    setSecondaryIndex(firstEnabledChild)
    if (expandedIndex === index) {
      secondaryFocusRequested.current = null
      focusItem('secondary', firstEnabledChild)
    }
  }

  function execute(item: RadialMenuItem, index: number) {
    if (item.disabled) return
    if (item.children !== undefined) {
      expandImmediately(index)
      return
    }
    onAction(item.id as RadialLeafAction)
  }

  function handleKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key === 'Escape') {
      event.preventDefault()
      requestClose()
      return
    }
    const level =
      expandedIndex !== null && document.activeElement?.getAttribute('data-level') === 'secondary'
        ? 'secondary'
        : 'primary'
    const items = level === 'primary' ? model : (expandedItem?.children ?? [])
    const current = level === 'primary' ? primaryIndex : (secondaryIndex ?? 0)
    if (event.key === 'ArrowRight' || event.key === 'ArrowLeft') {
      event.preventDefault()
      const delta = event.key === 'ArrowRight' ? 1 : -1
      const next = nextEnabled(items, current, delta)
      if (level === 'primary') setPrimaryIndex(next)
      else setSecondaryIndex(next)
      focusItem(level, next)
      return
    }
    if (event.key === 'ArrowUp' && level === 'primary') {
      event.preventDefault()
      expandImmediately(primaryIndex)
      return
    }
    if (event.key === 'ArrowDown' && level === 'secondary') {
      event.preventDefault()
      setSecondaryIndex(null)
      focusItem('primary', expandedIndex ?? primaryIndex)
      return
    }
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault()
      const item = items[current]
      if (item !== undefined) execute(item, current)
    }
  }

  function focusItem(level: 'primary' | 'secondary', index: number) {
    rootRef.current
      ?.querySelector<HTMLButtonElement>(`[data-level="${level}"][data-index="${index}"]`)
      ?.focus()
  }

  function scheduleClose() {
    if (!clickMode) return
    closeTimer.current = window.setTimeout(requestClose, 250)
  }

  function cancelClose() {
    if (closeTimer.current !== null) window.clearTimeout(closeTimer.current)
  }

  return (
    <div
      ref={rootRef}
      className="radial-file-menu"
      style={
        {
          '--radial-origin-x': `${fittedOrigin.x}px`,
          '--radial-origin-y': `${fittedOrigin.y}px`,
        } as CSSProperties
      }
      onKeyDown={handleKeyDown}
      onPointerEnter={cancelClose}
      onPointerLeave={scheduleClose}
    >
      <svg className="radial-file-menu-shapes" viewBox="0 0 336 336" aria-hidden="true">
        {expandedItem?.children?.map((item, index) => {
          const start = secondaryAnchor - (expandedItem.children!.length * 30) / 2 + index * 30
          return (
            <path
              key={item.id}
              className="radial-secondary-shape"
              data-active={secondaryIndex === index || undefined}
              data-disabled={item.disabled || undefined}
              d={annularSectorPath(
                { x: 168, y: 168 },
                SECONDARY_INNER_RADIUS,
                SECONDARY_OUTER_RADIUS,
                start,
                start + 30,
              )}
            />
          )
        })}
        {model.map((item, index) => (
          <path
            key={item.id}
            className="radial-primary-shape"
            data-active={displayedPrimaryIndex === index || undefined}
            data-disabled={item.disabled || undefined}
            data-sector-start={-120 + index * 60}
            data-sector-end={-60 + index * 60}
            d={annularSectorPath(
              { x: 168, y: 168 },
              PRIMARY_INNER_RADIUS,
              PRIMARY_OUTER_RADIUS,
              -120 + index * 60,
              -60 + index * 60,
            )}
          />
        ))}
      </svg>
      <div className="radial-primary-menu" role="menu" aria-label="文件操作">
        {model.map((item, index) => (
          <RadialButton
            key={item.id}
            id={primaryButtonId(item)}
            item={item}
            level="primary"
            index={index}
            point={polarPoint({ x: 0, y: 0 }, 78, primaryCenterAngle(index))}
            tabIndex={secondaryIndex === null && primaryIndex === index ? 0 : -1}
            controls={
              item.children !== undefined && expandedIndex === index
                ? secondaryMenuId(item)
                : undefined
            }
            expanded={item.children === undefined ? undefined : expandedIndex === index}
            onFocus={() => {
              setPrimaryIndex(index)
              setSecondaryIndex(null)
            }}
            onPointerEnter={() => {
              if (!clickMode) return
              setPrimaryIndex(index)
              scheduleExpansion(index)
            }}
            onClick={() => execute(item, index)}
          />
        ))}
      </div>
      {expandedItem?.children !== undefined ? (
        <div
          id={secondaryMenuId(expandedItem)}
          className="radial-secondary-menu"
          role="menu"
          aria-labelledby={primaryButtonId(expandedItem)}
          data-anchor-degrees={secondaryAnchor}
        >
          {expandedItem.children.map((item, index) => {
            const start = secondaryAnchor - (expandedItem.children!.length * 30) / 2
            return (
              <RadialButton
                key={item.id}
                item={item}
                level="secondary"
                index={index}
                point={polarPoint({ x: 0, y: 0 }, 140, start + index * 30 + 15)}
                tabIndex={secondaryIndex === index ? 0 : -1}
                onFocus={() => setSecondaryIndex(index)}
                onClick={() => execute(item, index)}
              />
            )
          })}
        </div>
      ) : null}
      <button
        type="button"
        className="radial-menu-center"
        tabIndex={-1}
        onClick={requestClose}
        onKeyDown={(event) => {
          if (event.key !== 'Enter' && event.key !== ' ') return
          event.preventDefault()
          event.stopPropagation()
          requestClose()
        }}
        aria-label="关闭文件操作"
      >
        <strong>{selectionCount} 个文件</strong>
        {readOnly && <span className="radial-center-context">只读</span>}
        <span>{readOnly ? '中心取消' : '回到中心取消'}</span>
      </button>
    </div>
  )
}

function RadialButton({
  id,
  item,
  level,
  index,
  point,
  tabIndex,
  controls,
  expanded,
  onFocus,
  onPointerEnter,
  onClick,
}: {
  id?: string
  item: RadialMenuItem
  level: 'primary' | 'secondary'
  index: number
  point: Point
  tabIndex: 0 | -1
  controls?: string
  expanded?: boolean
  onFocus: () => void
  onPointerEnter?: () => void
  onClick: () => void
}) {
  const checkbox = item.checked !== undefined
  return (
    <button
      id={id}
      type="button"
      role={checkbox ? 'menuitemcheckbox' : 'menuitem'}
      tabIndex={tabIndex}
      aria-checked={checkbox ? item.checked : undefined}
      aria-disabled={item.disabled}
      aria-haspopup={item.children === undefined ? undefined : 'menu'}
      aria-controls={controls}
      aria-expanded={expanded}
      title={item.disabled ? item.disabledReason : item.label}
      className="radial-menu-button"
      data-level={level}
      data-index={index}
      data-label-orientation="upright"
      data-tone={item.tone}
      style={
        { '--radial-x': `${point.x}px`, '--radial-y': `${point.y}px` } as CSSProperties
      }
      onFocus={onFocus}
      onPointerEnter={onPointerEnter}
      onClick={onClick}
    >
      <span aria-hidden="true">{item.symbol}</span>
      <span>{item.label}</span>
      {item.checked === true && (
        <span className="radial-state-cue" aria-hidden="true">
          ✓
        </span>
      )}
      {item.checked === 'mixed' && (
        <span className="radial-state-cue" aria-hidden="true">
          ±
        </span>
      )}
    </button>
  )
}

function primaryButtonId(item: RadialMenuItem): string {
  return `radial-primary-${item.id}`
}

function secondaryMenuId(item: RadialMenuItem): string {
  return `radial-secondary-${item.id}`
}

function isLeaf(item: RadialMenuItem): item is RadialMenuItem & { id: RadialLeafAction } {
  return item.children === undefined
}

function nextEnabled(items: RadialMenuItem[], current: number, delta: number): number {
  for (let offset = 1; offset <= items.length; offset += 1) {
    const index = (current + delta * offset + items.length) % items.length
    if (!items[index]?.disabled) return index
  }
  return current
}
