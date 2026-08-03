import type { CSSProperties, ReactNode } from 'react'

export interface ViewerTaskSurfaceProps {
  label: string
  current: number
  total: number
  indeterminate?: boolean
  children: ReactNode
}

export default function ViewerTaskSurface({
  label,
  current,
  total,
  indeterminate = false,
  children,
}: ViewerTaskSurfaceProps) {
  const safeTotal = Math.max(0, total)
  const safeCurrent = Math.min(Math.max(0, current), safeTotal)
  const progress = indeterminate ? 0.35 : safeTotal === 0 ? 0 : safeCurrent / safeTotal

  return (
    <article
      className="viewer-task-surface"
      role="status"
      aria-label={label}
      aria-live="polite"
      data-indeterminate={indeterminate || undefined}
      style={{ '--viewer-progress': String(progress) } as CSSProperties}
    >
      <div className="viewer-task-surface__header">
        <strong>{children}</strong>
        <span>
          {safeCurrent} / {safeTotal}
        </span>
      </div>
      <div className="viewer-task-surface__track" aria-hidden="true">
        <div className="viewer-task-surface__fill" />
      </div>
      <progress
        className="visually-hidden"
        aria-label={label}
        max={safeTotal || 1}
        value={indeterminate ? undefined : safeCurrent}
      />
    </article>
  )
}
