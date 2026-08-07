import { type CSSProperties, type RefObject, useRef } from 'react'
import type {
  MagnifierArea,
  MagnifierMagnification,
  MagnifierPreferences,
  MagnifierShape,
  ThumbnailDensity,
} from '../api/types'
import {
  THUMBNAIL_LEVELS,
  thumbnailDensityForLevel,
  thumbnailLevelForDensity,
  thumbnailLevelValueText,
} from '../settings/thumbnailDensity'
import {
  MAGNIFIER_AREAS,
  MAGNIFIER_MAGNIFICATIONS,
  MAGNIFIER_SHAPES,
} from '../settings/viewerSettings'
import ModalSheet from './ModalSheet'
import ViewerButton from './ui/ViewerButton'
import ViewerChoiceChip from './ui/ViewerChoiceChip'
import ViewerLocalFeedback from './ui/ViewerLocalFeedback'

const SHAPE_LABEL = { circle: '圆形', rounded_rectangle: '圆角矩形' } as const
const AREA_LABEL = { small: '小', medium: '中', large: '大' } as const

interface SettingsDialogProps {
  density: ThumbnailDensity
  magnifier: MagnifierPreferences
  error: string | null
  onDensityChange: (density: ThumbnailDensity) => void
  onMagnifierShapeChange: (shape: MagnifierShape) => void
  onMagnifierMagnificationChange: (magnification: MagnifierMagnification) => void
  onMagnifierAreaChange: (area: MagnifierArea) => void
  onClose: () => void
  returnFocusRef?: RefObject<HTMLElement | null>
}

export default function SettingsDialog({
  density,
  magnifier,
  error,
  onDensityChange,
  onMagnifierShapeChange,
  onMagnifierMagnificationChange,
  onMagnifierAreaChange,
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
          <h4 className="settings-dialog-subheading">图片预览</h4>
          <fieldset className="magnifier-setting-group">
            <legend>放大镜形状</legend>
            <div className="magnifier-setting-options">
              {MAGNIFIER_SHAPES.map((shape) => (
                <ViewerChoiceChip
                  key={shape}
                  type="radio"
                  name="magnifier-shape"
                  checked={magnifier.shape === shape}
                  onCheckedChange={(checked) => checked && onMagnifierShapeChange(shape)}
                >
                  {SHAPE_LABEL[shape]}
                </ViewerChoiceChip>
              ))}
            </div>
          </fieldset>
          <fieldset className="magnifier-setting-group">
            <legend>放大倍数</legend>
            <div className="magnifier-setting-options">
              {MAGNIFIER_MAGNIFICATIONS.map((magnification) => (
                <ViewerChoiceChip
                  key={magnification}
                  type="radio"
                  name="magnifier-magnification"
                  checked={magnifier.magnification === magnification}
                  onCheckedChange={(checked) =>
                    checked && onMagnifierMagnificationChange(magnification)
                  }
                >
                  {magnification} 倍
                </ViewerChoiceChip>
              ))}
            </div>
          </fieldset>
          <fieldset className="magnifier-setting-group">
            <legend>显示面积</legend>
            <div className="magnifier-setting-options">
              {MAGNIFIER_AREAS.map((area) => (
                <ViewerChoiceChip
                  key={area}
                  type="radio"
                  name="magnifier-area"
                  checked={magnifier.area === area}
                  onCheckedChange={(checked) => checked && onMagnifierAreaChange(area)}
                >
                  {AREA_LABEL[area]}
                </ViewerChoiceChip>
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
