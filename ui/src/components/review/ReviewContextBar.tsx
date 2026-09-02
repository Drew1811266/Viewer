import type { ReviewSessionSnapshot } from '../../app/review/reviewModel'
import ViewerButton from '../ui/ViewerButton'

interface ReviewContextBarProps {
  protocol?: 'legacy' | 'continuous'
  placement?: 'workspace' | 'toolbar'
  snapshot: ReviewSessionSnapshot
  inspectorOpen: boolean
  onReturnToMembers(): void
  onToggleInspector(): void
  onPrepareCompletion(): void
  onRequestAbandon(): void
  onArchive?: () => void
  onHistory?: () => void
  onUsageImport?: () => void
  continuousFeedbackCount?: number
  archiveDisabled?: boolean
  historyDisabled?: boolean
  archiveNotice?: string | null
}

export default function ReviewContextBar({
  protocol = 'legacy',
  placement = 'workspace',
  snapshot,
  inspectorOpen,
  onReturnToMembers,
  onToggleInspector,
  onPrepareCompletion,
  onRequestAbandon,
  onArchive,
  onHistory,
  onUsageImport,
  continuousFeedbackCount,
  archiveDisabled = false,
  historyDisabled = false,
  archiveNotice = null,
}: ReviewContextBarProps) {
  if (protocol === 'continuous') {
    const toolbar = placement === 'toolbar'
    const feedbackCount = continuousFeedbackCount ?? snapshot.counts.feedbackItems
    return (
      <section
        className={toolbar ? 'continuous-review-toolbar' : 'review-context-bar'}
        aria-label="持续评审上下文"
      >
        <div
          className={toolbar ? 'continuous-review-toolbar__status' : 'review-context-bar__identity'}
          aria-label={toolbar ? '持续评审状态' : undefined}
        >
          <strong>持续评审</strong>
          {toolbar ? (
            <span
              className="continuous-review-toolbar__count"
              aria-label={`当前意见 ${feedbackCount}`}
            >
              <span className="continuous-review-toolbar__count-label">当前意见</span>
              <span className="continuous-review-toolbar__compact-label">评审</span>
              <b>{feedbackCount}</b>
            </span>
          ) : (
            <span>当前意见 {feedbackCount}</span>
          )}
          {toolbar && archiveNotice !== null && (
            <span className="continuous-review-toolbar__notice" role="status">
              {archiveNotice}
            </span>
          )}
        </div>
        {toolbar && <span className="continuous-review-toolbar__divider" aria-hidden="true" />}
        <div
          className={toolbar ? 'continuous-review-toolbar__actions' : 'review-context-bar__actions'}
          role={toolbar ? 'group' : undefined}
          aria-label={toolbar ? '持续评审操作' : undefined}
        >
          <ViewerButton tone="quiet" onClick={onReturnToMembers}>
            返回素材
          </ViewerButton>
          <ViewerButton
            tone="secondary"
            onClick={onHistory}
            disabled={onHistory === undefined || historyDisabled}
          >
            历史
          </ViewerButton>
          {onUsageImport !== undefined && (
            <ViewerButton tone="secondary" onClick={onUsageImport}>
              导入声明
            </ViewerButton>
          )}
          <ViewerButton
            tone="primary"
            onClick={onArchive}
            disabled={onArchive === undefined || archiveDisabled}
          >
            存档意见
          </ViewerButton>
        </div>
        {!toolbar && archiveNotice !== null && (
          <span className="review-context-bar__notice" role="status">
            {archiveNotice}
          </span>
        )}
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
