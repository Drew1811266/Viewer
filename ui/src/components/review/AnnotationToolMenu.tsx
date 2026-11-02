import { useId, useLayoutEffect, useMemo, useRef, useState } from 'react'
import type {
  AnnotationTool,
  AnnotationToolDescriptor,
} from '../../app/review/annotationToolRegistry'
import { ANNOTATION_TOOLS } from '../../app/review/annotationToolRegistry'
import ViewerButton from '../ui/ViewerButton'
import ViewerIcon from '../ui/ViewerIcon'

interface AnnotationToolMenuProps {
  activeTool: AnnotationTool
  disabledReason?: string | null
  tools?: ReadonlyArray<AnnotationToolDescriptor>
  onSelect(tool: AnnotationTool): void
}

interface MenuPosition {
  left: number
  top: number
}

const VIEWPORT_GUTTER = 8
const MENU_GAP = 6
const MENU_COLUMNS = 4

export default function AnnotationToolMenu({
  activeTool,
  disabledReason = null,
  tools = ANNOTATION_TOOLS,
  onSelect,
}: AnnotationToolMenuProps) {
  const [open, setOpen] = useState(false)
  const [focusIndex, setFocusIndex] = useState(0)
  const [position, setPosition] = useState<MenuPosition>({ left: VIEWPORT_GUTTER, top: 0 })
  const triggerRef = useRef<HTMLButtonElement>(null)
  const menuRef = useRef<HTMLDivElement>(null)
  const itemRefs = useRef<Array<HTMLButtonElement | null>>([])
  const disabledMessageId = useId()
  const entries = useMemo(() => {
    const ordered = [
      'browse',
      'point',
      'arrow',
      'line',
      'brush',
      'highlighter',
      'rectangle',
      'ellipse',
      'polygon',
      'text',
      'ruler',
    ] as const
    const browse = {
      id: 'browse' as const,
      label: '选择',
      shortcut: 'V',
      icon: 'move' as const,
      gesture: 'click' as const,
      position: -1,
    }
    return ordered
      .map((id) => (id === 'browse' ? browse : tools.find((tool) => tool.id === id)))
      .filter((entry): entry is NonNullable<typeof entry> => entry !== undefined)
  }, [tools])
  const groups = useMemo(
    () =>
      [
        { label: '基础', ids: ['browse', 'point', 'arrow', 'line'] },
        { label: '绘制', ids: ['brush', 'highlighter', 'rectangle', 'ellipse'] },
        { label: '标注', ids: ['polygon', 'text', 'ruler'] },
      ]
        .map((group) => ({
          label: group.label,
          tools: group.ids
            .map((id) => entries.find((entry) => entry.id === id))
            .filter((entry): entry is (typeof entries)[number] => entry !== undefined),
        }))
        .filter((group) => group.tools.length > 0),
    [entries],
  )

  useLayoutEffect(() => {
    if (!open) return
    const index = Math.max(
      0,
      entries.findIndex((tool) => tool.id === activeTool),
    )
    setFocusIndex(index)
    itemRefs.current[index]?.focus()

    function positionMenu() {
      const trigger = triggerRef.current
      const menu = menuRef.current
      if (trigger === null || menu === null) return
      const triggerRect = trigger.getBoundingClientRect()
      const menuRect = menu.getBoundingClientRect()
      // Keep the palette on the canvas instead of covering the feedback rail.
      const railLeft = document
        .querySelector<HTMLElement>('.review-feedback-rail')
        ?.getBoundingClientRect().left
      const maximumRight = railLeft === undefined ? window.innerWidth : railLeft
      const maximumLeft = Math.max(VIEWPORT_GUTTER, maximumRight - menuRect.width - VIEWPORT_GUTTER)
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
  }, [activeTool, entries, open])

  function moveFocus(nextIndex: number) {
    const normalized = (nextIndex + entries.length) % entries.length
    setFocusIndex(normalized)
    itemRefs.current[normalized]?.focus()
  }

  function select(tool: AnnotationTool) {
    if (tool !== 'browse' && disabledReason !== null) return
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
          data-native-input-exclusion="true"
          role="menu"
          aria-label="标记工具"
          aria-describedby={disabledReason === null ? undefined : disabledMessageId}
          style={position}
          onKeyDown={(event) => {
            if (event.key === 'Escape') {
              event.preventDefault()
              event.stopPropagation()
              setOpen(false)
              triggerRef.current?.focus()
            } else if (event.key === 'ArrowDown') {
              event.preventDefault()
              moveFocus(focusIndex + MENU_COLUMNS)
            } else if (event.key === 'ArrowUp') {
              event.preventDefault()
              moveFocus(focusIndex - MENU_COLUMNS)
            } else if (event.key === 'ArrowRight') {
              event.preventDefault()
              moveFocus(focusIndex + 1)
            } else if (event.key === 'ArrowLeft') {
              event.preventDefault()
              moveFocus(focusIndex - 1)
            } else if (event.key === 'Home') {
              event.preventDefault()
              moveFocus(0)
            } else if (event.key === 'End') {
              event.preventDefault()
              moveFocus(entries.length - 1)
            }
          }}
        >
          <header className="annotation-tool-menu__header">
            <strong>标记工具</strong>
            <p>{activeToolInstruction(activeTool, tools)} · Esc 关闭</p>
          </header>
          {disabledReason !== null && (
            <p id={disabledMessageId} className="annotation-tool-menu__notice" role="status">
              {disabledReason}
            </p>
          )}
          {groups.map((group) => (
            <section className="annotation-tool-menu__group" key={group.label}>
              <h3>{group.label}</h3>
              <div className="annotation-tool-menu__grid">
                {group.tools.map((tool) => {
                  const index = entries.findIndex((entry) => entry.id === tool.id)
                  const disabled = tool.id !== 'browse' && disabledReason !== null
                  return (
                    <button
                      key={tool.id}
                      ref={(element) => {
                        itemRefs.current[index] = element
                      }}
                      type="button"
                      className="annotation-tool-menu__tool"
                      role="menuitemradio"
                      aria-checked={activeTool === tool.id}
                      aria-disabled={disabled ? 'true' : undefined}
                      aria-keyshortcuts={tool.shortcut}
                      tabIndex={focusIndex === index ? 0 : -1}
                      onClick={() => select(tool.id)}
                      onFocus={() => setFocusIndex(index)}
                    >
                      <ViewerIcon name={tool.icon} />
                      <span>{tool.label}</span>
                      <kbd>{tool.shortcut}</kbd>
                    </button>
                  )
                })}
              </div>
            </section>
          ))}
        </div>
      )}
    </div>
  )
}

function activeToolInstruction(
  activeTool: AnnotationTool,
  tools: ReadonlyArray<AnnotationToolDescriptor>,
): string {
  if (activeTool === 'polygon') return '连续点击添加顶点 · Enter/双击/点起点完成'
  return tools.find((tool) => tool.id === activeTool)?.instruction ?? '选择工具后在画布上操作'
}

function clamp(value: number, minimum: number, maximum: number) {
  return Math.max(minimum, Math.min(maximum, value))
}
