import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { ImageReviewWorkbenchController } from '../../app/review/useImageReviewWorkbench'
import ReviewFeedbackRail from './ReviewFeedbackRail'

function controller(
  overrides: Partial<ImageReviewWorkbenchController> = {},
): ImageReviewWorkbenchController {
  return {
    tool: 'browse',
    editor: {
      status: 'idle',
      tool: 'browse',
      temporarilyPanning: false,
      selectedFeedbackId: null,
    },
    dirty: false,
    feedback: [
      {
        feedbackId: 'feedback-1',
        ordinal: 1,
        text: '调整领口',
        createdAtMs: 1,
        anchor: { kind: 'image_rect', x: 0.1, y: 0.2, width: 0.3, height: 0.4 },
      },
      {
        feedbackId: 'feedback-2',
        ordinal: null,
        text: '整体降低饱和度',
        createdAtMs: 2,
        anchor: { kind: 'asset' },
      },
    ],
    selectedFeedbackId: null,
    railOpen: true,
    readOnlyReason: null,
    restorableFeedbackId: 'feedback-3',
    leaveConfirmation: null,
    setTool: vi.fn(),
    setTemporaryPan: vi.fn(),
    beginAnnotation: vi.fn(),
    updateDraftAnchor: vi.fn(),
    updateDraftText: vi.fn(),
    saveDraft: vi.fn(async () => undefined),
    cancelDraft: vi.fn(),
    selectFeedback: vi.fn(),
    updateFeedbackText: vi.fn(async () => undefined),
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
  it('provides selection, text edit, geometry edit, delete and session restore controls', async () => {
    const review = controller()
    render(<ReviewFeedbackRail controller={review} />)

    fireEvent.click(screen.getByRole('button', { name: '选择意见 1：调整领口' }))
    expect(review.selectFeedback).toHaveBeenCalledWith('feedback-1')
    fireEvent.click(screen.getByRole('button', { name: '编辑意见 1 文字' }))
    const input = screen.getByRole('textbox', { name: '意见 1 文字' })
    fireEvent.change(input, { target: { value: '调整领口边缘' } })
    fireEvent.click(screen.getByRole('button', { name: '保存意见 1 文字' }))
    expect(review.updateFeedbackText).toHaveBeenCalledWith('feedback-1', '调整领口边缘')

    await waitFor(() =>
      expect(screen.getByRole('button', { name: '调整意见 1 区域' })).toBeVisible(),
    )

    fireEvent.click(screen.getByRole('button', { name: '调整意见 1 区域' }))
    expect(review.setTool).toHaveBeenCalledWith('rectangle')
    fireEvent.click(screen.getByRole('button', { name: '删除意见 1' }))
    expect(review.deleteFeedback).toHaveBeenCalledWith('feedback-1')
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

  it('retains edited text and announces a persistence failure', async () => {
    const review = controller({
      updateFeedbackText: vi.fn().mockRejectedValue(new Error('write failed')),
    })
    render(<ReviewFeedbackRail controller={review} />)

    fireEvent.click(screen.getByRole('button', { name: '编辑意见 1 文字' }))
    const input = screen.getByRole('textbox', { name: '意见 1 文字' })
    fireEvent.change(input, { target: { value: '保留这段修改' } })
    fireEvent.keyDown(input, { key: 'Enter', metaKey: true })

    expect(await screen.findByRole('status')).toHaveTextContent('意见尚未保存，请重试。')
    expect(input).toHaveValue('保留这段修改')
  })
})
