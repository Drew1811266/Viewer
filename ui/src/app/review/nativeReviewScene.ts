import type { ReviewAnchor, ReviewAnnotationStyle } from '../../api/types'
import type {
  ImageRendererAnnotation,
  ImageRendererAnnotationGeometry,
  ImageRendererEvent,
  ImageRendererScene,
} from '../../rendering/imageRendererTypes'
import type { AnnotationEditorAction, AnnotationInteractionState } from './annotationModel'
import {
  annotationColorHex,
  annotationLineWidthCss,
  defaultAnnotationStyle,
  styleForAnchor,
} from './annotationStyle'
import type { ImageReviewWorkbenchFeedback } from './imageReviewWorkbenchAdapter'

// The native renderer targets a Bgra8UnormSrgb surface, so vertex colors must
// be linear-encoded; an sRGB value written as-is gets gamma-encoded a second
// time and appears washed out (pale salmon instead of coral).
const srgbChannelToLinear = (channel: number): number =>
  channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4

export interface NativeReviewSceneVersion {
  revision: number
  clientMutationId: string | null
  annotationsEditable?: boolean
  /** Authored image size, used to keep a 4px corner round in image space. */
  imageSize?: { width: number; height: number }
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
        phase.sourceItemId === item.itemId &&
        (phase.status === 'drawing' || phase.operation === 'geometry')
      const replacement = replacingGeometry ? reviewAnchorToNativeGeometry(phase.draftAnchor) : null
      return [
        annotationNode({
          id: item.itemId,
          ordinal: item.ordinal,
          geometry: replacement ?? geometry,
          selected: editor.selectedItemId === item.itemId,
          draft: replacingGeometry,
          dashed: replacingGeometry && phase.status !== 'saving',
          style:
            replacingGeometry && phase.status !== 'saving'
              ? phase.style
              : (styleForAnchor(item.anchor, item.style) ?? undefined),
          cornerRadius: cornerRadiusFor(geometry, version.imageSize),
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
          style: phase.style,
          cornerRadius: cornerRadiusFor(geometry, version.imageSize),
        }),
      )
    }
  }

  const draftGeometry =
    phase.status === 'idle' ||
    (phase.status === 'drawing' && phase.sourceItemId !== undefined) ||
    (phase.status === 'saving' && phase.sourceItemId === null) ||
    (phase.status !== 'drawing' && phase.sourceItemId !== null)
      ? null
      : reviewAnchorToNativeGeometry(phase.draftAnchor)

  return {
    annotations,
    annotationsEditable: version.annotationsEditable !== false && phase.status === 'idle',
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
            style: phase.status === 'idle' ? undefined : phase.style,
            cornerRadius: cornerRadiusFor(draftGeometry, version.imageSize),
          }),
  }
}

export function reviewAnchorToNativeGeometry(
  anchor: ReviewAnchor,
): ImageRendererAnnotationGeometry | null {
  switch (anchor.kind) {
    case 'image_point':
    case 'image_text':
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
    case 'image_line':
      return {
        type: 'stroke',
        points: [anchor.tail, anchor.head].map((point) => ({ ...point })),
      }
    case 'image_polygon':
      return {
        type: 'stroke',
        points: [...anchor.points, anchor.points[0] as (typeof anchor.points)[number]].map(
          (point) => ({ ...point }),
        ),
      }
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
    case 'geometry_edit_started':
    case 'geometry_edit_changed':
    case 'geometry_edit_completed':
    case 'geometry_edit_cancelled':
    case 'frame_presented':
    case 'detail_availability_changed':
    case 'camera_changed':
    case 'editor_placement_changed':
    case 'recovering':
    case 'backend_activated':
    case 'failed':
      return null
  }
}

function cornerRadiusFor(
  geometry: ImageRendererAnnotationGeometry,
  imageSize: { width: number; height: number } | undefined,
): [number, number] | undefined {
  if (geometry.type !== 'rectangle' || imageSize === undefined) return undefined
  if (imageSize.width <= 0 || imageSize.height <= 0) return undefined
  return [4 / imageSize.width, 4 / imageSize.height]
}

function annotationNode(input: {
  id: string
  ordinal: number
  geometry: ImageRendererAnnotationGeometry
  selected: boolean
  draft: boolean
  dashed: boolean
  style?: ReviewAnnotationStyle
  cornerRadius?: [number, number]
}): ImageRendererAnnotation {
  const style = input.style ?? defaultAnnotationStyle('brush')
  return {
    id: input.id,
    ordinal: input.ordinal,
    geometry: input.geometry,
    style: {
      color: nativeStyleColor(style),
      lineWidthPx: annotationLineWidthCss(style),
      dashed: input.dashed || style.dashed,
      ...(style.tool === 'highlighter' ? { halo: false, multiply: true } : {}),
      ...(input.cornerRadius === undefined ? {} : { cornerRadius: input.cornerRadius }),
    },
    selected: input.selected,
    draft: input.draft,
    visible: true,
  }
}

function nativeStyleColor(style: ReviewAnnotationStyle): [number, number, number, number] {
  const hex = annotationColorHex(style.color)
  const alpha = Math.max(0.2, Math.min(1, style.opacityPercent / 100))
  return [
    srgbChannelToLinear(Number.parseInt(hex.slice(1, 3), 16) / 255),
    srgbChannelToLinear(Number.parseInt(hex.slice(3, 5), 16) / 255),
    srgbChannelToLinear(Number.parseInt(hex.slice(5, 7), 16) / 255),
    alpha,
  ]
}

function nextOrdinal(annotations: ReadonlyArray<ImageRendererAnnotation>): number {
  return annotations.reduce((maximum, annotation) => Math.max(maximum, annotation.ordinal), 0) + 1
}
