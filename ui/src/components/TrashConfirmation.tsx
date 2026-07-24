import { useRef } from 'react'
import ModalSheet from './ModalSheet'

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
    >
      <p>将 {count} 项移入 macOS 废纸篓。之后可通过系统废纸篓恢复。</p>
      <p>此操作不支持 Viewer 内撤销。</p>
      <div className="modal-actions">
        <button ref={cancelRef} type="button" onClick={onCancel}>
          取消
        </button>
        <button type="button" className="destructive-button" disabled={busy} onClick={onConfirm}>
          {busy ? '正在处理…' : '移入废纸篓'}
        </button>
      </div>
    </ModalSheet>
  )
}
