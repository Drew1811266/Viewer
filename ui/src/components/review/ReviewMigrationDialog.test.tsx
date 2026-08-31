import { fireEvent, render, screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { ReviewMigrationInspection } from '../../api/reviewWorkspaceTypes'
import ReviewMigrationDialog from './ReviewMigrationDialog'

const anchor = { kind: 'asset' as const }

function inspection(): ReviewMigrationInspection {
  return {
    legacyProtocol: 'viewer.review/2',
    indexDigest: '12'.repeat(32),
    inspectionDigest: '34'.repeat(32),
    legacyRecords: [
      {
        streamId: 'stream-1',
        roundId: 'draft-round',
        protocol: 'viewer.review/2',
        isDraft: true,
        blake3: '56'.repeat(32),
      },
      {
        streamId: 'stream-1',
        roundId: 'completed-round',
        protocol: 'viewer.review/2',
        isDraft: false,
        blake3: '78'.repeat(32),
      },
    ],
    activeDraft: {
      protocolVersion: 'viewer.review/2',
      draft: {
        projectId: 'project-1',
        reviewStreamId: 'stream-1',
        reviewRoundId: 'draft-round',
        production: null,
        previousCompletedRoundId: 'completed-round',
        createdAtMs: 2,
        assets: [],
        feedback: [
          {
            id: 'draft-feedback',
            text: '活动草稿意见',
            createdAtMs: 2,
            targets: [{ assetVersionId: 'draft-asset', anchor }],
          },
        ],
        unreviewable: [{ assetVersionId: 'draft-asset', failure: 'missing' }],
      },
    },
    completedCandidates: [
      {
        projectId: 'project-1',
        reviewStreamId: 'stream-1',
        reviewRoundId: 'completed-round',
        production: null,
        previousCompletedRoundId: null,
        createdAtMs: 1,
        completedAtMs: 1,
        assets: [],
        feedback: [
          {
            id: 'completed-feedback',
            text: '历史完成意见',
            createdAtMs: 1,
            targets: [{ assetVersionId: 'completed-asset', anchor }],
          },
        ],
        outcomes: [],
      },
    ],
    limitations: ['usage_unconfirmed'],
  }
}

describe('ReviewMigrationDialog', () => {
  it('defaults active Draft targets into the explicit continuation plan but not Completed history', () => {
    const onConfirm = vi.fn()
    render(
      <ReviewMigrationDialog inspection={inspection()} onConfirm={onConfirm} onCancel={vi.fn()} />,
    )

    const dialog = screen.getByRole('dialog', { name: '迁移旧评审记录' })
    expect(within(dialog).getByRole('checkbox', { name: /活动草稿意见/ })).toBeChecked()
    expect(within(dialog).getByRole('checkbox', { name: /历史完成意见/ })).not.toBeChecked()
    expect(within(dialog).getByText('源素材缺失，需要重新确认')).toBeVisible()
    expect(within(dialog).getByText('使用情况未确认')).toBeVisible()

    fireEvent.click(within(dialog).getByRole('button', { name: '继续选中意见' }))
    expect(onConfirm).toHaveBeenCalledWith({
      inspectionDigest: '34'.repeat(32),
      choice: {
        kind: 'continue_selected',
        legacyTargets: [{ roundId: 'draft-round', feedbackId: 'draft-feedback', targetIndex: 0 }],
        bindings: [],
      },
    })
  })

  it('can preserve all legacy data as history without publishing it as current feedback', () => {
    const onConfirm = vi.fn()
    const onCancel = vi.fn()
    render(
      <ReviewMigrationDialog inspection={inspection()} onConfirm={onConfirm} onCancel={onCancel} />,
    )

    fireEvent.click(screen.getByRole('button', { name: '仅保留历史' }))
    expect(onConfirm).toHaveBeenCalledWith({
      inspectionDigest: '34'.repeat(32),
      choice: { kind: 'keep_history_only' },
    })
    expect(screen.queryByText('已修复')).not.toBeInTheDocument()

    fireEvent.keyDown(screen.getByRole('dialog'), { key: 'Escape' })
    expect(onCancel).toHaveBeenCalledOnce()
  })

  it('resets internal choices when a different project inspection replaces the dialog', () => {
    const first = inspection()
    const rendered = render(
      <ReviewMigrationDialog inspection={first} onConfirm={vi.fn()} onCancel={vi.fn()} />,
    )
    fireEvent.click(screen.getByRole('checkbox', { name: /活动草稿意见/ }))
    expect(screen.getByRole('checkbox', { name: /活动草稿意见/ })).not.toBeChecked()

    const replacement = inspection()
    replacement.inspectionDigest = '90'.repeat(32)
    if (replacement.activeDraft === null) throw new Error('Missing active Draft fixture')
    replacement.activeDraft.draft.reviewRoundId = 'replacement-round'
    replacement.activeDraft.draft.feedback[0] = {
      id: 'replacement-feedback',
      text: '新项目活动意见',
      createdAtMs: 3,
      targets: [{ assetVersionId: 'replacement-asset', anchor }],
    }
    replacement.activeDraft.draft.unreviewable = []
    rendered.rerender(
      <ReviewMigrationDialog inspection={replacement} onConfirm={vi.fn()} onCancel={vi.fn()} />,
    )

    expect(screen.getByRole('checkbox', { name: /新项目活动意见/ })).toBeChecked()
  })
})
