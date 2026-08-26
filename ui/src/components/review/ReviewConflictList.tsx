import type { ReviewConflictSnapshot } from '../../app/review/reviewModel'

interface ReviewConflictListProps {
  conflicts: ReviewConflictSnapshot[]
}

export default function ReviewConflictList({ conflicts }: ReviewConflictListProps) {
  if (conflicts.length === 0) return null
  return (
    <section className="review-conflicts" aria-labelledby="review-conflict-title">
      <h3 id="review-conflict-title">需要处理的素材变化</h3>
      <p>以下素材与固定版本不一致，处理前不能完成本轮。</p>
      <ul>
        {conflicts.map((conflict) => (
          <li key={conflict.assetVersionId}>
            <span>{conflict.relativePath}</span>
            <small>{conflictLabel(conflict.kind)}</small>
          </li>
        ))}
      </ul>
    </section>
  )
}

function conflictLabel(kind: ReviewConflictSnapshot['kind']): string {
  const labels: Record<ReviewConflictSnapshot['kind'], string> = {
    missing: '文件缺失',
    moved: '位置变化',
    replaced: '文件已替换',
    size_changed: '大小变化',
    content_changed: '内容变化',
    media_changed: '媒体信息变化',
  }
  return labels[kind]
}
