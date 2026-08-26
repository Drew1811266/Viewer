import { act, renderHook, waitFor } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { ReviewAnchor, ReviewSessionSnapshot } from '../../api/types'
import { useImageReviewWorkbench } from './useImageReviewWorkbench'
import type { ReviewSessionCoordinator } from './useReviewSessionCoordinator'

const RECT: ReviewAnchor = { kind: 'image_rect', x: 0.1, y: 0.2, width: 0.3, height: 0.4 }

function snapshot(
  phase: ReviewSessionSnapshot['phase'] = 'idle',
  feedback: ReviewSessionSnapshot['feedback'] = [],
): ReviewSessionSnapshot {
  return {
    phase,
    resume: null,
    reviewStreamId: phase === 'active' ? 'stream-1' : null,
    reviewRoundId: phase === 'active' ? 'round-1' : null,
    revision: phase === 'active' ? 1 : 0,
    members:
      phase === 'active'
        ? [
            {
              assetVersionId: 'asset-1',
              entityId: 'image-1',
              relativePath: 'image-1.png',
              displayName: 'image-1.png',
              kind: 'image',
              feedbackItems: feedback.length,
            },
          ]
        : [],
    feedback,
    restorableFeedbackId: null,
    unreviewable: [],
    conflicts: [],
    counts: {
      total: phase === 'active' ? 1 : 0,
      feedbackItems: feedback.length,
      revise: feedback.length > 0 ? 1 : 0,
      unreviewable: 0,
      pass: 0,
    },
    error: null,
  }
}

function saved(text = '服务端意见'): ReviewSessionSnapshot {
  return snapshot('active', [
    {
      feedbackId: 'feedback-1',
      text,
      createdAtMs: 10,
      targetEntityIds: ['image-1'],
      targets: [{ assetVersionId: 'asset-1', entityId: 'image-1', anchor: RECT }],
      targetCount: 1,
    },
  ])
}

function coordinator(current = snapshot()): ReviewSessionCoordinator {
  return {
    snapshot: current,
    proposal: null,
    completion: null,
    editor: {
      mode: 'create',
      feedbackId: null,
      text: '',
      savedText: '',
      targetEntityIds: [],
      saveState: 'idle',
      error: null,
    },
    progress: null,
    error: null,
    discardConfirmation: null,
    previewStart: vi.fn().mockResolvedValue(undefined),
    startWithFeedback: vi.fn().mockResolvedValue(saved()),
    addAnchoredFeedback: vi.fn().mockResolvedValue(saved()),
    updateAnchoredFeedbackText: vi.fn().mockResolvedValue(saved()),
    replaceAnchoredFeedbackAnchor: vi.fn().mockResolvedValue(saved()),
    restoreDeletedFeedback: vi.fn().mockResolvedValue(saved()),
    confirmStart: vi.fn().mockResolvedValue(undefined),
    dismissStart: vi.fn(),
    resume: vi.fn().mockResolvedValue(undefined),
    beginCreate: vi.fn(),
    setEditorText: vi.fn(),
    submitFeedback: vi.fn().mockResolvedValue(undefined),
    beginEdit: vi.fn(),
    saveEdit: vi.fn().mockResolvedValue(undefined),
    deleteFeedback: vi.fn().mockResolvedValue(saved()),
    prepareCompletion: vi.fn().mockResolvedValue(undefined),
    confirmCompletion: vi.fn().mockResolvedValue(undefined),
    dismissCompletion: vi.fn(),
    abandon: vi.fn().mockResolvedValue(undefined),
    cancelTask: vi.fn().mockResolvedValue(undefined),
    requestDiscard: vi.fn().mockReturnValue(true),
    confirmDiscard: vi.fn(),
    cancelDiscard: vi.fn(),
  }
}

describe('useImageReviewWorkbench', () => {
  it('previews the captured scope once and atomically saves the first feedback', async () => {
    const review = coordinator()
    const scope = { kind: 'selection' as const, entityIds: ['image-1', 'image-2'] }
    const hook = renderHook(() =>
      useImageReviewWorkbench({ coordinator: review, entityId: 'image-1', scope }),
    )

    await waitFor(() => expect(review.previewStart).toHaveBeenCalledOnce())
    expect(review.previewStart).toHaveBeenCalledWith(scope)
    act(() => {
      hook.result.current.beginAnnotation(RECT)
      hook.result.current.updateDraftText('修正袖口')
    })
    await act(() => hook.result.current.saveDraft())

    expect(review.startWithFeedback).toHaveBeenCalledWith({
      entityId: 'image-1',
      text: '修正袖口',
      anchor: RECT,
    })
    expect(review.previewStart).toHaveBeenCalledOnce()
    expect(hook.result.current.feedback).toEqual([
      expect.objectContaining({ feedbackId: 'feedback-1', text: '服务端意见', anchor: RECT }),
    ])
    expect(hook.result.current.editor).toEqual(
      expect.objectContaining({ status: 'idle', selectedFeedbackId: 'feedback-1' }),
    )
  })

  it('keeps unsaved text and anchor after a command failure', async () => {
    const review = coordinator(snapshot('active'))
    vi.mocked(review.addAnchoredFeedback).mockRejectedValue({
      userMessage: '意见尚未保存，请重试。',
    })
    const hook = renderHook(() =>
      useImageReviewWorkbench({
        coordinator: review,
        entityId: 'image-1',
        scope: { kind: 'selection', entityIds: ['image-1'] },
      }),
    )
    act(() => {
      hook.result.current.beginAnnotation(RECT)
      hook.result.current.updateDraftText('保留这段文字')
    })
    await act(() => hook.result.current.saveDraft())

    expect(hook.result.current.editor).toEqual(
      expect.objectContaining({
        status: 'save_error',
        draftAnchor: RECT,
        text: '保留这段文字',
        message: '意见尚未保存，请重试。',
      }),
    )
  })

  it('blocks every leave intent while dirty and proceeds only after explicit discard', async () => {
    const onLeave = vi.fn().mockResolvedValue(undefined)
    const review = coordinator(snapshot('active'))
    const hook = renderHook(() =>
      useImageReviewWorkbench({
        coordinator: review,
        entityId: 'image-1',
        scope: { kind: 'selection', entityIds: ['image-1'] },
        onLeave,
      }),
    )
    act(() => hook.result.current.beginAnnotation(RECT))

    for (const intent of [
      { kind: 'navigate', offset: 1 },
      { kind: 'return_grid' },
      { kind: 'close_project' },
      { kind: 'finish_review' },
    ] as const) {
      await expect(hook.result.current.requestLeave(intent)).resolves.toBe('blocked')
    }
    expect(onLeave).not.toHaveBeenCalled()

    await act(() => hook.result.current.discardUnsavedAndProceed())
    expect(onLeave).toHaveBeenCalledWith({ kind: 'finish_review' })
    expect(hook.result.current.editor.status).toBe('idle')
  })
})
