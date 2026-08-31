import type { ReviewSessionSnapshot } from '../../app/review/reviewModel'
import ViewerButton from '../ui/ViewerButton'

interface ReviewContextBarProps {
  protocol?: 'legacy' | 'continuous'
  snapshot: ReviewSessionSnapshot
  inspectorOpen: boolean
  onReturnToMembers(): void
  onToggleInspector(): void
  onPrepareCompletion(): void
  onRequestAbandon(): void
  onArchive?: () => void
  onHistory?: () => void
}

export default function ReviewContextBar({
  protocol = 'legacy',
  snapshot,
  inspectorOpen,
  onReturnToMembers,
  onToggleInspector,
  onPrepareCompletion,
  onRequestAbandon,
  onArchive,
  onHistory,
}: ReviewContextBarProps) {
  if (protocol === 'continuous') {
    return (
      <section className="review-context-bar" aria-label="持续评审上下文">
        <div className="review-context-bar__identity">
          <strong>持续评审</strong>
          <span>当前意见 {snapshot.counts.feedbackItems}</span>
        </div>
        <div className="review-context-bar__actions">
          <ViewerButton tone="quiet" onClick={onReturnToMembers}>
            返回素材
          </ViewerButton>
          <ViewerButton tone="secondary" onClick={onHistory} disabled={onHistory === undefined}>
            历史
          </ViewerButton>
          <ViewerButton tone="primary" onClick={onArchive} disabled={onArchive === undefined}>
            存档意见
          </ViewerButton>
        </div>
      </section>
    )
  }
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
