import type { ReviewAnchor } from '../../api/types'
import type {
  ImageRendererAnnotation,
  ImageRendererAnnotationGeometry,
  ImageRendererEvent,
  ImageRendererScene,
} from '../../rendering/imageRendererTypes'
import type { AnnotationEditorAction, AnnotationInteractionState } from './annotationModel'
import type { ImageReviewWorkbenchFeedback } from './imageReviewWorkbenchAdapter'

const REVIEW_COLOR: [number, number, number, number] = [0.7, 0.13, 0.09, 1]

export interface NativeReviewSceneVersion {
  revision: number
  clientMutationId: string | null
}

export function toNativeScene(
  feedback: ReadonlyArray<ImageReviewWorkbenchFeedback>,
  editor: AnnotationInteractionState,
  version: NativeReviewSceneVersion,
): ImageRendererScene {
  const phase = editor.phase
  const annotations = feedback
    .flatMap((item) => {
      const geometry = reviewAnchorToNativeGeometry(item.anchor)
      if (geometry === null || item.ordinal === null) return []
      const replacingGeometry =
        phase.status !== 'idle' &&
        phase.status !== 'drawing' &&
        phase.sourceItemId === item.itemId &&
        phase.operation === 'geometry'
      const replacement = replacingGeometry ? reviewAnchorToNativeGeometry(phase.draftAnchor) : null
      return [
        annotationNode({
          id: item.itemId,
          ordinal: item.ordinal,
          geometry: replacement ?? geometry,
          selected: editor.selectedItemId === item.itemId,
          draft: replacingGeometry,
          dashed: replacingGeometry && phase.status !== 'saving',
        }),
      ]
    })
    .sort((left, right) => left.ordinal - right.ordinal || left.id.localeCompare(right.id))

  if (phase.status === 'saving' && phase.sourceItemId === null) {
    const geometry = reviewAnchorToNativeGeometry(phase.draftAnchor)
    if (geometry !== null) {
      annotations.push(
        annotationNode({
          id: `provisional:${version.clientMutationId ?? version.revision}`,
          ordinal: nextOrdinal(annotations),
          geometry,
          selected: true,
          draft: true,
          dashed: false,
        }),
      )
    }
  }

  const draftGeometry =
    phase.status === 'idle' ||
    (phase.status === 'saving' && phase.sourceItemId === null) ||
    (phase.status !== 'drawing' && phase.sourceItemId !== null)
      ? null
      : reviewAnchorToNativeGeometry(phase.draftAnchor)

  return {
    annotations,
    draft:
      draftGeometry === null
        ? null
        : annotationNode({
            id: `draft:${version.clientMutationId ?? version.revision}`,
            ordinal: 0,
            geometry: draftGeometry,
            selected: false,
            draft: true,
            dashed: phase.status !== 'saving',
          }),
  }
}

export function reviewAnchorToNativeGeometry(
  anchor: ReviewAnchor,
): ImageRendererAnnotationGeometry | null {
  switch (anchor.kind) {
    case 'image_point':
      return { type: 'point', position: { x: anchor.x, y: anchor.y } }
    case 'image_arrow':
      return { type: 'arrow', tail: { ...anchor.tail }, head: { ...anchor.head } }
    case 'image_rect':
      return {
        type: 'rectangle',
        rect: { x: anchor.x, y: anchor.y, width: anchor.width, height: anchor.height },
      }
    case 'image_ellipse':
      return {
        type: 'ellipse',
        rect: { x: anchor.x, y: anchor.y, width: anchor.width, height: anchor.height },
      }
    case 'image_stroke':
      return { type: 'stroke', points: anchor.points.map((point) => ({ ...point })) }
    case 'asset':
    case 'video_point':
    case 'video_range':
      return null
  }
}

export function nativeGeometryToReviewAnchor(
  geometry: ImageRendererAnnotationGeometry,
): ReviewAnchor {
  switch (geometry.type) {
    case 'point':
      return { kind: 'image_point', x: geometry.position.x, y: geometry.position.y }
    case 'arrow':
      return { kind: 'image_arrow', tail: { ...geometry.tail }, head: { ...geometry.head } }
    case 'rectangle':
      return { kind: 'image_rect', ...geometry.rect }
    case 'ellipse':
      return { kind: 'image_ellipse', ...geometry.rect }
    case 'stroke':
      return { kind: 'image_stroke', points: geometry.points.map((point) => ({ ...point })) }
  }
}

export function toAnnotationAction(event: ImageRendererEvent): AnnotationEditorAction | null {
  switch (event.type) {
    case 'draft_started':
      return { type: 'begin_drawing', anchor: nativeGeometryToReviewAnchor(event.geometry) }
    case 'draft_changed':
    case 'draft_completed':
      return { type: 'update_draft_anchor', anchor: nativeGeometryToReviewAnchor(event.geometry) }
    case 'draft_cancelled':
      return { type: 'cancel_draft' }
    case 'selection_changed':
      return { type: 'select_feedback', itemId: event.annotationId }
    case 'ready':
    case 'frame_presented':
    case 'camera_changed':
    case 'editor_placement_changed':
    case 'recovering':
    case 'backend_activated':
    case 'failed':
      return null
  }
}

function annotationNode(input: {
  id: string
  ordinal: number
  geometry: ImageRendererAnnotationGeometry
  selected: boolean
  draft: boolean
  dashed: boolean
}): ImageRendererAnnotation {
  return {
    id: input.id,
    ordinal: input.ordinal,
    geometry: input.geometry,
    style: { color: REVIEW_COLOR, lineWidthPx: 2, dashed: input.dashed },
    selected: input.selected,
    draft: input.draft,
    visible: true,
  }
}

function nextOrdinal(annotations: ReadonlyArray<ImageRendererAnnotation>): number {
  return annotations.reduce((maximum, annotation) => Math.max(maximum, annotation.ordinal), 0) + 1
}
