import type { ReactNode } from 'react'
import { useCallback, useMemo, useRef, useState } from 'react'
import {
  nativeGeometryToReviewAnchor,
  toAnnotationAction,
  toNativeScene,
} from '../../app/review/nativeReviewScene'
import type { ImageReviewWorkbenchController } from '../../app/review/useImageReviewWorkbench'
import type {
  ImageRendererEvent,
  ImageRendererPoint,
  ImageRendererViewportBinding,
} from '../../rendering/imageRendererTypes'
import type { ImagePreviewProjection } from '../imagePreview/ImagePreviewSurface'
import InlineFeedbackEditor from './InlineFeedbackEditor'

export interface NativeReviewViewportBridgeValue {
  binding: ImageRendererViewportBinding
  editor(projection: ImagePreviewProjection): ReactNode
}

interface NativeReviewViewportBridgeProps {
  controller: ImageReviewWorkbenchController
  children(value: NativeReviewViewportBridgeValue): ReactNode
}

export default function NativeReviewViewportBridge({
  controller,
  children,
}: NativeReviewViewportBridgeProps) {
  const controllerRef = useRef(controller)
  controllerRef.current = controller
  const sceneVersion = useRef({ signature: '', revision: 0 })
  const [editorPlacement, setEditorPlacement] = useState<ImageRendererPoint | null>(null)
  const phase = controller.editor.phase
  const signature = JSON.stringify([
    controller.feedback,
    controller.editor,
    controller.pendingMutationId ?? null,
  ])
  if (sceneVersion.current.signature !== signature) {
    sceneVersion.current = {
      signature,
      revision: sceneVersion.current.revision + 1,
    }
  }
  const revision = sceneVersion.current.revision
  const scene = useMemo(
    () =>
      toNativeScene(controller.feedback, controller.editor, {
        revision,
        clientMutationId: controller.pendingMutationId ?? null,
      }),
    [controller.editor, controller.feedback, controller.pendingMutationId, revision],
  )

  const onEvent = useCallback((event: ImageRendererEvent) => {
    const current = controllerRef.current
    const action = toAnnotationAction(event)
    if (action !== null) {
      switch (action.type) {
        case 'begin_drawing':
          current.beginDrawing(action.anchor, current.redrawItemId ?? undefined)
          break
        case 'update_draft_anchor':
          current.updateDraftAnchor(action.anchor)
          break
        case 'cancel_draft':
          current.cancelDraft()
          break
        case 'select_feedback':
          current.selectFeedback(action.itemId)
          break
        default:
          break
      }
    }
    if (event.type === 'draft_completed') {
      void current.finishDrawing(nativeGeometryToReviewAnchor(event.geometry))
    } else if (event.type === 'editor_placement_changed') {
      setEditorPlacement(event.position)
    } else if (event.type === 'draft_started') {
      setEditorPlacement(null)
    }
  }, [])

  const binding = useMemo<ImageRendererViewportBinding>(
    () => ({
      sceneRevision: revision,
      scene,
      tool:
        controller.editor.temporarilyPanning ||
        (phase.status !== 'idle' && phase.status !== 'drawing')
          ? 'browse'
          : controller.tool,
      inputExclusionRevision: phase.status === 'idle' || phase.status === 'drawing' ? 0 : revision,
      onEvent,
    }),
    [controller.editor.temporarilyPanning, controller.tool, onEvent, phase.status, revision, scene],
  )

  const editor = (projection: ImagePreviewProjection) =>
    phase.status !== 'idle' &&
    phase.status !== 'drawing' &&
    phase.sourceItemId === null &&
    phase.draftAnchor.kind !== 'asset' ? (
      <InlineFeedbackEditor
        controller={controller}
        anchor={phase.draftAnchor}
        projection={projection}
        nativePosition={editorPlacement}
      />
    ) : null

  return children({ binding, editor })
}
