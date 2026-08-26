import type { RefObject } from 'react'
import { useRef } from 'react'
import type { ReviewScopeProposal } from '../../api/types'
import ModalSheet from '../ModalSheet'
import ViewerButton from '../ui/ViewerButton'

interface ReviewStartDialogProps {
  proposal: ReviewScopeProposal
  busy: boolean
  onConfirm(): void
  onCancel(): void
  returnFocusRef?: RefObject<HTMLElement | null>
}

export default function ReviewStartDialog({
  proposal,
  busy,
  onConfirm,
  onCancel,
  returnFocusRef,
}: ReviewStartDialogProps) {
  const cancelRef = useRef<HTMLButtonElement>(null)
  const included = proposal.resolution.imageCount + proposal.resolution.videoCount
  return (
    <ModalSheet
      title="确认本轮评审范围"
      size="medium"
      onCancel={() => {
        if (!busy) onCancel()
      }}
      initialFocusRef={cancelRef}
      returnFocusRef={returnFocusRef}
      footer={
        <>
          <ViewerButton ref={cancelRef} disabled={busy} onClick={onCancel}>
            取消
          </ViewerButton>
          <ViewerButton tone="primary" loading={busy} onClick={onConfirm}>
            固定 {included} 项并开始
          </ViewerButton>
        </>
      }
    >
      <p>Viewer 将在后端重新核对并固定本轮素材。固定后，工作区导航不会改变本轮范围。</p>
      <dl className="review-count-grid" aria-label="评审范围统计">
        <div>
          <dt>图片</dt>
          <dd>{proposal.resolution.imageCount}</dd>
        </div>
        <div>
          <dt>视频</dt>
          <dd>{proposal.resolution.videoCount}</dd>
        </div>
        <div>
          <dt>排除</dt>
          <dd>{proposal.resolution.excludedCount}</dd>
        </div>
      </dl>
    </ModalSheet>
  )
}
