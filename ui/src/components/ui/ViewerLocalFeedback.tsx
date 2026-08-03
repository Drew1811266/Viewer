import type { ReactNode } from 'react'

export interface ViewerLocalFeedbackProps {
  tone: 'info' | 'warning' | 'danger' | 'recovery'
  title: string
  children: ReactNode
  action?: ReactNode
}

export default function ViewerLocalFeedback({
  tone,
  title,
  children,
  action,
}: ViewerLocalFeedbackProps) {
  return (
    <section
      className="viewer-local-feedback"
      role={tone === 'danger' ? 'alert' : 'status'}
      data-tone={tone}
    >
      <div className="viewer-local-feedback__content">
        <strong>{title}</strong>
        <p>{children}</p>
      </div>
      {action === undefined ? null : <div className="viewer-local-feedback__action">{action}</div>}
    </section>
  )
}
