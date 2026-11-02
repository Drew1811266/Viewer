import type { ReactNode, RefObject } from 'react'
import ViewerDialog from './ui/ViewerDialog'

interface ModalSheetProps {
  title: string
  description?: string
  children: ReactNode
  footer?: ReactNode
  onCancel: () => void
  initialFocusRef?: RefObject<HTMLElement | null>
  returnFocusRef?: RefObject<HTMLElement | null>
  destructive?: boolean
  size?: 'small' | 'medium' | 'large'
  showClose?: boolean
}

export default function ModalSheet({
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
}: ModalSheetProps) {
  return (
    <ViewerDialog
      title={title}
      description={description}
      onCancel={onCancel}
      initialFocusRef={initialFocusRef}
      returnFocusRef={returnFocusRef}
      destructive={destructive}
      size={size}
      showClose={showClose}
      footer={footer}
    >
      {children}
    </ViewerDialog>
  )
}
