import { useRef } from 'react'
import ModalSheet from './ModalSheet'

interface CloseOperationDialogProps {
  busy: boolean
  onWait: () => void
  onCancelPending: () => void
  onStay: () => void
}

export default function CloseOperationDialog({
  busy,
  onWait,
  onCancelPending,
  onStay,
}: CloseOperationDialogProps) {
  const stayRef = useRef<HTMLButtonElement>(null)
  return (
    <ModalSheet
      title="文件操作尚未完成"
      onCancel={() => {
        if (!busy) onStay()
      }}
      initialFocusRef={stayRef}
    >
      <p>可以等待当前批次完成，也可以取消尚未开始的项目后安全关闭。</p>
      <p>已经完成的项目不会撤销；当前原子步骤会先结束。</p>
      {busy && <p role="status">正在安全结束文件操作…</p>}
      <div className="modal-actions close-operation-actions">
        <button ref={stayRef} type="button" disabled={busy} onClick={onStay}>
          保持打开
        </button>
        <button type="button" className="primary-button" disabled={busy} onClick={onWait}>
          等待完成后关闭
        </button>
        <button
          type="button"
          className="destructive-button"
          disabled={busy}
          onClick={onCancelPending}
        >
          取消待处理项目并关闭
        </button>
      </div>
    </ModalSheet>
  )
}
