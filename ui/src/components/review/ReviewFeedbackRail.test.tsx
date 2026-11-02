import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { ImageReviewWorkbenchController } from '../../app/review/useImageReviewWorkbench'
import { defined } from '../../defined'
import { type SpeechPreferences, SpeechServiceProvider } from '../../speech/SpeechServiceContext'
import ReviewFeedbackRail from './ReviewFeedbackRail'

const SPEECH_PREFERENCES: SpeechPreferences = {
  inputMode: 'click',
  locale: 'simplified_chinese',
  autoPunctuation: true,
}

function controller(
  overrides: Partial<ImageReviewWorkbenchController> = {},
): ImageReviewWorkbenchController {
  return {
    protocol: 'legacy',
    tool: 'browse',
    annotationStyle: {
      tool: 'rectangle',
      color: 'red',
      width: 'medium',
      opacityPercent: 100,
      dashed: false,
    },
    styleBusy: false,
    canUndo: false,
    canRedo: false,
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
    feedback: [
      {
        itemId: 'target-1',
        feedbackId: 'feedback-1',
        targetKey: null,
        assetVersionId: 'asset-1',
        ordinal: 1,
        text: '调整领口',
        createdAtMs: 1,
        anchor: { kind: 'image_rect', x: 0.1, y: 0.2, width: 0.3, height: 0.4 },
      },
      {
        itemId: 'feedback-2',
        feedbackId: 'feedback-2',
        targetKey: null,
        assetVersionId: 'asset-1',
        ordinal: null,
        text: '整体降低饱和度',
        createdAtMs: 2,
        anchor: { kind: 'asset' },
      },
    ],
    selectedItemId: null,
    railOpen: true,
    readOnlyReason: null,
    restorableItemId: 'feedback-3',
    statusMessage: null,
    preparedImage: null,
    leaveConfirmation: null,
    setTool: vi.fn(),
    setAnnotationStyle: vi.fn(),
    undo: vi.fn(),
    redo: vi.fn(),
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
    ...overrides,
  }
}

describe('ReviewFeedbackRail', () => {
  it('scrolls the selected opinion into view when selection changes', () => {
    const scrollIntoView = vi.fn()
    const original = Element.prototype.scrollIntoView
    Element.prototype.scrollIntoView = scrollIntoView
    try {
      const rendered = render(
        <ReviewFeedbackRail controller={controller({ selectedItemId: 'target-1' })} />,
      )
      expect(scrollIntoView).toHaveBeenCalledOnce()

      scrollIntoView.mockClear()
      rendered.rerender(
        <ReviewFeedbackRail controller={controller({ selectedItemId: 'feedback-2' })} />,
      )
      expect(scrollIntoView).toHaveBeenCalledOnce()
    } finally {
      Element.prototype.scrollIntoView = original
    }
  })

  it('selects an adjustable annotation for direct adjustment', () => {
    const review = controller()
    render(<ReviewFeedbackRail controller={review} />)
    fireEvent.click(screen.getByRole('button', { name: '调整意见 1 标注' }))
    expect(review.selectFeedback).toHaveBeenCalledWith('target-1')
    expect(review.setTool).toHaveBeenCalledWith('browse')
    expect(review.beginRedraw).not.toHaveBeenCalled()
  })

  it('provides selection, text edit, geometry edit, delete and session restore controls', async () => {
    const review = controller()
    const rendered = render(<ReviewFeedbackRail controller={review} />)

    fireEvent.click(screen.getByRole('button', { name: '选择意见 1：调整领口' }))
    expect(review.selectFeedback).toHaveBeenCalledWith('target-1')
    fireEvent.click(screen.getByRole('button', { name: '编辑意见 1 文字' }))
    expect(review.beginFeedbackTextEdit).toHaveBeenCalledWith('target-1')
    rendered.rerender(
      <ReviewFeedbackRail
        controller={{
          ...review,
          editor: {
            activeTool: 'browse',
            temporarilyPanning: false,
            selectedItemId: 'target-1',
            phase: {
              status: 'editing',
              sourceItemId: 'target-1',
              text: '调整领口',
              draftAnchor: defined(review.feedback[0]).anchor,
            },
          },
        }}
      />,
    )
    const input = screen.getByRole('textbox', { name: '意见 1 文字' })
    fireEvent.change(input, { target: { value: '调整领口边缘' } })
    fireEvent.click(screen.getByRole('button', { name: '保存意见 1 文字' }))
    expect(review.updateDraftText).toHaveBeenCalledWith('调整领口边缘')
    expect(review.saveDraft).toHaveBeenCalledOnce()
    rendered.rerender(<ReviewFeedbackRail controller={review} />)

    await waitFor(() =>
      expect(screen.getByRole('button', { name: '调整意见 1 标注' })).toBeVisible(),
    )

    fireEvent.click(screen.getByRole('button', { name: '调整意见 1 标注' }))
    expect(review.setTool).toHaveBeenCalledWith('browse')
    fireEvent.click(screen.getByRole('button', { name: '删除意见 1' }))
    expect(review.deleteFeedback).toHaveBeenCalledWith('target-1')
    fireEvent.click(screen.getByRole('button', { name: '撤销删除' }))
    expect(review.restoreDeletedFeedback).toHaveBeenCalledWith('feedback-3')
  })

  it('opens a whole-image editor without inventing a stage marker and collapses accessibly', () => {
    const review = controller()
    const rendered = render(<ReviewFeedbackRail controller={review} />)
    fireEvent.click(screen.getByRole('button', { name: '整图意见' }))
    expect(review.beginAnnotation).toHaveBeenCalledWith({ kind: 'asset' })
    fireEvent.click(screen.getByRole('button', { name: '收起意见栏' }))
    expect(review.setRailOpen).toHaveBeenCalledWith(false)

    rendered.rerender(<ReviewFeedbackRail controller={controller({ railOpen: false })} />)
    expect(screen.getByRole('button', { name: '展开意见栏' })).toBeVisible()
  })

  it('displays controller-owned retained text and persistence failure', async () => {
    const review = controller({
      editor: {
        activeTool: 'browse',
        temporarilyPanning: false,
        selectedItemId: 'target-1',
        phase: {
          status: 'save_error',
          sourceItemId: 'target-1',
          draftAnchor: { kind: 'image_rect', x: 0.1, y: 0.2, width: 0.3, height: 0.4 },
          text: '保留这段修改',
          message: '意见尚未保存，请重试。',
        },
      },
    })
    render(<ReviewFeedbackRail controller={review} />)

    const input = screen.getByRole('textbox', { name: '意见 1 文字' })
    fireEvent.keyDown(input, { key: 'Enter', metaKey: true })

    expect(await screen.findByRole('status')).toHaveTextContent('意见尚未保存，请重试。')
    expect(input).toHaveValue('保留这段修改')
  })

  it('dictates into an ongoing rail text edit without leaving the card', async () => {
    const review = controller()
    const editingReview = {
      ...review,
      editor: {
        activeTool: 'browse' as const,
        temporarilyPanning: false,
        selectedItemId: 'target-1',
        phase: {
          status: 'editing' as const,
          sourceItemId: 'target-1',
          text: '调整领口',
          draftAnchor: defined(review.feedback[0]).anchor,
        },
      },
    }
    const readyStatus = {
      available: true,
      speechAuthorized: true,
      microphoneAuthorized: true,
      onDevice: true,
      localeSupported: true,
      locale: 'zh-CN',
    }
    const speechPort = {
      speechStatus: vi.fn().mockResolvedValue(readyStatus),
      requestSpeechPermissions: vi
        .fn()
        .mockResolvedValue({ speechAuthorized: true, microphoneAuthorized: true }),
      startSpeechCapture: vi.fn().mockResolvedValue(readyStatus),
      stopSpeechCapture: vi
        .fn()
        .mockResolvedValue({ text: '，再收紧一些', elapsedMs: 1200, onDevice: true }),
      cancelSpeechCapture: vi.fn(),
      openSpeechSettings: vi.fn().mockResolvedValue(undefined),
    }
    render(
      <SpeechServiceProvider port={speechPort} preferences={SPEECH_PREFERENCES}>
        <ReviewFeedbackRail controller={editingReview} />
      </SpeechServiceProvider>,
    )

    fireEvent.click(await screen.findByRole('button', { name: '语音输入' }))

    expect(await screen.findByText('正在聆听')).toBeVisible()
    expect(screen.getByText('本地转写 · 音频不上传')).toBeVisible()

    fireEvent.click(screen.getByRole('button', { name: '停止并转写' }))

    await waitFor(() => expect(review.updateDraftText).toHaveBeenCalledWith('调整领口，再收紧一些'))
    expect(speechPort.stopSpeechCapture).toHaveBeenCalledOnce()
  })

  it('explains denied microphone access without losing the draft and can open settings', async () => {
    const review = controller()
    const editingReview = {
      ...review,
      editor: {
        activeTool: 'browse' as const,
        temporarilyPanning: false,
        selectedItemId: 'target-1',
        phase: {
          status: 'editing' as const,
          sourceItemId: 'target-1',
          text: '调整领口',
          draftAnchor: defined(review.feedback[0]).anchor,
        },
      },
    }
    const speechPort = {
      speechStatus: vi.fn().mockResolvedValue({
        available: true,
        speechAuthorized: true,
        microphoneAuthorized: false,
        onDevice: true,
        localeSupported: true,
        locale: 'zh-CN',
      }),
      requestSpeechPermissions: vi
        .fn()
        .mockResolvedValue({ speechAuthorized: true, microphoneAuthorized: false }),
      startSpeechCapture: vi.fn(),
      stopSpeechCapture: vi.fn(),
      cancelSpeechCapture: vi.fn(),
      openSpeechSettings: vi.fn().mockResolvedValue(undefined),
    }
    render(
      <SpeechServiceProvider port={speechPort} preferences={SPEECH_PREFERENCES}>
        <ReviewFeedbackRail controller={editingReview} />
      </SpeechServiceProvider>,
    )

    fireEvent.click(await screen.findByRole('button', { name: '语音输入' }))

    expect(await screen.findByText('Viewer 无法访问麦克风')).toBeVisible()
    expect(screen.getByText('草稿已保留')).toBeVisible()
    expect(screen.getByRole('textbox', { name: '意见 1 文字' })).toHaveValue('调整领口')
    expect(screen.getByText(/不使用麦克风也可以继续写文字意见/)).toBeVisible()

    fireEvent.click(screen.getByRole('button', { name: '打开系统设置' }))

    await waitFor(() => expect(speechPort.openSpeechSettings).toHaveBeenCalledOnce())
  })

  it('explains why annotation actions are temporarily unavailable', () => {
    render(
      <ReviewFeedbackRail
        controller={controller({ protocol: 'continuous', readOnlyReason: 'source_confirmation' })}
      />,
    )
    expect(screen.getByRole('status')).toHaveTextContent('素材来源或版本需要确认')
    expect(screen.getByRole('button', { name: '整图意见' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '编辑意见 1 文字' })).toBeDisabled()
  })
})
