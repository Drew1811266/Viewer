import type { CSSProperties } from 'react'
import type { OrganizationDragMode } from '../state/useOrganizationPointerDrag'

export default function OrganizationDragPreview({
  clientX,
  clientY,
  itemCount,
  mode,
}: {
  clientX: number
  clientY: number
  itemCount: number
  mode: OrganizationDragMode
}) {
  return (
    <div
      className="organization-drag-preview"
      role="status"
      aria-label="正在整理文件"
      style={
        {
          '--organization-drag-x': `${clientX}px`,
          '--organization-drag-y': `${clientY}px`,
        } as CSSProperties
      }
    >
      {mode === 'copy' ? '复制' : '移动'} {itemCount} 项
    </div>
  )
}
