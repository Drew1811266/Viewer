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
import type { NativeAnnotationFocus } from './NativeAnnotationSemantics'
import NativeAnnotationSemantics from './NativeAnnotationSemantics'

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
  const geometryEditItemId = useRef<string | null>(null)
  const geometryEditStaged = useRef(false)
  const geometryEditTool = useRef(controller.tool)
  const sceneVersion = useRef({ signature: '', revision: 0 })
  const [editorPlacement, setEditorPlacement] = useState<ImageRendererPoint | null>(null)
  const [semanticFocus, setSemanticFocus] = useState<NativeAnnotationFocus | null>(null)
  const exclusionVersion = useRef({ signature: '', revision: 0 })
  const phase = controller.editor.phase
  const signature = JSON.stringify([
    controller.feedback,
    controller.editor,
    controller.pendingMutationId ?? null,
    controller.readOnlyReason,
  ])
  if (sceneVersion.current.signature !== signature) {
    sceneVersion.current = {
      signature,
      revision: sceneVersion.current.revision + 1,
    }
  }
  const revision = sceneVersion.current.revision
  const exclusionSignature = JSON.stringify([phase.status, revision, editorPlacement])
  if (exclusionVersion.current.signature !== exclusionSignature) {
    exclusionVersion.current = {
      signature: exclusionSignature,
      revision: exclusionVersion.current.revision + 1,
    }
  }
  const inputExclusionRevision = exclusionVersion.current.revision
  const scene = useMemo(
    () =>
      toNativeScene(controller.feedback, controller.editor, {
        revision,
        clientMutationId: controller.pendingMutationId ?? null,
        annotationsEditable: controller.readOnlyReason === null,
      }),
    [
      controller.editor,
      controller.feedback,
      controller.pendingMutationId,
      controller.readOnlyReason,
      revision,
    ],
  )

  const onEvent = useCallback((event: ImageRendererEvent) => {
    const current = controllerRef.current
    if (event.type === 'geometry_edit_started') {
      setSemanticFocus({
        itemId: event.annotationId,
        handle: event.handle === 'point' ? undefined : (event.handle ?? undefined),
      })
      geometryEditTool.current = current.tool
      if (current.readOnlyReason === null && current.editor.phase.status === 'idle') {
        geometryEditItemId.current = event.annotationId
        geometryEditStaged.current = false
      }
      return
    }
    if (event.type === 'geometry_edit_changed') {
      if (geometryEditItemId.current === event.annotationId) {
        geometryEditStaged.current = current.stageFeedbackAnchor(
          event.annotationId,
          nativeGeometryToReviewAnchor(event.geometry),
        )
      }
      return
    }
    if (event.type === 'geometry_edit_completed') {
      if (geometryEditItemId.current === event.annotationId) {
        // Completed guarantees native movement; it may arrive before a
        // coalesced Changed event. Stage that final candidate when necessary.
        const accepted =
          geometryEditStaged.current ||
          current.stageFeedbackAnchor(
            event.annotationId,
            nativeGeometryToReviewAnchor(event.geometry),
          )
        geometryEditItemId.current = null
        geometryEditStaged.current = false
        if (accepted)
          void current.replaceFeedbackAnchor(
            event.annotationId,
            nativeGeometryToReviewAnchor(event.geometry),
          )
      }
      return
    }
    if (event.type === 'geometry_edit_cancelled') {
      if (geometryEditItemId.current === event.annotationId) {
        geometryEditItemId.current = null
        if (geometryEditStaged.current) current.cancelDraft()
        geometryEditStaged.current = false
      }
      return
    }
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
          setSemanticFocus({ itemId: action.itemId })
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

  const geometryEditing =
    phase.status === 'drawing' && phase.sourceItemId === geometryEditItemId.current
  const binding = useMemo<ImageRendererViewportBinding>(
    () => ({
      sceneRevision: revision,
      scene,
      tool: geometryEditing
        ? geometryEditTool.current
        : controller.editor.temporarilyPanning ||
            (phase.status !== 'idle' && phase.status !== 'drawing')
          ? 'browse'
          : controller.tool,
      inputExclusionRevision,
      onEvent,
    }),
    [
      controller.editor.temporarilyPanning,
      controller.tool,
      geometryEditing,
      inputExclusionRevision,
      onEvent,
      phase.status,
      revision,
      scene,
    ],
  )

  const editor = (projection: ImagePreviewProjection) => (
    <>
      <NativeAnnotationSemantics
        controller={controller}
        projection={projection}
        focusRequest={semanticFocus}
      />
      {phase.status !== 'idle' &&
      phase.status !== 'drawing' &&
      phase.sourceItemId === null &&
      phase.draftAnchor.kind !== 'asset' ? (
        <InlineFeedbackEditor
          controller={controller}
          anchor={phase.draftAnchor}
          projection={projection}
          nativePosition={editorPlacement}
        />
      ) : null}
    </>
  )

  return children({ binding, editor })
}
