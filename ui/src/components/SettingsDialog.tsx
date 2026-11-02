import { type CSSProperties, type RefObject, useEffect, useRef, useState } from 'react'
import type {
  MagnifierArea,
  MagnifierMagnification,
  MagnifierPreferences,
  MagnifierShape,
  SpeechInputMode,
  SpeechLocale,
  SpeechPreferences,
  SpeechStatus,
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
  DEFAULT_VIEWER_SETTINGS_UPDATE,
  MAGNIFIER_AREAS,
  MAGNIFIER_MAGNIFICATIONS,
  MAGNIFIER_SHAPES,
} from '../settings/viewerSettings'
import { useSpeechService } from '../speech/SpeechServiceContext'
import ModalSheet from './ModalSheet'
import ViewerButton from './ui/ViewerButton'
import ViewerChoiceChip from './ui/ViewerChoiceChip'
import ViewerIcon from './ui/ViewerIcon'
import ViewerLocalFeedback from './ui/ViewerLocalFeedback'
import ViewerSegmentedControl from './ui/ViewerSegmentedControl'

const SHAPE_LABEL = { circle: '圆形', rounded_rectangle: '圆角矩形' } as const
const AREA_LABEL = { small: '小', medium: '中', large: '大' } as const
const SPEECH_LOCALE_LABEL: Record<SpeechLocale, string> = {
  simplified_chinese: '简体中文（普通话）',
  english: 'English (United States)',
}
const SPEECH_MODEL_LABEL: Record<SpeechLocale, string> = {
  simplified_chinese: '简体中文',
  english: 'English (United States)',
}
const VIDEO_CACHE_BUDGET_BYTES = 1_073_741_824
type SettingsCategory = 'appearance' | 'speech'

interface SettingsDialogProps {
  bridge: SettingsPort
  density: ThumbnailDensity
  magnifier: MagnifierPreferences
  error: string | null
  onDensityChange: (density: ThumbnailDensity) => void
  onMagnifierShapeChange: (shape: MagnifierShape) => void
  onMagnifierMagnificationChange: (magnification: MagnifierMagnification) => void
  onMagnifierAreaChange: (area: MagnifierArea) => void
  speech?: SpeechPreferences
  onSpeechInputModeChange?: (mode: SpeechInputMode) => void
  onSpeechLocaleChange?: (locale: SpeechLocale) => void
  onSpeechAutoPunctuationChange?: (enabled: boolean) => void
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
  speech = DEFAULT_VIEWER_SETTINGS_UPDATE.speech,
  onSpeechInputModeChange,
  onSpeechLocaleChange,
  onSpeechAutoPunctuationChange,
  onClose,
  returnFocusRef,
}: SettingsDialogProps) {
  const thumbnailSizeRef = useRef<HTMLInputElement>(null)
  const [category, setCategory] = useState<SettingsCategory>('appearance')
  const [cacheStats, setCacheStats] = useState<VideoCacheStats | null>(null)
  const [cacheStatus, setCacheStatus] = useState<'loading' | 'ready' | 'cleared' | 'failed'>(
    'loading',
  )
  const [confirmingCacheClear, setConfirmingCacheClear] = useState(false)
  const [clearingCache, setClearingCache] = useState(false)
  const [speechStatus, setSpeechStatus] = useState<SpeechStatus | null>(null)
  const speechService = useSpeechService()
  const level = thumbnailLevelForDensity(density)
  const progress = ((level - 1) / (THUMBNAIL_LEVELS.length - 1)) * 100

  useEffect(() => {
    let open = true
    if (speechService === null) return
    void speechService.port
      .speechStatus({
        locale: speech.locale,
        autoPunctuation: speech.autoPunctuation,
      })
      .then(
        (status) => {
          if (open) setSpeechStatus(status)
        },
        () => {
          if (open) setSpeechStatus(null)
        },
      )
    return () => {
      open = false
    }
  }, [speech.autoPunctuation, speech.locale, speechService])

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
      showClose={false}
      onCancel={onClose}
      initialFocusRef={thumbnailSizeRef}
      returnFocusRef={returnFocusRef}
      footer={
        <>
          <span className="settings-dialog-footer-note">所有更改立即保存在本机。</span>
          <ViewerButton tone="secondary" onClick={onClose}>
            关闭
          </ViewerButton>
        </>
      }
    >
      <div className="settings-dialog-shell">
        <nav className="settings-dialog-navigation" aria-label="设置分类">
          <span className="settings-dialog-title">软件设置</span>
          <ViewerButton
            tone="quiet"
            active={category === 'appearance'}
            leadingIcon="eye"
            onClick={() => setCategory('appearance')}
          >
            显示与外观
          </ViewerButton>
          <ViewerButton
            tone="quiet"
            active={category === 'speech'}
            leadingIcon="mic"
            onClick={() => setCategory('speech')}
          >
            语音与转写
          </ViewerButton>
        </nav>
        <section className="settings-dialog-content">
          {category === 'appearance' ? (
            <>
              <header className="settings-dialog-section-header">
                <h3>显示与外观</h3>
                <p>调整缩略图与图片预览的显示方式。</p>
              </header>
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
              <section className="settings-dialog-group" aria-labelledby="settings-image-heading">
                <h4 id="settings-image-heading">图片预览</h4>
                <div className="settings-choice-row" role="group" aria-label="放大镜形状">
                  <span className="settings-choice-label" aria-hidden="true">
                    放大镜形状
                  </span>
                  <ViewerSegmentedControl label="放大镜形状选项" className="settings-segmented">
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
                  </ViewerSegmentedControl>
                </div>
                <div className="settings-choice-row" role="group" aria-label="放大倍数">
                  <span className="settings-choice-label" aria-hidden="true">
                    放大倍数
                  </span>
                  <ViewerSegmentedControl label="放大倍数选项" className="settings-segmented">
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
                  </ViewerSegmentedControl>
                </div>
                <div className="settings-choice-row" role="group" aria-label="显示面积">
                  <span className="settings-choice-label" aria-hidden="true">
                    显示面积
                  </span>
                  <ViewerSegmentedControl label="显示面积选项" className="settings-segmented">
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
                  </ViewerSegmentedControl>
                </div>
              </section>
              <section className="settings-dialog-group" aria-labelledby="settings-cache-heading">
                <h4 id="settings-cache-heading">视频缓存</h4>
                <div className="video-cache-setting" aria-label="视频缓存">
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
                </div>
                <p className="settings-dialog-cache-note">
                  只清理 Viewer 标记的派生缓存，不删除原视频。
                </p>
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
            </>
          ) : (
            <>
              <header className="settings-dialog-section-header">
                <h3>语音与转写</h3>
                <p>设置语音输入方式与离线识别。</p>
              </header>
              <fieldset className="settings-speech-input-mode">
                <legend>输入方式</legend>
                <ViewerSegmentedControl
                  label="语音输入方式"
                  className="settings-segmented settings-segmented--wide"
                >
                  {(['click', 'hold'] as const).map((mode) => (
                    <ViewerChoiceChip
                      key={mode}
                      type="radio"
                      name="speech-input-mode"
                      checked={speech.inputMode === mode}
                      onCheckedChange={(checked) => checked && onSpeechInputModeChange?.(mode)}
                    >
                      {mode === 'click' ? '点击开始' : '按住说话'}
                    </ViewerChoiceChip>
                  ))}
                </ViewerSegmentedControl>
              </fieldset>
              <section className="settings-dialog-group" aria-labelledby="settings-speech-heading">
                <h4 id="settings-speech-heading">语音识别</h4>
                <div className="settings-speech-row">
                  <span>识别模型</span>
                  <span className="settings-speech-row__value">
                    <span>
                      内置 · {SPEECH_MODEL_LABEL[speech.locale]}
                      {speechStatus?.onDevice ? '（离线）' : ''}
                    </span>
                    <span
                      className="settings-status-chip"
                      data-tone={
                        speechStatus?.available && speechStatus.onDevice ? 'ready' : 'warning'
                      }
                    >
                      {speechStatus === null
                        ? '检查中'
                        : speechStatus.available && speechStatus.onDevice
                          ? '已就绪'
                          : '不可用'}
                    </span>
                  </span>
                </div>
                <label className="settings-speech-row">
                  <span>识别语言</span>
                  <select
                    className="settings-speech-select"
                    aria-label="识别语言"
                    value={speech.locale}
                    onChange={(event) =>
                      onSpeechLocaleChange?.(event.currentTarget.value as SpeechLocale)
                    }
                  >
                    {(Object.keys(SPEECH_LOCALE_LABEL) as SpeechLocale[]).map((locale) => (
                      <option key={locale} value={locale}>
                        {SPEECH_LOCALE_LABEL[locale]}
                      </option>
                    ))}
                  </select>
                </label>
                <div className="settings-speech-row">
                  <span>自动标点</span>
                  <span className="settings-speech-row__value">
                    <span>{speech.autoPunctuation ? '自动添加' : '关闭'}</span>
                    <label className="settings-switch">
                      <input
                        type="checkbox"
                        aria-label="自动标点"
                        checked={speech.autoPunctuation}
                        onChange={(event) =>
                          onSpeechAutoPunctuationChange?.(event.currentTarget.checked)
                        }
                      />
                      <span aria-hidden="true" />
                    </label>
                  </span>
                </div>
              </section>
              <section className="settings-speech-privacy">
                <ViewerIcon name="lock" />
                <div>
                  <h4>语音只在本机处理</h4>
                  <p>录音与转写结果不会离开这台 Mac；原始音频只用于本次评审。</p>
                  <div className="settings-speech-chips">
                    <span>
                      <ViewerIcon name="check" />
                      离线可用
                    </span>
                    <span>
                      <ViewerIcon name="check" />
                      不上传音频
                    </span>
                  </div>
                </div>
              </section>
              <p className="settings-dialog-cache-note">
                语音输入使用 macOS 内置的普通话模型，无需下载额外模型。
              </p>
            </>
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
