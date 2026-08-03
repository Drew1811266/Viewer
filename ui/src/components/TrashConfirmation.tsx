import { useRef } from 'react'
import ModalSheet from './ModalSheet'
import ViewerButton from './ui/ViewerButton'

interface TrashConfirmationProps {
  count: number
  busy: boolean
  onConfirm: () => void
  onCancel: () => void
}

export default function TrashConfirmation({
  count,
  busy,
  onConfirm,
  onCancel,
}: TrashConfirmationProps) {
  const cancelRef = useRef<HTMLButtonElement>(null)
  return (
    <ModalSheet
      title="将文件移到废纸篓？"
      onCancel={onCancel}
      initialFocusRef={cancelRef}
      destructive
      footer={
        <>
          <ViewerButton ref={cancelRef} tone="secondary" disabled={busy} onClick={onCancel}>
            取消
          </ViewerButton>
          <ViewerButton tone="danger" loading={busy} onClick={onConfirm}>
            移到废纸篓
          </ViewerButton>
        </>
      }
    >
      <p>将 {count} 项移入 macOS 废纸篓。之后可通过系统废纸篓恢复。</p>
      <p>此操作不支持 Viewer 内撤销。</p>
    </ModalSheet>
  )
}
