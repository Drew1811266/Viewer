import { useRef } from 'react'
import type {
  ReviewArchiveBasis,
  ReviewArchivePlan,
  ReviewArchiveSelection,
  ReviewTargetVersionKey,
} from '../../api/reviewWorkspaceTypes'
import ModalSheet from '../ModalSheet'
import ViewerButton from '../ui/ViewerButton'

export type ArchivePlanDto = ReviewArchivePlan
export type ArchiveSelectionDto = ReviewArchiveSelection

export interface ReviewArchiveDialogProps {
  preview: ArchivePlanDto
  selection: ArchiveSelectionDto
  busy: boolean
  error: string | null
  onSelectionChange(selection: ArchiveSelectionDto): void
  onConfirm(): void
  onCancel(): void
}

export default function ReviewArchiveDialog({
  preview,
  selection,
  busy,
  error,
  onSelectionChange,
  onConfirm,
  onCancel,
}: ReviewArchiveDialogProps) {
  const cancelRef = useRef<HTMLButtonElement>(null)
  const selected = selectedKeys(selection)
  const groups = availableGroups(preview, selection)
  const archiveable =
    selection.groups.some((group) => group.targets.length > 0) &&
    preview.groups.some((group) => group.targets.length > 0)

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
      size="large"
      onCancel={() => {
        if (!busy) onCancel()
      }}
      initialFocusRef={cancelRef}
      footer={
        <>
          <ViewerButton ref={cancelRef} disabled={busy} onClick={onCancel}>
            返回检查
          </ViewerButton>
          <ViewerButton tone="primary" loading={busy} disabled={!archiveable} onClick={onConfirm}>
            确认存档
          </ViewerButton>
        </>
      }
    >
      <p>存档会将下列精确意见版本移入历史，不会表示意见已被执行或修复。历史中的内容可撤销。</p>
      <p className="review-archive-dialog__snapshot">
        当前预览绑定的意见版本：<code>{preview.expectedSnapshotId}</code>
      </p>
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
                      {formatTarget(key)}
                    </label>
                  </li>
                )
              })}
            </ul>
          </fieldset>
        ))}
      </section>
      <ArchiveKeyList
        title="将移入历史"
        keys={preview.removed}
        empty="当前选择没有会移入历史的意见。"
      />
      <section className="review-archive-dialog__section" aria-labelledby="archive-retained-title">
        <h3 id="archive-retained-title">保留后补意见</h3>
        {preview.retained.length === 0 ? (
          <p>没有后补意见需要保留。</p>
        ) : (
          <ul>
            {preview.retained.map((retention) => (
              <li key={`${targetKey(retention.basis)}:${retention.disposition}`}>
                <span>{formatTarget(retention.basis)}</span>
                {retention.current !== null && (
                  <span>保留当前版本：{formatTarget(retention.current)}</span>
                )}
              </li>
            ))}
          </ul>
        )}
      </section>
      <ArchiveKeyList
        title="未选素材"
        keys={groups
          .flatMap((group) => group.targets)
          .filter((key) => !selected.has(targetKey(key)))}
        empty="没有未选素材。"
      />
      {preview.alreadyCovered.length > 0 && (
        <ArchiveKeyList title="已有效覆盖" keys={preview.alreadyCovered} empty="" />
      )}
    </ModalSheet>
  )
}

function ArchiveKeyList({
  title,
  keys,
  empty,
}: {
  title: string
  keys: ReviewTargetVersionKey[]
  empty: string
}) {
  const id = `archive-${title}`
  return (
    <section className="review-archive-dialog__section" aria-labelledby={id}>
      <h3 id={id}>{title}</h3>
      {keys.length === 0 ? (
        <p>{empty}</p>
      ) : (
        <ul>
          {keys.map((key) => (
            <li key={targetKey(key)}>{formatTarget(key)}</li>
          ))}
        </ul>
      )}
    </section>
  )
}

function availableGroups(preview: ArchivePlanDto, selection: ArchiveSelectionDto) {
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

function formatTarget(key: ReviewTargetVersionKey) {
  return `Feedback ID ${key.feedbackId} · Text revision ${key.textRevisionId} · Target ID ${key.targetId} · Target revision ${key.targetRevisionId}`
}
