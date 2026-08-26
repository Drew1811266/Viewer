import { fireEvent, render, screen, within } from '@testing-library/react'
import { createRef } from 'react'
import { describe, expect, it, vi } from 'vitest'
import type {
  ReviewCompletionProposal,
  ReviewScopeRequest,
  ReviewSessionSnapshot,
} from '../../api/types'
import type { ReviewSessionCoordinator } from '../../app/review/useReviewSessionCoordinator'
import ReviewCompletionDialog from './ReviewCompletionDialog'
import ReviewInspector from './ReviewInspector'
import ReviewRecoveryNotice from './ReviewRecoveryNotice'
import ReviewStartDialog from './ReviewStartDialog'
import ReviewWorkspaceLayer, { ReviewToolbarAction } from './ReviewWorkspaceLayer'

function idle(): ReviewSessionSnapshot {
  return {
    phase: 'idle',
    resume: null,
    reviewStreamId: null,
    reviewRoundId: null,
    revision: 0,
    members: [],
    feedback: [],
    unreviewable: [],
    conflicts: [],
    counts: { total: 0, feedbackItems: 0, revise: 0, unreviewable: 0, pass: 0 },
    error: null,
  }
}

function active(): ReviewSessionSnapshot {
  return {
    ...idle(),
    phase: 'active',
    reviewStreamId: 'stream-1',
    reviewRoundId: 'round-1',
    revision: 3,
    members: [
      {
        assetVersionId: 'asset-1',
        entityId: 'image-1',
        relativePath: 'shots/image-1.png',
        displayName: 'image-1.png',
        kind: 'image',
        feedbackItems: 1,
      },
      {
        assetVersionId: 'asset-2',
        entityId: 'video-1',
        relativePath: 'shots/video-1.mp4',
        displayName: 'video-1.mp4',
        kind: 'video',
        feedbackItems: 0,
      },
    ],
    feedback: [
      {
        feedbackId: 'feedback-1',
        text: '降低高光强度',
        createdAtMs: 1,
        targetEntityIds: ['image-1'],
        targetCount: 1,
      },
    ],
    counts: { total: 2, feedbackItems: 1, revise: 0, unreviewable: 0, pass: 0 },
  }
}

function completion(canComplete = true): ReviewCompletionProposal {
  return {
    proposalId: 9,
    summary: {
      reviewRoundId: 'round-1',
      revision: 3,
      total: 4,
      revise: 2,
      unreviewable: 1,
      defaultPass: 1,
      feedback: [{ feedbackId: 'feedback-1', text: '降低高光强度', targetCount: 2 }],
      conflicts: canComplete
        ? []
        : [
            {
              assetVersionId: 'asset-1',
              relativePath: 'shots/image-1.png',
              kind: 'content_changed',
            },
          ],
      pending: [],
      canComplete,
    },
  }
}

function coordinator(
  snapshot: ReviewSessionSnapshot = idle(),
  overrides: Partial<ReviewSessionCoordinator> = {},
): ReviewSessionCoordinator {
  return {
    snapshot,
    proposal: null,
    completion: null,
    editor: {
      mode: 'create',
      feedbackId: null,
      text: '',
      savedText: '',
      targetEntityIds: ['image-1'],
      saveState: 'idle',
      error: null,
    },
    progress: null,
    error: null,
    discardConfirmation: null,
    previewStart: vi.fn().mockResolvedValue(undefined),
    confirmStart: vi.fn().mockResolvedValue(undefined),
    dismissStart: vi.fn(),
    resume: vi.fn().mockResolvedValue(undefined),
    beginCreate: vi.fn(),
    setEditorText: vi.fn(),
    submitFeedback: vi.fn().mockResolvedValue(undefined),
    beginEdit: vi.fn(),
    saveEdit: vi.fn().mockResolvedValue(undefined),
    deleteFeedback: vi.fn().mockResolvedValue(undefined),
    prepareCompletion: vi.fn().mockResolvedValue(undefined),
    confirmCompletion: vi.fn().mockResolvedValue(undefined),
    dismissCompletion: vi.fn(),
    abandon: vi.fn().mockResolvedValue(undefined),
    cancelTask: vi.fn().mockResolvedValue(undefined),
    requestDiscard: vi.fn().mockImplementation((_reason, continuation) => {
      continuation?.()
      return true
    }),
    confirmDiscard: vi.fn(),
    cancelDiscard: vi.fn(),
    ...overrides,
  }
}

describe('review workspace components', () => {
  it('keeps start review visible in the main toolbar and explains disabled search scope', () => {
    const review = coordinator()
    const scope: ReviewScopeRequest = {
      kind: 'folder',
      folderId: 'folder-1',
      includeDescendants: true,
    }
    const rendered = render(
      <ReviewToolbarAction review={review} scope={scope} projectAccess="read_write" />,
    )

    fireEvent.click(screen.getByRole('button', { name: '开始评审' }))
    expect(review.previewStart).toHaveBeenCalledWith(scope)

    rendered.rerender(
      <ReviewToolbarAction review={review} scope={null} projectAccess="read_write" />,
    )
    expect(screen.getByRole('button', { name: '开始评审' })).toBeDisabled()
    expect(screen.getByText('请先选择一个或多个搜索结果。')).toBeInTheDocument()

    rendered.rerender(
      <ReviewToolbarAction review={review} scope={scope} projectAccess="read_only" />,
    )
    expect(screen.getByRole('button', { name: '开始评审' })).toBeDisabled()
    expect(screen.getByText('当前项目为只读，不能开始新的评审。')).toBeInTheDocument()
  })

  it('confirms the backend-resolved start counts without reconstructing evidence', () => {
    const confirm = vi.fn()
    const cancel = vi.fn()
    const trigger = createRef<HTMLButtonElement>()
    render(
      <>
        <button ref={trigger} type="button">
          入口
        </button>
        <ReviewStartDialog
          proposal={{
            proposalId: 7,
            resolution: { candidateCount: 5, imageCount: 2, videoCount: 1, excludedCount: 2 },
          }}
          busy={false}
          onConfirm={confirm}
          onCancel={cancel}
          returnFocusRef={trigger}
        />
      </>,
    )

    const dialog = screen.getByRole('dialog', { name: '确认本轮评审范围' })
    expect(dialog).toHaveTextContent('图片2')
    expect(dialog).toHaveTextContent('视频1')
    expect(dialog).toHaveTextContent('排除2')
    fireEvent.click(within(dialog).getByRole('button', { name: '固定 3 项并开始' }))
    expect(confirm).toHaveBeenCalledOnce()
    fireEvent.keyDown(dialog, { key: 'Escape' })
    expect(cancel).toHaveBeenCalledOnce()
  })

  it('shows fixed backend counts, no viewed metric, and routes review context actions', () => {
    const review = coordinator(active())
    const returnToMembers = vi.fn()
    render(
      <ReviewWorkspaceLayer
        review={review}
        selectedEntityIds={['image-1']}
        projectAccess="read_write"
        onReturnToMembers={returnToMembers}
      />,
    )

    const context = screen.getByRole('region', { name: '本轮评审上下文' })
    expect(context).toHaveTextContent('固定素材 2')
    expect(context).toHaveTextContent('意见 1')
    expect(context).toHaveTextContent('不可评审 0')
    expect(context).not.toHaveTextContent(/已浏览|浏览进度/)
    fireEvent.click(within(context).getByRole('button', { name: '返回本轮素材' }))
    expect(returnToMembers).toHaveBeenCalledWith(['image-1', 'video-1'])
    fireEvent.click(within(context).getByRole('button', { name: '完成本轮' }))
    expect(review.requestDiscard).toHaveBeenCalledWith('context_replacement', expect.any(Function))
    expect(review.prepareCompletion).toHaveBeenCalledOnce()
  })

  it('supports natural-language create/edit/delete with frozen member targets and Command-Enter', () => {
    const review = coordinator(active())
    render(
      <ReviewInspector
        review={review}
        selectedEntityIds={['image-1', 'outside-1']}
        readOnly={false}
        onClose={vi.fn()}
      />,
    )

    expect(screen.getByText('1 个已选目标可添加意见')).toBeVisible()
    expect(screen.getByText('1 个所选项目不属于本轮素材')).toBeVisible()
    const editor = screen.getByRole('textbox', { name: '返工意见' })
    fireEvent.change(editor, { target: { value: '减少运动模糊' } })
    expect(review.setEditorText).toHaveBeenCalledWith('减少运动模糊')
    fireEvent.keyDown(editor, { key: 'Enter', metaKey: true })
    expect(review.submitFeedback).toHaveBeenCalledOnce()

    fireEvent.click(screen.getByRole('button', { name: '编辑意见：降低高光强度' }))
    expect(review.beginEdit).toHaveBeenCalledWith('feedback-1')
    fireEvent.click(screen.getByRole('button', { name: '删除意见：降低高光强度' }))
    expect(review.deleteFeedback).toHaveBeenCalledWith('feedback-1')
  })

  it('keeps unsaved and saving feedback explicit and guards inspector close', () => {
    const review = coordinator(active(), {
      editor: {
        mode: 'create',
        feedbackId: null,
        text: '未保存的意见',
        savedText: '',
        targetEntityIds: ['image-1'],
        saveState: 'error',
        error: '意见尚未保存，请重试。',
      },
    })
    const close = vi.fn()
    render(
      <ReviewInspector
        review={review}
        selectedEntityIds={['image-1']}
        readOnly={false}
        onClose={close}
      />,
    )

    expect(screen.getByRole('status')).toHaveTextContent('意见尚未保存，请重试。')
    fireEvent.click(screen.getByRole('button', { name: '关闭评审意见面板' }))
    expect(review.requestDiscard).toHaveBeenCalledWith('inspector_close', close)
  })

  it('requires explicit resume and contains busy/read-only/recovery failures locally', () => {
    const resume = {
      ...idle(),
      resume: {
        reviewStreamId: 'stream-1',
        reviewRoundId: 'round-1',
        createdAtMs: 1,
        total: 8,
        feedbackItems: 2,
      },
    }
    const review = coordinator(resume)
    const rendered = render(<ReviewRecoveryNotice review={review} projectAccess="read_write" />)
    expect(screen.getByText('发现未完成的评审')).toBeVisible()
    fireEvent.click(screen.getByRole('button', { name: '继续评审' }))
    expect(review.resume).toHaveBeenCalledOnce()

    rendered.rerender(
      <ReviewRecoveryNotice
        review={coordinator({
          ...idle(),
          phase: 'write_unavailable',
          error: { code: 'review_busy', retryable: true, affectedPaths: [] },
        })}
        projectAccess="read_write"
      />,
    )
    expect(screen.getByText('评审暂时不可写')).toBeVisible()
    expect(screen.getByText(/普通浏览和预览仍可继续/)).toBeVisible()

    rendered.rerender(
      <ReviewRecoveryNotice
        review={coordinator({ ...idle(), phase: 'recovery_required' })}
        projectAccess="read_write"
      />,
    )
    expect(screen.getByText('评审记录需要恢复处理')).toBeVisible()
  })

  it('renders backend completion outcomes and blocks conflict publication', () => {
    const confirm = vi.fn()
    const rendered = render(
      <ReviewCompletionDialog
        proposal={completion(false)}
        busy={false}
        onConfirm={confirm}
        onCancel={vi.fn()}
      />,
    )

    const dialog = screen.getByRole('dialog', { name: '完成本轮评审' })
    expect(dialog).toHaveTextContent('返工2')
    expect(dialog).toHaveTextContent('不可评审1')
    expect(dialog).toHaveTextContent('默认通过1')
    expect(dialog).toHaveTextContent('降低高光强度')
    expect(dialog).toHaveTextContent('shots/image-1.png')
    expect(within(dialog).getByRole('button', { name: '确认完成本轮' })).toBeDisabled()

    rendered.rerender(
      <ReviewCompletionDialog
        proposal={completion(true)}
        busy={false}
        onConfirm={confirm}
        onCancel={vi.fn()}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: '确认完成本轮' }))
    expect(confirm).toHaveBeenCalledOnce()
  })

  it('keeps a completed round read-only', () => {
    const review = coordinator({ ...active(), phase: 'completed_read_only' })
    render(
      <ReviewWorkspaceLayer
        review={review}
        selectedEntityIds={['image-1']}
        projectAccess="read_write"
        onReturnToMembers={vi.fn()}
      />,
    )

    expect(screen.getByText('本轮已完成，只读保存')).toBeVisible()
    fireEvent.click(screen.getByRole('button', { name: '查看评审意见' }))
    expect(screen.queryByRole('textbox', { name: '返工意见' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /编辑意见/ })).not.toBeInTheDocument()
  })
})
