import type { ReviewState, SelectionAgreement, SelectionInfo } from '../api/types'
import useReviewShortcuts from '../state/useReviewShortcuts'
import ViewerButton from './ui/ViewerButton'
import type { ViewerIconName } from './ui/ViewerIcon'

interface MarkerControlsProps {
  selectedCount: number
  selectionInfo: SelectionInfo | null
  readOnly: boolean
  onSetReview: (state: ReviewState | null) => void
  onToggleFavorite: () => void
  shortcutsDisabled?: boolean
}

export default function MarkerControls({
  selectedCount,
  selectionInfo,
  readOnly,
  onSetReview,
  onToggleFavorite,
  shortcutsDisabled = false,
}: MarkerControlsProps) {
  const disabled = readOnly || selectedCount === 0 || shortcutsDisabled
  useReviewShortcuts({ disabled, onSetReview, onToggleFavorite })

  return (
    <section className="marker-controls" aria-label="批量标记">
      <MarkerButtons
        reviewState={commonReview(selectionInfo)}
        favorite={commonFavorite(selectionInfo)}
        disabled={disabled}
        onSetReview={onSetReview}
        onToggleFavorite={onToggleFavorite}
      />
      <div className="marker-selection-summary" role="status">
        <span>{selectedCount === 0 ? '未选择项目' : `已选 ${selectedCount} 项`}</span>
        {selectedCount > 0 && (
          <>
            <span>审阅状态：{agreementLabel(selectionInfo?.commonReview, reviewLabel)}</span>
            <span>收藏：{agreementLabel(selectionInfo?.commonFavorite, yesNoLabel)}</span>
          </>
        )}
        {readOnly && <span>只读模式不可修改</span>}
      </div>
    </section>
  )
}

export function MarkerButtons({
  reviewState,
  favorite,
  disabled,
  labelPrefix = '',
  showShortcuts = true,
  onSetReview,
  onToggleFavorite,
}: {
  reviewState: ReviewState | null | 'mixed' | undefined
  favorite: boolean | 'mixed' | undefined
  disabled: boolean
  labelPrefix?: string
  showShortcuts?: boolean
  onSetReview: (state: ReviewState | null) => void
  onToggleFavorite: () => void
}) {
  return (
    <div className="marker-buttons">
      <MarkerButton
        label={`${labelPrefix}标记为保留`}
        text="保留"
        icon="check"
        shortcut={showShortcuts ? '1' : undefined}
        pressed={reviewState === 'keep'}
        disabled={disabled}
        onClick={() => onSetReview('keep')}
      />
      <MarkerButton
        label={`${labelPrefix}标记为待定`}
        text="待定"
        icon="circle-dot"
        shortcut={showShortcuts ? '2' : undefined}
        pressed={reviewState === 'pending'}
        disabled={disabled}
        onClick={() => onSetReview('pending')}
      />
      <MarkerButton
        label={`${labelPrefix}标记为淘汰`}
        text="淘汰"
        icon="x"
        shortcut={showShortcuts ? '3' : undefined}
        pressed={reviewState === 'reject'}
        disabled={disabled}
        onClick={() => onSetReview('reject')}
      />
      <MarkerButton
        label={`${labelPrefix}清除审阅状态`}
        text="清除"
        icon="circle"
        shortcut={showShortcuts ? '0' : undefined}
        pressed={reviewState === null}
        disabled={disabled}
        onClick={() => onSetReview(null)}
      />
      <MarkerButton
        label={`${labelPrefix}切换收藏`}
        text="收藏"
        icon="star"
        shortcut={showShortcuts ? 'F' : undefined}
        pressed={favorite === true}
        disabled={disabled}
        onClick={onToggleFavorite}
      />
    </div>
  )
}

function MarkerButton({
  label,
  text,
  icon,
  shortcut,
  pressed,
  disabled,
  onClick,
}: {
  label: string
  text: string
  icon: ViewerIconName
  shortcut?: string
  pressed: boolean
  disabled: boolean
  onClick: () => void
}) {
  return (
    <ViewerButton
      tone="secondary"
      leadingIcon={icon}
      aria-label={label}
      active={pressed}
      disabled={disabled}
      title={shortcut ? `${label}（${shortcut}）` : label}
      onClick={onClick}
    >
      {text}
      {shortcut && <kbd>{shortcut}</kbd>}
    </ViewerButton>
  )
}

function commonReview(info: SelectionInfo | null): ReviewState | null | 'mixed' | undefined {
  if (info?.commonReview.state === 'common') return info.commonReview.value
  return info?.commonReview.state === 'mixed' ? 'mixed' : undefined
}

function commonFavorite(info: SelectionInfo | null): boolean | 'mixed' | undefined {
  if (info?.commonFavorite.state === 'common') return info.commonFavorite.value
  return info?.commonFavorite.state === 'mixed' ? 'mixed' : undefined
}

function agreementLabel<T>(
  agreement: SelectionAgreement<T> | undefined,
  valueLabel: (value: T) => string,
): string {
  if (agreement === undefined || agreement.state === 'none_selected') return '未知'
  if (agreement.state === 'mixed') return '混合'
  return valueLabel(agreement.value)
}

function reviewLabel(value: ReviewState | null): string {
  if (value === 'keep') return '保留'
  if (value === 'pending') return '待定'
  if (value === 'reject') return '淘汰'
  return '未标记'
}

function yesNoLabel(value: boolean): string {
  return value ? '是' : '否'
}
