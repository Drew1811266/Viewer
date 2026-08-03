import type { ReactNode, RefObject } from 'react'
import ViewerDialog from './ui/ViewerDialog'

interface ModalSheetProps {
  title: string
  children: ReactNode
  footer?: ReactNode
  onCancel: () => void
  initialFocusRef?: RefObject<HTMLElement | null>
  returnFocusRef?: RefObject<HTMLElement | null>
  destructive?: boolean
}

export default function ModalSheet({
  title,
  children,
  footer,
  onCancel,
  initialFocusRef,
  returnFocusRef,
  destructive = false,
}: ModalSheetProps) {
  return (
    <ViewerDialog
      title={title}
      onCancel={onCancel}
      initialFocusRef={initialFocusRef}
      returnFocusRef={returnFocusRef}
      destructive={destructive}
      size={destructive ? 'small' : 'large'}
      footer={footer}
    >
      {children}
    </ViewerDialog>
  )
}
