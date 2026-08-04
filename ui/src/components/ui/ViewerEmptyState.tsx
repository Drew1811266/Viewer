import type { ReactNode } from 'react'

export interface ViewerEmptyStateProps {
  title: string
  description: string
  action?: ReactNode
  appearance?: 'panel' | 'plain'
}

export default function ViewerEmptyState({
  title,
  description,
  action,
  appearance = 'panel',
}: ViewerEmptyStateProps) {
  return (
    <section className="viewer-empty-state" data-appearance={appearance}>
      <h2>{title}</h2>
      <p>{description}</p>
      {action === undefined ? null : <div className="viewer-empty-state__action">{action}</div>}
    </section>
  )
}
