import { describe, expect, it } from 'vitest'
import type {
  ImageReviewWorkbenchController,
  SavedImageFeedback,
} from '../../app/review/useImageReviewWorkbench'
import { shouldShowAnnotationStyleBar } from './AnnotationStyleBar'

type ReviewAnchor = SavedImageFeedback['anchor']

function feedback(itemId: string, anchor: ReviewAnchor): SavedImageFeedback {
  return {
    itemId,
    feedbackId: itemId,
    targetKey: null,
    assetVersionId: 'asset-1',
    text: '需要返工',
    createdAtMs: 1,
    ordinal: anchor.kind === 'asset' ? null : 1,
    anchor,
  }
}

function controller(
  overrides: Partial<ImageReviewWorkbenchController> = {},
): ImageReviewWorkbenchController {
  return {
    protocol: 'continuous',
    readOnlyReason: null,
    editor: { phase: { status: 'idle' } },
    tool: 'browse',
    selectedItemId: null,
    feedback: [],
    ...overrides,
  } as unknown as ImageReviewWorkbenchController
}

describe('shouldShowAnnotationStyleBar', () => {
  it('hides for a whole-image opinion with the browse tool', () => {
    expect(
      shouldShowAnnotationStyleBar(
        controller({
          selectedItemId: 'asset-opinion',
          feedback: [feedback('asset-opinion', { kind: 'asset' })],
        }),
      ),
    ).toBe(false)
  })

  it('shows for a selected image annotation with the browse tool', () => {
    expect(
      shouldShowAnnotationStyleBar(
        controller({
          selectedItemId: 'image-opinion',
          feedback: [
            feedback('image-opinion', {
              kind: 'image_rect',
              x: 0.1,
              y: 0.2,
              width: 0.3,
              height: 0.2,
            }),
          ],
        }),
      ),
    ).toBe(true)
  })

  it('shows while a drawing tool is active even with a whole-image selection', () => {
    expect(
      shouldShowAnnotationStyleBar(
        controller({
          tool: 'rectangle',
          selectedItemId: 'asset-opinion',
          feedback: [feedback('asset-opinion', { kind: 'asset' })],
        }),
      ),
    ).toBe(true)
  })

  it('stays visible while a region is being drawn', () => {
    expect(
      shouldShowAnnotationStyleBar(
        controller({
          tool: 'rectangle',
          editor: {
            activeTool: 'rectangle',
            temporarilyPanning: false,
            selectedItemId: null,
            phase: {
              status: 'drawing',
              draftAnchor: { kind: 'image_rect', x: 0.1, y: 0.1, width: 0.2, height: 0.2 },
            },
          },
        }),
      ),
    ).toBe(true)
  })

  it('stays visible while a new region waits for its text', () => {
    expect(
      shouldShowAnnotationStyleBar(
        controller({
          tool: 'rectangle',
          editor: {
            activeTool: 'rectangle',
            temporarilyPanning: false,
            selectedItemId: null,
            phase: {
              status: 'editing',
              draftAnchor: { kind: 'image_rect', x: 0.1, y: 0.1, width: 0.2, height: 0.2 },
              text: '需要返工',
              sourceItemId: null,
            },
          },
        }),
      ),
    ).toBe(true)
  })

  it('hides for a whole-image draft and while saving', () => {
    expect(
      shouldShowAnnotationStyleBar(
        controller({
          tool: 'browse',
          editor: {
            activeTool: 'browse',
            temporarilyPanning: false,
            selectedItemId: null,
            phase: {
              status: 'editing',
              draftAnchor: { kind: 'asset' },
              text: '整图意见',
              sourceItemId: null,
            },
          },
        }),
      ),
    ).toBe(false)
    expect(
      shouldShowAnnotationStyleBar(
        controller({
          tool: 'rectangle',
          editor: {
            activeTool: 'rectangle',
            temporarilyPanning: false,
            selectedItemId: null,
            phase: {
              status: 'saving',
              draftAnchor: { kind: 'image_rect', x: 0.1, y: 0.1, width: 0.2, height: 0.2 },
              text: '需要返工',
              sourceItemId: null,
            },
          },
        }),
      ),
    ).toBe(false)
  })
})
