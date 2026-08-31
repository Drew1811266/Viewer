import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { createRef } from 'react'
import { describe, expect, it, vi } from 'vitest'
import type { ReviewArchivePlan, ReviewArchiveSelection } from '../../api/reviewWorkspaceTypes'
import type {
  ReviewCompletionProposal,
  ReviewScopeRequest,
  ReviewSessionSnapshot,
} from '../../app/review/reviewModel'
import type { ContinuousReviewCoordinator } from '../../app/review/useContinuousReviewCoordinator'
import type { ReviewSessionCoordinator } from '../../app/review/useReviewSessionCoordinator'
import ReviewCompletionDialog from './ReviewCompletionDialog'
import ReviewContextBar from './ReviewContextBar'
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
    restorableFeedbackId: null,
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
        targets: [
          {
            assetVersionId: 'asset-1',
            entityId: 'image-1',
            anchor: { kind: 'asset' },
          },
        ],
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
    captureStart: vi.fn().mockResolvedValue(undefined),
    startWithFeedback: vi.fn().mockResolvedValue(snapshot),
    addAnchoredFeedback: vi.fn().mockResolvedValue(snapshot),
    updateAnchoredFeedbackText: vi.fn().mockResolvedValue(snapshot),
    replaceAnchoredFeedbackAnchor: vi.fn().mockResolvedValue(snapshot),
    restoreDeletedFeedback: vi.fn().mockResolvedValue(snapshot),
    confirmStart: vi.fn().mockResolvedValue(undefined),
    dismissStart: vi.fn(),
    resume: vi.fn().mockResolvedValue(undefined),
    beginCreate: vi.fn(),
    setEditorText: vi.fn(),
    submitFeedback: vi.fn().mockResolvedValue(undefined),
    beginEdit: vi.fn(),
    saveEdit: vi.fn().mockResolvedValue(undefined),
    deleteFeedback: vi.fn().mockResolvedValue(snapshot),
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

const archiveTarget = {
  feedbackId: 'feedback-c',
  textRevisionId: 'text-c',
  targetId: 'target-c',
  targetRevisionId: 'target-c-revision',
}

function archivePlan(selection: ReviewArchiveSelection): ReviewArchivePlan {
  return {
    ...structuredClone(selection),
    removed: structuredClone(selection.groups.flatMap((group) => group.targets)),
    retained: [],
    alreadyCovered: [],
  }
}

function continuousArchiveCoordinator(
  workbenchSessionKey: string,
  overrides: Partial<ContinuousReviewCoordinator> = {},
): ContinuousReviewCoordinator {
  return {
    workbenchSessionKey,
    currentSnapshotId: 'snapshot-c',
    hasUncommittedInput: false,
    view: {
      current: {
        reference: { snapshotId: 'snapshot-c', blake3: 'c'.repeat(64) },
        production: null,
        state: {
          projectId: 'project-c',
          streamId: 'stream-c',
          snapshotId: 'snapshot-c',
          parent: null,
          assets: [],
          feedback: [
            {
              id: archiveTarget.feedbackId,
              textRevisionId: archiveTarget.textRevisionId,
              text: '袖口收紧',
              createdAtMs: 1,
              historyRef: null,
              targets: [
                {
                  id: archiveTarget.targetId,
                  revisionId: archiveTarget.targetRevisionId,
                  assetVersionId: 'asset-c',
                  anchor: { kind: 'asset' },
                  availability: { kind: 'ready' },
                },
              ],
            },
          ],
        },
        commandId: 'command-c',
        payloadDigest: 'd'.repeat(64),
        changes: [],
        evidence: [],
      },
      streamId: 'stream-c',
      sourceChecks: [],
      projection: { actionable: [], needsConfirmation: [] },
      recovery: [],
      migration: null,
      capabilities: { continuousEditing: true, usageImport: true, migration: false },
    },
    previewArchive: vi.fn(async (selection: ReviewArchiveSelection) => archivePlan(selection)),
    commitArchive: vi.fn(async () => undefined),
    ...overrides,
  } as unknown as ContinuousReviewCoordinator
}

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (cause: unknown) => void
  const promise = new Promise<T>((done, fail) => {
    resolve = done
    reject = fail
  })
  return { promise, resolve, reject }
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
    fireEvent.click(within(context).getByRole('button', { name: '完成本轮评审' }))
    expect(review.requestDiscard).toHaveBeenCalledWith('context_replacement', expect.any(Function))
    expect(review.prepareCompletion).toHaveBeenCalledOnce()
  })

  it('presents continuous review as current feedback plus archive/history, never round completion', () => {
    const archive = vi.fn()
    const history = vi.fn()
    render(
      <ReviewContextBar
        protocol="continuous"
        snapshot={active()}
        inspectorOpen={false}
        onReturnToMembers={vi.fn()}
        onToggleInspector={vi.fn()}
        onPrepareCompletion={vi.fn()}
        onRequestAbandon={vi.fn()}
        onArchive={archive}
        onHistory={history}
      />,
    )
    const context = screen.getByRole('region', { name: '持续评审上下文' })
    expect(context).toHaveTextContent('当前意见 1')
    expect(within(context).queryByRole('button', { name: '完成本轮评审' })).toBeNull()
    fireEvent.click(within(context).getByRole('button', { name: '存档意见' }))
    fireEvent.click(within(context).getByRole('button', { name: '历史' }))
    expect(archive).toHaveBeenCalledOnce()
    expect(history).toHaveBeenCalledOnce()
  })

  it('previews a supplied exact known basis instead of replacing it with an unknown selection', async () => {
    const selection: ReviewArchiveSelection = {
      expectedSnapshotId: 'snapshot-c',
      groups: [
        {
          basis: {
            kind: 'known',
            snapshot: { snapshotId: 'snapshot-b', blake3: 'b'.repeat(64) },
            source: { kind: 'agent_declared', usageId: 'usage-b' },
          },
          targets: [structuredClone(archiveTarget)],
        },
      ],
    }
    const original = structuredClone(selection)
    const pending = deferred<ReviewArchivePlan>()
    const continuous = continuousArchiveCoordinator('continuous-c', {
      previewArchive: vi.fn(() => pending.promise),
    })
    render(
      <ReviewWorkspaceLayer
        review={coordinator(active())}
        selectedEntityIds={[]}
        projectAccess="read_write"
        continuousReview={continuous}
        archiveSelection={selection}
        onReturnToMembers={vi.fn()}
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: '存档意见' }))
    await waitFor(() => expect(continuous.previewArchive).toHaveBeenCalledOnce())
    const suppliedTarget = selection.groups[0]?.targets[0]
    if (suppliedTarget === undefined) throw new Error('Missing supplied archive target')
    suppliedTarget.targetId = 'mutated-target'
    expect(continuous.previewArchive).toHaveBeenCalledWith(original)
    await act(async () => {
      pending.resolve(archivePlan(original))
      await pending.promise
    })
    expect(screen.getByText('已核对交接版本')).toBeVisible()
  })

  it('preserves a manually selected B basis as an exact archive selection', async () => {
    const selection: ReviewArchiveSelection = {
      expectedSnapshotId: 'snapshot-c',
      groups: [
        {
          basis: {
            kind: 'known',
            snapshot: { snapshotId: 'snapshot-b', blake3: 'b'.repeat(64) },
            source: { kind: 'user_selected' },
          },
          targets: [structuredClone(archiveTarget)],
        },
      ],
    }
    const continuous = continuousArchiveCoordinator('continuous-c')
    render(
      <ReviewWorkspaceLayer
        review={coordinator(active())}
        selectedEntityIds={[]}
        projectAccess="read_write"
        continuousReview={continuous}
        archiveSelection={selection}
        onReturnToMembers={vi.fn()}
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: '存档意见' }))
    await waitFor(() => expect(continuous.previewArchive).toHaveBeenCalledWith(selection))
    expect(screen.getByText('已核对交接版本')).toBeVisible()
  })

  it('rejects a supplied selection that is no longer bound to the current C snapshot', () => {
    const continuous = continuousArchiveCoordinator('continuous-c')
    render(
      <ReviewWorkspaceLayer
        review={coordinator(active())}
        selectedEntityIds={[]}
        projectAccess="read_write"
        continuousReview={continuous}
        archiveSelection={{
          expectedSnapshotId: 'snapshot-before-c',
          groups: [{ basis: { kind: 'unknown' }, targets: [structuredClone(archiveTarget)] }],
        }}
        onReturnToMembers={vi.fn()}
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: '存档意见' }))
    expect(continuous.previewArchive).not.toHaveBeenCalled()
    expect(screen.getByRole('status')).toHaveTextContent('意见已变化，请重新查看存档范围')
  })

  it('does not preview an empty supplied selection', () => {
    const continuous = continuousArchiveCoordinator('continuous-c')
    render(
      <ReviewWorkspaceLayer
        review={coordinator(active())}
        selectedEntityIds={[]}
        projectAccess="read_write"
        continuousReview={continuous}
        archiveSelection={{ expectedSnapshotId: 'snapshot-c', groups: [] }}
        onReturnToMembers={vi.fn()}
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: '存档意见' }))
    expect(continuous.previewArchive).not.toHaveBeenCalled()
    expect(screen.getByRole('status')).toHaveTextContent('当前没有可存档的意见。')
  })

  it('clears the exact preview when deselecting the final archive target', async () => {
    const continuous = continuousArchiveCoordinator('continuous-c')
    render(
      <ReviewWorkspaceLayer
        review={coordinator(active())}
        selectedEntityIds={[]}
        projectAccess="read_write"
        continuousReview={continuous}
        onReturnToMembers={vi.fn()}
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: '存档意见' }))
    await waitFor(() => expect(continuous.previewArchive).toHaveBeenCalledOnce())

    const targetName =
      /Feedback ID feedback-c · Text revision text-c · Target ID target-c · Target revision target-c-revision/
    fireEvent.click(screen.getByRole('checkbox', { name: targetName }))

    const moved = screen.getByRole('heading', { name: '将移入历史' }).parentElement
    if (moved === null) throw new Error('Missing moved-to-history section')
    expect(within(moved).queryByText(targetName)).toBeNull()
    expect(screen.getByRole('checkbox', { name: targetName })).not.toBeChecked()
    expect(screen.getByRole('button', { name: '确认存档' })).toBeDisabled()
    expect(continuous.previewArchive).toHaveBeenCalledOnce()

    fireEvent.click(screen.getByRole('checkbox', { name: targetName }))
    await waitFor(() => expect(continuous.previewArchive).toHaveBeenCalledTimes(2))
    expect(screen.getByRole('button', { name: '确认存档' })).toBeEnabled()
  })

  it('preserves a selection-only covered choice for a fresh preview after deselection', async () => {
    const selection: ReviewArchiveSelection = {
      expectedSnapshotId: 'snapshot-c',
      groups: [
        {
          basis: {
            kind: 'known',
            snapshot: { snapshotId: 'snapshot-b', blake3: 'b'.repeat(64) },
            source: { kind: 'agent_declared', usageId: 'usage-b' },
          },
          targets: [structuredClone(archiveTarget)],
        },
      ],
    }
    const previewArchive = vi.fn(async (requested: ReviewArchiveSelection) =>
      archivePlan(requested),
    )
    previewArchive.mockResolvedValueOnce({
      expectedSnapshotId: 'snapshot-c',
      groups: [],
      removed: [],
      retained: [],
      alreadyCovered: [structuredClone(archiveTarget)],
    })
    const continuous = continuousArchiveCoordinator('continuous-c', { previewArchive })
    render(
      <ReviewWorkspaceLayer
        review={coordinator(active())}
        selectedEntityIds={[]}
        projectAccess="read_write"
        continuousReview={continuous}
        archiveSelection={selection}
        onReturnToMembers={vi.fn()}
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: '存档意见' }))
    await waitFor(() => expect(previewArchive).toHaveBeenCalledOnce())

    const targetName =
      /Feedback ID feedback-c · Text revision text-c · Target ID target-c · Target revision target-c-revision/
    expect(screen.getByRole('checkbox', { name: targetName })).toBeChecked()
    expect(screen.getByRole('button', { name: '确认存档' })).toBeDisabled()

    fireEvent.click(screen.getByRole('checkbox', { name: targetName }))

    expect(screen.getByRole('checkbox', { name: targetName })).not.toBeChecked()
    expect(screen.queryByText('没有新的可存档内容')).toBeNull()
    expect(previewArchive).toHaveBeenCalledOnce()

    fireEvent.click(screen.getByRole('checkbox', { name: targetName }))
    await waitFor(() => expect(previewArchive).toHaveBeenCalledTimes(2))
    await waitFor(() => expect(screen.getByRole('button', { name: '确认存档' })).toBeEnabled())
    const moved = screen.getByRole('heading', { name: '将移入历史' }).parentElement
    if (moved === null) throw new Error('Missing moved-to-history section')
    expect(within(moved).getByText(targetName)).toBeVisible()
  })

  it('does not show an old session preview after the coordinator session changes', async () => {
    const pending = deferred<ReviewArchivePlan>()
    const oldCoordinator = continuousArchiveCoordinator('continuous-old', {
      previewArchive: vi.fn(() => pending.promise),
    })
    const rendered = render(
      <ReviewWorkspaceLayer
        review={coordinator(active())}
        selectedEntityIds={[]}
        projectAccess="read_write"
        continuousReview={oldCoordinator}
        onReturnToMembers={vi.fn()}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: '存档意见' }))
    await waitFor(() => expect(oldCoordinator.previewArchive).toHaveBeenCalledOnce())

    const replacement = continuousArchiveCoordinator('continuous-new')
    rendered.rerender(
      <ReviewWorkspaceLayer
        review={coordinator(active())}
        selectedEntityIds={[]}
        projectAccess="read_write"
        continuousReview={replacement}
        onReturnToMembers={vi.fn()}
      />,
    )
    await act(async () => {
      pending.resolve(
        archivePlan({
          expectedSnapshotId: 'snapshot-c',
          groups: [{ basis: { kind: 'unknown' }, targets: [structuredClone(archiveTarget)] }],
        }),
      )
      await pending.promise
    })

    expect(screen.queryByRole('dialog', { name: '确认存档范围' })).toBeNull()
    expect(replacement.commitArchive).not.toHaveBeenCalled()
  })

  it('does not carry an old session preview error into the replacement context bar', async () => {
    const pending = deferred<ReviewArchivePlan>()
    const oldCoordinator = continuousArchiveCoordinator('continuous-old', {
      previewArchive: vi.fn(() => pending.promise),
    })
    const rendered = render(
      <ReviewWorkspaceLayer
        review={coordinator(active())}
        selectedEntityIds={[]}
        projectAccess="read_write"
        continuousReview={oldCoordinator}
        onReturnToMembers={vi.fn()}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: '存档意见' }))
    await waitFor(() => expect(oldCoordinator.previewArchive).toHaveBeenCalledOnce())

    rendered.rerender(
      <ReviewWorkspaceLayer
        review={coordinator(active())}
        selectedEntityIds={[]}
        projectAccess="read_write"
        continuousReview={continuousArchiveCoordinator('continuous-new')}
        onReturnToMembers={vi.fn()}
      />,
    )
    await act(async () => {
      pending.reject({ message: 'old preview failure' })
      await pending.promise.catch(() => undefined)
    })

    expect(screen.queryByText('old preview failure')).toBeNull()
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
    expect(dialog).toHaveTextContent('意见1')
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
