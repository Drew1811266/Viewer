import { type HTMLAttributes, type ReactNode, type RefObject, useEffect, useRef } from 'react'

export type ViewerPopoverAlign = 'start' | 'end'

export interface ViewerPopoverProps extends Omit<HTMLAttributes<HTMLDivElement>, 'role'> {
  open: boolean
  label: string
  triggerRef: RefObject<HTMLElement | null>
  onOpenChange(open: boolean): void
  align?: ViewerPopoverAlign
  children: ReactNode
}

export default function ViewerPopover({
  open,
  label,
  triggerRef,
  onOpenChange,
  align = 'end',
  className,
  children,
  ...containerProps
}: ViewerPopoverProps) {
  const popoverRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!open) return

    const closeAndRestoreFocus = () => {
      onOpenChange(false)
      triggerRef.current?.focus()
    }
    const handlePointerDown = (event: PointerEvent) => {
      const target = event.target
      if (!(target instanceof Node)) return
      if (popoverRef.current?.contains(target) || triggerRef.current?.contains(target)) return
      closeAndRestoreFocus()
    }
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return
      event.preventDefault()
      closeAndRestoreFocus()
    }
    document.addEventListener('pointerdown', handlePointerDown)
    document.addEventListener('keydown', handleKeyDown)
    return () => {
      document.removeEventListener('pointerdown', handlePointerDown)
      document.removeEventListener('keydown', handleKeyDown)
    }
  }, [onOpenChange, open, triggerRef])

  if (!open) return null

  return (
    <div
      {...containerProps}
      ref={popoverRef}
      className={className === undefined ? 'viewer-popover' : `viewer-popover ${className}`}
      role="region"
      aria-label={label}
      data-align={align}
    >
      {children}
    </div>
  )
}
