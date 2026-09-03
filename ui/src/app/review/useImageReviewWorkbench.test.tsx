import { act, renderHook, waitFor } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { ReviewAnchor, ReviewSessionSnapshot } from '../../api/types'
import {
  type ImageReviewSaveRequest,
  type ImageReviewWorkbenchAdapter,
  legacyImageReviewWorkbenchAdapter,
} from './imageReviewWorkbenchAdapter'
import { useImageReviewWorkbench } from './useImageReviewWorkbench'
import type { ReviewSessionCoordinator } from './useReviewSessionCoordinator'

const RECT: ReviewAnchor = { kind: 'image_rect', x: 0.1, y: 0.2, width: 0.3, height: 0.4 }
const ELLIPSE: ReviewAnchor = {
  kind: 'image_ellipse',
  x: 0.2,
  y: 0.2,
  width: 0.3,
  height: 0.4,
}
const STROKE: ReviewAnchor = {
  kind: 'image_stroke',
  points: [
    { x: 0.1, y: 0.2 },
    { x: 0.3, y: 0.4 },
  ],
}

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
    captureStart: vi.fn().mockResolvedValue(undefined),
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
    const adapter = legacyImageReviewWorkbenchAdapter(review, scope)
    const hook = renderHook(() => useImageReviewWorkbench({ adapter, entityId: 'image-1' }))

    await waitFor(() => expect(review.captureStart).toHaveBeenCalledOnce())
    expect(review.captureStart).toHaveBeenCalledWith(scope)
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
    expect(review.captureStart).toHaveBeenCalledOnce()
    expect(hook.result.current.feedback).toEqual([
      expect.objectContaining({ feedbackId: 'feedback-1', text: '服务端意见', anchor: RECT }),
    ])
    expect(hook.result.current.editor).toEqual(
      expect.objectContaining({
        selectedItemId: 'feedback-1',
        phase: { status: 'idle' },
      }),
    )
    expect(hook.result.current.statusMessage).toBeNull()
  })

  it('keeps an extended markup tool active after its feedback saves', async () => {
    const review = coordinator()
    const adapter = legacyImageReviewWorkbenchAdapter(review, {
      kind: 'selection',
      entityIds: ['image-1'],
    })
    const hook = renderHook(() => useImageReviewWorkbench({ adapter, entityId: 'image-1' }))

    act(() => {
      hook.result.current.setTool('ellipse')
      expect(hook.result.current.beginDrawing(ELLIPSE)).toBe(true)
    })
    await act(() => hook.result.current.finishDrawing(ELLIPSE))
    act(() => hook.result.current.updateDraftText('调整脸部轮廓'))
    await act(() => hook.result.current.saveDraft())

    expect(hook.result.current.tool).toBe('ellipse')
    expect(hook.result.current.editor.phase.status).toBe('idle')
  })

  it('keeps unsaved text and anchor after a command failure', async () => {
    const review = coordinator(snapshot('active'))
    vi.mocked(review.addAnchoredFeedback).mockRejectedValue({
      userMessage: '意见尚未保存，请重试。',
    })
    const adapter = legacyImageReviewWorkbenchAdapter(review, {
      kind: 'selection',
      entityIds: ['image-1'],
    })
    const hook = renderHook(() =>
      useImageReviewWorkbench({
        adapter,
        entityId: 'image-1',
      }),
    )
    act(() => {
      hook.result.current.beginAnnotation(RECT)
      hook.result.current.updateDraftText('保留这段文字')
    })
    await act(() => hook.result.current.saveDraft())

    expect(hook.result.current.editor.phase).toEqual(
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
    const adapter = legacyImageReviewWorkbenchAdapter(review, {
      kind: 'selection',
      entityIds: ['image-1'],
    })
    const hook = renderHook(() =>
      useImageReviewWorkbench({
        adapter,
        entityId: 'image-1',
        onLeave,
      }),
    )
    act(() => hook.result.current.beginAnnotation(ELLIPSE))

    for (const intent of [
      { kind: 'navigate', offset: 1 },
      { kind: 'return_grid' },
      { kind: 'close_project' },
      { kind: 'finish_review' },
    ] as const) {
      let outcome: 'blocked' | 'proceeded' | null = null
      await act(async () => {
        outcome = await hook.result.current.requestLeave(intent)
      })
      expect(outcome).toBe('blocked')
    }
    expect(onLeave).not.toHaveBeenCalled()
    expect(hook.result.current.leaveConfirmation).toEqual({ kind: 'finish_review' })

    act(() => hook.result.current.cancelLeave())
    expect(hook.result.current.leaveConfirmation).toBeNull()
    await act(() => hook.result.current.requestLeave({ kind: 'finish_review' }))

    await act(() => hook.result.current.discardUnsavedAndProceed())
    expect(onLeave).toHaveBeenCalledWith({ kind: 'finish_review' })
    expect(hook.result.current.editor.phase.status).toBe('idle')
  })

  it('never rebinds retained draft geometry when the opened entity changes underneath it', async () => {
    const review = coordinator(snapshot('active'))
    const adapter = legacyImageReviewWorkbenchAdapter(review, {
      kind: 'selection',
      entityIds: ['image-1', 'image-2'],
    })
    const hook = renderHook(({ entityId }) => useImageReviewWorkbench({ adapter, entityId }), {
      initialProps: { entityId: 'image-1' },
    })
    act(() => {
      hook.result.current.beginAnnotation(RECT)
      hook.result.current.updateDraftText('只属于图1')
    })
    hook.rerender({ entityId: 'image-2' })
    await act(() => hook.result.current.saveDraft())

    expect(review.addAnchoredFeedback).not.toHaveBeenCalled()
    expect(hook.result.current.editor.phase).toMatchObject({
      status: 'save_error',
      text: '只属于图1',
      draftAnchor: RECT,
      message: '这条未保存意见属于上一素材版本或评审会话，请返回原上下文或放弃后重画。',
    })
  })

  it('discards retained adapter input before allowing a failed draft to be cancelled', async () => {
    const review = coordinator(snapshot('active'))
    vi.mocked(review.addAnchoredFeedback)
      .mockRejectedValueOnce({ userMessage: '暂时写入失败' })
      .mockResolvedValueOnce(saved('第二条已保存'))
    const discardPendingInput = vi.fn(() => true)
    const adapter = {
      ...legacyImageReviewWorkbenchAdapter(review, {
        kind: 'selection',
        entityIds: ['image-1'],
      }),
      discardPendingInput,
    }
    const hook = renderHook(() => useImageReviewWorkbench({ adapter, entityId: 'image-1' }))

    act(() => {
      hook.result.current.beginAnnotation(RECT)
      hook.result.current.updateDraftText('第一条失败意见')
    })
    await act(() => hook.result.current.saveDraft())
    expect(hook.result.current.editor.phase.status).toBe('save_error')
    act(() => hook.result.current.cancelDraft())
    expect(discardPendingInput).toHaveBeenCalledOnce()
    expect(hook.result.current.editor.phase.status).toBe('idle')

    act(() => {
      hook.result.current.beginAnnotation(RECT)
      hook.result.current.updateDraftText('第二条新意见')
    })
    await act(() => hook.result.current.saveDraft())
    expect(review.addAnchoredFeedback).toHaveBeenCalledTimes(2)
    expect(hook.result.current.editor.phase.status).toBe('idle')
  })

  it('keeps the complete item revision frozen while a text editor is open', async () => {
    let revision = 'target-revision-1'
    let viewVersion = 1
    const saveFeedback = vi.fn(async () => workbenchView(revision))
    const adapter: ImageReviewWorkbenchAdapter = {
      protocol: 'continuous',
      preparationKey: (entityId) => `test:${entityId}`,
      viewKey: () => `view:${viewVersion}`,
      prepareEntity: vi.fn(async (entityId) => ({ key: `test:${entityId}`, prepared: null })),
      view: () => workbenchView(revision),
      saveFeedback,
      deleteFeedback: vi.fn(async () => workbenchView(revision)),
      restoreDeletedFeedback: vi.fn(async () => workbenchView(revision)),
      discardPendingInput: vi.fn(() => true),
    }
    const hook = renderHook(() => useImageReviewWorkbench({ adapter, entityId: 'image-1' }))
    await waitFor(() => expect(hook.result.current.feedback).toHaveLength(1))
    act(() => hook.result.current.beginFeedbackTextEdit('target-1'))

    revision = 'target-revision-2'
    viewVersion += 1
    hook.rerender()
    await waitFor(() =>
      expect(hook.result.current.feedback[0]?.targetKey?.targetRevisionId).toBe(
        'target-revision-2',
      ),
    )
    act(() => hook.result.current.updateDraftText('基于旧版本的编辑'))
    await act(() => hook.result.current.saveDraft())

    expect(saveFeedback).toHaveBeenCalledWith(
      expect.objectContaining({
        item: expect.objectContaining({
          targetKey: expect.objectContaining({ targetRevisionId: 'target-revision-1' }),
        }),
      }),
    )
  })

  it('keeps the prepared preview identity stable while annotation input rerenders', async () => {
    const adapter: ImageReviewWorkbenchAdapter = {
      protocol: 'continuous',
      preparationKey: (entityId) => `test:${entityId}`,
      viewKey: () => 'view:1',
      prepareEntity: vi.fn(async () => ({
        key: 'test:image-1',
        prepared: {
          asset: {
            id: 'asset-1',
            sourceEntityId: 'image-1',
            relativePath: 'image-1.png',
            evidence: { sizeBytes: 100, modifiedNs: '1', blake3: 'ab'.repeat(32) },
            media: { kind: 'image' as const, width: 640, height: 480 },
            producerAssetId: null,
            parentAssetVersionId: null,
          },
          preview: {
            assetVersionId: 'asset-1',
            role: 'base' as const,
            url: 'viewer-review-image://localhost/asset-1',
            width: 640,
            height: 480,
            sourceWidth: 640,
            sourceHeight: 480,
          },
        },
      })),
      view: () => ({
        feedback: [],
        readOnlyReason: null,
        restorableItemId: null,
        statusMessage: null,
      }),
      saveFeedback: vi.fn(async () => workbenchView('target-revision-1')),
      deleteFeedback: vi.fn(async () => workbenchView('target-revision-1')),
      restoreDeletedFeedback: vi.fn(async () => workbenchView('target-revision-1')),
      discardPendingInput: vi.fn(() => true),
    }
    const hook = renderHook(() => useImageReviewWorkbench({ adapter, entityId: 'image-1' }))
    await waitFor(() => expect(hook.result.current.preparedImage?.assetVersionId).toBe('asset-1'))
    const preparedImage = hook.result.current.preparedImage

    act(() => {
      hook.result.current.beginAnnotation(RECT)
      hook.result.current.updateDraftText('输入不会重载图片')
    })

    expect(hook.result.current.preparedImage).toBe(preparedImage)
  })

  it('redraws with the latest text revision immediately after a text save', async () => {
    let currentView = workbenchView('target-revision-1', 'text-revision-1')
    const saveFeedback = vi.fn(async (_request: ImageReviewSaveRequest) => {
      currentView = workbenchView('target-revision-1', 'text-revision-2')
      return currentView
    })
    const adapter: ImageReviewWorkbenchAdapter = {
      protocol: 'continuous',
      preparationKey: (entityId) => `test:${entityId}`,
      viewKey: () => 'view:1',
      prepareEntity: vi.fn(async (entityId) => ({ key: `test:${entityId}`, prepared: null })),
      view: () => currentView,
      saveFeedback,
      deleteFeedback: vi.fn(async () => currentView),
      restoreDeletedFeedback: vi.fn(async () => currentView),
      discardPendingInput: vi.fn(() => true),
    }
    const hook = renderHook(() => useImageReviewWorkbench({ adapter, entityId: 'image-1' }))
    await waitFor(() => expect(hook.result.current.feedback).toHaveLength(1))
    act(() => {
      hook.result.current.beginFeedbackTextEdit('target-1')
      hook.result.current.updateDraftText('更新后的文字')
    })
    await act(() => hook.result.current.saveDraft())

    act(() => {
      hook.result.current.setTool('brush')
      expect(hook.result.current.beginDrawing(STROKE, 'target-1')).toBe(true)
    })
    await act(() =>
      hook.result.current.finishDrawing({
        kind: 'image_stroke',
        points: [
          { x: 0.2, y: 0.2 },
          { x: 0.4, y: 0.4 },
        ],
      }),
    )

    expect(saveFeedback).toHaveBeenCalledTimes(2)
    expect(saveFeedback.mock.calls[1]?.[0].item?.targetKey?.textRevisionId).toBe('text-revision-2')
  })

  it('never submits a draft into a replacement session with the same entity id', async () => {
    const first = sessionAdapter('session-a')
    const replacement = sessionAdapter('session-b')
    const hook = renderHook(
      ({ adapter }) => useImageReviewWorkbench({ adapter, entityId: 'image-1' }),
      { initialProps: { adapter: first } },
    )
    await waitFor(() => expect(first.prepareEntity).toHaveBeenCalledOnce())
    act(() => {
      hook.result.current.beginAnnotation(RECT)
      hook.result.current.updateDraftText('只属于会话 A')
    })

    hook.rerender({ adapter: replacement })
    await waitFor(() => expect(replacement.prepareEntity).toHaveBeenCalledOnce())
    await act(() => hook.result.current.saveDraft())

    expect(replacement.saveFeedback).not.toHaveBeenCalled()
    expect(hook.result.current.editor.phase).toMatchObject({
      status: 'save_error',
      text: '只属于会话 A',
      message: '这条未保存意见属于上一素材版本或评审会话，请返回原上下文或放弃后重画。',
    })
  })
})

function sessionAdapter(sessionKey: string): ImageReviewWorkbenchAdapter {
  return {
    protocol: 'continuous',
    preparationKey: (entityId) => `${sessionKey}:${entityId}`,
    viewKey: () => `${sessionKey}:view`,
    prepareEntity: vi.fn(async (entityId) => ({
      key: `${sessionKey}:${entityId}`,
      prepared: null,
    })),
    view: () => ({
      feedback: [],
      readOnlyReason: null,
      restorableItemId: null,
      statusMessage: null,
    }),
    saveFeedback: vi.fn(async () => workbenchView('target-revision-1')),
    deleteFeedback: vi.fn(async () => workbenchView('target-revision-1')),
    restoreDeletedFeedback: vi.fn(async () => workbenchView('target-revision-1')),
    discardPendingInput: vi.fn(() => true),
  }
}

function workbenchView(targetRevisionId: string, textRevisionId = 'text-revision-1') {
  return {
    feedback: [
      {
        itemId: 'target-1',
        feedbackId: 'feedback-1',
        targetKey: {
          feedbackId: 'feedback-1',
          textRevisionId,
          targetId: 'target-1',
          targetRevisionId,
        },
        assetVersionId: 'asset-1',
        text: '原意见',
        createdAtMs: 10,
        ordinal: 1,
        anchor: RECT,
      },
    ],
    readOnlyReason: null,
    restorableItemId: null,
    statusMessage: null,
  }
}
