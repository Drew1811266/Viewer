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
  it('sends readonly permission changes to native without clearing the selected marker', () => {
    const controller = controllerFixture()
    controller.editor.selectedItemId = 'target-1'
    controller.feedback = [
      {
        itemId: 'target-1',
        feedbackId: 'feedback-1',
        targetKey: null,
        assetVersionId: 'asset-1',
        ordinal: 1,
        text: '意见',
        createdAtMs: 1,
        anchor: { kind: 'image_point', x: 0.2, y: 0.3 },
      },
    ]
    let binding:
      | import('../../rendering/imageRendererTypes').ImageRendererViewportBinding
      | undefined
    const child = (value: {
      binding: import('../../rendering/imageRendererTypes').ImageRendererViewportBinding
    }) => {
      binding = value.binding
      return null
    }
    const rendered = render(
      <NativeReviewViewportBridge controller={controller}>{child}</NativeReviewViewportBridge>,
    )
    expect(binding?.scene.annotationsEditable).toBe(true)
    const revision = binding?.sceneRevision
    rendered.rerender(
      <NativeReviewViewportBridge
        controller={{ ...controller, readOnlyReason: 'write_unavailable' }}
      >
        {child}
      </NativeReviewViewportBridge>,
    )
    expect(binding?.scene.annotationsEditable).toBe(false)
    expect(binding?.sceneRevision).not.toBe(revision)
    expect(binding?.scene.annotations[0]?.selected).toBe(true)
  })

  it('cancels only the active geometry edit and ignores completion rejected by staging', () => {
    const controller = controllerFixture()
    let publish: ((event: ImageRendererEvent) => void) | null = null
    render(
      <NativeReviewViewportBridge controller={controller}>
        {({ binding }) => {
          publish = binding.onEvent
          return null
        }}
      </NativeReviewViewportBridge>,
    )
    act(() => {
      publish?.({
        type: 'geometry_edit_started',
        sessionId: '1',
        assetGeneration: 1,
        annotationId: 'target-1',
        geometry: { type: 'point', position: { x: 0.2, y: 0.3 } },
      })
      publish?.({
        type: 'geometry_edit_changed',
        sessionId: '1',
        assetGeneration: 1,
        annotationId: 'target-1',
        geometry: { type: 'point', position: { x: 0.25, y: 0.3 } },
      })
      publish?.({
        type: 'geometry_edit_cancelled',
        sessionId: '1',
        assetGeneration: 1,
        annotationId: 'other',
      })
    })
    expect(controller.cancelDraft).not.toHaveBeenCalled()
    act(() =>
      publish?.({
        type: 'geometry_edit_cancelled',
        sessionId: '1',
        assetGeneration: 1,
        annotationId: 'target-1',
      }),
    )
    expect(controller.cancelDraft).toHaveBeenCalledOnce()
    vi.mocked(controller.stageFeedbackAnchor).mockReturnValue(false)
    act(() => {
      publish?.({
        type: 'geometry_edit_started',
        sessionId: '1',
        assetGeneration: 1,
        annotationId: 'target-2',
        geometry: { type: 'point', position: { x: 0.2, y: 0.3 } },
      })
      publish?.({
        type: 'geometry_edit_completed',
        sessionId: '1',
        assetGeneration: 1,
        annotationId: 'target-2',
        geometry: { type: 'point', position: { x: 0.4, y: 0.5 } },
      })
    })
    expect(controller.replaceFeedbackAnchor).not.toHaveBeenCalled()
  })

  it('keeps the browse input tool while a geometry edit stages without changing redraw tools', () => {
    const controller = controllerFixture()
    controller.tool = 'browse'
    let publish: ((event: ImageRendererEvent) => void) | null = null
    const child = ({
      binding,
    }: {
      binding: import('../../rendering/imageRendererTypes').ImageRendererViewportBinding
    }) => {
      publish = binding.onEvent
      return <div data-testid="native-tool">{binding.tool}</div>
    }
    const rendered = render(
      <NativeReviewViewportBridge controller={controller}>{child}</NativeReviewViewportBridge>,
    )
    act(() =>
      publish?.({
        type: 'geometry_edit_started',
        sessionId: '1',
        assetGeneration: 1,
        annotationId: 'target-1',
        geometry: { type: 'point', position: { x: 0.2, y: 0.3 } },
      }),
    )
    const staging = {
      ...controller,
      tool: 'point' as const,
      editor: {
        ...controller.editor,
        activeTool: 'point' as const,
        phase: {
          status: 'drawing' as const,
          sourceItemId: 'target-1',
          draftAnchor: { kind: 'image_point' as const, x: 0.2, y: 0.3 },
        },
      },
    }
    rendered.rerender(
      <NativeReviewViewportBridge controller={staging}>{child}</NativeReviewViewportBridge>,
    )
    expect(screen.getByTestId('native-tool')).toHaveTextContent('browse')
    rendered.unmount()
    render(
      <NativeReviewViewportBridge controller={{ ...staging, redrawItemId: 'target-1' }}>
        {child}
      </NativeReviewViewportBridge>,
    )
    expect(screen.getByTestId('native-tool')).toHaveTextContent('point')
  })

  it('stages existing geometry by item id and persists completion through replacement', () => {
    const controller = controllerFixture()
    let publish: ((event: ImageRendererEvent) => void) | null = null
    render(
      <NativeReviewViewportBridge controller={controller}>
        {({ binding }) => {
          publish = binding.onEvent
          return null
        }}
      </NativeReviewViewportBridge>,
    )
    act(() => {
      publish?.({
        type: 'geometry_edit_started',
        sessionId: '1',
        assetGeneration: 1,
        annotationId: 'target-1',
        geometry: { type: 'point', position: { x: 0.2, y: 0.3 } },
      })
      publish?.({
        type: 'geometry_edit_changed',
        sessionId: '1',
        assetGeneration: 1,
        annotationId: 'target-1',
        geometry: { type: 'point', position: { x: 0.4, y: 0.5 } },
      })
      publish?.({
        type: 'geometry_edit_completed',
        sessionId: '1',
        assetGeneration: 1,
        annotationId: 'target-1',
        geometry: { type: 'point', position: { x: 0.4, y: 0.5 } },
      })
    })
    expect(controller.stageFeedbackAnchor).toHaveBeenCalledOnce()
    expect(controller.stageFeedbackAnchor).toHaveBeenCalledWith('target-1', {
      kind: 'image_point',
      x: 0.4,
      y: 0.5,
    })
    expect(controller.replaceFeedbackAnchor).toHaveBeenCalledWith('target-1', {
      kind: 'image_point',
      x: 0.4,
      y: 0.5,
    })
    expect(controller.finishDrawing).not.toHaveBeenCalled()
  })

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
    let exclusionRevision = 0
    render(
      <NativeReviewViewportBridge controller={controller}>
        {({ binding, editor }) => {
          publish = binding.onEvent
          exclusionRevision = binding.inputExclusionRevision
          return editor(PROJECTION)
        }}
      </NativeReviewViewportBridge>,
    )

    const previousRevision = exclusionRevision
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
    expect(exclusionRevision).toBeGreaterThan(previousRevision)
    const movedRevision = exclusionRevision
    act(() =>
      publish?.({
        type: 'editor_placement_changed',
        sessionId: '1',
        assetGeneration: 1,
        position: { x: 380, y: 220 },
      }),
    )
    expect(exclusionRevision).toBe(movedRevision)
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
