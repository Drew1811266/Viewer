import { useEffect } from 'react'
import {
  type AnnotationToolDescriptor,
  toolForShortcut,
  toolsForProtocol,
} from '../../app/review/annotationToolRegistry'
import type { ImageReviewWorkbenchController } from '../../app/review/useImageReviewWorkbench'
import ViewerButton from '../ui/ViewerButton'
import AnnotationToolMenu from './AnnotationToolMenu'

interface AnnotationToolbarProps {
  controller: ImageReviewWorkbenchController
}

export default function AnnotationToolbar({ controller }: AnnotationToolbarProps) {
  const readOnly = controller.readOnlyReason !== null
  const tools = toolsForProtocol(controller.protocol)

  useEffect(() => {
    function keyDown(event: KeyboardEvent) {
      if (event.defaultPrevented || event.isComposing) return
      const commandEnter = event.key === 'Enter' && (event.metaKey || event.ctrlKey)
      if (commandEnter) {
        if (controller.dirty) {
          event.preventDefault()
          void controller.saveDraft()
        }
        return
      }
      if (event.key === 'Escape') {
        // Escape is layered so one key never skips straight from editing to
        // leaving the review: cancel the draft, then clear the selection,
        // then leave the markup tool, and only then return to the grid. It
        // also has to run for text inputs, which is where a draft is typed.
        event.preventDefault()
        if (controller.dirty) {
          controller.cancelDraft()
          return
        }
        if (controller.selectedItemId !== null) {
          controller.selectFeedback(null)
          return
        }
        if (controller.tool !== 'browse') {
          controller.setTool('browse')
          return
        }
        void controller.requestLeave({ kind: 'return_grid' })
        return
      }
      if (ownsTextInput(event.target)) return
      if (
        (event.key === 'Delete' || event.key === 'Backspace') &&
        !event.metaKey &&
        !event.ctrlKey &&
        !event.altKey &&
        controller.selectedItemId !== null &&
        !controller.dirty &&
        !readOnly
      ) {
        event.preventDefault()
        void controller.deleteFeedback(controller.selectedItemId)
        return
      }
      const command = event.metaKey || event.ctrlKey
      if (command && !event.altKey) {
        const key = event.key.toLowerCase()
        if (key === 'z') {
          event.preventDefault()
          if (event.shiftKey) controller.redo()
          else controller.undo()
          return
        }
        if (key === 'y' && !event.shiftKey) {
          event.preventDefault()
          controller.redo()
          return
        }
      }
      if (event.code === 'Space' && !event.repeat) {
        event.preventDefault()
        controller.setTemporaryPan(true)
        return
      }
      if (readOnly || event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return
      const tool = shortcutTool(event.key, tools)
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
  }, [controller, readOnly, tools])

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
      <AnnotationToolMenu
        activeTool={controller.tool}
        tools={tools}
        disabledReason={annotationDisabledReason(controller)}
        onSelect={controller.setTool}
      />
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

function shortcutTool(key: string, tools: ReadonlyArray<AnnotationToolDescriptor>) {
  if (key.toLowerCase() === 'v') return 'browse' as const
  return toolForShortcut(key, tools)?.id ?? null
}

function ownsTextInput(target: EventTarget | null): boolean {
  return (
    target instanceof HTMLInputElement ||
    target instanceof HTMLTextAreaElement ||
    target instanceof HTMLSelectElement ||
    (target instanceof HTMLElement && target.isContentEditable)
  )
}

function annotationDisabledReason(controller: ImageReviewWorkbenchController): string | null {
  if (controller.readOnlyReason !== null) return '当前素材暂时不可修改标记。'
  if (controller.dirty) {
    const phase = controller.editor.phase
    if (phase.status === 'drawing' && phase.draftAnchor.kind === 'image_polygon') {
      return '多边形尚未完成：按 Enter / 双击 / 点回起点完成，按 Esc 取消后再切换工具。'
    }
    return '请先保存或取消当前意见，再切换标记工具。'
  }
  return null
}
