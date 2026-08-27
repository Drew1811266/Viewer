import type { ReviewSessionSnapshot } from '../../app/review/reviewModel'
import ViewerButton from '../ui/ViewerButton'

interface ReviewContextBarProps {
  snapshot: ReviewSessionSnapshot
  inspectorOpen: boolean
  onReturnToMembers(): void
  onToggleInspector(): void
  onPrepareCompletion(): void
  onRequestAbandon(): void
}

export default function ReviewContextBar({
  snapshot,
  inspectorOpen,
  onReturnToMembers,
  onToggleInspector,
  onPrepareCompletion,
  onRequestAbandon,
}: ReviewContextBarProps) {
  const completed = snapshot.phase === 'completed_read_only'
  return (
    <section className="review-context-bar" aria-label="本轮评审上下文">
      <div className="review-context-bar__identity">
        <strong>{completed ? '本轮已完成，只读保存' : '本轮评审'}</strong>
        <span>固定素材 {snapshot.counts.total}</span>
        <span>意见 {snapshot.counts.feedbackItems}</span>
        <span>不可评审 {snapshot.counts.unreviewable}</span>
      </div>
      <div className="review-context-bar__actions">
        <ViewerButton tone="quiet" onClick={onReturnToMembers}>
          返回本轮素材
        </ViewerButton>
        <ViewerButton tone="secondary" active={inspectorOpen} onClick={onToggleInspector}>
          {completed ? '查看评审意见' : '评审意见'}
        </ViewerButton>
        {!completed && (
          <>
            <ViewerButton tone="primary" onClick={onPrepareCompletion}>
              完成本轮评审
            </ViewerButton>
            <ViewerButton tone="quiet" onClick={onRequestAbandon}>
              放弃本轮
            </ViewerButton>
          </>
        )}
      </div>
    </section>
  )
}
