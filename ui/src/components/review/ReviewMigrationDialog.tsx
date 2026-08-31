import { useMemo, useState } from 'react'
import type {
  ReviewLegacyFailure,
  ReviewLegacyTargetRef,
  ReviewMigrationInspection,
  ReviewMigrationPlan,
} from '../../api/reviewWorkspaceTypes'
import ModalSheet from '../ModalSheet'
import ViewerButton from '../ui/ViewerButton'

export interface ReviewMigrationDialogProps {
  inspection: ReviewMigrationInspection
  onConfirm(plan: ReviewMigrationPlan): void
  onCancel(): void
}

interface MigrationCandidate {
  key: string
  target: ReviewLegacyTargetRef
  text: string
  source: 'draft' | 'completed'
  pendingFailure: ReviewLegacyFailure | null
}

export default function ReviewMigrationDialog({
  inspection,
  onConfirm,
  onCancel,
}: ReviewMigrationDialogProps) {
  return (
    <ReviewMigrationDialogContent
      key={inspection.inspectionDigest}
      inspection={inspection}
      onConfirm={onConfirm}
      onCancel={onCancel}
    />
  )
}

function ReviewMigrationDialogContent({
  inspection,
  onConfirm,
  onCancel,
}: ReviewMigrationDialogProps) {
  const candidates = useMemo(() => migrationCandidates(inspection), [inspection])
  const [selected, setSelected] = useState<Set<string>>(
    () => new Set(candidates.filter((candidate) => candidate.source === 'draft').map((c) => c.key)),
  )
  const selectedTargets = candidates
    .filter((candidate) => selected.has(candidate.key))
    .map((candidate) => structuredClone(candidate.target))

  function toggle(key: string) {
    setSelected((current) => {
      const next = new Set(current)
      if (next.has(key)) next.delete(key)
      else next.add(key)
      return next
    })
  }

  return (
    <ModalSheet
      title="迁移旧评审记录"
      size="large"
      onCancel={onCancel}
      footer={
        <>
          <ViewerButton onClick={onCancel}>暂不迁移</ViewerButton>
          <ViewerButton
            tone="secondary"
            onClick={() =>
              onConfirm({
                inspectionDigest: inspection.inspectionDigest,
                choice: { kind: 'keep_history_only' },
              })
            }
          >
            仅保留历史
          </ViewerButton>
          <ViewerButton
            tone="primary"
            disabled={selectedTargets.length === 0}
            onClick={() =>
              onConfirm({
                inspectionDigest: inspection.inspectionDigest,
                choice: {
                  kind: 'continue_selected',
                  legacyTargets: selectedTargets,
                  bindings: [],
                },
              })
            }
          >
            继续选中意见
          </ViewerButton>
        </>
      }
    >
      <p>
        新流程会创建 viewer.review/3 索引；旧记录保留原位置与原语义。迁移不表示 Agent
        已读取或意见已执行。
      </p>
      {inspection.limitations.includes('usage_unconfirmed') && (
        <p className="review-migration__notice">使用情况未确认</p>
      )}
      <MigrationCandidates
        title="活动草稿"
        empty="没有活动草稿。"
        candidates={candidates.filter((candidate) => candidate.source === 'draft')}
        selected={selected}
        onToggle={toggle}
      />
      <MigrationCandidates
        title="已完成的历史记录"
        empty="没有已完成的历史记录。"
        candidates={candidates.filter((candidate) => candidate.source === 'completed')}
        selected={selected}
        onToggle={toggle}
      />
    </ModalSheet>
  )
}

function MigrationCandidates({
  title,
  empty,
  candidates,
  selected,
  onToggle,
}: {
  title: string
  empty: string
  candidates: MigrationCandidate[]
  selected: ReadonlySet<string>
  onToggle(key: string): void
}) {
  return (
    <section className="review-migration__section">
      <h3>{title}</h3>
      {candidates.length === 0 ? (
        <p>{empty}</p>
      ) : (
        <ul className="review-migration__list">
          {candidates.map((candidate) => (
            <li key={candidate.key}>
              <label>
                <input
                  type="checkbox"
                  checked={selected.has(candidate.key)}
                  onChange={() => onToggle(candidate.key)}
                />
                <span>{candidate.text}</span>
              </label>
              {candidate.pendingFailure !== null && (
                <span className="review-migration__pending">
                  {legacyFailureMessage(candidate.pendingFailure)}
                </span>
              )}
            </li>
          ))}
        </ul>
      )}
    </section>
  )
}

function migrationCandidates(inspection: ReviewMigrationInspection): MigrationCandidate[] {
  const candidates: MigrationCandidate[] = []
  const active = inspection.activeDraft?.draft
  if (active !== undefined) {
    const failures = new Map(
      active.unreviewable.map((entry) => [entry.assetVersionId, entry.failure]),
    )
    for (const feedback of active.feedback) {
      feedback.targets.forEach((target, targetIndex) => {
        const reference = {
          roundId: active.reviewRoundId,
          feedbackId: feedback.id,
          targetIndex,
        }
        candidates.push({
          key: candidateKey(reference),
          target: reference,
          text: feedback.text,
          source: 'draft',
          pendingFailure: failures.get(target.assetVersionId) ?? null,
        })
      })
    }
  }
  for (const completed of inspection.completedCandidates) {
    for (const feedback of completed.feedback) {
      feedback.targets.forEach((_target, targetIndex) => {
        const reference = {
          roundId: completed.reviewRoundId,
          feedbackId: feedback.id,
          targetIndex,
        }
        candidates.push({
          key: candidateKey(reference),
          target: reference,
          text: feedback.text,
          source: 'completed',
          pendingFailure: null,
        })
      })
    }
  }
  return candidates
}

function candidateKey(target: ReviewLegacyTargetRef): string {
  return `${target.roundId}\u0000${target.feedbackId}\u0000${target.targetIndex}`
}

function legacyFailureMessage(failure: ReviewLegacyFailure): string {
  if (failure === 'missing') return '源素材缺失，需要重新确认'
  if (failure === 'permission_denied' || failure === 'unreadable')
    return '源素材不可读，需要重新确认'
  if (failure === 'damaged' || failure === 'decode_failed') return '源素材无法核验，需要重新确认'
  return '旧素材能力受限，需要重新确认'
}
