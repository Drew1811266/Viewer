import { useEffect } from 'react'
import type { ImageReviewWorkbenchController } from '../../app/review/useImageReviewWorkbench'
import ViewerButton from '../ui/ViewerButton'

interface AnnotationToolbarProps {
  controller: ImageReviewWorkbenchController
}

export default function AnnotationToolbar({ controller }: AnnotationToolbarProps) {
  const readOnly = controller.readOnlyReason !== null

  useEffect(() => {
    function keyDown(event: KeyboardEvent) {
      if (event.defaultPrevented) return
      const commandEnter = event.key === 'Enter' && (event.metaKey || event.ctrlKey)
      if (commandEnter) {
        if (controller.dirty) {
          event.preventDefault()
          void controller.saveDraft()
        }
        return
      }
      if (ownsTextInput(event.target)) return
      if (event.key === 'Escape') {
        if (controller.dirty) {
          event.preventDefault()
          controller.cancelDraft()
        } else {
          controller.setTool('browse')
        }
        return
      }
      if (event.code === 'Space' && !event.repeat) {
        event.preventDefault()
        controller.setTemporaryPan(true)
        return
      }
      if (readOnly || event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return
      const tool = shortcutTool(event.key)
      if (tool !== null) {
        event.preventDefault()
        controller.setTool(tool)
      }
    }

    function keyUp(event: KeyboardEvent) {
      if (event.code !== 'Space') return
      controller.setTemporaryPan(false)
    }

    window.addEventListener('keydown', keyDown)
    window.addEventListener('keyup', keyUp)
    return () => {
      window.removeEventListener('keydown', keyDown)
      window.removeEventListener('keyup', keyUp)
    }
  }, [controller, readOnly])

  return (
    <div className="annotation-toolbar" role="toolbar" aria-label="图片标注工具">
      <ViewerButton
        leadingIcon="move"
        tone="quiet"
        active={controller.tool === 'browse'}
        onClick={() => controller.setTool('browse')}
      >
        浏览
      </ViewerButton>
      <ViewerButton
        leadingIcon="pencil"
        tone="quiet"
        active={controller.tool === 'brush'}
        disabled={readOnly}
        onClick={() => controller.setTool('brush')}
      >
        画笔
      </ViewerButton>
      <ViewerButton
        leadingIcon="maximize"
        tone="quiet"
        active={controller.tool === 'rectangle'}
        disabled={readOnly}
        onClick={() => controller.setTool('rectangle')}
      >
        矩形
      </ViewerButton>
      <ViewerButton
        leadingIcon="panel-right"
        tone="quiet"
        active={controller.railOpen}
        onClick={() => controller.setRailOpen(!controller.railOpen)}
      >
        意见栏
      </ViewerButton>
    </div>
  )
}

function shortcutTool(key: string) {
  if (key.toLowerCase() === 'v') return 'browse' as const
  if (key.toLowerCase() === 'b') return 'brush' as const
  if (key.toLowerCase() === 'r') return 'rectangle' as const
  return null
}

function ownsTextInput(target: EventTarget | null): boolean {
  return (
    target instanceof HTMLInputElement ||
    target instanceof HTMLTextAreaElement ||
    target instanceof HTMLSelectElement ||
    (target instanceof HTMLElement && target.isContentEditable)
  )
}
