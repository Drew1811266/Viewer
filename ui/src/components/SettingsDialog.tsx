import { type CSSProperties, type RefObject, useEffect, useRef, useState } from 'react'
import type {
  MagnifierArea,
  MagnifierMagnification,
  MagnifierPreferences,
  MagnifierShape,
  ThumbnailDensity,
  VideoCacheStats,
} from '../api/types'
import type { SettingsPort } from '../app/workspace/ports'
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
const VIDEO_CACHE_BUDGET_BYTES = 1_073_741_824

interface SettingsDialogProps {
  bridge: SettingsPort
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
  bridge,
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
  const [cacheStats, setCacheStats] = useState<VideoCacheStats | null>(null)
  const [cacheStatus, setCacheStatus] = useState<'loading' | 'ready' | 'cleared' | 'failed'>(
    'loading',
  )
  const [confirmingCacheClear, setConfirmingCacheClear] = useState(false)
  const [clearingCache, setClearingCache] = useState(false)
  const level = thumbnailLevelForDensity(density)
  const progress = ((level - 1) / (THUMBNAIL_LEVELS.length - 1)) * 100

  useEffect(() => {
    let open = true
    void bridge.videoCacheStats().then(
      (stats) => {
        if (!open) return
        setCacheStats(stats)
        setCacheStatus('ready')
      },
      () => {
        if (open) setCacheStatus('failed')
      },
    )
    return () => {
      open = false
    }
  }, [bridge])

  async function clearVideoCache() {
    setClearingCache(true)
    try {
      const stats = await bridge.videoCacheClear()
      setCacheStats(stats)
      setCacheStatus('cleared')
      setConfirmingCacheClear(false)
    } catch {
      setCacheStatus('failed')
    } finally {
      setClearingCache(false)
    }
  }

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
          <h4 className="settings-dialog-subheading">视频缓存</h4>
          <section className="video-cache-setting" aria-label="视频缓存">
            <div className="video-cache-setting__summary">
              <span>
                {cacheStats === null
                  ? '正在读取缓存用量…'
                  : `${formatCacheBytes(cacheStats.bytesUsed)} / 1 GiB`}
              </span>
              <span>
                {cacheStats === null ? '— 个缓存项' : `${cacheStats.entryCount} 个缓存项`}
              </span>
            </div>
            <ViewerButton
              tone="secondary"
              disabled={cacheStatus === 'loading' || clearingCache}
              onClick={() => {
                setCacheStatus(cacheStats === null ? 'loading' : 'ready')
                setConfirmingCacheClear(true)
              }}
            >
              清除视频缓存
            </ViewerButton>
          </section>
          {confirmingCacheClear && (
            <ViewerLocalFeedback
              tone="warning"
              title="清除视频缓存？"
              action={
                <div className="video-cache-setting__confirmation-actions">
                  <ViewerButton
                    tone="secondary"
                    disabled={clearingCache}
                    onClick={() => setConfirmingCacheClear(false)}
                  >
                    取消
                  </ViewerButton>
                  <ViewerButton tone="danger" loading={clearingCache} onClick={clearVideoCache}>
                    确认清除视频缓存
                  </ViewerButton>
                </div>
              }
            >
              只会移除 Viewer 管理的视频缩略图缓存，不会中断当前画面。
            </ViewerLocalFeedback>
          )}
          {cacheStatus === 'cleared' && (
            <ViewerLocalFeedback tone="recovery" title="视频缓存已清除">
              缓存会在需要时自动重新生成。
            </ViewerLocalFeedback>
          )}
          {cacheStatus === 'failed' && (
            <ViewerLocalFeedback tone="danger" title="无法处理视频缓存">
              请稍后重试。
            </ViewerLocalFeedback>
          )}
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

function formatCacheBytes(bytes: number): string {
  const safeBytes = Number.isFinite(bytes) ? Math.max(0, bytes) : 0
  if (safeBytes === 0) return '0 B'
  if (safeBytes >= VIDEO_CACHE_BUDGET_BYTES) {
    return `${formatUnit(safeBytes / VIDEO_CACHE_BUDGET_BYTES)} GiB`
  }
  const mebibytes = safeBytes / 1_048_576
  if (mebibytes >= 1) return `${formatUnit(mebibytes)} MiB`
  return `${Math.round(safeBytes)} B`
}

function formatUnit(value: number): string {
  return Number.isInteger(value) ? String(value) : value.toFixed(1)
}
