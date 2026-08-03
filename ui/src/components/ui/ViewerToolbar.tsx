import type { ReactNode } from 'react'

export interface ViewerToolbarProps {
  label: string
  leading: ReactNode
  center?: ReactNode
  actions: ReactNode
}

export default function ViewerToolbar({ label, leading, center, actions }: ViewerToolbarProps) {
  return (
    <header className="viewer-toolbar" role="toolbar" aria-label={label}>
      <div className="viewer-toolbar__leading">{leading}</div>
      <div className="viewer-toolbar__center">{center}</div>
      <div className="viewer-toolbar__actions">{actions}</div>
    </header>
  )
}
