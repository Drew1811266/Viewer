import type { HTMLAttributes, ReactNode } from 'react'

export type ViewerStatusTagTone = 'neutral' | 'info' | 'success' | 'warning' | 'danger'

export interface ViewerStatusTagProps extends HTMLAttributes<HTMLSpanElement> {
  tone?: ViewerStatusTagTone
  children: ReactNode
}

export default function ViewerStatusTag({
  tone = 'neutral',
  className,
  children,
  ...spanProps
}: ViewerStatusTagProps) {
  return (
    <span
      {...spanProps}
      className={className === undefined ? 'viewer-status-tag' : `viewer-status-tag ${className}`}
      data-tone={tone}
    >
      {children}
    </span>
  )
}
