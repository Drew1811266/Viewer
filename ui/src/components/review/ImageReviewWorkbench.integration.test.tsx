import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import {
  emptyReviewEditor,
  idleReviewSnapshot,
  type ReviewSessionSnapshot,
} from '../../app/review/reviewModel'
import {
  type ImageReviewWorkbenchController,
  type ReviewAnchor,
  useImageReviewWorkbench,
} from '../../app/review/useImageReviewWorkbench'
import type { ReviewSessionCoordinator } from '../../app/review/useReviewSessionCoordinator'
import { defined } from '../../defined'
import type { ImagePreviewSurfaceProps } from '../imagePreview/ImagePreviewSurface'
import ImageReviewWorkspace from './ImageReviewWorkspace'

type BrowserFile = ImagePreviewSurfaceProps['file']

const RECT: ReviewAnchor = { kind: 'image_rect', x: 0.2, y: 0.2, width: 0.2, height: 0.2 }
const files: BrowserFile[] = [1, 2, 3].map((id) => ({
  entityId: `image-${id}`,
  relativePath: `${id}.png`,
  name: `${id}.png`,
  kind: 'png',
  size: 100,
  modifiedNs: '1',
  marker: { reviewState: null, favorite: false },
  imageMetadata: { width: 640, height: 480 },
  imageUrl: null,
  videoMetadata: null,
}))

function reviewCoordinator(anchor: ReviewAnchor | null = RECT): ReviewSessionCoordinator {
  let state: ReviewSessionSnapshot = {
    ...idleReviewSnapshot(),
    phase: 'active',
    reviewStreamId: 'stream',
    reviewRoundId: 'round',
    revision: 1,
    members: [
      {
        assetVersionId: 'asset',
        entityId: 'image-2',
        relativePath: '2.png',
        displayName: '2.png',
        kind: 'image',
        feedbackItems: anchor === null ? 0 : 1,
      },
    ],
    feedback:
      anchor === null
        ? []
        : [
            {
              feedbackId: 'first',
              text: '原意见',
              createdAtMs: 1,
              targetCount: 1,
              targetEntityIds: ['image-2'],
              targets: [{ assetVersionId: 'asset', entityId: 'image-2', anchor }],
            },
          ],
  }
  const noCommand = vi.fn(async () => undefined)
  return {
    snapshot: state,
    proposal: null,
    completion: null,
    editor: emptyReviewEditor(),
    progress: null,
    error: null,
    discardConfirmation: null,
    previewStart: noCommand,
    captureStart: noCommand,
    confirmStart: noCommand,
    dismissStart: vi.fn(),
    resume: noCommand,
    beginCreate: vi.fn(),
    setEditorText: vi.fn(),
    submitFeedback: noCommand,
    beginEdit: vi.fn(),
    saveEdit: noCommand,
    prepareCompletion: noCommand,
    confirmCompletion: noCommand,
    dismissCompletion: vi.fn(),
    abandon: noCommand,
    cancelTask: noCommand,
    requestDiscard: vi.fn(() => true),
    confirmDiscard: vi.fn(),
    cancelDiscard: vi.fn(),
    startWithFeedback: vi.fn(async () => null),
    addAnchoredFeedback: vi.fn(async ({ text, anchor: nextAnchor }) => {
      const id = `added-${state.feedback.length + 1}`
      state = {
        ...state,
        revision: state.revision + 1,
        feedback: [
          ...state.feedback,
          {
            feedbackId: id,
            text,
            createdAtMs: state.feedback.length + 2,
            targetCount: 1,
            targetEntityIds: ['image-2'],
            targets: [{ assetVersionId: 'asset', entityId: 'image-2', anchor: nextAnchor }],
          },
        ],
      }
      return state
    }),
    updateAnchoredFeedbackText: vi.fn(async (feedbackId, text) => {
      state = {
        ...state,
        revision: state.revision + 1,
        feedback: state.feedback.map((item) =>
          item.feedbackId === feedbackId ? { ...item, text } : item,
        ),
      }
      return state
    }),
    replaceAnchoredFeedbackAnchor: vi.fn(async (feedbackId, _entityId, nextAnchor) => {
      state = {
        ...state,
        revision: state.revision + 1,
        feedback: state.feedback.map((item) =>
          item.feedbackId === feedbackId
            ? {
                ...item,
                targets: item.targets.map((target) => ({ ...target, anchor: nextAnchor })),
              }
            : item,
        ),
      }
      return state
    }),
    deleteFeedback: vi.fn(async () => state),
    restoreDeletedFeedback: vi.fn(async () => state),
  }
}

async function mount(review = reviewCoordinator()) {
  let controller!: ImageReviewWorkbenchController
  const leave = vi.fn(async () => undefined)
  const requestImage = vi.fn(async (file: BrowserFile) => ({
    cacheKey: file.entityId,
    url: `viewer-image://localhost/${file.entityId}`,
    width: 640,
    height: 480,
    backend: 'image_io' as const,
  }))
  function Harness() {
    controller = useImageReviewWorkbench({
      coordinator: review,
      entityId: 'image-2',
      scope: { kind: 'selection', entityIds: ['image-2'] },
      onLeave: leave,
    })
    return (
      <ImageReviewWorkspace
        controller={controller}
        file={defined(files[1])}
        files={files}
        magnifier={{ shape: 'circle', magnification: 2, area: 'small' }}
        pointerClientPoint={{ current: null }}
        requestImage={requestImage}
      />
    )
  }
  render(<Harness />)
  await waitFor(() => expect(document.querySelector('.image-preview-image')).not.toBeNull())
  const previewImage = document.querySelector('.image-preview-image')
  if (previewImage === null) throw new Error('Expected loaded preview image')
  fireEvent.load(previewImage)
  await screen.findByTestId('annotation-canvas')
  return { current: () => controller, review, leave }
}

function draw() {
  const canvas = screen.getByTestId('annotation-canvas')
  fireEvent.pointerDown(canvas, { pointerId: 1, clientX: 200, clientY: 150 })
  fireEvent.pointerMove(canvas, { pointerId: 1, clientX: 300, clientY: 250 })
  fireEvent.pointerUp(canvas, { pointerId: 1, clientX: 350, clientY: 280 })
}

beforeEach(() => {
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(null)
  vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue({
    x: 0,
    y: 0,
    left: 0,
    top: 0,
    right: 640,
    bottom: 480,
    width: 640,
    height: 480,
    toJSON: () => undefined,
  })
  vi.stubGlobal(
    'PointerEvent',
    class extends MouseEvent {
      pointerId: number
      constructor(type: string, options: PointerEventInit = {}) {
        super(type, options)
        this.pointerId = options.pointerId ?? 1
      }
    },
  )
})
afterEach(() => {
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
})

describe('real image workbench editing ownership', () => {
  it('guards every rail text leave intent and explicitly discards local text', async () => {
    const work = await mount()
    fireEvent.click(screen.getByRole('button', { name: '编辑意见 1 文字' }))
    fireEvent.change(screen.getByRole('textbox', { name: '意见 1 文字' }), {
      target: { value: '未保存修改' },
    })
    for (const intent of [
      { kind: 'return_grid' },
      { kind: 'navigate', offset: 1 },
      { kind: 'close_project' },
      { kind: 'finish_review' },
      { kind: 'abandon_review' },
    ] as const) {
      await act(async () => {
        expect(await work.current().requestLeave(intent)).toBe('blocked')
      })
      expect(work.current().leaveConfirmation).toEqual(intent)
      act(() => work.current().cancelLeave())
    }
    expect(work.leave).not.toHaveBeenCalled()
    await act(async () => {
      await work.current().requestLeave({ kind: 'return_grid' })
      await work.current().discardUnsavedAndProceed()
    })
    expect(work.leave).toHaveBeenCalledWith({ kind: 'return_grid' })
    expect(screen.queryByRole('textbox')).toBeNull()
    expect(defined(work.current().feedback[0]).text).toBe('原意见')
  })

  it('guards active drawing and refuses to replace dirty inline work', async () => {
    const work = await mount()
    fireEvent.click(screen.getByRole('button', { name: '画笔' }))
    fireEvent.pointerDown(screen.getByTestId('annotation-canvas'), {
      pointerId: 1,
      clientX: 200,
      clientY: 150,
    })
    await act(async () => {
      expect(await work.current().requestLeave({ kind: 'finish_review' })).toBe('blocked')
    })
    act(() => work.current().cancelDraft())
    fireEvent.pointerUp(screen.getByTestId('annotation-canvas'), {
      pointerId: 1,
      clientX: 300,
      clientY: 250,
    })
    expect(work.current().dirty).toBe(false)
    fireEvent.click(screen.getByRole('button', { name: '矩形' }))
    draw()
    fireEvent.change(screen.getByRole('textbox', { name: '标注意见' }), {
      target: { value: '不能丢失' },
    })
    const before = work.current().editor
    fireEvent.click(screen.getByRole('button', { name: '整图意见' }))
    draw()
    expect(work.current().editor).toEqual(before)
  })

  it('retains failed rectangle replacement with retry/cancel and guards in-flight persistence', async () => {
    const review = reviewCoordinator()
    let reject!: (error: Error) => void
    vi.mocked(review.replaceAnchoredFeedbackAnchor).mockImplementationOnce(
      () =>
        new Promise((_resolve, fail) => {
          reject = fail
        }),
    )
    const work = await mount(review)
    fireEvent.keyDown(screen.getByRole('button', { name: '意见 1：原意见' }), { key: 'ArrowRight' })
    await act(async () => {
      expect(await work.current().requestLeave({ kind: 'return_grid' })).toBe('blocked')
    })
    await act(async () => reject(new Error('disk unavailable')))
    expect(work.current().dirty).toBe(true)
    expect(screen.getByTestId('annotation-canvas')).toHaveAttribute('data-has-candidate', 'true')
    await act(async () => {
      expect(await work.current().requestLeave({ kind: 'finish_review' })).toBe('blocked')
    })
    fireEvent.click(screen.getByRole('button', { name: '重试保存标记' }))
    await waitFor(() => expect(work.current().dirty).toBe(false))
    expect(defined(work.current().feedback[0]).anchor).toEqual({ ...RECT, x: 0.2 + 1 / 640 })
    vi.mocked(review.replaceAnchoredFeedbackAnchor).mockRejectedValueOnce(
      new Error('disk unavailable'),
    )
    fireEvent.keyDown(screen.getByRole('button', { name: '意见 1：原意见' }), { key: 'ArrowRight' })
    await screen.findByRole('button', { name: '取消标记修改' })
    fireEvent.click(screen.getByRole('button', { name: '取消标记修改' }))
    expect(work.current().dirty).toBe(false)
    expect(defined(work.current().feedback[0]).anchor).toEqual({ ...RECT, x: 0.2 + 1 / 640 })
  })

  it.each(['inline', 'rail'] as const)(
    'keeps q/arrows/composition in the %s editor and preserves save/Escape',
    async (kind) => {
      const work = await mount()
      if (kind === 'rail') fireEvent.click(screen.getByRole('button', { name: '编辑意见 1 文字' }))
      else {
        fireEvent.click(screen.getByRole('button', { name: '矩形' }))
        draw()
      }
      const input = screen.getByRole('textbox')
      fireEvent.change(input, { target: { value: '编辑 q 意见' } })
      for (const key of ['q', 'ArrowLeft', 'ArrowRight'])
        expect(fireEvent.keyDown(input, { key })).toBe(true)
      expect(screen.getByRole('button', { name: '放大镜' })).toHaveAttribute(
        'aria-pressed',
        'false',
      )
      expect(work.current().leaveConfirmation).toBeNull()
      expect(work.leave).not.toHaveBeenCalled()
      fireEvent.keyDown(input, { key: 'Enter', metaKey: true, isComposing: true })
      expect(work.current().editor.status).toBe('editing')
      fireEvent.keyDown(input, { key: 'Escape', isComposing: true })
      expect(work.current().dirty).toBe(true)
      fireEvent.keyDown(input, { key: 'Enter', metaKey: true })
      await waitFor(() => expect(screen.queryByRole('textbox')).toBeNull())
      expect(work.current().feedback.some((item) => item.text === '编辑 q 意见')).toBe(true)
      fireEvent.click(screen.getByRole('button', { name: '编辑意见 1 文字' }))
      fireEvent.keyDown(screen.getByRole('textbox'), { key: 'Escape' })
      expect(work.current().dirty).toBe(false)
    },
  )

  it('creates two independent brush opinions and only explicit redraw replaces identity', async () => {
    const work = await mount(reviewCoordinator(null))
    for (const text of ['第一条', '第二条']) {
      fireEvent.click(screen.getByRole('button', { name: '画笔' }))
      draw()
      fireEvent.change(screen.getByRole('textbox'), { target: { value: text } })
      fireEvent.keyDown(screen.getByRole('textbox'), { key: 'Enter', metaKey: true })
      await waitFor(() => expect(work.current().dirty).toBe(false))
    }
    expect(work.current().feedback.map((item) => item.text)).toEqual(['第一条', '第二条'])
    const original = defined(work.current().feedback[0])
    fireEvent.click(screen.getByRole('button', { name: '重绘意见 1' }))
    draw()
    await waitFor(() => expect(work.current().dirty).toBe(false))
    expect(work.current().feedback).toHaveLength(2)
    expect(work.current().feedback[0]).toMatchObject({
      feedbackId: original.feedbackId,
      text: original.text,
      createdAtMs: original.createdAtMs,
    })
    expect(work.review.replaceAnchoredFeedbackAnchor).toHaveBeenCalledWith(
      original.feedbackId,
      'image-2',
      expect.objectContaining({ kind: 'image_stroke' }),
    )
  })

  it('preserves ordinary preview shortcuts outside text entry', async () => {
    const work = await mount()
    const surface = screen.getByRole('dialog', { name: '图片评审 2.png' })
    fireEvent.keyDown(surface, { key: 'q' })
    expect(screen.getByRole('button', { name: '放大镜' })).toHaveAttribute('aria-pressed', 'true')
    fireEvent.keyDown(surface, { key: 'ArrowRight' })
    expect(work.leave).toHaveBeenCalledWith({ kind: 'navigate', offset: 1 })
  })

  it('owns pointer rectangle moves until release and cancels active pointer listeners on discard', async () => {
    const work = await mount()
    const marker = screen.getByRole('button', { name: '意见 1：原意见' })
    fireEvent.click(marker)
    fireEvent.pointerDown(marker, { pointerId: 7, clientX: 260, clientY: 160 })
    expect(work.current().editor.status).toBe('drawing')
    fireEvent.pointerMove(window, { pointerId: 7, clientX: 280, clientY: 180 })
    expect(screen.getByTestId('annotation-canvas')).toHaveAttribute('data-has-candidate', 'true')
    fireEvent.pointerUp(window, { pointerId: 7, clientX: 280, clientY: 180 })
    await waitFor(() => expect(work.current().dirty).toBe(false))
    expect(work.review.replaceAnchoredFeedbackAnchor).toHaveBeenCalledOnce()
    const moved = defined(work.current().feedback[0]).anchor
    expect(moved).not.toEqual(RECT)
    fireEvent.pointerDown(marker, { pointerId: 8, clientX: 280, clientY: 180 })
    await act(async () => {
      expect(await work.current().requestLeave({ kind: 'navigate', offset: 1 })).toBe('blocked')
      await work.current().discardUnsavedAndProceed()
    })
    fireEvent.pointerMove(window, { pointerId: 8, clientX: 310, clientY: 220 })
    fireEvent.pointerUp(window, { pointerId: 8, clientX: 310, clientY: 220 })
    expect(work.review.replaceAnchoredFeedbackAnchor).toHaveBeenCalledOnce()
    expect(work.current().dirty).toBe(false)
    expect(defined(work.current().feedback[0]).anchor).toEqual(moved)
  })

  it('keeps failed explicit redraw retryable and guards every leave intent', async () => {
    const originalAnchor: ReviewAnchor = {
      kind: 'image_stroke',
      points: [
        { x: 0.1, y: 0.1 },
        { x: 0.2, y: 0.2 },
      ],
    }
    const review = reviewCoordinator(originalAnchor)
    vi.mocked(review.replaceAnchoredFeedbackAnchor).mockRejectedValueOnce(new Error('disk full'))
    const work = await mount(review)
    fireEvent.click(screen.getByRole('button', { name: '重绘意见 1' }))
    draw()
    await screen.findByRole('button', { name: '重试保存标记' })
    expect(defined(work.current().feedback[0]).anchor).toEqual(originalAnchor)
    for (const intent of [
      { kind: 'return_grid' },
      { kind: 'navigate', offset: -1 },
      { kind: 'close_project' },
      { kind: 'finish_review' },
    ] as const) {
      await act(async () => {
        expect(await work.current().requestLeave(intent)).toBe('blocked')
      })
    }
    fireEvent.click(screen.getByRole('button', { name: '重试保存标记' }))
    await waitFor(() => expect(work.current().dirty).toBe(false))
    expect(defined(work.current().feedback[0])).toMatchObject({
      feedbackId: 'first',
      text: '原意见',
      createdAtMs: 1,
    })
    expect(defined(work.current().feedback[0]).anchor).not.toEqual(originalAnchor)
  })

  it('retains rail text through failed saving and prevents replacing its pending operation', async () => {
    const review = reviewCoordinator()
    let reject!: (error: Error) => void
    vi.mocked(review.updateAnchoredFeedbackText).mockImplementationOnce(
      () =>
        new Promise((_resolve, fail) => {
          reject = fail
        }),
    )
    const work = await mount(review)
    fireEvent.click(screen.getByRole('button', { name: '编辑意见 1 文字' }))
    const input = screen.getByRole('textbox')
    fireEvent.change(input, { target: { value: '保留这段文字' } })
    fireEvent.keyDown(input, { key: 'Enter', metaKey: true })
    expect(work.current().editor.status).toBe('saving')
    await act(async () => {
      await work.current().requestLeave({ kind: 'finish_review' })
      await work.current().discardUnsavedAndProceed()
    })
    expect(work.leave).not.toHaveBeenCalled()
    expect(work.current().dirty).toBe(true)
    await act(async () => reject(new Error('save failed')))
    expect(input).toHaveValue('保留这段文字')
    expect(input).toHaveFocus()
    fireEvent.click(screen.getByRole('button', { name: '保存意见 1 文字' }))
    await waitFor(() => expect(work.current().dirty).toBe(false))
    expect(defined(work.current().feedback[0]).text).toBe('保留这段文字')
  })
  it('does not submit saved-item mutations over a dirty text operation', async () => {
    const work = await mount()
    fireEvent.click(screen.getByRole('button', { name: '编辑意见 1 文字' }))
    fireEvent.change(screen.getByRole('textbox'), { target: { value: '未保存文字' } })
    await act(async () => {
      await work.current().deleteFeedback('first')
      await work.current().restoreDeletedFeedback('deleted')
    })
    expect(work.review.deleteFeedback).not.toHaveBeenCalled()
    expect(work.review.restoreDeletedFeedback).not.toHaveBeenCalled()
    expect(screen.getByRole('textbox')).toHaveValue('未保存文字')
  })
})
