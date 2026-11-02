import type { ReactNode } from 'react'
import { useCallback, useMemo, useRef, useState } from 'react'
import { type ReviewAnchor, withAnnotationTool } from '../../app/review/annotationStyle'
import {
  nativeGeometryToReviewAnchor,
  toAnnotationAction,
  toNativeScene,
} from '../../app/review/nativeReviewScene'
import type { ImageReviewWorkbenchController } from '../../app/review/useImageReviewWorkbench'
import type {
  ImageRendererEvent,
  ImageRendererPoint,
  ImageRendererTool,
  ImageRendererViewportBinding,
} from '../../rendering/imageRendererTypes'
import type { ImagePreviewProjection } from '../imagePreview/ImagePreviewSurface'
import AnnotationOverlayDetails from './AnnotationOverlayDetails'
import AnnotationStyleBar, { shouldShowAnnotationStyleBar } from './AnnotationStyleBar'
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
  const lastGeometryEdit = useRef<{ itemId: string; at: number } | null>(null)
  const lastSelectionClick = useRef<{ itemId: string; at: number } | null>(null)
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
        imageSize: controller.preparedImage
          ? {
              width: controller.preparedImage.width,
              height: controller.preparedImage.height,
            }
          : undefined,
      }),
    [
      controller.editor,
      controller.feedback,
      controller.pendingMutationId,
      controller.preparedImage,
      controller.readOnlyReason,
      revision,
    ],
  )

  const onEvent = useCallback((event: ImageRendererEvent) => {
    const current = controllerRef.current
    if (event.type === 'selection_changed') {
      const annotationId = event.annotationId
      if (annotationId === null) {
        lastSelectionClick.current = null
      } else {
        const now = performance.now()
        const repeated =
          lastSelectionClick.current?.itemId === annotationId &&
          now - lastSelectionClick.current.at < 400
        lastSelectionClick.current = { itemId: annotationId, at: now }
        if (repeated && current.readOnlyReason === null && current.editor.phase.status === 'idle') {
          lastSelectionClick.current = null
          current.beginFeedbackTextEdit(annotationId)
          return
        }
      }
    }
    if (event.type === 'geometry_edit_started') {
      const now = performance.now()
      const repeated =
        lastGeometryEdit.current?.itemId === event.annotationId &&
        now - lastGeometryEdit.current.at < 400
      lastGeometryEdit.current = { itemId: event.annotationId, at: now }
      lastSelectionClick.current = null
      if (repeated && current.readOnlyReason === null && current.editor.phase.status === 'idle') {
        lastSelectionClick.current = null
        current.beginFeedbackTextEdit(event.annotationId)
        return
      }
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
          nativeEditAnchor(
            nativeGeometryToReviewAnchor(event.geometry),
            current.feedback.find((item) => item.itemId === event.annotationId)?.anchor,
          ),
        )
      }
      return
    }
    if (event.type === 'geometry_edit_completed') {
      if (geometryEditItemId.current === event.annotationId) {
        // Completed guarantees native movement; it may arrive before a
        // coalesced Changed event. Stage that final candidate when necessary.
        const anchor = nativeEditAnchor(
          nativeGeometryToReviewAnchor(event.geometry),
          current.feedback.find((item) => item.itemId === event.annotationId)?.anchor,
        )
        const accepted =
          geometryEditStaged.current || current.stageFeedbackAnchor(event.annotationId, anchor)
        geometryEditItemId.current = null
        geometryEditStaged.current = false
        if (accepted) void current.replaceFeedbackAnchor(event.annotationId, anchor)
      }
      return
    }
    if (event.type === 'geometry_edit_cancelled') {
      lastGeometryEdit.current = null
      lastSelectionClick.current = null
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
          current.beginDrawing(
            nativeDrawingAnchor(action.anchor, current.tool),
            current.redrawItemId ?? undefined,
            withAnnotationTool(current.annotationStyle, toolWithoutBrowse(current.tool)),
          )
          break
        case 'update_draft_anchor':
          current.updateDraftAnchor(nativeDrawingAnchor(action.anchor, current.tool))
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
      void current.finishDrawing(
        nativeDrawingAnchor(nativeGeometryToReviewAnchor(event.geometry), current.tool),
      )
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
        ? nativeToolFor(geometryEditTool.current)
        : controller.editor.temporarilyPanning ||
            (phase.status !== 'idle' && phase.status !== 'drawing')
          ? 'browse'
          : nativeToolFor(controller.tool),
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
      <AnnotationOverlayDetails controller={controller} projection={projection} />
      {shouldShowAnnotationStyleBar(controller) && <AnnotationStyleBar controller={controller} />}
      {phase.status !== 'idle' &&
      phase.status !== 'drawing' &&
      phase.sourceItemId === null &&
      phase.draftAnchor.kind !== 'asset' ? (
        <InlineFeedbackEditor
          controller={controller}
          anchor={phase.draftAnchor}
          projection={projection}
          nativePosition={editorPlacement}
          topInset={shouldShowAnnotationStyleBar(controller) ? 64 : 8}
        />
      ) : null}
    </>
  )

  return children({ binding, editor })
}

function nativeToolFor(tool: ImageReviewWorkbenchController['tool']): ImageRendererTool {
  switch (tool) {
    case 'browse':
    case 'polygon':
      // Polygon is a multi-click Canvas gesture; ImageReviewWorkspace routes
      // that tool to the Web viewport so native input stays in browse mode.
      return 'browse'
    case 'point':
    case 'text':
      return 'point'
    case 'arrow':
    case 'line':
    case 'ruler':
      return 'arrow'
    case 'brush':
    case 'highlighter':
      return 'brush'
    case 'rectangle':
      return 'rectangle'
    case 'ellipse':
      return 'ellipse'
  }
}

function toolWithoutBrowse(
  tool: ImageReviewWorkbenchController['tool'],
): Exclude<ImageReviewWorkbenchController['tool'], 'browse'> {
  return tool === 'browse' ? 'brush' : tool
}

function nativeDrawingAnchor(
  anchor: ReviewAnchor,
  tool: ImageReviewWorkbenchController['tool'],
): ReviewAnchor {
  if (tool === 'text' && anchor.kind === 'image_point')
    return { kind: 'image_text', x: anchor.x, y: anchor.y }
  if ((tool === 'line' || tool === 'ruler') && anchor.kind === 'image_arrow')
    return { kind: 'image_line', tail: anchor.tail, head: anchor.head }
  return anchor
}

function nativeEditAnchor(anchor: ReviewAnchor, original: ReviewAnchor | undefined): ReviewAnchor {
  if (original?.kind === 'image_text' && anchor.kind === 'image_point')
    return { kind: 'image_text', x: anchor.x, y: anchor.y }
  return anchor
}
