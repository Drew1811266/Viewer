import { type CSSProperties, type RefObject, useRef } from 'react'
import type { ThumbnailDensity } from '../api/types'
import {
  THUMBNAIL_LEVELS,
  thumbnailDensityForLevel,
  thumbnailLevelForDensity,
  thumbnailLevelValueText,
} from '../settings/thumbnailDensity'
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

export default function SettingsDialog({
  density,
  error,
  onDensityChange,
  onClose,
  returnFocusRef,
}: SettingsDialogProps) {
  const thumbnailSizeRef = useRef<HTMLInputElement>(null)
  const level = thumbnailLevelForDensity(density)
  const progress = ((level - 1) / (THUMBNAIL_LEVELS.length - 1)) * 100
  return (
    <ModalSheet
      title="软件设置"
      size="large"
      onCancel={onClose}
      initialFocusRef={thumbnailSizeRef}
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
          <fieldset className="thumbnail-size-control">
            <legend>缩略图大小</legend>
            <input
              ref={thumbnailSizeRef}
              className="thumbnail-size-slider"
              type="range"
              name="thumbnail-size"
              min={1}
              max={5}
              step={1}
              value={level}
              aria-label="缩略图大小"
              aria-valuetext={thumbnailLevelValueText(level)}
              style={{ '--thumbnail-level-progress': `${progress}%` } as CSSProperties}
              onChange={(event) =>
                onDensityChange(thumbnailDensityForLevel(Number(event.currentTarget.value)))
              }
            />
            <div className="thumbnail-size-levels" aria-hidden="true">
              {THUMBNAIL_LEVELS.map((option) => (
                <span key={option.level} data-current={option.level === level || undefined}>
                  {option.level}
                </span>
              ))}
            </div>
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
