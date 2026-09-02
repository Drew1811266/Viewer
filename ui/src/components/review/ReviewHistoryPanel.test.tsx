import { fireEvent, render, screen } from '@testing-library/react'
import { expect, it, vi } from 'vitest'
import type {
  ReviewHistoryRef,
  ReviewHistoryView,
  ReviewRestorePlan,
  ReviewTargetVersionKey,
} from '../../api/reviewWorkspaceTypes'
import ReviewHistoryPanel from './ReviewHistoryPanel'

const key: ReviewTargetVersionKey = {
  feedbackId: 'feedback-old',
  textRevisionId: 'text-old',
  targetId: 'target-shared',
  targetRevisionId: 'target-old',
}

const historyRef: ReviewHistoryRef = {
  projectId: 'project-1',
  streamId: 'stream-1',
  source: {
    kind: 'snapshot',
    snapshot: { snapshotId: 'snapshot-old', blake3: 'a'.repeat(64) },
    keys: [key],
  },
}

const history: ReviewHistoryView = {
  selector: { kind: 'archive', archiveId: 'archive-1' },
  entries: [
    {
      snapshot: { snapshotId: 'snapshot-old', blake3: 'a'.repeat(64) },
      feedback: [
        {
          id: key.feedbackId,
          textRevisionId: key.textRevisionId,
          text: '旧文字：收紧袖口',
          createdAtMs: 1,
          historyRef: null,
          targets: [
            {
              id: key.targetId,
              revisionId: key.targetRevisionId,
              assetVersionId: 'asset-old',
              anchor: { kind: 'asset' },
              availability: { kind: 'ready' },
            },
          ],
        },
      ],
      assets: [
        {
          id: 'asset-old',
          sourceEntityId: null,
          relativePath: 'old.png',
          evidence: { sizeBytes: 1, modifiedNs: '1', blake3: 'a'.repeat(64) },
          media: { kind: 'image', width: 100, height: 100 },
          producerAssetId: null,
          parentAssetVersionId: null,
        },
      ],
      evidence: [
        {
          assetVersionId: 'asset-old',
          capability: {
            kind: 'image',
            base: { blake3: 'b'.repeat(64), sizeBytes: 1, width: 100, height: 100 },
            annotated: null,
            annotations: [],
          },
        },
      ],
      selected: [key],
    },
  ],
  legacy: null,
  limitations: ['background_only'],
  restoreActions: [key],
}

const restorePlan: ReviewRestorePlan = {
  expectedSnapshotId: 'snapshot-current',
  restored: [],
  conflicts: [key],
  coverageReversals: [],
  requiresSourceCheck: [],
}

function firstEntry(value: ReviewHistoryView) {
  const entry = value.entries[0]
  if (entry === undefined) throw new Error('Fixture requires one history entry')
  return entry
}

it('requires an explicit conflict choice before historical text can restore over current text', () => {
  const onRestore = vi.fn()
  render(
    <ReviewHistoryPanel
      history={history}
      historyRef={historyRef}
      restorePlan={restorePlan}
      currentFeedback={[
        {
          id: 'feedback-current',
          textRevisionId: 'text-current',
          text: '新文字：保留袖口褶皱',
          createdAtMs: 2,
          historyRef: null,
          targets: [
            {
              id: 'target-shared',
              revisionId: 'target-current',
              assetVersionId: 'asset-current',
              anchor: { kind: 'asset' },
              availability: { kind: 'ready' },
            },
          ],
        },
      ]}
      busy={false}
      error={null}
      onClose={vi.fn()}
      onContinue={vi.fn()}
      onRestore={onRestore}
      onRequestEvidence={vi.fn()}
    />,
  )

  expect(screen.getByText('保留当前意见')).toBeVisible()
  expect(screen.getByText('使用历史意见')).toBeVisible()
  expect(screen.getByText('作为新意见继续提出')).toBeVisible()
  expect(screen.getByRole('button', { name: '确认恢复' })).toBeDisabled()
  expect(screen.getByText('旧文字：收紧袖口')).toBeVisible()
  expect(screen.getByText(/新文字：保留袖口褶皱/)).toBeVisible()

  fireEvent.click(screen.getByLabelText('使用历史意见'))
  expect(screen.getByRole('button', { name: '确认恢复' })).toBeEnabled()
  fireEvent.click(screen.getByRole('button', { name: '确认恢复' }))
  expect(onRestore).toHaveBeenCalledWith([
    { historicalKey: key, choice: { kind: 'use_historical' } },
  ])
})

it('shows local conflicts before the first restore preview and requires a choice before previewing', () => {
  const onRestore = vi.fn()
  render(
    <ReviewHistoryPanel
      history={history}
      historyRef={historyRef}
      restorePlan={null}
      currentFeedback={[
        {
          id: 'feedback-current',
          textRevisionId: 'text-current',
          text: '新文字：保留袖口褶皱',
          createdAtMs: 2,
          historyRef: null,
          targets: [
            {
              id: key.targetId,
              revisionId: 'target-current',
              assetVersionId: 'asset-current',
              anchor: { kind: 'asset' },
              availability: { kind: 'ready' },
            },
          ],
        },
      ]}
      busy={false}
      error={null}
      onClose={vi.fn()}
      onContinue={vi.fn()}
      onRestore={onRestore}
      onRequestEvidence={vi.fn()}
    />,
  )

  fireEvent.click(screen.getByLabelText(/Target ID target-shared/))
  expect(screen.getByText('保留当前意见')).toBeVisible()
  expect(screen.getByText('使用历史意见')).toBeVisible()
  expect(screen.getByText('作为新意见继续提出')).toBeVisible()
  expect(screen.getByRole('button', { name: '查看恢复影响' })).toBeDisabled()
  fireEvent.click(screen.getByLabelText('使用历史意见'))
  expect(screen.getByRole('button', { name: '查看恢复影响' })).toBeEnabled()
  fireEvent.click(screen.getByRole('button', { name: '查看恢复影响' }))
  expect(onRestore).toHaveBeenCalledOnce()
})

it('shows the exact restored targets, coverage reversals, and source checks from a reviewed plan', () => {
  const firstAsset = firstEntry(history).assets[0]
  if (firstAsset === undefined) throw new Error('Fixture requires one asset')
  const plan = {
    ...restorePlan,
    restored: [
      {
        historicalKey: key,
        feedback: {
          id: key.feedbackId,
          textRevisionId: key.textRevisionId,
          text: '旧文字：收紧袖口',
          createdAtMs: 1,
          historyRef: null,
        },
        target: {
          id: key.targetId,
          revisionId: key.targetRevisionId,
          assetVersionId: 'asset-old',
          anchor: { kind: 'asset' as const },
          availability: { kind: 'ready' as const },
        },
        asset: firstAsset,
        continuedAsNew: false,
      },
    ],
    coverageReversals: [{ archiveId: 'archive-1', key, active: false }],
    requiresSourceCheck: ['asset-old'],
  }
  render(
    <ReviewHistoryPanel
      history={history}
      historyRef={historyRef}
      restorePlan={plan}
      currentFeedback={[]}
      busy={false}
      error={null}
      onClose={vi.fn()}
      onContinue={vi.fn()}
      onRestore={vi.fn()}
      onRequestEvidence={vi.fn()}
    />,
  )

  expect(screen.getByText(/将恢复目标.*target-shared/)).toBeVisible()
  expect(screen.getByText(/将撤销存档覆盖.*archive-1/)).toBeVisible()
  expect(screen.getByText(/恢复后需重新核验素材：asset-old/)).toBeVisible()
  expect(screen.getByText(/不会使意见自动可执行/)).toBeVisible()
})

it('renders real legacy records as background content even when entries are empty', () => {
  const firstAsset = firstEntry(history).assets[0]
  if (firstAsset === undefined) throw new Error('Fixture requires one asset')
  const legacy = structuredClone(history)
  legacy.entries = []
  legacy.restoreActions = []
  legacy.legacy = {
    reference: {
      streamId: 'legacy-stream',
      roundId: 'legacy-round',
      protocol: 'viewer.review/2',
      isDraft: false,
      blake3: 'f'.repeat(64),
    },
    contents: {
      kind: 'completed',
      record: {
        projectId: 'project-1',
        reviewStreamId: 'legacy-stream',
        reviewRoundId: 'legacy-round',
        production: null,
        previousCompletedRoundId: null,
        createdAtMs: 1,
        completedAtMs: 2,
        assets: [firstAsset],
        feedback: [
          {
            id: 'legacy-feedback',
            text: '旧协议原文',
            createdAtMs: 1,
            targets: [{ assetVersionId: 'asset-old', anchor: { kind: 'asset' } }],
          },
        ],
        outcomes: [],
      },
    },
  }
  render(
    <ReviewHistoryPanel
      history={legacy}
      historyRef={null}
      restorePlan={null}
      currentFeedback={[]}
      busy={false}
      error={null}
      onClose={vi.fn()}
      onContinue={vi.fn()}
      onRestore={vi.fn()}
      onRequestEvidence={vi.fn()}
    />,
  )
  expect(screen.getByText('旧协议原文')).toBeVisible()
  expect(screen.getByText('目标素材 asset-old')).toBeVisible()
  expect(screen.getByText('素材 old.png')).toBeVisible()
  expect(screen.getByText('此历史记录来自旧协议，只能作为背景查看。')).toBeVisible()
})

it('allows a pending read-only history operation to close without presenting it as a cancelled write', () => {
  const onClose = vi.fn()
  render(
    <ReviewHistoryPanel
      history={history}
      historyRef={historyRef}
      restorePlan={null}
      currentFeedback={[]}
      busy
      canClose
      error={null}
      onClose={onClose}
      onContinue={vi.fn()}
      onRestore={vi.fn()}
      onRequestEvidence={vi.fn()}
    />,
  )
  fireEvent.click(screen.getByRole('button', { name: '关闭历史' }))
  expect(onClose).toHaveBeenCalledOnce()
})

it('shows legacy evidence absence as a limitation but treats broken required evidence as an integrity error', () => {
  const legacy = structuredClone(history)
  legacy.limitations = ['background_only', 'legacy_evidence_absent']
  firstEntry(legacy).evidence = [
    { assetVersionId: 'asset-old', capability: { kind: 'legacy_absent' } },
  ]
  const rendered = render(
    <ReviewHistoryPanel
      history={legacy}
      historyRef={historyRef}
      restorePlan={null}
      currentFeedback={[]}
      busy={false}
      error={null}
      onClose={vi.fn()}
      onContinue={vi.fn()}
      onRestore={vi.fn()}
      onRequestEvidence={vi.fn()}
    />,
  )
  expect(screen.getByText('旧记录没有可用的历史证据。')).toBeVisible()
  rendered.rerender(
    <ReviewHistoryPanel
      history={history}
      historyRef={historyRef}
      restorePlan={null}
      currentFeedback={[]}
      busy={false}
      error={{
        code: 'integrity',
        message: '必需历史证据损坏',
        retryable: false,
        committedReceipt: null,
        committedAuthoringReceipt: null,
      }}
      onClose={vi.fn()}
      onContinue={vi.fn()}
      onRestore={vi.fn()}
      onRequestEvidence={vi.fn()}
    />,
  )
  expect(screen.getByRole('alert')).toHaveTextContent('必需历史证据损坏')
})

it('keeps archive history scoped to selected keys and requests its available base evidence role', () => {
  const scoped = structuredClone(history)
  const entry = firstEntry(scoped)
  const feedback = entry.feedback[0]
  const target = feedback?.targets[0]
  if (feedback === undefined || target === undefined) throw new Error('Fixture requires one target')
  entry.feedback.push({
    ...feedback,
    id: 'feedback-not-archived',
    text: '不属于这次存档的意见',
    targets: [{ ...target, id: 'target-not-archived' }],
  })
  const onRequestEvidence = vi.fn()
  render(
    <ReviewHistoryPanel
      history={scoped}
      historyRef={historyRef}
      restorePlan={null}
      currentFeedback={[]}
      busy={false}
      error={null}
      onClose={vi.fn()}
      onContinue={vi.fn()}
      onRestore={vi.fn()}
      onRequestEvidence={onRequestEvidence}
    />,
  )
  expect(screen.getByText('旧文字：收紧袖口')).toBeVisible()
  expect(screen.queryByText('不属于这次存档的意见')).toBeNull()
  fireEvent.click(screen.getByRole('button', { name: '查看历史证据' }))
  expect(onRequestEvidence).toHaveBeenCalledWith('asset-old', 'base')
})

it('does not submit an already-current historical key as a duplicate restore action', () => {
  const onRestore = vi.fn()
  render(
    <ReviewHistoryPanel
      history={history}
      historyRef={historyRef}
      restorePlan={null}
      currentFeedback={[
        {
          id: key.feedbackId,
          textRevisionId: key.textRevisionId,
          text: '旧文字：收紧袖口',
          createdAtMs: 1,
          historyRef: null,
          targets: [
            {
              id: key.targetId,
              revisionId: key.targetRevisionId,
              assetVersionId: 'asset-old',
              anchor: { kind: 'asset' },
              availability: { kind: 'ready' },
            },
          ],
        },
      ]}
      busy={false}
      error={null}
      onClose={vi.fn()}
      onContinue={vi.fn()}
      onRestore={onRestore}
      onRequestEvidence={vi.fn()}
    />,
  )
  expect(screen.getByText('以下历史目标已在当前意见中，不会重复恢复。')).toBeVisible()
  expect(screen.getByRole('button', { name: '查看恢复影响' })).toBeDisabled()
  expect(onRestore).not.toHaveBeenCalled()
})

it('requires a user to choose one of several historical targets before continuing', () => {
  const onContinue = vi.fn()
  const otherKey = { ...key, targetId: 'target-other', targetRevisionId: 'target-other' }
  if (historyRef.source.kind !== 'snapshot')
    throw new Error('Fixture requires a snapshot history ref')
  const otherRef: ReviewHistoryRef = {
    projectId: historyRef.projectId,
    streamId: historyRef.streamId,
    source: {
      kind: 'snapshot',
      snapshot: historyRef.source.snapshot,
      keys: [otherKey],
    },
  }
  render(
    <ReviewHistoryPanel
      history={history}
      historyRef={historyRef}
      historyRefs={[historyRef, otherRef]}
      restorePlan={null}
      currentFeedback={[]}
      busy={false}
      error={null}
      onClose={vi.fn()}
      onContinue={onContinue}
      onRestore={vi.fn()}
      onRequestEvidence={vi.fn()}
    />,
  )
  expect(screen.getByRole('button', { name: '继续提出' })).toBeDisabled()
  fireEvent.click(screen.getByLabelText(/Target ID target-other/))
  fireEvent.click(screen.getByRole('button', { name: '继续提出' }))
  expect(onContinue).toHaveBeenCalledWith(otherRef)
})

it('sends exactly the manually selected restore target and does not write an all-preserve choice', () => {
  const onRestore = vi.fn()
  const otherKey = { ...key, targetId: 'target-other', targetRevisionId: 'target-other' }
  const partial = structuredClone(history)
  partial.restoreActions = [key, otherKey]
  firstEntry(partial).selected = [key, otherKey]
  render(
    <ReviewHistoryPanel
      history={partial}
      historyRef={historyRef}
      restorePlan={null}
      currentFeedback={[]}
      busy={false}
      error={null}
      onClose={vi.fn()}
      onContinue={vi.fn()}
      onRestore={onRestore}
      onRequestEvidence={vi.fn()}
    />,
  )
  fireEvent.click(screen.getByLabelText(/Target ID target-other/))
  fireEvent.click(screen.getByRole('button', { name: '查看恢复影响' }))
  expect(onRestore).toHaveBeenCalledWith([
    { historicalKey: otherKey, choice: { kind: 'use_historical' } },
  ])

  onRestore.mockClear()
  const conflicted = structuredClone(history)
  const conflictedFeedback = firstEntry(conflicted).feedback[0]
  const conflictedTarget = conflictedFeedback?.targets[0]
  if (conflictedFeedback === undefined || conflictedTarget === undefined)
    throw new Error('Fixture requires a feedback target')
  render(
    <ReviewHistoryPanel
      history={conflicted}
      historyRef={historyRef}
      restorePlan={restorePlan}
      currentFeedback={[
        {
          ...conflictedFeedback,
          targets: [{ ...conflictedTarget, revisionId: 'current' }],
        },
      ]}
      busy={false}
      error={null}
      onClose={vi.fn()}
      onContinue={vi.fn()}
      onRestore={onRestore}
      onRequestEvidence={vi.fn()}
    />,
  )
  fireEvent.click(screen.getByLabelText('保留当前意见'))
  expect(screen.getByRole('button', { name: '确认恢复' })).toBeDisabled()
  expect(onRestore).not.toHaveBeenCalled()
})
