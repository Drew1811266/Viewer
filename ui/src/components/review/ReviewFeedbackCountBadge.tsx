import ViewerStatusTag from '../ui/ViewerStatusTag'

export interface ReviewFeedbackCountBadgeProps {
  count?: number
  className?: string
}

export default function ReviewFeedbackCountBadge({
  count = 0,
  className,
}: ReviewFeedbackCountBadgeProps) {
  if (count <= 0) return null
  return (
    <ViewerStatusTag
      tone="danger"
      className={
        className === undefined
          ? 'review-feedback-count-badge'
          : `review-feedback-count-badge ${className}`
      }
    >
      返工 · {count} 条
    </ViewerStatusTag>
  )
}
