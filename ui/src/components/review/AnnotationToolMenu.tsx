import { useId, useLayoutEffect, useRef, useState } from 'react'
import type { AnnotationTool, MarkupTool } from '../../app/review/annotationToolRegistry'
import { ANNOTATION_TOOLS } from '../../app/review/annotationToolRegistry'
import ViewerButton from '../ui/ViewerButton'
import ViewerIcon from '../ui/ViewerIcon'

interface AnnotationToolMenuProps {
  activeTool: AnnotationTool
  disabledReason?: string | null
  onSelect(tool: MarkupTool): void
}

interface MenuPosition {
  left: number
  top: number
}

const VIEWPORT_GUTTER = 8
const MENU_GAP = 6

export default function AnnotationToolMenu({
  activeTool,
  disabledReason = null,
  onSelect,
}: AnnotationToolMenuProps) {
  const [open, setOpen] = useState(false)
  const [focusIndex, setFocusIndex] = useState(0)
  const [position, setPosition] = useState<MenuPosition>({ left: VIEWPORT_GUTTER, top: 0 })
  const triggerRef = useRef<HTMLButtonElement>(null)
  const menuRef = useRef<HTMLDivElement>(null)
  const itemRefs = useRef<Array<HTMLButtonElement | null>>([])
  const disabledMessageId = useId()

  useLayoutEffect(() => {
    if (!open) return
    const index = Math.max(
      0,
      ANNOTATION_TOOLS.findIndex((tool) => tool.id === activeTool),
    )
    setFocusIndex(index)
    itemRefs.current[index]?.focus()

    function positionMenu() {
      const trigger = triggerRef.current
      const menu = menuRef.current
      if (trigger === null || menu === null) return
      const triggerRect = trigger.getBoundingClientRect()
      const menuRect = menu.getBoundingClientRect()
      const maximumLeft = Math.max(
        VIEWPORT_GUTTER,
        window.innerWidth - menuRect.width - VIEWPORT_GUTTER,
      )
      const left = clamp(triggerRect.left, VIEWPORT_GUTTER, maximumLeft)
      const below = triggerRect.bottom + MENU_GAP
      const above = triggerRect.top - menuRect.height - MENU_GAP
      const top =
        below + menuRect.height <= window.innerHeight - VIEWPORT_GUTTER || above < VIEWPORT_GUTTER
          ? clamp(
              below,
              VIEWPORT_GUTTER,
              Math.max(VIEWPORT_GUTTER, window.innerHeight - menuRect.height - VIEWPORT_GUTTER),
            )
          : Math.max(VIEWPORT_GUTTER, above)
      setPosition({ left, top })
    }

    function closeFromOutside(event: PointerEvent) {
      const target = event.target
      if (!(target instanceof Node)) return
      if (triggerRef.current?.contains(target) || menuRef.current?.contains(target)) return
      setOpen(false)
    }

    function closeFromEscape(event: KeyboardEvent) {
      if (event.key !== 'Escape') return
      event.preventDefault()
      setOpen(false)
      triggerRef.current?.focus()
    }

    positionMenu()
    window.addEventListener('resize', positionMenu)
    window.addEventListener('scroll', positionMenu, true)
    document.addEventListener('pointerdown', closeFromOutside)
    document.addEventListener('keydown', closeFromEscape)
    return () => {
      window.removeEventListener('resize', positionMenu)
      window.removeEventListener('scroll', positionMenu, true)
      document.removeEventListener('pointerdown', closeFromOutside)
      document.removeEventListener('keydown', closeFromEscape)
    }
  }, [activeTool, open])

  function moveFocus(nextIndex: number) {
    const normalized = (nextIndex + ANNOTATION_TOOLS.length) % ANNOTATION_TOOLS.length
    setFocusIndex(normalized)
    itemRefs.current[normalized]?.focus()
  }

  function select(tool: MarkupTool) {
    if (disabledReason !== null) return
    onSelect(tool)
    setOpen(false)
    triggerRef.current?.focus()
  }

  return (
    <div className="annotation-tool-menu">
      <ViewerButton
        ref={triggerRef}
        leadingIcon="pencil"
        tone="quiet"
        active={activeTool !== 'browse'}
        aria-haspopup="menu"
        aria-expanded={open}
        title={disabledReason ?? '选择标记工具'}
        onClick={() => setOpen((value) => !value)}
      >
        标记
      </ViewerButton>
      {open && (
        <div
          ref={menuRef}
          className="annotation-tool-menu__popover"
          role="menu"
          aria-label="标记工具"
          aria-describedby={disabledReason === null ? undefined : disabledMessageId}
          style={position}
          onKeyDown={(event) => {
            if (event.key === 'ArrowDown') {
              event.preventDefault()
              moveFocus(focusIndex + 1)
            } else if (event.key === 'ArrowUp') {
              event.preventDefault()
              moveFocus(focusIndex - 1)
            } else if (event.key === 'Home') {
              event.preventDefault()
              moveFocus(0)
            } else if (event.key === 'End') {
              event.preventDefault()
              moveFocus(ANNOTATION_TOOLS.length - 1)
            }
          }}
        >
          {disabledReason !== null && (
            <p id={disabledMessageId} className="annotation-tool-menu__notice" role="status">
              {disabledReason}
            </p>
          )}
          {ANNOTATION_TOOLS.map((tool, index) => (
            <button
              key={tool.id}
              ref={(element) => {
                itemRefs.current[index] = element
              }}
              type="button"
              className="annotation-tool-menu__item"
              role="menuitemradio"
              aria-checked={activeTool === tool.id}
              aria-disabled={disabledReason === null ? undefined : 'true'}
              aria-keyshortcuts={tool.shortcut}
              tabIndex={focusIndex === index ? 0 : -1}
              onClick={() => select(tool.id)}
              onFocus={() => setFocusIndex(index)}
            >
              <ViewerIcon name={tool.icon} />
              <span>{tool.label}</span>
              <kbd>{tool.shortcut}</kbd>
            </button>
          ))}
        </div>
      )}
    </div>
  )
}

function clamp(value: number, minimum: number, maximum: number) {
  return Math.max(minimum, Math.min(maximum, value))
}
