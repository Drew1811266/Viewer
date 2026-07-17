import { useEffect } from 'react'
import type { ReviewState, SelectionAgreement, SelectionInfo } from '../api/types'

interface MarkerControlsProps {
  selectedCount: number
  selectionInfo: SelectionInfo | null
  readOnly: boolean
  onSetReview: (state: ReviewState | null) => void
  onToggleFavorite: () => void
}

export default function MarkerControls({
  selectedCount,
  selectionInfo,
  readOnly,
  onSetReview,
  onToggleFavorite,
}: MarkerControlsProps) {
  const disabled = readOnly || selectedCount === 0
  useEffect(() => {
    function shortcut(event: KeyboardEvent) {
      if (disabled || event.metaKey || event.ctrlKey || event.altKey || ownsTextInput(event.target)) {
        return
      }
      const key = event.key.toLowerCase()
      const review =
        key === '1'
          ? 'keep'
          : key === '2'
            ? 'pending'
            : key === '3'
              ? 'reject'
              : key === '0'
                ? null
                : undefined
      if (review !== undefined) {
        event.preventDefault()
        onSetReview(review)
      } else if (key === 'f') {
        event.preventDefault()
        onToggleFavorite()
      }
    }
    window.addEventListener('keydown', shortcut)
    return () => window.removeEventListener('keydown', shortcut)
  }, [disabled, onSetReview, onToggleFavorite])

  return (
    <section className="marker-controls" aria-label="批量标记">
      <div className="marker-buttons">
        <MarkerButton
          label="标记为保留"
          text="✓ 保留"
          shortcut="1"
          pressed={commonReview(selectionInfo) === 'keep'}
          disabled={disabled}
          onClick={() => onSetReview('keep')}
        />
        <MarkerButton
          label="标记为待定"
          text="• 待定"
          shortcut="2"
          pressed={commonReview(selectionInfo) === 'pending'}
          disabled={disabled}
          onClick={() => onSetReview('pending')}
        />
        <MarkerButton
          label="标记为淘汰"
          text="× 淘汰"
          shortcut="3"
          pressed={commonReview(selectionInfo) === 'reject'}
          disabled={disabled}
          onClick={() => onSetReview('reject')}
        />
        <MarkerButton
          label="清除审阅状态"
          text="○ 清除"
          shortcut="0"
          pressed={commonReview(selectionInfo) === null}
          disabled={disabled}
          onClick={() => onSetReview(null)}
        />
        <MarkerButton
          label="切换收藏"
          text="★ 收藏"
          shortcut="F"
          pressed={commonFavorite(selectionInfo) === true}
          disabled={disabled}
          onClick={onToggleFavorite}
        />
      </div>
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

function MarkerButton({
  label,
  text,
  shortcut,
  pressed,
  disabled,
  onClick,
}: {
  label: string
  text: string
  shortcut: string
  pressed: boolean
  disabled: boolean
  onClick: () => void
}) {
  return (
    <button
      type="button"
      aria-label={label}
      aria-pressed={pressed}
      disabled={disabled}
      title={`${label}（${shortcut}）`}
      onClick={onClick}
    >
      {text}
      <kbd>{shortcut}</kbd>
    </button>
  )
}

function ownsTextInput(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false
  return (
    target instanceof HTMLInputElement ||
    target instanceof HTMLTextAreaElement ||
    target instanceof HTMLSelectElement ||
    target.isContentEditable ||
    target.closest('[contenteditable="true"]') !== null
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
