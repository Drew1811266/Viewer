import type { ReactNode, RefObject } from 'react'
import { useEffect, useId, useRef } from 'react'

interface ModalSheetProps {
  title: string
  children: ReactNode
  onCancel: () => void
  initialFocusRef?: RefObject<HTMLElement | null>
  returnFocusRef?: RefObject<HTMLElement | null>
  destructive?: boolean
}

const FOCUSABLE =
  'button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [href], [tabindex]:not([tabindex="-1"])'

export default function ModalSheet({
  title,
  children,
  onCancel,
  initialFocusRef,
  returnFocusRef,
  destructive = false,
}: ModalSheetProps) {
  const titleId = useId()
  const dialogRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null
    const target =
      initialFocusRef?.current ?? dialogRef.current?.querySelector<HTMLElement>(FOCUSABLE)
    target?.focus()
    return () => (returnFocusRef?.current ?? previous)?.focus()
  }, [initialFocusRef, returnFocusRef])

  function containFocus(event: React.KeyboardEvent<HTMLDivElement>) {
    if (event.key === 'Escape') {
      event.preventDefault()
      event.stopPropagation()
      onCancel()
      return
    }
    if (event.key !== 'Tab' || dialogRef.current === null) return
    const focusable = [...dialogRef.current.querySelectorAll<HTMLElement>(FOCUSABLE)]
    if (focusable.length === 0) {
      event.preventDefault()
      dialogRef.current.focus()
      return
    }
    const [first] = focusable
    const last = focusable.at(-1)
    if (first === undefined || last === undefined) {
      throw new Error('Non-empty modal focus list is missing a boundary element')
    }
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault()
      last.focus()
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault()
      first.focus()
    }
  }

  return (
    <div className="modal-backdrop" data-destructive={destructive || undefined}>
      <div
        ref={dialogRef}
        className="modal-sheet"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        tabIndex={-1}
        onKeyDown={containFocus}
      >
        <header className="modal-sheet-header">
          <h2 id={titleId}>{title}</h2>
        </header>
        <div className="modal-sheet-body">{children}</div>
      </div>
    </div>
  )
}
