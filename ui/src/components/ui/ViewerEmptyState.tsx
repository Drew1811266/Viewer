import type { ReactNode } from 'react'

export interface ViewerEmptyStateProps {
  title: string
  description: string
  action?: ReactNode
}

export default function ViewerEmptyState({ title, description, action }: ViewerEmptyStateProps) {
  return (
    <section className="viewer-empty-state">
      <h2>{title}</h2>
      <p>{description}</p>
      {action === undefined ? null : <div className="viewer-empty-state__action">{action}</div>}
    </section>
  )
}
