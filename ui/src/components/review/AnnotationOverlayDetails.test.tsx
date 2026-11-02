import { render } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type {
  ImageReviewWorkbenchController,
  SavedImageFeedback,
} from '../../app/review/useImageReviewWorkbench'
import type { ImagePreviewProjection } from '../imagePreview/ImagePreviewSurface'
import AnnotationOverlayDetails from './AnnotationOverlayDetails'

const projection = {
  stageRect: { left: 0, top: 0, width: 200, height: 100 },
  normalizedToStage: ({ x, y }: { x: number; y: number }) => ({ x: x * 200, y: y * 100 }),
  sourceSize: { width: 1000, height: 500 },
} as unknown as ImagePreviewProjection

function controller(
  anchor: SavedImageFeedback['anchor'],
  style?: SavedImageFeedback['style'],
  editor?: Partial<ImageReviewWorkbenchController['editor']>,
): ImageReviewWorkbenchController {
  return {
    feedback: [
      {
        itemId: 'target',
        feedbackId: 'feedback',
        targetKey: null,
        assetVersionId: 'asset',
        text: '标记',
        createdAtMs: 1,
        ordinal: 1,
        anchor,
        style,
      },
    ],
    selectedItemId: null,
    editor: {
      activeTool: 'browse',
      temporarilyPanning: false,
      selectedItemId: null,
      phase: { status: 'idle' },
      ...editor,
    },
  } as unknown as ImageReviewWorkbenchController
}

describe('AnnotationOverlayDetails', () => {
  it('adds the upgraded translucent fill to saved rectangles and ellipses', () => {
    const rectangle = render(
      <AnnotationOverlayDetails
        controller={controller({ kind: 'image_rect', x: 0.1, y: 0.2, width: 0.3, height: 0.4 })}
        projection={projection}
      />,
    )
    expect(rectangle.container.querySelector('rect')).toHaveAttribute(
      'fill',
      'rgba(198, 40, 40, 0.1)',
    )

    const ellipse = render(
      <AnnotationOverlayDetails
        controller={controller({
          kind: 'image_ellipse',
          x: 0.2,
          y: 0.3,
          width: 0.2,
          height: 0.2,
        })}
        projection={projection}
      />,
    )
    expect(ellipse.container.querySelector('ellipse')).toHaveAttribute(
      'fill',
      'rgba(198, 40, 40, 0.1)',
    )
  })

  it('keeps an explicitly saved annotation color for the fill', () => {
    const rendered = render(
      <AnnotationOverlayDetails
        controller={controller(
          { kind: 'image_rect', x: 0.1, y: 0.2, width: 0.3, height: 0.4 },
          {
            tool: 'rectangle',
            color: 'blue',
            width: 'medium',
            opacityPercent: 100,
            dashed: false,
          },
        )}
        projection={projection}
      />,
    )
    expect(rendered.container.querySelector('rect')).toHaveAttribute(
      'fill',
      'rgba(21, 101, 192, 0.1)',
    )
  })

  it('follows the draft geometry while a saved region is being adjusted', () => {
    const rendered = render(
      <AnnotationOverlayDetails
        controller={controller(
          { kind: 'image_rect', x: 0.1, y: 0.2, width: 0.3, height: 0.4 },
          undefined,
          {
            phase: {
              status: 'editing',
              operation: 'geometry',
              sourceItemId: 'target',
              text: '标记',
              draftAnchor: { kind: 'image_rect', x: 0.5, y: 0.1, width: 0.2, height: 0.2 },
            },
          },
        )}
        projection={projection}
      />,
    )
    const rect = rendered.container.querySelector('rect')
    expect(Number(rect?.getAttribute('x'))).toBeCloseTo(100)
    expect(Number(rect?.getAttribute('y'))).toBeCloseTo(10)
    expect(Number(rect?.getAttribute('width'))).toBeCloseTo(40)
    expect(Number(rect?.getAttribute('height'))).toBeCloseTo(20)
  })

  it('follows the draft position while a text note is being moved', () => {
    const rendered = render(
      <AnnotationOverlayDetails
        controller={controller(
          { kind: 'image_text', x: 0.1, y: 0.1 },
          {
            tool: 'text',
            color: 'yellow',
            width: 'medium',
            opacityPercent: 100,
            dashed: false,
            textSize: 'medium',
          },
          {
            phase: {
              status: 'editing',
              operation: 'geometry',
              sourceItemId: 'target',
              text: '标记',
              draftAnchor: { kind: 'image_text', x: 0.6, y: 0.5 },
            },
          },
        )}
        projection={projection}
      />,
    )
    const note = rendered.container.querySelector<HTMLElement>('.annotation-text-note')
    expect(note?.style.left).toBe('120px')
    expect(note?.style.top).toBe('50px')
  })
})
