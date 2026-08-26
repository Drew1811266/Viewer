import { act, renderHook, waitFor } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { ReviewProgress, ReviewSessionSnapshot, ViewerCommandError } from '../../api/types'
import type { ReviewPort } from '../workspace/ports'
import { useReviewSessionCoordinator } from './useReviewSessionCoordinator'

function idle(): ReviewSessionSnapshot {
  return {
    phase: 'idle',
    resume: null,
    reviewStreamId: null,
    reviewRoundId: null,
    revision: 0,
    members: [],
    feedback: [],
    restorableFeedbackId: null,
    unreviewable: [],
    conflicts: [],
    counts: { total: 0, feedbackItems: 0, revise: 0, unreviewable: 0, pass: 0 },
    error: null,
  }
}

function active(revision = 1): ReviewSessionSnapshot {
  return {
    ...idle(),
    phase: 'active',
    reviewStreamId: 'stream-1',
    reviewRoundId: 'round-1',
    revision,
    members: [
      {
        assetVersionId: 'asset-version-1',
        entityId: 'image-1',
        relativePath: 'image-1.png',
        displayName: 'image-1.png',
        kind: 'image',
        feedbackItems: 0,
      },
      {
        assetVersionId: 'asset-version-2',
        entityId: 'video-1',
        relativePath: 'video-1.mp4',
        displayName: 'video-1.mp4',
        kind: 'video',
        feedbackItems: 0,
      },
    ],
    counts: { total: 2, feedbackItems: 0, revise: 0, unreviewable: 0, pass: 0 },
  }
}

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (error: unknown) => void
  const promise = new Promise<T>((onResolve, onReject) => {
    resolve = onResolve
    reject = onReject
  })
  return { promise, resolve, reject }
}

function port(status: ReviewSessionSnapshot = idle()): ReviewPort {
  return {
    reviewStatus: vi.fn().mockResolvedValue(status),
    reviewPreviewStart: vi.fn().mockResolvedValue({
      proposalId: 10,
      resolution: { candidateCount: 2, imageCount: 1, videoCount: 1, excludedCount: 0 },
    }),
    reviewStart: vi.fn().mockResolvedValue(active()),
    reviewStartWithFeedback: vi.fn().mockResolvedValue(active()),
    reviewResume: vi.fn().mockResolvedValue(active()),
    reviewAddFeedback: vi.fn().mockResolvedValue(active(2)),
    reviewUpdateFeedback: vi.fn().mockResolvedValue(active(2)),
    reviewUpdateFeedbackText: vi.fn().mockResolvedValue(active(2)),
    reviewReplaceFeedbackAnchor: vi.fn().mockResolvedValue(active(2)),
    reviewDeleteFeedback: vi.fn().mockResolvedValue(active(2)),
    reviewRestoreDeletedFeedback: vi.fn().mockResolvedValue(active(2)),
    reviewCompletionSummary: vi.fn().mockResolvedValue({
      proposalId: 20,
      summary: {
        reviewRoundId: 'round-1',
        revision: 1,
        total: 2,
        revise: 0,
        unreviewable: 0,
        defaultPass: 2,
        feedback: [],
        conflicts: [],
        pending: [],
        canComplete: true,
      },
    }),
    reviewComplete: vi.fn().mockResolvedValue({ ...active(2), phase: 'completed_read_only' }),
    reviewAbandon: vi.fn().mockResolvedValue(idle()),
    reviewCancelTask: vi.fn().mockResolvedValue(true),
    listenReviewProgress: vi.fn().mockResolvedValue(vi.fn()),
  }
}

function options(reviewPort: ReviewPort, sessionId = 'session-1', generation = 1) {
  return {
    port: reviewPort,
    sessionId,
    generation,
    selectedEntityIds: ['image-1'],
  }
}

describe('useReviewSessionCoordinator', () => {
  it('resets owned state, reloads status, and ignores progress from another project generation', async () => {
    const firstPort = port(active())
    let progressHandler: ((progress: ReviewProgress) => void) | undefined
    const unlisten = vi.fn()
    vi.mocked(firstPort.listenReviewProgress).mockImplementation(async (handler) => {
      progressHandler = handler
      return unlisten
    })
    const hook = renderHook(
      ({ sessionId, generation }) =>
        useReviewSessionCoordinator(options(firstPort, sessionId, generation)),
      { initialProps: { sessionId: 'session-1', generation: 1 } },
    )

    await waitFor(() => expect(hook.result.current.snapshot.phase).toBe('active'))
    act(() => {
      hook.result.current.beginCreate()
      hook.result.current.setEditorText('需要返工')
      progressHandler?.({
        sessionId: 'other-session',
        generation: 1,
        taskKind: 'start',
        completed: 1,
        total: 2,
        cancellable: true,
      })
    })
    expect(hook.result.current.progress).toBeNull()

    hook.rerender({ sessionId: 'session-2', generation: 2 })

    await waitFor(() =>
      expect(firstPort.reviewStatus).toHaveBeenLastCalledWith({
        sessionId: 'session-2',
        generation: 2,
      }),
    )
    expect(hook.result.current.editor.text).toBe('')
    expect(hook.result.current.proposal).toBeNull()
    expect(unlisten).toHaveBeenCalledOnce()
  })

  it('previews explicit scopes and confirms only the backend proposal id', async () => {
    const reviewPort = port()
    const hook = renderHook(() => useReviewSessionCoordinator(options(reviewPort)))
    await waitFor(() => expect(reviewPort.reviewStatus).toHaveBeenCalled())

    await act(() => hook.result.current.previewStart({ kind: 'selection', entityIds: ['image-1'] }))
    await act(() => hook.result.current.confirmStart())

    expect(reviewPort.reviewPreviewStart).toHaveBeenCalledWith({
      sessionId: 'session-1',
      generation: 1,
      scope: { kind: 'selection', entityIds: ['image-1'] },
    })
    expect(reviewPort.reviewStart).toHaveBeenCalledWith({
      sessionId: 'session-1',
      generation: 1,
      proposalId: 10,
    })
  })

  it('serializes mutations and builds every guard from the latest saved snapshot', async () => {
    const reviewPort = port(active())
    const first = deferred<ReviewSessionSnapshot>()
    vi.mocked(reviewPort.reviewDeleteFeedback)
      .mockReturnValueOnce(first.promise)
      .mockResolvedValueOnce(active(3))
    const hook = renderHook(() => useReviewSessionCoordinator(options(reviewPort)))
    await waitFor(() => expect(hook.result.current.snapshot.phase).toBe('active'))

    let firstDelete!: Promise<void>
    let secondDelete!: Promise<void>
    act(() => {
      firstDelete = hook.result.current.deleteFeedback('feedback-1')
      secondDelete = hook.result.current.deleteFeedback('feedback-2')
    })
    await waitFor(() => expect(reviewPort.reviewDeleteFeedback).toHaveBeenCalledTimes(1))
    expect(reviewPort.reviewDeleteFeedback).toHaveBeenNthCalledWith(1, {
      sessionId: 'session-1',
      generation: 1,
      reviewRoundId: 'round-1',
      expectedRevision: 1,
      feedbackId: 'feedback-1',
    })

    first.resolve(active(2))
    await act(async () => {
      await firstDelete
      await secondDelete
    })
    expect(reviewPort.reviewDeleteFeedback).toHaveBeenNthCalledWith(2, {
      sessionId: 'session-1',
      generation: 1,
      reviewRoundId: 'round-1',
      expectedRevision: 2,
      feedbackId: 'feedback-2',
    })
    expect(hook.result.current.snapshot.revision).toBe(3)
  })

  it('keeps failed editor text and frozen targets without replacing the saved snapshot', async () => {
    const reviewPort = port(active())
    vi.mocked(reviewPort.reviewAddFeedback).mockRejectedValue({
      code: 'review_save_failed',
      category: 'environment',
      userMessage: '意见尚未保存，请重试。',
      retryable: true,
      taskId: null,
      itemId: null,
    } satisfies ViewerCommandError)
    const hook = renderHook(
      ({ selectedEntityIds }) =>
        useReviewSessionCoordinator({
          ...options(reviewPort),
          selectedEntityIds,
        }),
      { initialProps: { selectedEntityIds: ['image-1'] } },
    )
    await waitFor(() => expect(hook.result.current.snapshot.phase).toBe('active'))

    act(() => {
      hook.result.current.beginCreate()
      hook.result.current.setEditorText('降低高光强度')
    })
    hook.rerender({ selectedEntityIds: ['video-1'] })
    await act(() => hook.result.current.submitFeedback())

    expect(reviewPort.reviewAddFeedback).toHaveBeenCalledWith(
      expect.objectContaining({
        expectedRevision: 1,
        text: '降低高光强度',
        targets: [{ entityId: 'image-1', anchor: { kind: 'asset' } }],
      }),
    )
    expect(hook.result.current.snapshot.revision).toBe(1)
    expect(hook.result.current.editor).toEqual(
      expect.objectContaining({
        text: '降低高光强度',
        targetEntityIds: ['image-1'],
        saveState: 'error',
        error: '意见尚未保存，请重试。',
      }),
    )
  })

  it('clears saved text while retaining the current eligible targets for the next opinion', async () => {
    const reviewPort = port(active())
    const hook = renderHook(() => useReviewSessionCoordinator(options(reviewPort)))
    await waitFor(() => expect(hook.result.current.snapshot.phase).toBe('active'))
    act(() => {
      hook.result.current.beginCreate()
      hook.result.current.setEditorText('第一条意见')
    })

    await act(() => hook.result.current.submitFeedback())

    expect(hook.result.current.editor).toEqual(
      expect.objectContaining({ text: '', targetEntityIds: ['image-1'], saveState: 'idle' }),
    )
  })

  it('reloads a stale snapshot while preserving edit text and targets', async () => {
    const saved = {
      ...active(),
      feedback: [
        {
          feedbackId: 'feedback-1',
          text: '原意见',
          createdAtMs: 1,
          targetEntityIds: ['image-1'],
          targets: [
            {
              assetVersionId: 'asset-version-1',
              entityId: 'image-1',
              anchor: { kind: 'asset' as const },
            },
          ],
          targetCount: 1,
        },
      ],
    }
    const reviewPort = port(saved)
    vi.mocked(reviewPort.reviewStatus)
      .mockResolvedValueOnce(saved)
      .mockResolvedValueOnce({ ...saved, revision: 2 })
    vi.mocked(reviewPort.reviewUpdateFeedbackText).mockRejectedValue({
      code: 'review_stale_revision',
      category: 'conflict',
      userMessage: '评审内容已变化，请刷新后重试。',
      retryable: true,
      taskId: null,
      itemId: null,
    } satisfies ViewerCommandError)
    const hook = renderHook(() => useReviewSessionCoordinator(options(reviewPort)))
    await waitFor(() => expect(hook.result.current.snapshot.revision).toBe(1))

    act(() => {
      hook.result.current.beginEdit('feedback-1')
      hook.result.current.setEditorText('保留我的修改')
    })
    await act(() => hook.result.current.saveEdit())

    expect(reviewPort.reviewStatus).toHaveBeenCalledTimes(2)
    expect(hook.result.current.snapshot.revision).toBe(2)
    expect(hook.result.current.editor).toEqual(
      expect.objectContaining({
        feedbackId: 'feedback-1',
        text: '保留我的修改',
        targetEntityIds: ['image-1'],
        saveState: 'error',
      }),
    )
  })

  it('uses an explicit discard guard for nonempty unsaved editor text', async () => {
    const reviewPort = port(active())
    const continueAction = vi.fn()
    const hook = renderHook(() => useReviewSessionCoordinator(options(reviewPort)))
    await waitFor(() => expect(hook.result.current.snapshot.phase).toBe('active'))
    act(() => {
      hook.result.current.beginCreate()
      hook.result.current.setEditorText('尚未保存')
    })

    let discardedImmediately = true
    act(() => {
      discardedImmediately = hook.result.current.requestDiscard('project_close', continueAction)
    })
    expect(discardedImmediately).toBe(false)
    expect(continueAction).not.toHaveBeenCalled()
    expect(hook.result.current.discardConfirmation).toEqual({ reason: 'project_close' })

    act(() => hook.result.current.confirmDiscard())
    expect(continueAction).toHaveBeenCalledOnce()
    expect(hook.result.current.editor.text).toBe('')
  })

  it('prepares and completes with backend counts, and cancels only the review task', async () => {
    const reviewPort = port(active())
    const hook = renderHook(() => useReviewSessionCoordinator(options(reviewPort)))
    await waitFor(() => expect(hook.result.current.snapshot.phase).toBe('active'))

    await act(() => hook.result.current.prepareCompletion())
    expect(hook.result.current.completion?.summary.defaultPass).toBe(2)
    await act(() => hook.result.current.confirmCompletion())
    await act(() => hook.result.current.cancelTask())

    expect(reviewPort.reviewComplete).toHaveBeenCalledWith({
      sessionId: 'session-1',
      generation: 1,
      reviewRoundId: 'round-1',
      expectedRevision: 1,
      proposalId: 20,
    })
    expect(reviewPort.reviewCancelTask).toHaveBeenCalledWith({
      sessionId: 'session-1',
      generation: 1,
    })
  })
})
