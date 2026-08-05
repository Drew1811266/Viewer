import { useRef } from 'react'
import ModalSheet from './ModalSheet'
import ViewerButton from './ui/ViewerButton'
import ViewerLocalFeedback from './ui/ViewerLocalFeedback'

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
      size="medium"
      onCancel={() => {
        if (!busy) onStay()
      }}
      initialFocusRef={stayRef}
      footer={
        <div className="close-operation-actions">
          <ViewerButton ref={stayRef} tone="secondary" disabled={busy} onClick={onStay}>
            停留在当前项目
          </ViewerButton>
          <ViewerButton tone="secondary" disabled={busy} onClick={onCancelPending}>
            取消待处理项目并关闭
          </ViewerButton>
          <ViewerButton tone="primary" loading={busy} onClick={onWait}>
            等待完成后关闭
          </ViewerButton>
        </div>
      }
    >
      <p>可以等待当前批次完成，也可以取消尚未开始的项目后安全关闭。</p>
      <p>已经完成的项目不会撤销；当前原子步骤会先结束。</p>
      {busy && (
        <ViewerLocalFeedback tone="info" title="正在安全结束文件操作">
          当前原子步骤完成后将继续关闭流程。
        </ViewerLocalFeedback>
      )}
    </ModalSheet>
  )
}
