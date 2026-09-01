import type { ReviewHistorySelector } from '../../api/reviewWorkspaceTypes'
import ModalSheet from '../ModalSheet'
import ViewerButton from '../ui/ViewerButton'

interface ReviewHistorySelectorDialogProps {
  selectors: ReviewHistorySelector[]
  selectedIndex: number | null
  onSelect(index: number): void
  onConfirm(): void
  onCancel(): void
}

export default function ReviewHistorySelectorDialog({
  selectors,
  selectedIndex,
  onSelect,
  onConfirm,
  onCancel,
}: ReviewHistorySelectorDialogProps) {
  return (
    <ModalSheet
      title="选择历史记录"
      onCancel={onCancel}
      footer={
        <>
          <ViewerButton onClick={onCancel}>取消</ViewerButton>
          <ViewerButton tone="primary" disabled={selectedIndex === null} onClick={onConfirm}>
            查看所选历史
          </ViewerButton>
        </>
      }
    >
      <p>选择一条历史记录查看。当前意见不会被修改。</p>
      <fieldset className="review-history-selector__choices">
        <legend>可用历史记录</legend>
        {selectors.map((selector, index) => (
          <label key={selectorKey(selector)}>
            <input
              type="radio"
              name="review-history-selector"
              checked={selectedIndex === index}
              onChange={() => onSelect(index)}
            />
            <span>{selectorLabel(selector, index)}</span>
          </label>
        ))}
      </fieldset>
    </ModalSheet>
  )
}

function selectorLabel(selector: ReviewHistorySelector, index: number): string {
  const position = index + 1
  if (selector.kind === 'archive') return `存档历史 ${position}`
  if (selector.kind === 'legacy') return `旧评审记录 ${position}`
  return `保存快照 ${position}`
}

function selectorKey(selector: ReviewHistorySelector): string {
  if (selector.kind === 'archive') return `archive:${selector.archiveId}`
  if (selector.kind === 'legacy') return `legacy:${selector.roundId}`
  return `snapshot:${selector.snapshot.snapshotId}`
}
