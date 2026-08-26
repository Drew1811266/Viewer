import { useRef } from 'react'
import type { ReviewCompletionProposal } from '../../app/review/reviewModel'
import ModalSheet from '../ModalSheet'
import ViewerButton from '../ui/ViewerButton'
import ReviewConflictList from './ReviewConflictList'

interface ReviewCompletionDialogProps {
  proposal: ReviewCompletionProposal
  busy: boolean
  onConfirm(): void
  onCancel(): void
}

export default function ReviewCompletionDialog({
  proposal,
  busy,
  onConfirm,
  onCancel,
}: ReviewCompletionDialogProps) {
  const cancelRef = useRef<HTMLButtonElement>(null)
  const { summary } = proposal
  return (
    <ModalSheet
      title="完成本轮评审"
      size="large"
      onCancel={() => {
        if (!busy) onCancel()
      }}
      initialFocusRef={cancelRef}
      footer={
        <>
          <ViewerButton ref={cancelRef} disabled={busy} onClick={onCancel}>
            返回检查
          </ViewerButton>
          <ViewerButton
            tone="primary"
            loading={busy}
            disabled={!summary.canComplete}
            onClick={onConfirm}
          >
            确认完成本轮
          </ViewerButton>
        </>
      }
    >
      <p>确认后将发布不可变的完成记录；没有返工意见且可评审的素材会在此时成为通过。</p>
      <dl className="review-count-grid review-count-grid--completion" aria-label="完成结果统计">
        <div>
          <dt>固定素材</dt>
          <dd>{summary.total}</dd>
        </div>
        <div>
          <dt>返工</dt>
          <dd>{summary.revise}</dd>
        </div>
        <div>
          <dt>不可评审</dt>
          <dd>{summary.unreviewable}</dd>
        </div>
        <div>
          <dt>默认通过</dt>
          <dd>{summary.defaultPass}</dd>
        </div>
      </dl>
      <section className="review-completion-feedback" aria-labelledby="review-feedback-summary">
        <h3 id="review-feedback-summary">返工意见</h3>
        {summary.feedback.length === 0 ? (
          <p>本轮没有返工意见。</p>
        ) : (
          <ul>
            {summary.feedback.map((feedback) => (
              <li key={feedback.feedbackId}>
                <p>{feedback.text}</p>
                <small>{feedback.targetCount} 个目标</small>
              </li>
            ))}
          </ul>
        )}
      </section>
      {summary.pending.length > 0 && (
        <section aria-labelledby="review-pending-title">
          <h3 id="review-pending-title">等待验证</h3>
          <ul>
            {summary.pending.map((path) => (
              <li key={path}>{path}</li>
            ))}
          </ul>
        </section>
      )}
      <ReviewConflictList conflicts={summary.conflicts} />
    </ModalSheet>
  )
}
