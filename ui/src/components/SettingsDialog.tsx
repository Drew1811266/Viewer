import type { ThumbnailDensity } from '../api/types'
import ModalSheet from './ModalSheet'

interface SettingsDialogProps {
  density: ThumbnailDensity
  error: string | null
  onDensityChange: (density: ThumbnailDensity) => void
  onClose: () => void
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
}: SettingsDialogProps) {
  return (
    <ModalSheet title="软件设置" onCancel={onClose}>
      <section className="settings-dialog-content">
        <h3>显示</h3>
        <fieldset className="density-options">
          <legend>缩略图密度</legend>
          {DENSITY_OPTIONS.map((option) => (
            <label key={option.value}>
              <input
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
          <p className="settings-error" role="alert">
            {error}
          </p>
        )}
      </section>
      <div className="modal-actions">
        <button type="button" onClick={onClose}>
          关闭
        </button>
      </div>
    </ModalSheet>
  )
}
