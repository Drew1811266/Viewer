import type { ProjectAccess } from '../../app/review/reviewModel'
import type { ContinuousReviewCoordinator } from '../../app/review/useContinuousReviewCoordinator'
import type { ReviewSessionCoordinator } from '../../app/review/useReviewSessionCoordinator'
import ViewerButton from '../ui/ViewerButton'
import ContinuousReviewRecoveryNotice from './ContinuousReviewRecoveryNotice'

interface ReviewRecoveryNoticeProps {
  review: ReviewSessionCoordinator
  projectAccess: ProjectAccess
  continuousReview?: ContinuousReviewCoordinator
}

export default function ReviewRecoveryNotice({
  review,
  projectAccess,
  continuousReview,
}: ReviewRecoveryNoticeProps) {
  if (continuousReview !== undefined) {
    return (
      <ContinuousReviewRecoveryNotice
        coordinator={continuousReview}
        projectAccess={projectAccess}
      />
    )
  }
  const { snapshot } = review
  if (snapshot.resume !== null) {
    return (
      <section className="review-local-notice" role="status" aria-live="polite">
        <div>
          <strong>发现未完成的评审</strong>
          <span>
            固定素材 {snapshot.resume.total} · 已保存意见 {snapshot.resume.feedbackItems}
            {projectAccess === 'read_only' ? ' · 当前项目只读，不能继续修改' : ''}
          </span>
        </div>
        <ViewerButton
          tone="primary"
          disabled={projectAccess === 'read_only'}
          onClick={() => void review.resume()}
        >
          继续评审
        </ViewerButton>
      </section>
    )
  }

  if (snapshot.phase === 'recovery_required') {
    return (
      <section className="review-local-notice" data-tone="danger" role="alert">
        <div>
          <strong>评审记录需要恢复处理</strong>
          <span>Viewer 不会猜测或覆盖记录；普通浏览和预览仍可继续。</span>
        </div>
      </section>
    )
  }

  if (snapshot.phase === 'write_unavailable') {
    return (
      <section className="review-local-notice" data-tone="warning" role="status">
        <div>
          <strong>评审暂时不可写</strong>
          <span>请稍后重试；普通浏览和预览仍可继续。</span>
        </div>
      </section>
    )
  }

  if (projectAccess === 'read_only' && snapshot.phase !== 'completed_read_only') {
    return (
      <section className="review-local-notice" data-tone="warning" role="status">
        <div>
          <strong>评审为只读</strong>
          <span>当前项目不能创建或修改评审；普通浏览和预览仍可继续。</span>
        </div>
      </section>
    )
  }

  if (snapshot.error !== null || review.error !== null) {
    return (
      <section className="review-local-notice" data-tone="warning" role="alert">
        <div>
          <strong>{errorTitle(snapshot.error?.code)}</strong>
          <span>{review.error ?? '普通浏览和预览仍可继续。'}</span>
        </div>
      </section>
    )
  }
  return null
}

function errorTitle(code: string | undefined): string {
  if (code === 'review_unsupported_version') return '评审协议版本暂不支持'
  if (code === 'review_invalid_data') return '评审记录无法安全读取'
  if (code === 'review_busy') return '评审暂时不可写'
  return '评审操作未完成'
}
