import { useRef } from 'react'
import type {
  ReviewArchiveBasis,
  ReviewArchivePlan,
  ReviewArchiveSelection,
  ReviewContinuousState,
  ReviewTargetVersionKey,
} from '../../api/reviewWorkspaceTypes'
import { reviewAnchorLabel } from '../../app/review/reviewAnchorLabel'
import ModalSheet from '../ModalSheet'
import ViewerButton from '../ui/ViewerButton'
import ViewerIcon from '../ui/ViewerIcon'

export type ArchivePlanDto = ReviewArchivePlan
export type ArchiveSelectionDto = ReviewArchiveSelection

export interface ReviewArchiveDialogProps {
  preview: ArchivePlanDto
  selection: ArchiveSelectionDto
  state?: ReviewContinuousState | null
  busy: boolean
  error: string | null
  onSelectionChange(selection: ArchiveSelectionDto): void
  onConfirm(): void
  onCancel(): void
}

export default function ReviewArchiveDialog({
  preview,
  selection,
  state,
  busy,
  error,
  onSelectionChange,
  onConfirm,
  onCancel,
}: ReviewArchiveDialogProps) {
  const cancelRef = useRef<HTMLButtonElement>(null)
  const selected = selectedKeys(selection)
  const groups = availableArchiveGroups(preview, selection)
  const retainedLaterEdits = preview.retained.filter(
    (retention) => retention.disposition === 'retain_later_edit',
  )
  const movedFromCurrent = uniqueTargets([
    ...preview.removed,
    ...preview.retained
      .filter((retention) => retention.disposition === 'remove_current')
      .map((retention) => retention.current ?? retention.basis),
  ])
  const alreadyAbsent = preview.retained.filter(
    (retention) => retention.disposition === 'already_absent',
  )
  const allSelectedCovered =
    selected.size > 0 &&
    [...selected].every((key) =>
      preview.alreadyCovered.some((covered) => targetKey(covered) === key),
    )
  const archiveable =
    selection.groups.some((group) => group.targets.length > 0) && !allSelectedCovered

  function toggle(basis: ReviewArchiveBasis, key: ReviewTargetVersionKey, checked: boolean) {
    const nextGroups = selection.groups
      .map((group) =>
        sameBasis(group.basis, basis)
          ? {
              ...group,
              targets: checked
                ? [...group.targets, key]
                : group.targets.filter((target) => !sameTarget(target, key)),
            }
          : group,
      )
      .filter((group) => group.targets.length > 0)
    if (checked && !selection.groups.some((group) => sameBasis(group.basis, basis))) {
      nextGroups.push({ basis, targets: [key] })
    }
    onSelectionChange({ expectedSnapshotId: preview.expectedSnapshotId, groups: nextGroups })
  }

  return (
    <ModalSheet
      title="确认存档范围"
      description="存档会将下列精确意见版本移入历史，不会表示意见已被执行或修复。"
      size="large"
      showClose={false}
      onCancel={() => {
        if (!busy) onCancel()
      }}
      initialFocusRef={cancelRef}
      footer={
        <div className="review-archive-footer">
          <ViewerButton ref={cancelRef} disabled={busy} onClick={onCancel}>
            返回检查
          </ViewerButton>
          <ViewerButton tone="primary" loading={busy} disabled={!archiveable} onClick={onConfirm}>
            确认存档
          </ViewerButton>
        </div>
      }
    >
      <p className="review-archive-dialog__snapshot">
        <ViewerIcon name="info" />
        <span>当前预览绑定版本</span>
        <code title={preview.expectedSnapshotId}>最近一次保存</code>
      </p>
      {allSelectedCovered && <p role="status">没有新的可存档内容</p>}
      {error !== null && (
        <p className="review-archive-dialog__error" role="alert">
          {error}
        </p>
      )}
      <section className="review-archive-dialog__groups" aria-label="存档选择">
        {groups.map((group) => (
          <fieldset key={basisKey(group.basis)} disabled={busy}>
            <legend>{basisLabel(group.basis)}</legend>
            <ul>
              {group.targets.map((key) => {
                const keyId = targetKey(key)
                return (
                  <li key={keyId}>
                    <label>
                      <input
                        type="checkbox"
                        checked={selected.has(keyId)}
                        onChange={(event) => toggle(group.basis, key, event.currentTarget.checked)}
                      />
                      <span>{formatTarget(key, state)}</span>
                    </label>
                  </li>
                )
              })}
            </ul>
          </fieldset>
        ))}
      </section>
      <ArchiveKeyList
        title={
          movedFromCurrent.length > 0 ? `将移入历史 · ${movedFromCurrent.length} 条` : '将移入历史'
        }
        description="这些意见会保留原文、素材版本和标注位置；不会改写已存档历史。"
        keys={movedFromCurrent}
        empty="当前选择没有会移入历史的意见。"
        state={state}
      />
      <div className="review-archive-warning">
        <ViewerIcon name="info" />
        <span>存档不证明 Agent 已读取、执行或修复，也不证明验收通过。</span>
      </div>
      <section className="review-archive-dialog__section" aria-labelledby="archive-retained-title">
        <h3 id="archive-retained-title">保留后补意见</h3>
        {retainedLaterEdits.length === 0 ? (
          <p>没有后补意见需要保留。</p>
        ) : (
          <ul>
            {retainedLaterEdits.map((retention) => (
              <li key={`${targetKey(retention.basis)}:${retention.disposition}`}>
                <span>{formatTarget(retention.basis, state)}</span>
                {retention.current !== null && (
                  <span>保留当前版本：{formatTarget(retention.current, state)}</span>
                )}
              </li>
            ))}
          </ul>
        )}
      </section>
      {alreadyAbsent.length > 0 && (
        <section className="review-archive-dialog__section" aria-labelledby="archive-absent-title">
          <h3 id="archive-absent-title">当前已不存在</h3>
          <p>这些目标当前已不在意见中；选中的历史依据仍会按预览记录。</p>
          <ul>
            {alreadyAbsent.map((retention) => (
              <li key={`${targetKey(retention.basis)}:${retention.disposition}`}>
                {formatTarget(retention.basis, state)}
              </li>
            ))}
          </ul>
        </section>
      )}
      {groups.flatMap((group) => group.targets).some((key) => !selected.has(targetKey(key))) && (
        <ArchiveKeyList
          title="未选素材"
          keys={groups
            .flatMap((group) => group.targets)
            .filter((key) => !selected.has(targetKey(key)))}
          empty="没有未选素材。"
          state={state}
        />
      )}
      {preview.alreadyCovered.length > 0 && (
        <ArchiveKeyList title="已有效覆盖" keys={preview.alreadyCovered} empty="" state={state} />
      )}
    </ModalSheet>
  )
}

function ArchiveKeyList({
  title,
  description,
  keys,
  empty,
  state,
}: {
  title: string
  description?: string
  keys: ReviewTargetVersionKey[]
  empty: string
  state?: ReviewContinuousState | null
}) {
  const id = `archive-${title}`
  return (
    <section className="review-archive-dialog__section" aria-labelledby={id}>
      <h3 id={id}>{title}</h3>
      {description !== undefined && keys.length > 0 && <p>{description}</p>}
      {keys.length === 0 ? (
        <p>{empty}</p>
      ) : (
        <ul>
          {keys.map((key) => (
            <li key={targetKey(key)}>{formatTarget(key, state)}</li>
          ))}
        </ul>
      )}
    </section>
  )
}

export function availableArchiveGroups(preview: ArchivePlanDto, selection: ArchiveSelectionDto) {
  const groups = preview.groups.map((group) => ({ ...group, targets: [...group.targets] }))
  for (const selected of selection.groups) {
    const existing = groups.find((group) => sameBasis(group.basis, selected.basis))
    if (existing === undefined) groups.push({ ...selected, targets: [...selected.targets] })
    else {
      for (const key of selected.targets) {
        if (!existing.targets.some((candidate) => sameTarget(candidate, key)))
          existing.targets.push(key)
      }
    }
  }
  return groups
}

function selectedKeys(selection: ArchiveSelectionDto) {
  return new Set(selection.groups.flatMap((group) => group.targets.map(targetKey)))
}

function uniqueTargets(keys: ReviewTargetVersionKey[]) {
  return keys.filter(
    (key, index) => keys.findIndex((candidate) => sameTarget(candidate, key)) === index,
  )
}

function basisLabel(basis: ReviewArchiveBasis) {
  return basis.kind === 'known' ? '已核对交接版本' : '交接版本未确认'
}

function basisKey(basis: ReviewArchiveBasis) {
  if (basis.kind === 'unknown') return 'unknown'
  return `${basis.snapshot.snapshotId}:${basis.snapshot.blake3}:${basis.source.kind}:${basis.source.kind === 'agent_declared' ? basis.source.usageId : ''}`
}

function sameBasis(left: ReviewArchiveBasis, right: ReviewArchiveBasis) {
  return basisKey(left) === basisKey(right)
}

function sameTarget(left: ReviewTargetVersionKey, right: ReviewTargetVersionKey) {
  return targetKey(left) === targetKey(right)
}

function targetKey(key: ReviewTargetVersionKey) {
  return `${key.feedbackId}\u0000${key.textRevisionId}\u0000${key.targetId}\u0000${key.targetRevisionId}`
}

function formatTarget(key: ReviewTargetVersionKey, state?: ReviewContinuousState | null) {
  if (state === null || state === undefined) {
    return `Feedback ID ${key.feedbackId} · Text revision ${key.textRevisionId} · Target ID ${key.targetId} · Target revision ${key.targetRevisionId}`
  }
  const feedback = state.feedback.find((entry) => entry.id === key.feedbackId)
  if (feedback === undefined) return '历史版本素材 · 当前意见中不可见'
  const ordinal = state === null || state === undefined ? 0 : state.feedback.indexOf(feedback) + 1
  const target = feedback.targets.find(
    (entry) => entry.id === key.targetId && entry.revisionId === key.targetRevisionId,
  )
  const asset = state?.assets.find((entry) => entry.id === target?.assetVersionId)
  const name = asset?.relativePath.split('/').at(-1) ?? '未知素材'
  const version = feedback.textRevisionId === key.textRevisionId ? '' : '（旧文字版本）'
  return `${name} · ${reviewAnchorLabel(target?.anchor.kind)} · 意见 ${ordinal}${version}`
}
