import type { ReactNode } from 'react'

export interface ViewerToolbarProps {
  label: string
  leading: ReactNode
  center?: ReactNode
  actions: ReactNode
  className?: string
}

export default function ViewerToolbar({
  label,
  leading,
  center,
  actions,
  className,
}: ViewerToolbarProps) {
  return (
    <header
      className={className === undefined ? 'viewer-toolbar' : `viewer-toolbar ${className}`}
      role="toolbar"
      aria-label={label}
    >
      <div className="viewer-toolbar__leading">{leading}</div>
      <div className="viewer-toolbar__center">{center}</div>
      <div className="viewer-toolbar__actions">{actions}</div>
    </header>
  )
}
