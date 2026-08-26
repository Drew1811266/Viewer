import { useEffect, useRef } from 'react'
import type {
  ImageReviewWorkbenchController,
  ReviewAnchor,
} from '../../app/review/useImageReviewWorkbench'
import type { ImagePreviewProjection } from '../imagePreview/ImagePreviewSurface'
import ViewerButton from '../ui/ViewerButton'

interface InlineFeedbackEditorProps {
  controller: ImageReviewWorkbenchController
  anchor: ReviewAnchor
  projection?: ImagePreviewProjection
  embedded?: boolean
}

export default function InlineFeedbackEditor({
  controller,
  anchor,
  projection,
  embedded = false,
}: InlineFeedbackEditorProps) {
  const input = useRef<HTMLTextAreaElement>(null)
  const previousFocus = useRef<HTMLElement | null>(null)
  const editor = controller.editor
  const text = editor.status === 'idle' || editor.status === 'drawing' ? '' : editor.text
  const saving = editor.status === 'saving'
  const error = editor.status === 'save_error' ? editor.message : null

  useEffect(() => {
    previousFocus.current =
      document.activeElement instanceof HTMLElement ? document.activeElement : null
    input.current?.focus()
    return () => {
      if (previousFocus.current?.isConnected) previousFocus.current.focus()
    }
  }, [])

  return (
    <section
      className="inline-feedback-editor"
      data-embedded={embedded || undefined}
      aria-label={anchor.kind === 'asset' ? '整图意见编辑器' : '标注意见编辑器'}
      style={embedded ? undefined : editorPosition(anchor, projection)}
    >
      <label>
        <span>返工意见</span>
        <textarea
          ref={input}
          aria-label="标注意见"
          value={text}
          disabled={saving}
          onChange={(event) => controller.updateDraftText(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === 'Enter' && (event.metaKey || event.ctrlKey)) {
              event.preventDefault()
              void controller.saveDraft()
            }
          }}
        />
      </label>
      {error !== null && (
        <p className="inline-feedback-editor__error" role="status" aria-live="polite">
          {error}
        </p>
      )}
      <div>
        <ViewerButton
          tone="primary"
          loading={saving}
          disabled={text.trim().length === 0}
          onClick={() => void controller.saveDraft()}
        >
          保存
        </ViewerButton>
        <ViewerButton tone="quiet" disabled={saving} onClick={controller.cancelDraft}>
          取消
        </ViewerButton>
      </div>
    </section>
  )
}

function editorPosition(anchor: ReviewAnchor, projection?: ImagePreviewProjection) {
  if (projection === undefined) return undefined
  const point = anchorPoint(anchor)
  if (point === null) return undefined
  const projected = projection.normalizedToStage(point)
  if (projected === null) return undefined
  const left = clamp(
    projected.x - projection.stageRect.left + 16,
    8,
    Math.max(8, projection.stageRect.width - 288),
  )
  const top = clamp(
    projected.y - projection.stageRect.top + 16,
    8,
    Math.max(8, projection.stageRect.height - 176),
  )
  return { left, top }
}

function anchorPoint(anchor: ReviewAnchor) {
  if (anchor.kind === 'image_rect') {
    return { x: anchor.x + anchor.width / 2, y: anchor.y + anchor.height / 2 }
  }
  if (anchor.kind === 'image_stroke') return anchor.points.at(-1) ?? null
  return null
}

function clamp(value: number, minimum: number, maximum: number) {
  return Math.max(minimum, Math.min(maximum, value))
}
