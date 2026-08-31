import { type RefObject, useRef, useState } from 'react'
import type {
  ReviewEvidenceImage,
  ReviewHistoryRef,
  ReviewHistoryView,
  ReviewRestoreChoice,
  ReviewRestoreDecision,
  ReviewRestorePlan,
  ReviewTargetVersionKey,
  ReviewVersionedFeedback,
  ReviewWorkspaceError,
} from '../../api/reviewWorkspaceTypes'
import ModalSheet from '../ModalSheet'
import ViewerButton from '../ui/ViewerButton'
import {
  displayHistoryFeedback,
  restoreSubmission,
  restoreTargetState,
  historyTargetKey as targetKey,
} from './reviewHistoryModel'

export type HistoryViewDto = ReviewHistoryView
export type RestoreDecisionDto = ReviewRestoreDecision

export interface ReviewHistoryPanelProps {
  history: HistoryViewDto
  historyRef: ReviewHistoryRef | null
  historyRefs?: ReviewHistoryRef[]
  restorePlan: ReviewRestorePlan | null
  currentFeedback: ReviewVersionedFeedback[]
  evidence?: ReviewEvidenceImage[]
  busy: boolean
  canClose?: boolean
  error: ReviewWorkspaceError | null
  onClose(): void
  onContinue(historyRef: ReviewHistoryRef): void
  onRestore(decisions: RestoreDecisionDto[]): void
  onInvalidateRestorePreview?(): void
  onRequestEvidence(assetVersionId: string, role: 'base' | 'annotated'): void
}

/** Historical content is background only. It never becomes a current instruction without a command. */
export default function ReviewHistoryPanel({
  history,
  historyRef,
  historyRefs,
  restorePlan,
  currentFeedback,
  evidence = [],
  busy,
  canClose = !busy,
  error,
  onClose,
  onContinue,
  onRestore,
  onInvalidateRestorePreview,
  onRequestEvidence,
}: ReviewHistoryPanelProps) {
  const closeRef = useRef<HTMLButtonElement>(null)
  const continuationRefs = historyRefs ?? (historyRef === null ? [] : [historyRef])
  const selection = useHistoryPanelSelection(
    history,
    currentFeedback,
    restorePlan,
    continuationRefs,
    onInvalidateRestorePreview,
  )

  return (
    <ModalSheet
      title="历史意见"
      size="large"
      onCancel={() => {
        if (canClose) onClose()
      }}
      initialFocusRef={closeRef}
      footer={
        <HistoryPanelFooter
          busy={busy}
          canClose={canClose}
          closeRef={closeRef}
          restorePlan={restorePlan}
          selectedContinuation={selection.selectedContinuation}
          submission={selection.submission}
          onClose={onClose}
          onContinue={onContinue}
          onRestore={onRestore}
        />
      }
    >
      <p>历史内容仅供背景查看，不是当前执行要求。</p>
      {error !== null && (
        <p className="review-history-panel__error" role="alert">
          {error.message}
        </p>
      )}
      {history.limitations.includes('legacy_evidence_absent') && (
        <p role="status">旧记录没有可用的历史证据。</p>
      )}
      <ContinuationChoices
        refs={continuationRefs}
        selectedKey={selection.continuationRefKey}
        busy={busy}
        onChange={selection.setContinuationRefKey}
      />
      <HistoryEntries
        history={history}
        evidence={evidence}
        busy={busy}
        onRequestEvidence={onRequestEvidence}
      />
      {history.legacy !== null && <LegacyHistoryRecord history={history} />}
      {history.restoreActions.length === 0 && <p role="status">这批历史没有可恢复目标。</p>}
      {selection.targets.alreadyCurrent.length > 0 && (
        <p role="status">以下历史目标已在当前意见中，不会重复恢复。</p>
      )}
      <RestoreOptions
        history={history}
        restorePlan={restorePlan}
        currentFeedback={currentFeedback}
        selectableKeys={selection.targets.selectable}
        conflicts={selection.conflicts}
        choices={selection.choices}
        selected={selection.selectedForRestore}
        busy={busy}
        onSelectionChange={selection.setSelectedForRestore}
        onChoice={selection.setChoice}
      />
    </ModalSheet>
  )
}

function useHistoryPanelSelection(
  history: ReviewHistoryView,
  currentFeedback: ReviewVersionedFeedback[],
  restorePlan: ReviewRestorePlan | null,
  continuationRefs: ReviewHistoryRef[],
  invalidateRestorePreview: (() => void) | undefined,
) {
  const [choices, setChoices] = useState<Record<string, ReviewRestoreChoice>>({})
  const [selectedForRestore, setSelectedForRestore] = useState<Record<string, boolean>>({})
  const [continuationRefKey, setContinuationRefKey] = useState<string | null>(() =>
    initialContinuationRefKey(continuationRefs),
  )
  const targets = restoreTargetState(history.restoreActions, currentFeedback)
  const selectedKeys = targets.selectable.filter((key) => selectedForRestore[targetKey(key)])
  const conflicts = uniqueKeys([...(restorePlan?.conflicts ?? []), ...targets.conflicts])
  const submission = restoreSubmission(selectedKeys, conflicts, choices)
  const selectedContinuation =
    continuationRefs.find((reference) => historyReferenceKey(reference) === continuationRefKey) ??
    null
  return {
    choices,
    selectedForRestore,
    continuationRefKey,
    targets,
    conflicts,
    submission,
    selectedContinuation,
    setContinuationRefKey,
    setSelectedForRestore: (
      change: (previous: Record<string, boolean>) => Record<string, boolean>,
    ) => {
      invalidateRestorePreview?.()
      setSelectedForRestore(change)
    },
    setChoice: (key: ReviewTargetVersionKey, choice: ReviewRestoreChoice) => {
      invalidateRestorePreview?.()
      setChoices((previous) => ({ ...previous, [targetKey(key)]: choice }))
      setSelectedForRestore((previous) => ({ ...previous, [targetKey(key)]: true }))
    },
  }
}

function initialContinuationRefKey(refs: ReviewHistoryRef[]) {
  return refs.length === 1 && refs[0] !== undefined ? historyReferenceKey(refs[0]) : null
}

function HistoryEntries({
  history,
  evidence,
  busy,
  onRequestEvidence,
}: Pick<ReviewHistoryPanelProps, 'history' | 'evidence' | 'busy' | 'onRequestEvidence'>) {
  return history.entries.map((entry) => (
    <section
      className="review-history-panel__entry"
      key={`${entry.snapshot.snapshotId}:${entry.snapshot.blake3}`}
    >
      <h3>历史快照 {entry.snapshot.snapshotId}</h3>
      {displayHistoryFeedback(history, entry).map((feedback) => (
        <HistoryFeedback
          key={`${feedback.id}:${feedback.textRevisionId}`}
          entry={entry}
          feedback={feedback}
          evidence={evidence ?? []}
          busy={busy}
          onRequestEvidence={onRequestEvidence}
        />
      ))}
    </section>
  ))
}

function HistoryPanelFooter({
  busy,
  canClose,
  closeRef,
  restorePlan,
  selectedContinuation,
  submission,
  onClose,
  onContinue,
  onRestore,
}: {
  busy: boolean
  canClose: boolean
  closeRef: RefObject<HTMLButtonElement | null>
  restorePlan: ReviewRestorePlan | null
  selectedContinuation: ReviewHistoryRef | null
  submission: ReturnType<typeof restoreSubmission>
  onClose(): void
  onContinue(historyRef: ReviewHistoryRef): void
  onRestore(decisions: RestoreDecisionDto[]): void
}) {
  const disabled = busy || submission.kind !== 'preview'
  return (
    <>
      <ViewerButton ref={closeRef} disabled={!canClose} onClick={onClose}>
        关闭历史
      </ViewerButton>
      <ViewerButton
        tone="secondary"
        disabled={busy || selectedContinuation === null}
        onClick={() => {
          if (selectedContinuation !== null) onContinue(selectedContinuation)
        }}
      >
        继续提出
      </ViewerButton>
      <ViewerButton
        tone="primary"
        loading={restorePlan !== null && busy}
        disabled={disabled}
        onClick={() => {
          if (submission.kind === 'preview') onRestore(submission.decisions)
        }}
      >
        {restorePlan === null ? '查看恢复影响' : '确认恢复'}
      </ViewerButton>
    </>
  )
}

function ContinuationChoices({
  refs,
  selectedKey,
  busy,
  onChange,
}: {
  refs: ReviewHistoryRef[]
  selectedKey: string | null
  busy: boolean
  onChange(key: string): void
}) {
  if (refs.length <= 1) return null
  return (
    <section className="review-history-panel__restore" aria-label="继续提出范围">
      <h3>选择一个历史目标继续提出</h3>
      {refs.map((reference) => (
        <label key={historyReferenceKey(reference)}>
          <input
            type="radio"
            name="continue-historical"
            checked={selectedKey === historyReferenceKey(reference)}
            disabled={busy}
            onChange={() => onChange(historyReferenceKey(reference))}
          />
          {historyReferenceLabel(reference)}
        </label>
      ))}
    </section>
  )
}

function HistoryFeedback({
  entry,
  feedback,
  evidence,
  busy,
  onRequestEvidence,
}: {
  entry: ReviewHistoryView['entries'][number]
  feedback: ReviewVersionedFeedback
  evidence: ReviewEvidenceImage[]
  busy: boolean
  onRequestEvidence(assetVersionId: string, role: 'base' | 'annotated'): void
}) {
  return (
    <article>
      <h4>历史原文</h4>
      <p>{feedback.text}</p>
      {feedback.targets.map((target) => {
        const capability = entry.evidence.find(
          (binding) => binding.assetVersionId === target.assetVersionId,
        )?.capability
        const role =
          capability?.kind === 'image' && capability.annotated !== null ? 'annotated' : 'base'
        const image = evidence.find(
          (candidate) =>
            candidate.assetVersionId === target.assetVersionId && candidate.role === role,
        )
        return (
          <div className="review-history-panel__target" key={`${target.id}:${target.revisionId}`}>
            <span>目标 {target.id}</span>
            {capability?.kind === 'image' && (
              <ViewerButton
                tone="quiet"
                disabled={busy}
                onClick={() => onRequestEvidence(target.assetVersionId, role)}
              >
                查看历史证据
              </ViewerButton>
            )}
            {capability?.kind === 'legacy_absent' && <span>证据能力限制</span>}
            {capability?.kind === 'not_image' && <span>该素材没有图片证据</span>}
            {image !== undefined && <img src={image.url} alt={`历史证据：${feedback.text}`} />}
          </div>
        )
      })}
    </article>
  )
}

function LegacyHistoryRecord({ history }: { history: ReviewHistoryView }) {
  const legacy = history.legacy
  if (legacy === null) return null
  const record = legacy.contents.record
  return (
    <section className="review-history-panel__entry" aria-label="旧协议历史内容">
      <h3>旧协议历史</h3>
      {record.feedback.map((feedback) => (
        <article key={feedback.id}>
          <h4>历史原文</h4>
          <p>{feedback.text}</p>
          {feedback.targets.map((target) => (
            <span key={`${target.assetVersionId}:${JSON.stringify(target.anchor)}`}>
              目标素材 {target.assetVersionId}
            </span>
          ))}
        </article>
      ))}
      {record.assets.map((asset) => (
        <span key={asset.id}>素材 {asset.relativePath}</span>
      ))}
      <p>此历史记录来自旧协议，只能作为背景查看。</p>
      {history.limitations.includes('legacy_evidence_absent') && (
        <p role="status">旧协议没有历史证据，这是证据能力限制。</p>
      )}
    </section>
  )
}

function RestoreOptions({
  history,
  restorePlan,
  currentFeedback,
  selectableKeys,
  conflicts,
  choices,
  selected,
  busy,
  onSelectionChange,
  onChoice,
}: {
  history: ReviewHistoryView
  restorePlan: ReviewRestorePlan | null
  currentFeedback: ReviewVersionedFeedback[]
  selectableKeys: ReviewTargetVersionKey[]
  conflicts: ReviewTargetVersionKey[]
  choices: Record<string, ReviewRestoreChoice>
  selected: Record<string, boolean>
  busy: boolean
  onSelectionChange(change: (previous: Record<string, boolean>) => Record<string, boolean>): void
  onChoice(key: ReviewTargetVersionKey, choice: ReviewRestoreChoice): void
}) {
  return (
    <>
      {selectableKeys.length > 0 && (
        <section className="review-history-panel__restore" aria-label="恢复范围">
          <h3>选择要恢复的历史目标</h3>
          {selectableKeys.map((key) => (
            <label key={targetKey(key)}>
              <input
                type="checkbox"
                checked={selected[targetKey(key)] === true}
                disabled={busy}
                onChange={(event) =>
                  onSelectionChange((previous) => ({
                    ...previous,
                    [targetKey(key)]: event.currentTarget.checked,
                  }))
                }
              />
              {formatTarget(key)}
            </label>
          ))}
        </section>
      )}
      {(restorePlan !== null || conflicts.some((key) => selected[targetKey(key)])) && (
        <section className="review-history-panel__restore" aria-label="恢复冲突">
          <h3>恢复选择</h3>
          {conflicts
            .filter((key) => restorePlan !== null || selected[targetKey(key)])
            .map((key) => (
              <RestoreConflict
                key={targetKey(key)}
                history={history}
                currentFeedback={currentFeedback}
                item={key}
                choice={choices[targetKey(key)]}
                busy={busy}
                onChoice={onChoice}
              />
            ))}
          {restorePlan !== null && <RestoreImpact plan={restorePlan} />}
        </section>
      )}
    </>
  )
}

function RestoreImpact({ plan }: { plan: ReviewRestorePlan }) {
  return (
    <section aria-label="已预览的恢复影响">
      <h4>已预览的恢复影响</h4>
      {plan.restored.length === 0 ? (
        <p>此预览不会恢复任何目标。</p>
      ) : (
        plan.restored.map((item) => (
          <p key={targetKey(item.historicalKey)}>
            将恢复目标 {item.target.id}（历史目标 {item.historicalKey.targetId}）
          </p>
        ))
      )}
      {plan.coverageReversals.map((reversal) => (
        <p key={`${reversal.archiveId}:${targetKey(reversal.key)}`}>
          将{reversal.active ? '重新启用' : '撤销'}存档覆盖 {reversal.archiveId}：目标{' '}
          {reversal.key.targetId}
        </p>
      ))}
      {plan.requiresSourceCheck.length > 0 && (
        <p>恢复后需重新核验素材：{plan.requiresSourceCheck.join('、')}；这不会使意见自动可执行。</p>
      )}
    </section>
  )
}

function RestoreConflict({
  history,
  currentFeedback,
  item,
  choice,
  busy,
  onChoice,
}: {
  history: ReviewHistoryView
  currentFeedback: ReviewVersionedFeedback[]
  item: ReviewTargetVersionKey
  choice: ReviewRestoreChoice | undefined
  busy: boolean
  onChoice(key: ReviewTargetVersionKey, choice: ReviewRestoreChoice): void
}) {
  const historical = historicalText(history, item)
  const current = currentText(currentFeedback, item.targetId)
  return (
    <fieldset disabled={busy}>
      <legend>目标 {item.targetId} 的当前版本与历史版本不同</legend>
      {historical !== null && <p>历史原文：{historical}</p>}
      {current !== null && <p>当前原文：{current}</p>}
      <Choice
        checked={choice?.kind === 'preserve_current'}
        label="保留当前意见"
        name={targetKey(item)}
        onChange={() => onChoice(item, { kind: 'preserve_current' })}
      />
      <Choice
        checked={choice?.kind === 'use_historical'}
        label="使用历史意见"
        name={targetKey(item)}
        onChange={() => onChoice(item, { kind: 'use_historical' })}
      />
      <Choice
        checked={choice?.kind === 'continue_as_new'}
        label="作为新意见继续提出"
        name={targetKey(item)}
        onChange={() => onChoice(item, continueAsNew(historicalTarget(history, item)))}
      />
    </fieldset>
  )
}

function Choice({
  checked,
  label,
  name,
  onChange,
}: {
  checked: boolean
  label: string
  name: string
  onChange(): void
}) {
  return (
    <label>
      <input type="radio" name={name} checked={checked} onChange={onChange} />
      {label}
    </label>
  )
}

/** IDs are generated once when the user chooses this branch, then retained across rerenders/previews. */
function continueAsNew(
  target: { assetVersionId: string } | null,
): Extract<ReviewRestoreChoice, { kind: 'continue_as_new' }> {
  return {
    kind: 'continue_as_new',
    feedbackId: crypto.randomUUID(),
    textRevisionId: crypto.randomUUID(),
    targetId: crypto.randomUUID(),
    targetRevisionId: crypto.randomUUID(),
    targetAssetVersionId: target?.assetVersionId ?? '',
    confirmedAnchor: null,
    createdAtMs: Date.now(),
  }
}

function historicalText(history: ReviewHistoryView, key: ReviewTargetVersionKey) {
  return (
    history.entries
      .flatMap((entry) => entry.feedback)
      .find(
        (feedback) =>
          feedback.id === key.feedbackId &&
          feedback.textRevisionId === key.textRevisionId &&
          feedback.targets.some(
            (target) => target.id === key.targetId && target.revisionId === key.targetRevisionId,
          ),
      )?.text ?? null
  )
}

function historicalTarget(history: ReviewHistoryView, key: ReviewTargetVersionKey) {
  return (
    history.entries
      .flatMap((entry) => entry.feedback)
      .find(
        (feedback) =>
          feedback.id === key.feedbackId && feedback.textRevisionId === key.textRevisionId,
      )
      ?.targets.find(
        (target) => target.id === key.targetId && target.revisionId === key.targetRevisionId,
      ) ?? null
  )
}

function currentText(feedback: ReviewVersionedFeedback[], targetId: string) {
  return (
    feedback.find((item) => item.targets.some((target) => target.id === targetId))?.text ?? null
  )
}

function formatTarget(key: ReviewTargetVersionKey) {
  return `Feedback ID ${key.feedbackId} · Target ID ${key.targetId}`
}

function uniqueKeys(keys: ReviewTargetVersionKey[]) {
  return keys.filter(
    (key, index) =>
      keys.findIndex((candidate) => targetKey(candidate) === targetKey(key)) === index,
  )
}

function historyReferenceKey(reference: ReviewHistoryRef) {
  if (reference.source.kind === 'legacy')
    return `legacy:${reference.source.roundId}:${reference.source.targets[0]?.targetIndex ?? ''}`
  const key = reference.source.keys[0]
  return `snapshot:${reference.source.snapshot.snapshotId}:${key === undefined ? '' : targetKey(key)}`
}

function historyReferenceLabel(reference: ReviewHistoryRef) {
  const key = reference.source.kind === 'snapshot' ? reference.source.keys[0] : undefined
  return key === undefined ? '历史目标不可用' : formatTarget(key)
}
