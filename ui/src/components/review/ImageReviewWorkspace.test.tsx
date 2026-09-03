import { fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type {
  ImageReviewWorkbenchController,
  SavedImageFeedback,
} from '../../app/review/useImageReviewWorkbench'
import type {
  ImagePreviewProjection,
  ImagePreviewSurfaceProps,
} from '../imagePreview/ImagePreviewSurface'
import ImageReviewWorkspace from './ImageReviewWorkspace'
import InlineFeedbackEditor from './InlineFeedbackEditor'

const TEST_PROJECTION: ImagePreviewProjection = {
  sourceSize: { width: 640, height: 480 },
  stageRect: { left: 0, top: 0, width: 640, height: 480 },
  stageToNormalized: ({ x, y }) => ({ x: x / 640, y: y / 480 }),
  normalizedToStage: ({ x, y }) => ({ x: x * 640, y: y * 480 }),
}

vi.mock('../imagePreview/ImagePreviewSurface', () => ({
  default: ({ slots, onEscape, prefetchFit }: ImagePreviewSurfaceProps) => (
    <section
      data-testid="preview-surface"
      data-prefetch-fit={prefetchFit === false ? 'false' : 'true'}
      data-has-magnifier-overlay={slots?.magnifierOverlayPainter !== undefined || undefined}
      className="image-preview"
      onKeyDown={(event) => event.key === 'Escape' && onEscape?.()}
    >
      <header>
        {slots?.toolbarLeading}
        {slots?.toolbarActions}
      </header>
      <div>{slots?.stageOverlay?.(TEST_PROJECTION)}</div>
      {slots?.sidePanel}
    </section>
  ),
}))

const REVIEW_FILE: ImagePreviewSurfaceProps['file'] = {
  entityId: 'image-1',
  relativePath: 'batch/image-1.jpg',
  name: 'image-1.jpg',
  kind: 'jpeg',
  size: 1_024,
  modifiedNs: '1',
  marker: { reviewState: null, favorite: false },
  imageMetadata: { width: 640, height: 480 },
  imageUrl: null,
  videoMetadata: null,
}

function surfaceFixture(): Omit<ImagePreviewSurfaceProps, 'slots'> {
  return {
    file: REVIEW_FILE,
    files: [REVIEW_FILE],
    magnifier: { shape: 'circle', magnification: 2, area: 'small' },
    pointerClientPoint: { current: null },
    requestImage: vi.fn(async () => ({
      cacheKey: 'image-1-fit',
      url: 'viewer-image://localhost/session/image-1-fit',
      width: 640,
      height: 480,
      backend: 'image_io' as const,
    })),
    onNavigate: vi.fn(),
  }
}

function savedRect(
  feedbackId: string,
  ordinal: number,
  text: string,
  x: number,
  y: number,
  width: number,
  height: number,
): SavedImageFeedback {
  return {
    itemId: feedbackId,
    feedbackId,
    targetKey: null,
    assetVersionId: 'asset-1',
    ordinal,
    text,
    createdAtMs: ordinal,
    anchor: { kind: 'image_rect', x, y, width, height },
  }
}

function savedStroke(
  feedbackId: string,
  ordinal: number,
  text: string,
  points: ReadonlyArray<{ x: number; y: number }>,
): SavedImageFeedback {
  return {
    itemId: feedbackId,
    feedbackId,
    targetKey: null,
    assetVersionId: 'asset-1',
    ordinal,
    text,
    createdAtMs: ordinal,
    anchor: { kind: 'image_stroke', points },
  }
}

function controllerFixture(
  feedback: ReadonlyArray<SavedImageFeedback>,
): ImageReviewWorkbenchController {
  return {
    protocol: 'legacy',
    tool: 'browse',
    editor: {
      activeTool: 'browse',
      temporarilyPanning: false,
      selectedItemId: null,
      phase: { status: 'idle' },
    },
    dirty: false,
    redrawItemId: null,
    beginDrawing: vi.fn(() => true),
    finishDrawing: vi.fn(async () => undefined),
    beginFeedbackTextEdit: vi.fn(),
    beginRedraw: vi.fn(),
    stageFeedbackAnchor: vi.fn(() => true),
    feedback,
    selectedItemId: null,
    railOpen: true,
    readOnlyReason: null,
    restorableItemId: null,
    statusMessage: null,
    preparedImage: null,
    leaveConfirmation: null,
    setTool: vi.fn(),
    setTemporaryPan: vi.fn(),
    beginAnnotation: vi.fn(),
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

describe('ImageReviewWorkspace', () => {
  beforeEach(() => vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(null))
  afterEach(() => {
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
  })
  it('uses a compact bounded editor when the logical canvas is short', () => {
    const controller = controllerFixture([])
    render(
      <InlineFeedbackEditor
        controller={controller}
        anchor={{ kind: 'image_rect', x: 0.4, y: 0.2, width: 0.2, height: 0.2 }}
        projection={{ ...TEST_PROJECTION, stageRect: { left: 0, top: 0, width: 512, height: 155 } }}
      />,
    )
    expect(screen.getByRole('region', { name: '标注意见编辑器' })).toHaveAttribute(
      'data-compact',
      'true',
    )
  })
  it('returns keyboard focus to the retained opinion after a failed write', () => {
    const controller = controllerFixture([])
    controller.editor = {
      activeTool: 'rectangle',
      temporarilyPanning: false,
      selectedItemId: null,
      phase: {
        status: 'saving',
        sourceItemId: null,
        draftAnchor: { kind: 'image_rect', x: 0.1, y: 0.1, width: 0.2, height: 0.2 },
        text: '保留文字',
      },
    }
    const draftAnchor =
      controller.editor.phase.status === 'idle' ? null : controller.editor.phase.draftAnchor
    if (draftAnchor === null) throw new Error('expected saving draft')
    const rendered = render(<InlineFeedbackEditor controller={controller} anchor={draftAnchor} />)
    expect(screen.getByRole('textbox')).toBeDisabled()
    const failed: ImageReviewWorkbenchController = {
      ...controller,
      editor: {
        ...controller.editor,
        phase: {
          status: 'save_error',
          sourceItemId: null,
          draftAnchor,
          text: '保留文字',
          message: '请重试',
        },
      },
    }
    rendered.rerender(<InlineFeedbackEditor controller={failed} anchor={draftAnchor} />)
    expect(screen.getByRole('textbox')).toHaveFocus()
  })
  it('collapses the rail from the actual logical surface width at 200% zoom', () => {
    vi.spyOn(HTMLElement.prototype, 'clientWidth', 'get').mockReturnValue(512)
    vi.stubGlobal('matchMedia', () => ({ matches: false }))
    const controller = controllerFixture([])
    render(<ImageReviewWorkspace {...surfaceFixture()} controller={controller} />)
    expect(controller.setRailOpen).toHaveBeenCalledWith(false)
  })
  it('keeps one fixed markup trigger beside Browse and the Opinion Rail', () => {
    const controller = controllerFixture([])
    render(<ImageReviewWorkspace {...surfaceFixture()} controller={controller} />)

    expect(screen.getByRole('button', { name: '浏览' })).toBeVisible()
    expect(screen.getByRole('button', { name: '标记' })).toBeVisible()
    expect(screen.getByRole('button', { name: '意见栏' })).toBeVisible()
    expect(screen.queryByRole('button', { name: '画笔' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '矩形' })).not.toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: '标记' }))
    fireEvent.click(screen.getByRole('menuitemradio', { name: /箭头/ }))
    expect(controller.setTool).toHaveBeenCalledWith('arrow')
  })
  it('shows four independent numbered comments without permanent text bubbles', () => {
    const controller = controllerFixture([
      savedRect('feedback-1', 1, '衣领边缘需要更平整', 0.1, 0.1, 0.2, 0.15),
      savedStroke('feedback-2', 2, '右袖阴影断裂', [
        { x: 0.2, y: 0.2 },
        { x: 0.35, y: 0.45 },
      ]),
      savedRect('feedback-3', 3, '左袖长度不一致', 0.6, 0.2, 0.18, 0.3),
      savedStroke('feedback-4', 4, '裤脚接缝需要修正', [
        { x: 0.42, y: 0.7 },
        { x: 0.58, y: 0.82 },
      ]),
    ])
    render(<ImageReviewWorkspace {...surfaceFixture()} controller={controller} />)

    expect(screen.getAllByTestId('annotation-marker')).toHaveLength(4)
    expect(screen.getAllByRole('listitem', { name: /意见/ })).toHaveLength(4)
    expect(screen.queryAllByTestId('permanent-text-bubble')).toHaveLength(0)
    expect(screen.getByTestId('preview-surface')).toHaveAttribute(
      'data-has-magnifier-overlay',
      'true',
    )
  })

  it('routes tool shortcuts, suppresses them in text input, and keeps completion separate', () => {
    const controller = controllerFixture([])
    render(<ImageReviewWorkspace {...surfaceFixture()} controller={controller} />)

    fireEvent.keyDown(window, { key: 'b' })
    expect(controller.setTool).toHaveBeenCalledWith('brush')
    fireEvent.keyDown(window, { key: 'r' })
    expect(controller.setTool).toHaveBeenCalledWith('rectangle')
    fireEvent.keyDown(window, { key: 'v' })
    expect(controller.setTool).toHaveBeenCalledWith('browse')
    fireEvent.keyDown(window, { key: ' ', code: 'Space' })
    fireEvent.keyUp(window, { key: ' ', code: 'Space' })
    expect(controller.setTemporaryPan).toHaveBeenNthCalledWith(1, true)
    expect(controller.setTemporaryPan).toHaveBeenNthCalledWith(2, false)

    fireEvent.click(screen.getByRole('button', { name: '整图意见' }))
    expect(controller.beginAnnotation).toHaveBeenCalledWith({ kind: 'asset' })
    fireEvent.click(screen.getByRole('button', { name: '完成本轮评审' }))
    expect(controller.requestLeave).toHaveBeenCalledWith({ kind: 'finish_review' })
    fireEvent.click(screen.getByRole('button', { name: '返回网格' }))
    expect(controller.requestLeave).toHaveBeenCalledWith({ kind: 'return_grid' })
  })

  it('delegates leave execution to the controller without invoking a second callback', async () => {
    const returnGrid = vi.fn()
    const finishReview = vi.fn()
    const controller = controllerFixture([])
    controller.requestLeave = vi.fn(async (intent) => {
      if (intent.kind === 'return_grid') returnGrid()
      if (intent.kind === 'finish_review') finishReview()
      return 'proceeded' as const
    })
    render(<ImageReviewWorkspace {...surfaceFixture()} controller={controller} />)

    fireEvent.click(screen.getByRole('button', { name: '返回网格' }))
    fireEvent.click(screen.getByRole('button', { name: '完成本轮评审' }))

    await vi.waitFor(() => {
      expect(returnGrid).toHaveBeenCalledOnce()
      expect(finishReview).toHaveBeenCalledOnce()
    })
  })

  it('uses ongoing-review archive and history actions without a completed-round lock', () => {
    const controller = controllerFixture([])
    controller.protocol = 'continuous'
    const onArchive = vi.fn()
    const onHistory = vi.fn()
    render(
      <ImageReviewWorkspace
        {...surfaceFixture()}
        controller={controller}
        onArchive={onArchive}
        onHistory={onHistory}
      />,
    )

    expect(screen.queryByRole('button', { name: '完成本轮评审' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '放弃本轮' })).not.toBeInTheDocument()
    expect(screen.getByTestId('preview-surface')).toHaveAttribute('data-prefetch-fit', 'false')
    fireEvent.click(screen.getByRole('button', { name: '存档意见' }))
    fireEvent.click(screen.getByRole('button', { name: '历史' }))
    expect(onArchive).toHaveBeenCalledOnce()
    expect(onHistory).toHaveBeenCalledOnce()
  })

  it('keeps editing keyboard-safe, announces save failures, and restores focus after cancel', () => {
    const idle = controllerFixture([])
    const rendered = render(<ImageReviewWorkspace {...surfaceFixture()} controller={idle} />)
    const returnButton = screen.getByRole('button', { name: '返回网格' })
    returnButton.focus()

    const editing = controllerFixture([])
    editing.dirty = true
    editing.editor = {
      activeTool: 'rectangle',
      temporarilyPanning: false,
      selectedItemId: null,
      phase: {
        status: 'save_error',
        sourceItemId: null,
        draftAnchor: { kind: 'image_rect', x: 0.1, y: 0.1, width: 0.2, height: 0.2 },
        text: '修正领口',
        message: '意见尚未保存，请重试。',
      },
    }
    rendered.rerender(<ImageReviewWorkspace {...surfaceFixture()} controller={editing} />)
    const input = screen.getByRole('textbox', { name: '标注意见' })
    expect(input).toHaveFocus()
    expect(screen.getByRole('status')).toHaveAttribute('aria-live', 'polite')
    fireEvent.keyDown(input, { key: 'b' })
    expect(editing.setTool).not.toHaveBeenCalled()
    fireEvent.keyDown(input, { key: 'Enter', metaKey: true })
    expect(editing.saveDraft).toHaveBeenCalledOnce()
    fireEvent.keyDown(window, { key: 'Escape' })
    expect(editing.cancelDraft).toHaveBeenCalledOnce()

    rendered.rerender(<ImageReviewWorkspace {...surfaceFixture()} controller={idle} />)
    expect(returnButton).toHaveFocus()
    expect(screen.getByRole('button', { name: '浏览' })).toHaveAttribute('aria-pressed', 'true')
  })

  it('defaults the feedback rail closed when the workbench opens in a compact viewport', () => {
    vi.stubGlobal(
      'matchMedia',
      vi.fn(() => ({
        matches: true,
        media: '(max-width: 700px)',
        onchange: null,
        addListener: vi.fn(),
        removeListener: vi.fn(),
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
        dispatchEvent: vi.fn(),
      })),
    )
    const controller = controllerFixture([])

    render(<ImageReviewWorkspace {...surfaceFixture()} controller={controller} />)

    expect(controller.setRailOpen).toHaveBeenCalledWith(false)
  })
})
