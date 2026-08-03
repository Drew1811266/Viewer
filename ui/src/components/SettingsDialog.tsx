import { type RefObject, useRef } from 'react'
import type { ThumbnailDensity } from '../api/types'
import ModalSheet from './ModalSheet'
import ViewerButton from './ui/ViewerButton'
import ViewerLocalFeedback from './ui/ViewerLocalFeedback'

interface SettingsDialogProps {
  density: ThumbnailDensity
  error: string | null
  onDensityChange: (density: ThumbnailDensity) => void
  onClose: () => void
  returnFocusRef?: RefObject<HTMLElement | null>
}

const DENSITY_OPTIONS: { value: ThumbnailDensity; label: string }[] = [
  { value: 'compact', label: '紧凑' },
  { value: 'standard', label: '标准' },
  { value: 'large', label: '大图' },
]

export default function SettingsDialog({
  density,
  error,
  onDensityChange,
  onClose,
  returnFocusRef,
}: SettingsDialogProps) {
  const firstDensityRef = useRef<HTMLInputElement>(null)
  return (
    <ModalSheet
      title="软件设置"
      onCancel={onClose}
      initialFocusRef={firstDensityRef}
      returnFocusRef={returnFocusRef}
      footer={
        <ViewerButton tone="secondary" onClick={onClose}>
          关闭
        </ViewerButton>
      }
    >
      <div className="settings-dialog-shell">
        <nav className="settings-dialog-navigation" aria-label="设置分类">
          <ViewerButton tone="quiet" active>
            显示与外观
          </ViewerButton>
        </nav>
        <section className="settings-dialog-content">
          <h3>显示与外观</h3>
          <fieldset className="density-options">
            <legend>缩略图密度</legend>
            {DENSITY_OPTIONS.map((option, index) => (
              <label key={option.value} data-checked={density === option.value || undefined}>
                <input
                  ref={index === 0 ? firstDensityRef : undefined}
                  type="radio"
                  name="thumbnail-density"
                  value={option.value}
                  checked={density === option.value}
                  onChange={() => onDensityChange(option.value)}
                />
                {option.label}
              </label>
            ))}
          </fieldset>
          {error && (
            <ViewerLocalFeedback tone="danger" title="无法保存设置">
              {error}
            </ViewerLocalFeedback>
          )}
        </section>
      </div>
    </ModalSheet>
  )
}
