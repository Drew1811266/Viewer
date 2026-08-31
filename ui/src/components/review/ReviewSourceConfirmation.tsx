import { useEffect, useRef, useState } from 'react'
import type {
  ReviewAssetVersion,
  ReviewSourceBindingDecision,
  ReviewTargetVersionKey,
  ReviewWorkspaceError,
} from '../../api/reviewWorkspaceTypes'
import ModalSheet from '../ModalSheet'
import ViewerButton from '../ui/ViewerButton'

export type SourceBindingDecisionDto = ReviewSourceBindingDecision

export interface ReviewSourceConfirmationProps {
  oldAsset: ReviewAssetVersion
  candidates: ReviewAssetVersion[]
  originalAnchor: ReviewSourceBindingDecision['anchor']
  targetKey: ReviewTargetVersionKey
  busy: boolean
  error: ReviewWorkspaceError | null
  onCancel(): void
  onConfirm(decision: SourceBindingDecisionDto): void
}

export default function ReviewSourceConfirmation({
  oldAsset,
  candidates,
  originalAnchor,
  targetKey,
  busy,
  error,
  onCancel,
  onConfirm,
}: ReviewSourceConfirmationProps) {
  const cancelRef = useRef<HTMLButtonElement>(null)
  const [selection, setSelection] = useState<{
    assetVersionId: string
    targetIdentity: string
  } | null>(null)
  const [positionConfirmed, setPositionConfirmed] = useState(false)
  const candidateIds = candidates.map((candidate) => candidate.id).join('\u0000')
  const targetIdentity = `${targetKey.feedbackId}\u0000${targetKey.textRevisionId}\u0000${targetKey.targetId}\u0000${targetKey.targetRevisionId}`
  const selectedAssetVersionId =
    selection?.targetIdentity === targetIdentity ? selection.assetVersionId : null
  const selectedCandidate = candidates.find((candidate) => candidate.id === selectedAssetVersionId)

  useEffect(() => {
    if (selectedAssetVersionId === null) return
    if (candidates.some((candidate) => candidate.id === selectedAssetVersionId)) return
    setSelection(null)
    setPositionConfirmed(false)
  }, [candidateIds, selectedAssetVersionId])

  useEffect(() => {
    setSelection(null)
    setPositionConfirmed(false)
  }, [targetIdentity])
  const canConfirm = selectedCandidate !== undefined && positionConfirmed

  return (
    <ModalSheet
      title="确认素材适用性"
      onCancel={() => {
        if (!busy) onCancel()
      }}
      initialFocusRef={cancelRef}
      footer={
        <>
          <ViewerButton ref={cancelRef} disabled={busy} onClick={onCancel}>
            返回历史
          </ViewerButton>
          <ViewerButton
            tone="primary"
            loading={busy}
            disabled={!canConfirm}
            onClick={() => {
              if (!canConfirm || selectedCandidate === undefined) {
                setSelection(null)
                setPositionConfirmed(false)
                return
              }
              onConfirm({
                targetKey,
                newAssetVersionId: selectedCandidate.id,
                anchor: originalAnchor,
                confirmation: { kind: 'user_confirmed' },
              })
            }}
          >
            确认素材与位置
          </ViewerButton>
        </>
      }
    >
      <p>历史意见原先指向：{oldAsset.relativePath}</p>
      <p>请明确选择已核验的候选素材；同一路径的新文件也不会自动视为同一版本。</p>
      {error !== null && (
        <p className="review-source-confirmation__error" role="alert">
          {error.message}
        </p>
      )}
      {candidates.length === 0 ? (
        <p role="status">没有可确认的候选素材；可以返回历史，或对独立新图开始新的评审。</p>
      ) : (
        <fieldset disabled={busy}>
          <legend>核验后的候选素材</legend>
          {candidates.map((candidate) => (
            <label key={candidate.id}>
              <input
                type="radio"
                name="source-candidate"
                checked={selectedAssetVersionId === candidate.id}
                onChange={() => {
                  setSelection({ assetVersionId: candidate.id, targetIdentity })
                  setPositionConfirmed(false)
                }}
              />
              {candidate.relativePath}（版本 {candidate.id}）
            </label>
          ))}
        </fieldset>
      )}
      <label>
        <input
          type="checkbox"
          checked={positionConfirmed}
          disabled={busy || selectedAssetVersionId === null}
          onChange={(event) => setPositionConfirmed(event.currentTarget.checked)}
        />
        我已确认该位置适用于所选素材
      </label>
      <p>这不会执行或修复意见。</p>
    </ModalSheet>
  )
}
