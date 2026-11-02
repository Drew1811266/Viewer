import { type CSSProperties, useCallback, useEffect, useMemo, useRef, useState } from 'react'
import type {
  BatchImageExecution,
  BatchImagePlanRow,
  BatchImageSettingsRequest,
} from '../api/types'
import { safeUserMessage } from '../api/viewer'
import ViewerButton from './ui/ViewerButton'
import ViewerChoiceChip from './ui/ViewerChoiceChip'
import ViewerDialog from './ui/ViewerDialog'
import ViewerField from './ui/ViewerField'
import ViewerLocalFeedback from './ui/ViewerLocalFeedback'
import ViewerSegmentedControl from './ui/ViewerSegmentedControl'

export interface BatchImageDialogProps {
  entityIds: readonly string[]
  busy: boolean
  previewBatchImage(
    entityIds: readonly string[],
    settings: BatchImageSettingsRequest,
  ): Promise<BatchImagePlanRow[]>
  executeBatchImage(
    entityIds: readonly string[],
    settings: BatchImageSettingsRequest,
  ): Promise<BatchImageExecution>
  onCancel(): void
}

const FORMATS = [
  { value: 'jpeg', label: 'JPEG' },
  { value: 'png', label: 'PNG' },
  { value: 'heic', label: 'HEIC' },
  { value: 'tiff', label: 'TIFF' },
  { value: 'webp', label: 'WebP' },
] as const

export default function BatchImageDialog({
  entityIds,
  busy,
  previewBatchImage,
  executeBatchImage,
  onCancel,
}: BatchImageDialogProps) {
  const [format, setFormat] = useState<(typeof FORMATS)[number]['value']>('jpeg')
  const [qualityPercent, setQualityPercent] = useState(85)
  const [resizeMode, setResizeMode] = useState<'none' | 'longest_edge' | 'percent' | 'exact'>(
    'none',
  )
  const [resizeEdge, setResizeEdge] = useState('2048')
  const [resizePercent, setResizePercent] = useState('50')
  const [resizeWidth, setResizeWidth] = useState('1920')
  const [resizeHeight, setResizeHeight] = useState('1080')
  const [suffix, setSuffix] = useState('-converted')
  const [subfolder, setSubfolder] = useState('converted')
  const [useSubfolder, setUseSubfolder] = useState(false)
  const [conflict, setConflict] = useState<'skip' | 'suffix' | 'overwrite'>('suffix')
  const [plan, setPlan] = useState<BatchImagePlanRow[] | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [executing, setExecuting] = useState(false)
  const [execution, setExecution] = useState<BatchImageExecution | null>(null)
  const formatRef = useRef<HTMLSelectElement>(null)

  const settings = useMemo<BatchImageSettingsRequest>(
    () => ({
      format,
      qualityMilli: Math.round(qualityPercent * 10),
      resize:
        resizeMode === 'none'
          ? { kind: 'none' }
          : resizeMode === 'longest_edge'
            ? { kind: 'longest_edge', edge: Number(resizeEdge) }
            : resizeMode === 'percent'
              ? { kind: 'percent', percent: Number(resizePercent) }
              : { kind: 'exact', width: Number(resizeWidth), height: Number(resizeHeight) },
      output: useSubfolder ? { kind: 'subfolder', name: subfolder } : { kind: 'sibling', suffix },
      conflict,
    }),
    [
      conflict,
      format,
      qualityPercent,
      resizeEdge,
      resizeHeight,
      resizeMode,
      resizePercent,
      resizeWidth,
      subfolder,
      suffix,
      useSubfolder,
    ],
  )

  const refreshPlan = useCallback(() => {
    setExecution(null)
    previewBatchImage(entityIds, settings)
      .then(setPlan)
      .catch((caught) => {
        setPlan(null)
        setError(safeUserMessage(caught))
      })
  }, [entityIds, previewBatchImage, settings])

  useEffect(() => {
    const timer = window.setTimeout(refreshPlan, 200)
    return () => window.clearTimeout(timer)
  }, [refreshPlan])

  useEffect(() => {
    setError(null)
  }, [settings])

  const convertCount =
    plan?.filter(
      (row) =>
        row.disposition.kind === 'convert' ||
        row.disposition.kind === 'convert_suffixed' ||
        row.disposition.kind === 'overwrite_existing',
    ).length ?? 0
  const skipCount = plan?.filter((row) => row.disposition.kind === 'skipped_existing').length ?? 0

  function confirm() {
    setExecuting(true)
    setError(null)
    executeBatchImage(entityIds, settings)
      .then((result) => {
        setExecution(result)
        setPlan(null)
      })
      .catch((caught) => setError(safeUserMessage(caught)))
      .finally(() => setExecuting(false))
  }

  return (
    <ViewerDialog
      title="批量处理图片"
      description={`已选择 ${entityIds.length} 个文件；转换产生新文件，不提供撤销。`}
      onCancel={onCancel}
      size="large"
      initialFocusRef={formatRef}
      footer={
        <div className="batch-image-footer">
          {execution ? (
            <span />
          ) : (
            <ViewerButton tone="secondary" onClick={onCancel} disabled={executing}>
              取消
            </ViewerButton>
          )}
          <ViewerButton
            tone="primary"
            onClick={execution ? onCancel : confirm}
            disabled={!execution && (busy || executing || plan === null || convertCount === 0)}
          >
            {execution ? '完成' : executing ? '处理中…' : `转换 ${convertCount} 个文件`}
          </ViewerButton>
        </div>
      }
    >
      {execution ? (
        <ViewerLocalFeedback tone="info" title="批量处理完成">
          成功 {execution.outcomes.filter((row) => row.outcome.kind === 'converted').length}{' '}
          个；跳过 {execution.outcomes.filter((row) => row.outcome.kind === 'skipped').length}{' '}
          个；失败 {execution.outcomes.filter((row) => row.outcome.kind === 'failed').length}{' '}
          个。新文件会随后出现在项目中。
        </ViewerLocalFeedback>
      ) : (
        <>
          <div className="batch-image-form">
            <ViewerField label="目标格式">
              <select
                ref={formatRef}
                aria-label="目标格式"
                value={format}
                onChange={(event) => setFormat(event.currentTarget.value as typeof format)}
              >
                {FORMATS.map((entry) => (
                  <option key={entry.value} value={entry.value}>
                    {entry.label}
                  </option>
                ))}
              </select>
            </ViewerField>
            <ViewerField label="质量（有损格式）">
              <div className="batch-image-quality">
                <input
                  type="range"
                  min={0}
                  max={100}
                  step={5}
                  value={qualityPercent}
                  aria-label="压缩质量"
                  style={{ '--batch-quality': `${qualityPercent}%` } as CSSProperties}
                  onChange={(event) => setQualityPercent(Number(event.currentTarget.value))}
                />
                <span>{qualityPercent}%</span>
              </div>
            </ViewerField>
            <ViewerField label="尺寸修改">
              <div className="batch-image-resize">
                <select
                  aria-label="尺寸修改方式"
                  value={resizeMode}
                  onChange={(event) =>
                    setResizeMode(
                      event.currentTarget.value as 'none' | 'longest_edge' | 'percent' | 'exact',
                    )
                  }
                >
                  <option value="none">不修改尺寸</option>
                  <option value="longest_edge">最长边</option>
                  <option value="percent">按百分比</option>
                  <option value="exact">精确像素</option>
                </select>
                {resizeMode === 'longest_edge' ? (
                  <input
                    type="number"
                    min={1}
                    value={resizeEdge}
                    aria-label="最长边像素"
                    placeholder="例如 2048"
                    onChange={(event) => setResizeEdge(event.currentTarget.value)}
                  />
                ) : null}
                {resizeMode === 'percent' ? (
                  <input
                    type="number"
                    min={1}
                    max={100}
                    value={resizePercent}
                    aria-label="缩放百分比"
                    onChange={(event) => setResizePercent(event.currentTarget.value)}
                  />
                ) : null}
                {resizeMode === 'exact' ? (
                  <>
                    <input
                      type="number"
                      min={1}
                      max={8192}
                      value={resizeWidth}
                      aria-label="输出宽度"
                      onChange={(event) => setResizeWidth(event.currentTarget.value)}
                    />
                    <span>×</span>
                    <input
                      type="number"
                      min={1}
                      max={8192}
                      value={resizeHeight}
                      aria-label="输出高度"
                      onChange={(event) => setResizeHeight(event.currentTarget.value)}
                    />
                  </>
                ) : null}
              </div>
            </ViewerField>
            <ViewerField label="输出位置">
              <div className="batch-image-output-choice">
                <label>
                  <input
                    type="radio"
                    checked={!useSubfolder}
                    onChange={() => setUseSubfolder(false)}
                  />
                  同目录后缀
                </label>
                <input
                  type="text"
                  value={suffix}
                  aria-label="文件名后缀"
                  disabled={useSubfolder}
                  onChange={(event) => setSuffix(event.currentTarget.value)}
                />
                <label>
                  <input
                    type="radio"
                    checked={useSubfolder}
                    onChange={() => setUseSubfolder(true)}
                  />
                  子文件夹
                </label>
                <input
                  type="text"
                  value={subfolder}
                  aria-label="子文件夹名称"
                  disabled={!useSubfolder}
                  onChange={(event) => setSubfolder(event.currentTarget.value)}
                />
              </div>
            </ViewerField>
            <ViewerField label="目标已存在时" className="batch-image-conflict-field">
              <ViewerSegmentedControl label="冲突策略" className="settings-segmented">
                {(
                  [
                    ['skip', '跳过'],
                    ['suffix', '追加序号'],
                    ['overwrite', '覆盖'],
                  ] as const
                ).map(([value, label]) => (
                  <ViewerChoiceChip
                    key={value}
                    type="radio"
                    name="batch-image-conflict"
                    checked={conflict === value}
                    onCheckedChange={(checked) => checked && setConflict(value)}
                  >
                    {label}
                  </ViewerChoiceChip>
                ))}
              </ViewerSegmentedControl>
            </ViewerField>
          </div>
          {error && (
            <ViewerLocalFeedback tone="danger" title="无法预览批量处理">
              {error}
            </ViewerLocalFeedback>
          )}
          {plan && (
            <div className="batch-image-plan" data-testid="batch-image-plan">
              <h3>转换预览 · {plan.length} 个文件</h3>
              <ul>
                {plan.map((row) => (
                  <li key={row.entityId}>
                    <span>{row.relativePath}</span>
                    <span className="batch-image-plan__arrow" aria-hidden="true">
                      →
                    </span>
                    <span>
                      {destinationLabel(row.disposition)}
                      {row.targetWidth > 0 && (
                        <small>
                          {row.targetWidth}×{row.targetHeight} · {formatBytes(row.sourceSizeBytes)}{' '}
                          → 约 {formatBytes(row.estimatedSizeBytes)}
                        </small>
                      )}
                    </span>
                  </li>
                ))}
              </ul>
              <p>
                预计转换 {convertCount} 个，跳过 {skipCount}{' '}
                个；输出始终为新文件，覆盖只作用于同名目标。
              </p>
            </div>
          )}
        </>
      )}
    </ViewerDialog>
  )
}

function formatBytes(bytes: number): string {
  if (bytes <= 0) return '—'
  const units = ['B', 'KB', 'MB', 'GB']
  let value = bytes
  let unit = 0
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024
    unit += 1
  }
  return `${value >= 10 || unit === 0 ? Math.round(value) : value.toFixed(1)} ${units[unit]}`
}

function destinationLabel(disposition: BatchImagePlanRow['disposition']): string {
  switch (disposition.kind) {
    case 'convert':
      return disposition.destination
    case 'convert_suffixed':
      return `${disposition.destination}（重名追加序号）`
    case 'overwrite_existing':
      return `${disposition.destination}（覆盖原件）`
    case 'skipped_existing':
      return '跳过（目标已存在）'
    case 'rejected':
      return '不支持'
  }
}
