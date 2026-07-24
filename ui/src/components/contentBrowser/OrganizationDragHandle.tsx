import type { PointerEvent } from 'react'
import { useRef } from 'react'
import type { BrowserFile } from '../../api/types'

export function OrganizationDragHandle({
  file,
  disabled,
  onPointerDown,
  onPointerMove,
  onPointerUp,
  onPointerCancel,
}: {
  file: BrowserFile
  disabled: boolean
  onPointerDown: (file: BrowserFile, event: PointerEvent<HTMLElement>) => void
  onPointerMove: (event: PointerEvent<HTMLElement>) => void
  onPointerUp: (event: PointerEvent<HTMLElement>) => void
  onPointerCancel: (event: PointerEvent<HTMLElement>) => void
}) {
  const organizationPointerId = useRef<number | null>(null)

  return (
    <button
      type="button"
      className="organization-drag-handle"
      aria-label={`整理 ${file.name}`}
      title="拖到左侧文件夹，按住 Option 复制"
      disabled={disabled}
      draggable={false}
      onClick={(event) => {
        event.preventDefault()
        event.stopPropagation()
      }}
      onDoubleClick={(event) => {
        event.preventDefault()
        event.stopPropagation()
      }}
      onDragStart={(event) => {
        event.preventDefault()
        event.stopPropagation()
      }}
      onPointerDown={(event) => {
        if (event.button === 0 && !event.ctrlKey) {
          organizationPointerId.current = event.pointerId
        }
        onPointerDown(file, event)
      }}
      onPointerMove={(event) => {
        if (organizationPointerId.current === event.pointerId) onPointerMove(event)
      }}
      onPointerUp={(event) => {
        if (organizationPointerId.current !== event.pointerId) return
        organizationPointerId.current = null
        onPointerUp(event)
      }}
      onPointerCancel={(event) => {
        if (organizationPointerId.current !== event.pointerId) return
        organizationPointerId.current = null
        onPointerCancel(event)
      }}
    >
      ⋮⋮
    </button>
  )
}
