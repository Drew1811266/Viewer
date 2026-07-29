import type { KeyboardEvent, RefObject } from 'react'
import { useEffect, useLayoutEffect, useRef } from 'react'
import type { SelectAllScope } from './adaptiveOtherFilePanelModel'

export interface SelectAllChoicePanelProps {
  open: boolean
  anchorRef: RefObject<HTMLButtonElement | null>
  onChoose(scope: SelectAllScope): void
  onCancel(): void
}

export default function SelectAllChoicePanel({
  open,
  anchorRef,
  onChoose,
  onCancel,
}: SelectAllChoicePanelProps) {
  const panelRef = useRef<HTMLDivElement>(null)
  const itemRefs = useRef<Array<HTMLButtonElement | null>>([])
  const restoreFocusFrame = useRef<number | null>(null)

  useLayoutEffect(() => {
    if (open) itemRefs.current[0]?.focus()
  }, [open])

  useEffect(() => {
    if (!open) return

    function handlePointerDown(event: globalThis.PointerEvent) {
      const target = event.target
      if (!(target instanceof Node)) return
      if (panelRef.current?.contains(target) || anchorRef.current?.contains(target)) return
      const anchor = anchorRef.current
      if (anchor !== null) {
        restoreFocusFrame.current = requestAnimationFrame(() => {
          restoreFocusFrame.current = null
          if (anchor.isConnected) anchor.focus()
        })
      }
      onCancel()
    }

    document.addEventListener('pointerdown', handlePointerDown, true)
    return () => document.removeEventListener('pointerdown', handlePointerDown, true)
  }, [anchorRef, onCancel, open])

  useEffect(
    () => () => {
      if (restoreFocusFrame.current !== null) {
        cancelAnimationFrame(restoreFocusFrame.current)
        restoreFocusFrame.current = null
      }
    },
    [],
  )

  if (!open) return null

  function handleKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    const items = itemRefs.current.filter((item): item is HTMLButtonElement => item !== null)
    if (items.length === 0 || event.key === 'Tab') return
    const focusedIndex = Math.max(0, items.indexOf(document.activeElement as HTMLButtonElement))

    let nextIndex: number | null = null
    if (event.key === 'ArrowDown') nextIndex = (focusedIndex + 1) % items.length
    if (event.key === 'ArrowUp') nextIndex = (focusedIndex - 1 + items.length) % items.length
    if (event.key === 'Home') nextIndex = 0
    if (event.key === 'End') nextIndex = items.length - 1
    if (nextIndex !== null) {
      event.preventDefault()
      items[nextIndex]?.focus()
      return
    }
    if (event.key === 'Enter' || event.key === ' ' || event.key === 'Spacebar') {
      event.preventDefault()
      items[focusedIndex]?.click()
      return
    }
    if (event.key === 'Escape') {
      event.preventDefault()
      cancelAndRestoreFocus(anchorRef, onCancel)
    }
  }

  return (
    <div
      ref={panelRef}
      className="select-all-choice-panel"
      role="menu"
      aria-label="选择全选范围"
      onKeyDown={handleKeyDown}
    >
      <button
        ref={(node) => {
          itemRefs.current[0] = node
        }}
        type="button"
        role="menuitem"
        onClick={() => onChoose('images')}
      >
        全选图片
      </button>
      <button
        ref={(node) => {
          itemRefs.current[1] = node
        }}
        type="button"
        role="menuitem"
        onClick={() => onChoose('other')}
      >
        全选其它文件
      </button>
      <button
        ref={(node) => {
          itemRefs.current[2] = node
        }}
        type="button"
        role="menuitem"
        onClick={() => onChoose('all')}
      >
        全部都选
      </button>
    </div>
  )
}

function cancelAndRestoreFocus(
  anchorRef: RefObject<HTMLButtonElement | null>,
  onCancel: () => void,
) {
  onCancel()
  const anchor = anchorRef.current
  if (anchor?.isConnected) anchor.focus()
}
