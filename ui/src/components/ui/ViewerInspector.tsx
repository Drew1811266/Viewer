import type { ReactNode } from 'react'
import { ViewerIconButton } from './ViewerButton'

export interface ViewerInspectorProps {
  label: string
  title: string
  status?: ReactNode
  children: ReactNode
  footer?: ReactNode
  onClose(): void
}

export default function ViewerInspector({
  label,
  title,
  status,
  children,
  footer,
  onClose,
}: ViewerInspectorProps) {
  return (
    <aside className="viewer-inspector" aria-label={label}>
      <header className="viewer-inspector__header">
        <div>
          <h2>{title}</h2>
          {status === undefined ? null : <div className="viewer-inspector__status">{status}</div>}
        </div>
        <ViewerIconButton icon="x" label={`关闭${title}`} tone="quiet" onClick={onClose} />
      </header>
      <div className="viewer-inspector__body">{children}</div>
      {footer === undefined ? null : <footer className="viewer-inspector__footer">{footer}</footer>}
    </aside>
  )
}
