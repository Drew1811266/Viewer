import { useCallback, useEffect, useMemo, useState } from 'react'
import type {
  BatchImageExecution,
  BatchImagePlanRow,
  BatchImageSettingsRequest,
} from '../api/types'
import { safeUserMessage } from '../api/viewer'
import ViewerButton from './ui/ViewerButton'
import ViewerDialog from './ui/ViewerDialog'
import ViewerField from './ui/ViewerField'
import ViewerLocalFeedback from './ui/ViewerLocalFeedback'

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
  const [resizeEdge, setResizeEdge] = useState('')
  const [suffix, setSuffix] = useState('-converted')
  const [subfolder, setSubfolder] = useState('converted')
  const [useSubfolder, setUseSubfolder] = useState(false)
  const [conflict, setConflict] = useState<'skip' | 'suffix' | 'overwrite'>('suffix')
  const [plan, setPlan] = useState<BatchImagePlanRow[] | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [executing, setExecuting] = useState(false)
  const [execution, setExecution] = useState<BatchImageExecution | null>(null)

  const settings = useMemo<BatchImageSettingsRequest>(
    () => ({
      format,
      qualityMilli: Math.round(qualityPercent * 10),
      resize:
        resizeEdge.trim() === ''
          ? { kind: 'none' }
          : { kind: 'longest_edge', edge: Number(resizeEdge) },
      output: useSubfolder ? { kind: 'subfolder', name: subfolder } : { kind: 'sibling', suffix },
      conflict,
    }),
    [conflict, format, qualityPercent, resizeEdge, subfolder, suffix, useSubfolder],
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
    refreshPlan()
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
      size="medium"
      footer={
        execution ? (
          <ViewerButton tone="primary" onClick={onCancel}>
            完成
          </ViewerButton>
        ) : (
          <>
            <ViewerButton tone="quiet" onClick={onCancel} disabled={executing}>
              取消
            </ViewerButton>
            <ViewerButton
              tone="primary"
              onClick={confirm}
              disabled={busy || executing || plan === null || convertCount === 0}
            >
              {executing ? '处理中…' : `转换 ${convertCount} 个文件`}
            </ViewerButton>
          </>
        )
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
            <ViewerField label="目标格式" inline>
              <select
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
            <ViewerField label="质量（有损格式）" inline>
              <div className="batch-image-quality">
                <input
                  type="range"
                  min={10}
                  max={100}
                  step={5}
                  value={qualityPercent}
                  aria-label="压缩质量"
                  onChange={(event) => setQualityPercent(Number(event.currentTarget.value))}
                />
                <span>{qualityPercent}%</span>
              </div>
            </ViewerField>
            <ViewerField label="最长边上限（像素，留空不缩放）" inline>
              <input
                type="number"
                min={1}
                value={resizeEdge}
                aria-label="最长边像素"
                placeholder="例如 2048"
                onChange={(event) => setResizeEdge(event.currentTarget.value)}
              />
            </ViewerField>
            <ViewerField label="输出位置" inline>
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
            <ViewerField label="目标已存在时" inline>
              <select
                aria-label="冲突策略"
                value={conflict}
                onChange={(event) => setConflict(event.currentTarget.value as typeof conflict)}
              >
                <option value="skip">跳过</option>
                <option value="suffix">追加序号</option>
                <option value="overwrite">覆盖（原件进废纸篓）</option>
              </select>
            </ViewerField>
          </div>
          {error && (
            <ViewerLocalFeedback tone="danger" title="无法预览批量处理">
              {error}
            </ViewerLocalFeedback>
          )}
          {plan && (
            <div className="batch-image-plan" data-testid="batch-image-plan">
              <p>
                将转换 {convertCount} 个文件；跳过已存在 {skipCount} 个。
              </p>
              <ul>
                {plan.slice(0, 8).map((row) => (
                  <li key={row.entityId}>
                    {row.relativePath} → {destinationLabel(row.disposition)}
                  </li>
                ))}
                {plan.length > 8 && <li>…共 {plan.length} 项</li>}
              </ul>
            </div>
          )}
        </>
      )}
    </ViewerDialog>
  )
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
