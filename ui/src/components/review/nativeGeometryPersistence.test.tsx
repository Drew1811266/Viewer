import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { expect, it } from 'vitest'
import type {
  ImageReviewSaveRequest,
  ImageReviewWorkbenchAdapter,
  ImageReviewWorkbenchView,
} from '../../app/review/imageReviewWorkbenchAdapter'
import { useImageReviewWorkbench } from '../../app/review/useImageReviewWorkbench'
import type { ImageRendererViewportBinding } from '../../rendering/imageRendererTypes'
import { createImagePreviewProjection } from '../imagePreview/imagePreviewProjection'
import NativeReviewViewportBridge from './NativeReviewViewportBridge'

it('keeps browse and a clean editor for a native handle click, and gates text/geometry editors', async () => {
  const requests: ImageReviewSaveRequest[] = []
  let resolveSave: ((view: ImageReviewWorkbenchView) => void) | undefined
  let view: ImageReviewWorkbenchView = {
    feedback: [
      {
        itemId: 'arrow-item',
        feedbackId: 'feedback-1',
        targetKey: null,
        assetVersionId: 'asset-1',
        ordinal: 1,
        text: '原文字',
        createdAtMs: 1,
        anchor: { kind: 'image_arrow', tail: { x: 0.2, y: 0.3 }, head: { x: 0.6, y: 0.7 } },
      },
    ],
    readOnlyReason: null,
    restorableItemId: null,
    statusMessage: null,
  }
  const adapter: ImageReviewWorkbenchAdapter = {
    protocol: 'legacy',
    preparationKey: () => 'click',
    viewKey: () => 'click',
    prepareEntity: async () => ({ key: 'click', prepared: null }),
    view: () => view,
    saveFeedback: (request) => {
      requests.push(request)
      return new Promise((resolve) => {
        resolveSave = resolve
      })
    },
    deleteFeedback: async () => view,
    restoreDeletedFeedback: async () => view,
    discardPendingInput: () => true,
  }
  let controller!: ReturnType<typeof useImageReviewWorkbench>
  let binding!: ImageRendererViewportBinding
  const projection = createImagePreviewProjection(
    { left: 0, top: 0, width: 500, height: 500 },
    { mode: 'fit', zoom: 1, rotation: 0, offset: { x: 0, y: 0 } },
    { stage: { width: 500, height: 500 }, source: { width: 1000, height: 1000 }, fitInset: 1 },
  )
  function Harness() {
    controller = useImageReviewWorkbench({ adapter, entityId: 'click-image' })
    return (
      <NativeReviewViewportBridge controller={controller}>
        {(value) => {
          binding = value.binding
          return value.editor(projection)
        }}
      </NativeReviewViewportBridge>
    )
  }
  render(<Harness />)
  await waitFor(() => expect(controller.feedback).toHaveLength(1))
  act(() => {
    controller.selectFeedback('arrow-item')
    controller.setTool('browse')
  })
  const start = () =>
    binding.onEvent({
      type: 'geometry_edit_started',
      sessionId: '1',
      assetGeneration: 1,
      annotationId: 'arrow-item',
      handle: 'head',
      geometry: { type: 'arrow', tail: { x: 0.2, y: 0.3 }, head: { x: 0.6, y: 0.7 } },
    })
  const change = () =>
    binding.onEvent({
      type: 'geometry_edit_changed',
      sessionId: '1',
      assetGeneration: 1,
      annotationId: 'arrow-item',
      geometry: { type: 'arrow', tail: { x: 0.2, y: 0.3 }, head: { x: 0.8, y: 0.7 } },
    })
  const cancel = () =>
    binding.onEvent({
      type: 'geometry_edit_cancelled',
      sessionId: '1',
      assetGeneration: 1,
      annotationId: 'arrow-item',
    })
  act(start)
  expect(document.activeElement).toBe(screen.getByRole('button', { name: '调整意见 1 箭头' }))
  expect(controller.editor.phase.status).toBe('idle')
  expect(controller.tool).toBe('browse')
  expect(binding.scene.annotations[0]?.draft).toBe(false)
  act(cancel)
  expect(controller.tool).toBe('browse')
  expect(controller.editor.phase.status).toBe('idle')
  expect(requests).toHaveLength(0)
  act(start)
  act(change)
  expect(controller.editor.phase.status).toBe('drawing')
  expect(binding.tool).toBe('browse')
  expect(binding.scene.annotationsEditable).toBe(false)
  act(cancel)
  expect(controller.editor.phase.status).toBe('idle')
  expect(controller.feedback[0]?.text).toBe('原文字')
  expect(requests).toHaveLength(0)
  act(() => controller.beginFeedbackTextEdit('arrow-item'))
  expect(binding.tool).toBe('browse')
  expect(binding.scene.annotationsEditable).toBe(false)
  const editor = controller.editor.phase
  const savedGeometry = binding.scene.annotations[0]?.geometry
  act(() => {
    start()
    change()
    cancel()
  })
  expect(controller.editor.phase).toEqual(editor)
  expect(binding.scene.annotations[0]?.geometry).toEqual(savedGeometry)
  let saving!: Promise<void>
  act(() => {
    controller.updateDraftText('新文字')
    saving = controller.saveDraft()
  })
  expect(controller.editor.phase.status).toBe('saving')
  expect(binding.scene.annotationsEditable).toBe(false)
  expect(binding.tool).toBe('browse')
  await act(async () => {
    view = { ...view, feedback: view.feedback.map((item) => ({ ...item, text: '新文字' })) }
    resolveSave?.(view)
    await saving
  })
  expect(binding.scene.annotationsEditable).toBe(true)
  expect(requests).toHaveLength(1)
  // Native completion discards an unflushed Changed event. It must still
  // stage and save the moved candidate exactly once.
  act(() => {
    controller.setTool('browse')
    start()
  })
  act(() =>
    binding.onEvent({
      type: 'geometry_edit_completed',
      sessionId: '1',
      assetGeneration: 1,
      annotationId: 'arrow-item',
      geometry: { type: 'arrow', tail: { x: 0.2, y: 0.3 }, head: { x: 0.8, y: 0.7 } },
    }),
  )
  expect(controller.editor.phase.status).toBe('saving')
  expect(binding.scene.annotationsEditable).toBe(false)
  expect(requests).toHaveLength(2)
  expect(requests[1]).toMatchObject({
    operation: 'geometry',
    text: '新文字',
    item: { itemId: 'arrow-item' },
    anchor: { kind: 'image_arrow', tail: { x: 0.2, y: 0.3 }, head: { x: 0.8, y: 0.7 } },
  })
  act(() => {
    change()
    cancel()
  })
  expect(controller.editor.phase.status).toBe('saving')
  const geometryRequest = requests[1]
  if (geometryRequest === undefined) throw new Error('missing geometry save')
  await act(async () => {
    view = {
      ...view,
      feedback: view.feedback.map((item) => ({ ...item, anchor: geometryRequest.anchor })),
    }
    resolveSave?.(view)
  })
  await waitFor(() => expect(controller.editor.phase.status).toBe('idle'))
  expect(binding.scene.annotationsEditable).toBe(true)
})

it('exposes semantic marker and handles, persists source-pixel keys, and isolates navigation', async () => {
  const requests: ImageReviewSaveRequest[] = []
  let fail = false
  let readonly = false
  let view = {
    feedback: [
      {
        itemId: 'keyboard-item',
        feedbackId: 'keyboard-feedback',
        targetKey: null,
        assetVersionId: 'asset-1',
        ordinal: 3,
        text: '键盘区域',
        createdAtMs: 1,
        anchor: { kind: 'image_rect' as const, x: 0.2, y: 0.3, width: 0.4, height: 0.2 },
      },
    ],
    readOnlyReason: null,
    restorableItemId: null,
    statusMessage: null,
  }
  const adapter: ImageReviewWorkbenchAdapter = {
    protocol: 'legacy',
    preparationKey: () => 'keyboard',
    viewKey: () => `keyboard-${readonly}`,
    prepareEntity: async () => ({ key: 'keyboard', prepared: null }),
    view: () => ({ ...view, readOnlyReason: readonly ? 'outside_scope' : null }),
    saveFeedback: async (request) => {
      requests.push(request)
      if (fail) throw new Error('keyboard persistence failure')
      view = {
        ...view,
        feedback: view.feedback.map((item) => ({
          ...item,
          anchor: request.anchor as typeof item.anchor,
        })),
      }
      return view
    },
    deleteFeedback: async () => view,
    restoreDeletedFeedback: async () => view,
    discardPendingInput: () => true,
  }
  const projection = createImagePreviewProjection(
    { left: 0, top: 0, width: 400, height: 200 },
    { mode: 'fit', zoom: 1, rotation: 90, offset: { x: 0, y: 0 } },
    { stage: { width: 400, height: 200 }, source: { width: 1000, height: 500 }, fitInset: 1 },
  )
  let controller!: ReturnType<typeof useImageReviewWorkbench>
  let nativeBinding!: ImageRendererViewportBinding
  let navigation = 0
  function Harness() {
    controller = useImageReviewWorkbench({ adapter, entityId: 'keyboard-image' })
    return (
      <div onKeyDown={() => navigation++}>
        <button type="button">画布导航</button>
        <NativeReviewViewportBridge controller={controller}>
          {(value) => {
            nativeBinding = value.binding
            return (
              <>
                {value.editor(projection)}
                <span data-testid="keyboard-phase">{controller.editor.phase.status}</span>
              </>
            )
          }}
        </NativeReviewViewportBridge>
      </div>
    )
  }
  const rendered = render(<Harness />)
  const marker = await screen.findByRole('button', { name: '意见 3：键盘区域' })
  act(() =>
    nativeBinding.onEvent({
      type: 'selection_changed',
      sessionId: '1',
      assetGeneration: 1,
      annotationId: 'keyboard-item',
    }),
  )
  expect(document.activeElement).toBe(marker)
  const initialTop = Number.parseFloat(marker.style.top)
  await act(async () => fireEvent.keyDown(marker, { key: 'ArrowRight' }))
  expect(view.feedback[0]?.anchor.x).toBe(0.201)
  expect(Number.parseFloat(marker.style.top) - initialTop).toBeCloseTo(0.2)
  expect(document.activeElement).toBe(marker)
  expect(navigation).toBe(0)
  const handle = screen.getByRole('button', { name: '调整意见 3 右下角' })
  act(() =>
    nativeBinding.onEvent({
      type: 'geometry_edit_started',
      sessionId: '1',
      assetGeneration: 1,
      annotationId: 'keyboard-item',
      handle: 'south_east',
      geometry: { type: 'rectangle', rect: { x: 0.201, y: 0.3, width: 0.4, height: 0.2 } },
    }),
  )
  expect(document.activeElement).toBe(handle)
  act(() =>
    nativeBinding.onEvent({
      type: 'geometry_edit_cancelled',
      sessionId: '1',
      assetGeneration: 1,
      annotationId: 'keyboard-item',
    }),
  )
  await act(async () => fireEvent.keyDown(handle, { key: 'ArrowDown', shiftKey: true }))
  expect(view.feedback[0]?.anchor.height).toBe(0.22)
  expect(document.activeElement).toBe(handle)
  fail = true
  await act(async () => fireEvent.keyDown(handle, { key: 'ArrowDown' }))
  expect(screen.getByTestId('keyboard-phase')).toHaveTextContent('save_error')
  const count = requests.length
  await act(async () => fireEvent.keyDown(handle, { key: 'ArrowDown' }))
  expect(requests).toHaveLength(count)
  fail = false
  await act(async () => controller.saveDraft())
  expect(view.feedback[0]?.anchor.height).toBe(0.222)
  expect(requests.at(-1)).toMatchObject({
    operation: 'geometry',
    text: '键盘区域',
    item: { itemId: 'keyboard-item' },
  })
  fail = true
  await act(async () => fireEvent.keyDown(handle, { key: 'ArrowDown' }))
  expect(screen.getByTestId('keyboard-phase')).toHaveTextContent('save_error')
  act(() => controller.cancelDraft())
  expect(screen.getByTestId('keyboard-phase')).toHaveTextContent('idle')
  expect(view.feedback[0]?.anchor.height).toBe(0.222)
  readonly = true
  rendered.rerender(<Harness />)
  await waitFor(() => expect(controller.readOnlyReason).toBe('outside_scope'))
  fireEvent.click(marker)
  expect(controller.selectedItemId).toBe('keyboard-item')
  const saved = requests.length
  fireEvent.keyDown(marker, { key: 'ArrowRight' })
  expect(requests).toHaveLength(saved)
  expect(navigation).toBe(0)
  act(() =>
    nativeBinding.onEvent({
      type: 'selection_changed',
      sessionId: '1',
      assetGeneration: 1,
      annotationId: null,
    }),
  )
  expect(document.activeElement).not.toBe(marker)
  expect(document.activeElement).not.toBe(handle)
  fireEvent.keyDown(screen.getByRole('button', { name: '画布导航' }), { key: 'ArrowRight' })
  expect(navigation).toBeGreaterThan(0)
})

it.each([
  {
    rotation: 0 as const,
    anchor: { kind: 'image_point', x: 0.999, y: 0.4 },
    handle: null,
    key: 'ArrowRight',
    shiftKey: true,
    expected: { kind: 'image_point', x: 1, y: 0.4 },
  },
  {
    rotation: 90 as const,
    anchor: { kind: 'image_arrow', tail: { x: 0.2, y: 0.3 }, head: { x: 0.6, y: 0.7 } },
    handle: '箭头',
    key: 'ArrowRight',
    shiftKey: true,
    expected: { kind: 'image_arrow', tail: { x: 0.2, y: 0.3 }, head: { x: 0.61, y: 0.7 } },
  },
  {
    rotation: 180 as const,
    anchor: { kind: 'image_arrow', tail: { x: 0.2, y: 0.3 }, head: { x: 0.6, y: 0.7 } },
    handle: '箭尾',
    key: 'ArrowUp',
    shiftKey: false,
    expected: { kind: 'image_arrow', tail: { x: 0.2, y: 0.298 }, head: { x: 0.6, y: 0.7 } },
  },
  {
    rotation: 270 as const,
    anchor: { kind: 'image_ellipse', x: 0.2, y: 0.3, width: 0.4, height: 0.2 },
    handle: '左上角',
    key: 'ArrowLeft',
    shiftKey: false,
    expected: { kind: 'image_ellipse', x: 0.199, y: 0.3, width: 0.401, height: 0.2 },
  },
])(
  'persists keyboard $handle geometry at rotation $rotation',
  async ({ rotation, anchor, handle, key, shiftKey, expected }) => {
    let view: ImageReviewWorkbenchView = {
      feedback: [
        {
          itemId: 'semantic-shape',
          feedbackId: 'shape-feedback',
          targetKey: null,
          assetVersionId: 'asset-1',
          ordinal: 1,
          text: '形状',
          createdAtMs: 1,
          anchor: anchor as ImageReviewWorkbenchView['feedback'][number]['anchor'],
        },
      ],
      readOnlyReason: null,
      restorableItemId: null,
      statusMessage: null,
    }
    const adapter: ImageReviewWorkbenchAdapter = {
      protocol: 'legacy',
      preparationKey: () => 'shape',
      viewKey: () => 'shape',
      prepareEntity: async () => ({ key: 'shape', prepared: null }),
      view: () => view,
      saveFeedback: async (request) => {
        view = {
          ...view,
          feedback: view.feedback.map((item) => ({ ...item, anchor: request.anchor })),
        }
        return view
      },
      deleteFeedback: async () => view,
      restoreDeletedFeedback: async () => view,
      discardPendingInput: () => true,
    }
    const projection = createImagePreviewProjection(
      { left: 0, top: 0, width: 400, height: 200 },
      { mode: 'fit', zoom: 1, rotation, offset: { x: 0, y: 0 } },
      { stage: { width: 400, height: 200 }, source: { width: 1000, height: 500 }, fitInset: 1 },
    )
    function Harness() {
      const controller = useImageReviewWorkbench({ adapter, entityId: 'shape-image' })
      return (
        <NativeReviewViewportBridge controller={controller}>
          {(value) => value.editor(projection)}
        </NativeReviewViewportBridge>
      )
    }
    render(<Harness />)
    const marker = await screen.findByRole('button', { name: '意见 1：形状' })
    act(() => marker.focus())
    const target =
      handle === null ? marker : screen.getByRole('button', { name: `调整意见 1 ${handle}` })
    act(() => target.focus())
    await act(async () => fireEvent.keyDown(target, { key, shiftKey }))
    expect(view.feedback[0]?.anchor).toEqual(expected)
    expect(document.activeElement).toBe(target)
  },
)

it('retains edited geometry and saved text after persistence failure and retries the same item', async () => {
  const requests: ImageReviewSaveRequest[] = []
  let fail = true
  const view = {
    feedback: [
      {
        itemId: 'item-1',
        feedbackId: 'feedback-1',
        targetKey: null,
        assetVersionId: 'asset-1',
        ordinal: 1,
        text: '保留原有文字',
        createdAtMs: 1,
        anchor: { kind: 'image_point' as const, x: 0.2, y: 0.3 },
      },
    ],
    readOnlyReason: null,
    restorableItemId: null,
    statusMessage: null,
  }
  const adapter: ImageReviewWorkbenchAdapter = {
    protocol: 'legacy',
    preparationKey: () => 'memory',
    viewKey: () => 'memory',
    prepareEntity: async () => ({ key: 'memory', prepared: null }),
    view: () => view,
    saveFeedback: async (request) => {
      requests.push(request)
      if (fail) throw new Error('temporary persistence failure')
      return {
        ...view,
        feedback: view.feedback.map((item) => ({ ...item, anchor: request.anchor })),
      }
    },
    deleteFeedback: async () => view,
    restoreDeletedFeedback: async () => view,
    discardPendingInput: () => true,
  }
  let binding: ImageRendererViewportBinding | undefined
  let retry: (() => Promise<void>) | undefined
  function Harness() {
    const controller = useImageReviewWorkbench({ adapter, entityId: 'image-1' })
    retry = controller.saveDraft
    return (
      <NativeReviewViewportBridge controller={controller}>
        {(value) => {
          binding = value.binding
          return <div data-testid="phase">{controller.editor.phase.status}</div>
        }}
      </NativeReviewViewportBridge>
    )
  }
  render(<Harness />)
  await waitFor(() => expect(binding?.scene.annotations).toHaveLength(1))
  act(() => {
    binding?.onEvent({
      type: 'geometry_edit_started',
      sessionId: '1',
      assetGeneration: 1,
      annotationId: 'item-1',
      geometry: { type: 'point', position: { x: 0.2, y: 0.3 } },
    })
  })
  expect(screen.getByTestId('phase')).toHaveTextContent('idle')
  expect(binding?.tool).toBe('browse')
  act(() => {
    binding?.onEvent({
      type: 'geometry_edit_changed',
      sessionId: '1',
      assetGeneration: 1,
      annotationId: 'item-1',
      geometry: { type: 'point', position: { x: 0.4, y: 0.5 } },
    })
  })
  expect(binding?.scene.draft).toBeNull()
  expect(screen.getByTestId('phase')).toHaveTextContent('drawing')
  expect(binding?.scene.annotations).toEqual([
    expect.objectContaining({
      id: 'item-1',
      draft: true,
      geometry: { type: 'point', position: { x: 0.4, y: 0.5 } },
    }),
  ])
  await act(async () => {
    binding?.onEvent({
      type: 'geometry_edit_completed',
      sessionId: '1',
      assetGeneration: 1,
      annotationId: 'item-1',
      geometry: { type: 'point', position: { x: 0.4, y: 0.5 } },
    })
  })
  expect(screen.getByTestId('phase')).toHaveTextContent('save_error')
  expect(binding?.scene.annotationsEditable).toBe(false)
  expect(binding?.scene.annotations[0]).toMatchObject({
    id: 'item-1',
    draft: true,
    geometry: { type: 'point', position: { x: 0.4, y: 0.5 } },
  })
  fail = false
  await act(async () => retry?.())
  expect(requests).toHaveLength(2)
  for (const request of requests) {
    expect(request).toMatchObject({
      operation: 'geometry',
      item: { itemId: 'item-1' },
      text: '保留原有文字',
      anchor: { kind: 'image_point', x: 0.4, y: 0.5 },
    })
  }
  expect(screen.getByTestId('phase')).toHaveTextContent('idle')
  expect(binding?.scene.annotations[0]).toMatchObject({
    id: 'item-1',
    draft: false,
    geometry: { type: 'point', position: { x: 0.4, y: 0.5 } },
  })
})
