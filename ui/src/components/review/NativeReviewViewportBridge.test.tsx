import { act, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { ImageReviewWorkbenchController } from '../../app/review/useImageReviewWorkbench'
import type { ImageRendererEvent } from '../../rendering/imageRendererTypes'
import type { ImagePreviewProjection } from '../imagePreview/ImagePreviewSurface'
import NativeReviewViewportBridge from './NativeReviewViewportBridge'

const PROJECTION: ImagePreviewProjection = {
  sourceSize: { width: 640, height: 480 },
  stageRect: { left: 0, top: 0, width: 640, height: 480 },
  stageToNormalized: ({ x, y }) => ({ x: x / 640, y: y / 480 }),
  normalizedToStage: ({ x, y }) => ({ x: x * 640, y: y * 480 }),
}

describe('NativeReviewViewportBridge', () => {
  it('routes drawing, selection and cancellation events into the existing workbench controller', () => {
    const controller = controllerFixture()
    let publish: ((event: ImageRendererEvent) => void) | null = null
    render(
      <NativeReviewViewportBridge controller={controller}>
        {({ binding }) => {
          publish = binding.onEvent
          return <div data-testid="scene-count">{binding.scene.annotations.length}</div>
        }}
      </NativeReviewViewportBridge>,
    )

    act(() => {
      publish?.({
        type: 'draft_started',
        sessionId: '1',
        assetGeneration: 1,
        geometry: { type: 'point', position: { x: 0.2, y: 0.3 } },
      })
      publish?.({
        type: 'draft_changed',
        sessionId: '1',
        assetGeneration: 1,
        geometry: { type: 'point', position: { x: 0.25, y: 0.35 } },
      })
      publish?.({
        type: 'draft_completed',
        sessionId: '1',
        assetGeneration: 1,
        geometry: { type: 'point', position: { x: 0.25, y: 0.35 } },
      })
      publish?.({
        type: 'selection_changed',
        sessionId: '1',
        assetGeneration: 1,
        annotationId: 'target-1',
      })
      publish?.({ type: 'draft_cancelled', sessionId: '1', assetGeneration: 1 })
    })

    expect(controller.beginDrawing).toHaveBeenCalledWith(
      { kind: 'image_point', x: 0.2, y: 0.3 },
      undefined,
    )
    expect(controller.updateDraftAnchor).toHaveBeenCalledWith({
      kind: 'image_point',
      x: 0.25,
      y: 0.35,
    })
    expect(controller.finishDrawing).toHaveBeenCalledWith({
      kind: 'image_point',
      x: 0.25,
      y: 0.35,
    })
    expect(controller.selectFeedback).toHaveBeenCalledWith('target-1')
    expect(controller.cancelDraft).toHaveBeenCalledOnce()
  })

  it('positions the editor from the native event and marks it as an input exclusion', () => {
    const controller = controllerFixture()
    controller.editor = {
      activeTool: 'rectangle',
      temporarilyPanning: false,
      selectedItemId: null,
      phase: {
        status: 'editing',
        sourceItemId: null,
        text: '',
        draftAnchor: { kind: 'image_rect', x: 0.1, y: 0.2, width: 0.3, height: 0.4 },
      },
    }
    let publish: ((event: ImageRendererEvent) => void) | null = null
    render(
      <NativeReviewViewportBridge controller={controller}>
        {({ binding, editor }) => {
          publish = binding.onEvent
          return editor(PROJECTION)
        }}
      </NativeReviewViewportBridge>,
    )

    act(() =>
      publish?.({
        type: 'editor_placement_changed',
        sessionId: '1',
        assetGeneration: 1,
        position: { x: 380, y: 220 },
      }),
    )

    const region = screen.getByRole('region', { name: '标注意见编辑器' })
    expect(region).toHaveAttribute('data-native-input-exclusion', 'true')
    expect(region).toHaveStyle({
      left: '352px',
      top: '236px',
    })
  })
})

function controllerFixture(): ImageReviewWorkbenchController {
  return {
    protocol: 'continuous',
    tool: 'point',
    editor: {
      activeTool: 'point',
      temporarilyPanning: false,
      selectedItemId: null,
      phase: { status: 'idle' },
    },
    dirty: false,
    feedback: [],
    selectedItemId: null,
    redrawItemId: null,
    railOpen: true,
    readOnlyReason: null,
    restorableItemId: null,
    statusMessage: null,
    preparedImage: null,
    leaveConfirmation: null,
    pendingMutationId: null,
    setTool: vi.fn(),
    setTemporaryPan: vi.fn(),
    beginAnnotation: vi.fn(),
    beginDrawing: vi.fn(() => true),
    finishDrawing: vi.fn(async () => undefined),
    beginFeedbackTextEdit: vi.fn(),
    beginRedraw: vi.fn(),
    stageFeedbackAnchor: vi.fn(() => true),
    updateDraftAnchor: vi.fn(),
    updateDraftText: vi.fn(),
    saveDraft: vi.fn(async () => undefined),
    cancelDraft: vi.fn(),
    selectFeedback: vi.fn(),
    replaceFeedbackAnchor: vi.fn(async () => undefined),
    deleteFeedback: vi.fn(async () => undefined),
    restoreDeletedFeedback: vi.fn(async () => undefined),
    setRailOpen: vi.fn(),
    requestLeave: vi.fn(async () => 'proceeded' as const),
    cancelLeave: vi.fn(),
    discardUnsavedAndProceed: vi.fn(async () => undefined),
  }
}
