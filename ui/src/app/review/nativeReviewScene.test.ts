import { describe, expect, it } from 'vitest'
import type { ReviewAnchor } from '../../api/types'
import type { AnnotationInteractionState } from './annotationModel'
import type { ImageReviewWorkbenchFeedback } from './imageReviewWorkbenchAdapter'
import {
  nativeGeometryToReviewAnchor,
  reviewAnchorToNativeGeometry,
  toAnnotationAction,
  toNativeScene,
} from './nativeReviewScene'

const POINT: ReviewAnchor = { kind: 'image_point', x: 0.1, y: 0.2 }
const ARROW: ReviewAnchor = {
  kind: 'image_arrow',
  tail: { x: 0.1, y: 0.2 },
  head: { x: 0.7, y: 0.8 },
}
const RECTANGLE: ReviewAnchor = {
  kind: 'image_rect',
  x: 0.2,
  y: 0.3,
  width: 0.4,
  height: 0.5,
}
const ELLIPSE: ReviewAnchor = {
  kind: 'image_ellipse',
  x: 0.25,
  y: 0.35,
  width: 0.3,
  height: 0.4,
}
const STROKE: ReviewAnchor = {
  kind: 'image_stroke',
  points: [
    { x: 0.1, y: 0.2 },
    { x: 0.3, y: 0.5 },
    { x: 0.8, y: 0.7 },
  ],
}
const IMAGE_ANCHORS = [POINT, ARROW, RECTANGLE, ELLIPSE, STROKE]

describe('nativeReviewScene', () => {
  it('does not turn resource-detail status into an annotation mutation', () => {
    for (const available of [false, true]) {
      expect(
        toAnnotationAction({
          type: 'detail_availability_changed',
          sessionId: '1',
          assetGeneration: 2,
          resourceRevision: 3,
          available,
        }),
      ).toBeNull()
    }
  })

  it('disables new annotation gestures in readonly and non-idle editor phases', () => {
    const version = { revision: 2, clientMutationId: null, annotationsEditable: true }
    expect(toNativeScene([], idleEditor(), version).annotationsEditable).toBe(true)
    expect(
      toNativeScene([], idleEditor(), { ...version, annotationsEditable: false })
        .annotationsEditable,
    ).toBe(false)
    for (const phase of [
      { status: 'drawing' as const, draftAnchor: POINT, sourceItemId: 'first' },
      { status: 'editing' as const, draftAnchor: POINT, sourceItemId: 'first', text: '文字' },
      { status: 'saving' as const, draftAnchor: POINT, sourceItemId: 'first', text: '文字' },
      {
        status: 'save_error' as const,
        draftAnchor: POINT,
        sourceItemId: 'first',
        text: '文字',
        message: '保存失败',
      },
    ]) {
      expect(toNativeScene([], { ...idleEditor(), phase }, version).annotationsEditable).toBe(false)
    }
  })

  it('replaces the original node while staging an existing geometry edit', () => {
    const scene = toNativeScene(
      [item('first', 1, POINT)],
      {
        ...idleEditor(),
        selectedItemId: 'first',
        phase: {
          status: 'drawing',
          sourceItemId: 'first',
          draftAnchor: { kind: 'image_point', x: 0.4, y: 0.5 },
        },
      },
      { revision: 2, clientMutationId: null },
    )
    expect(scene.annotations).toHaveLength(1)
    expect(scene.annotations[0]).toMatchObject({
      id: 'first',
      selected: true,
      draft: true,
      geometry: { type: 'point', position: { x: 0.4, y: 0.5 } },
    })
    expect(scene.draft).toBeNull()
  })

  it('round-trips every image anchor without changing normalized coordinates or stroke order', () => {
    for (const anchor of IMAGE_ANCHORS) {
      const geometry = reviewAnchorToNativeGeometry(anchor)
      expect(geometry).not.toBeNull()
      if (geometry === null) throw new Error('expected image geometry')
      expect(nativeGeometryToReviewAnchor(geometry)).toEqual(anchor)
    }
  })

  it('sorts saved image feedback by ordinal and excludes whole-asset and video opinions', () => {
    const feedback: ImageReviewWorkbenchFeedback[] = [
      item('second', 2, RECTANGLE),
      item('asset', null, { kind: 'asset' }),
      item('first', 1, POINT),
      item('video', 3, { kind: 'video_point', positionUs: 20 }),
    ]

    const scene = toNativeScene(feedback, idleEditor(), {
      revision: 7,
      clientMutationId: null,
    })

    expect(scene.annotations.map(({ id, ordinal }) => [id, ordinal])).toEqual([
      ['first', 1],
      ['second', 2],
    ])
    expect(scene.draft).toBeNull()
  })

  it('publishes a stable provisional annotation while a new opinion is saving', () => {
    const editor: AnnotationInteractionState = {
      activeTool: 'rectangle',
      temporarilyPanning: false,
      selectedItemId: null,
      phase: {
        status: 'saving',
        sourceItemId: null,
        text: '修正领口',
        draftAnchor: RECTANGLE,
      },
    }

    const scene = toNativeScene([item('saved', 1, POINT)], editor, {
      revision: 8,
      clientMutationId: 'mutation-42',
    })

    expect(scene.draft).toBeNull()
    expect(scene.annotations.at(-1)).toMatchObject({
      id: 'provisional:mutation-42',
      ordinal: 2,
      draft: true,
      selected: true,
      geometry: reviewAnchorToNativeGeometry(RECTANGLE),
    })
  })

  it('maps native semantic events to reducer actions without interpreting coordinates twice', () => {
    const geometry = reviewAnchorToNativeGeometry(ARROW)
    if (geometry === null) throw new Error('expected arrow geometry')
    expect(
      toAnnotationAction({
        type: 'draft_started',
        sessionId: '1',
        assetGeneration: 1,
        geometry,
      }),
    ).toEqual({ type: 'begin_drawing', anchor: ARROW })
    expect(
      toAnnotationAction({
        type: 'draft_changed',
        sessionId: '1',
        assetGeneration: 1,
        geometry,
      }),
    ).toEqual({ type: 'update_draft_anchor', anchor: ARROW })
    expect(
      toAnnotationAction({
        type: 'selection_changed',
        sessionId: '1',
        assetGeneration: 1,
        annotationId: 'first',
      }),
    ).toEqual({ type: 'select_feedback', itemId: 'first' })
    expect(
      toAnnotationAction({
        type: 'editor_placement_changed',
        sessionId: '1',
        assetGeneration: 1,
        position: { x: 120, y: 80 },
      }),
    ).toBeNull()
  })
})

function item(
  itemId: string,
  ordinal: number | null,
  anchor: ReviewAnchor,
): ImageReviewWorkbenchFeedback {
  return {
    itemId,
    feedbackId: `feedback-${itemId}`,
    targetKey: null,
    assetVersionId: 'asset-1',
    text: itemId,
    createdAtMs: ordinal ?? 0,
    ordinal,
    anchor,
  }
}

function idleEditor(): AnnotationInteractionState {
  return {
    activeTool: 'browse',
    temporarilyPanning: false,
    selectedItemId: null,
    phase: { status: 'idle' },
  }
}
