import type { CSSProperties, ReactNode, RefObject } from 'react'
import { useEffect, useId, useRef } from 'react'
import { ViewerIconButton } from './ViewerButton'

export interface ViewerDialogProps {
  title: string
  description?: string
  children: ReactNode
  footer?: ReactNode
  onCancel(): void
  initialFocusRef?: RefObject<HTMLElement | null>
  returnFocusRef?: RefObject<HTMLElement | null>
  destructive?: boolean
  size?: 'small' | 'medium' | 'large'
  showClose?: boolean
}

const FOCUSABLE =
  'button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [href], [tabindex]:not([tabindex="-1"])'

export default function ViewerDialog({
  title,
  description,
  children,
  footer,
  onCancel,
  initialFocusRef,
  returnFocusRef,
  destructive = false,
  size = 'small',
  showClose = true,
}: ViewerDialogProps) {
  const titleId = useId()
  const descriptionId = useId()
  const dialogRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null
    const target =
      initialFocusRef?.current ??
      dialogRef.current?.querySelector<HTMLElement>(`.viewer-dialog__body ${FOCUSABLE}`) ??
      dialogRef.current?.querySelector<HTMLElement>(FOCUSABLE)
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
    const first = focusable[0]
    const last = focusable.at(-1)
    if (first === undefined || last === undefined) return
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault()
      last.focus()
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault()
      first.focus()
    }
  }

  return (
    <div className="viewer-dialog-backdrop" data-destructive={destructive || undefined}>
      <div
        ref={dialogRef}
        className="viewer-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={description === undefined ? undefined : descriptionId}
        data-size={size}
        data-destructive={destructive || undefined}
        tabIndex={-1}
        onKeyDown={containFocus}
        style={{ '--viewer-dialog-footer': footer === undefined ? '0' : '1' } as CSSProperties}
      >
        <header className="viewer-dialog__header">
          <div className="viewer-dialog__heading">
            <h2 id={titleId}>{title}</h2>
            {description === undefined ? null : <p id={descriptionId}>{description}</p>}
          </div>
          {showClose && (
            <ViewerIconButton
              icon="x"
              label="关闭对话框"
              tone="quiet"
              className="viewer-dialog__close"
              onClick={onCancel}
            />
          )}
        </header>
        <div className="viewer-dialog__body">{children}</div>
        {footer === undefined ? null : <footer className="viewer-dialog__footer">{footer}</footer>}
      </div>
    </div>
  )
}
