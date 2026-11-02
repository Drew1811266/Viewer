import { useRef } from 'react'
import {
  ANNOTATION_COLORS,
  ANNOTATION_TEXT_SIZES,
  ANNOTATION_WIDTHS,
  annotationColorHex,
  nextAnnotationOpacity,
  type ReviewAnnotationColor,
  type ReviewAnnotationStyle,
  supportsColor,
  supportsDashed,
  supportsOpacity,
  supportsTextSize,
  supportsWidth,
} from '../../app/review/annotationStyle'
import type { ImageReviewWorkbenchController } from '../../app/review/useImageReviewWorkbench'
import { ViewerIconButton } from '../ui/ViewerButton'

const COLOR_LABELS: Record<ReviewAnnotationColor, string> = {
  red: '红',
  orange: '橙',
  yellow: '黄',
  green: '绿',
  teal: '青',
  blue: '蓝',
  purple: '紫',
  black: '黑',
}

const WIDTH_LABELS = { thin: '细', medium: '中', thick: '粗' } as const

export function shouldShowAnnotationStyleBar(controller: ImageReviewWorkbenchController): boolean {
  if (controller.protocol !== 'continuous' || controller.readOnlyReason !== null) return false
  const phase = controller.editor.phase
  if (phase.status === 'saving') return false
  // Keep the style bar reachable while a shape is being drawn or its text is
  // being composed; otherwise it vanishes exactly when the user just drew a
  // region and wants to adjust its color or width before saving.
  if (phase.status === 'drawing') return true
  if (phase.status !== 'idle') return phase.draftAnchor.kind.startsWith('image_')
  if (controller.tool !== 'browse') return true
  const selected = controller.feedback.find((item) => item.itemId === controller.selectedItemId)
  return selected?.anchor.kind.startsWith('image_') === true
}

export default function AnnotationStyleBar({
  controller,
}: {
  controller: ImageReviewWorkbenchController
}) {
  const style = controller.annotationStyle
  const styleRef = useRef(style)
  styleRef.current = style
  const disabled = controller.styleBusy || controller.readOnlyReason !== null
  const update = (patch: Partial<ReviewAnnotationStyle>) => {
    const next = { ...styleRef.current, ...patch }
    // Keep rapid color/width clicks coherent even before React commits the
    // previous selection back through controller.annotationStyle.
    styleRef.current = next
    controller.setAnnotationStyle(next)
  }

  return (
    <div
      className="annotation-style-bar"
      data-testid="annotation-style-bar"
      data-native-input-exclusion="true"
      role="toolbar"
      aria-label="标注样式"
    >
      <div className="annotation-style-bar__group" role="group" aria-label="颜色">
        {ANNOTATION_COLORS.map((color) => (
          <button
            key={color}
            type="button"
            className="annotation-style-bar__color"
            data-color={color}
            data-selected={style.color === color || undefined}
            aria-label={`颜色${COLOR_LABELS[color]}`}
            aria-pressed={style.color === color}
            disabled={disabled || !supportsColor(style.tool)}
            onClick={() => update({ color })}
          />
        ))}
      </div>
      <span className="annotation-style-bar__divider" />
      <div className="annotation-style-bar__group" role="group" aria-label="线条粗细">
        {ANNOTATION_WIDTHS.map((width) => (
          <button
            key={width}
            type="button"
            className="annotation-style-bar__width"
            data-width={width}
            data-selected={style.width === width || undefined}
            aria-label={`线条${WIDTH_LABELS[width]}`}
            aria-pressed={style.width === width}
            disabled={disabled || !supportsWidth(style.tool)}
            onClick={() => update({ width })}
          >
            <span style={{ background: annotationColorHex(style.color) }} />
          </button>
        ))}
      </div>
      <span className="annotation-style-bar__divider" />
      <button
        type="button"
        className="annotation-style-bar__control annotation-style-bar__opacity"
        aria-label="不透明度"
        disabled={disabled || !supportsOpacity(style.tool)}
        onClick={() => update({ opacityPercent: nextAnnotationOpacity(style.opacityPercent) })}
      >
        {style.opacityPercent}%
      </button>
      <button
        type="button"
        className="annotation-style-bar__control annotation-style-bar__dash-button"
        data-active={style.dashed || undefined}
        aria-label="虚线"
        aria-pressed={style.dashed}
        disabled={disabled || !supportsDashed(style.tool)}
        onClick={() => update({ dashed: !style.dashed })}
      >
        <span className="annotation-style-bar__dash" aria-hidden="true" />
      </button>
      <span className="annotation-style-bar__divider" />
      {ANNOTATION_TEXT_SIZES.map((size, index) => (
        <button
          key={size}
          type="button"
          className="annotation-style-bar__text-size"
          data-size={size}
          data-selected={style.textSize === size || undefined}
          aria-label={['小字号', '中字号', '大字号'][index]}
          aria-pressed={style.textSize === size}
          disabled={disabled || !supportsTextSize(style.tool)}
          onClick={() => update({ textSize: size })}
        >
          {['A−', 'A', 'A+'][index]}
        </button>
      ))}
      <span className="annotation-style-bar__divider" />
      <ViewerIconButton
        icon="undo-2"
        label="撤销"
        tone="quiet"
        disabled={!controller.canUndo}
        onClick={controller.undo}
      />
      <ViewerIconButton
        icon="redo-2"
        label="重做"
        tone="quiet"
        disabled={!controller.canRedo}
        onClick={controller.redo}
      />
    </div>
  )
}
