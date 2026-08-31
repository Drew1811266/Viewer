import { useState } from 'react'
import type { ReviewUsageImportPreview, ReviewWorkspaceError } from '../../api/reviewWorkspaceTypes'
import ModalSheet from '../ModalSheet'
import ViewerButton from '../ui/ViewerButton'

export interface ReviewUsageImportProps {
  onSelect(): Promise<ReviewUsageImportPreview | null>
  onConfirm(preview: ReviewUsageImportPreview): void | Promise<void>
  onCancel(): void
}

export default function ReviewUsageImport({
  onSelect,
  onConfirm,
  onCancel,
}: ReviewUsageImportProps) {
  const [preview, setPreview] = useState<ReviewUsageImportPreview | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  async function select() {
    setBusy(true)
    setError(null)
    try {
      const selected = await onSelect()
      if (selected !== null) setPreview(structuredClone(selected))
    } catch (cause) {
      setPreview(null)
      setError(usageImportErrorMessage(cause))
    } finally {
      setBusy(false)
    }
  }

  async function confirm() {
    if (preview === null) return
    setBusy(true)
    setError(null)
    try {
      await onConfirm(structuredClone(preview))
    } catch (cause) {
      setError(usageAdoptionErrorMessage(cause))
    } finally {
      setBusy(false)
    }
  }

  return (
    <ModalSheet
      title="导入返工依据声明"
      size="medium"
      onCancel={onCancel}
      footer={
        <>
          <ViewerButton disabled={busy} onClick={onCancel}>
            不使用声明
          </ViewerButton>
          <ViewerButton
            tone="primary"
            disabled={busy || preview === null}
            onClick={() => void confirm()}
          >
            采用声明
          </ViewerButton>
        </>
      }
    >
      <p>声明是可选依据，不证明意见已执行；不提供声明仍可手动存档。</p>
      <ViewerButton tone="secondary" disabled={busy} onClick={() => void select()}>
        {busy ? '正在核验…' : '选择声明文件'}
      </ViewerButton>
      {error !== null && (
        <p className="review-usage-import__error" role="alert">
          {error}
        </p>
      )}
      {preview !== null && (
        <section className="review-usage-import__preview" aria-label="已核验声明">
          <strong>{preview.source}</strong>
          <span>依据快照 {preview.declaration.basis.snapshotId}</span>
          <span>声明范围 {preview.declaration.targets.length} 项</span>
          {preview.outputs.length > 0 && <span>产出候选 {preview.outputs.length} 项</span>}
        </section>
      )}
    </ModalSheet>
  )
}

function usageAdoptionErrorMessage(cause: unknown): string {
  const code = reviewWorkspaceErrorCode(cause)
  if (code === 'read_only') return '当前项目只读，声明未采用'
  if (code === 'source_changed') return '声明在采用前发生变化，请重新选择'
  if (code === 'wrong_context') return '声明不属于当前项目'
  return '声明未采用，请稍后重试'
}

function usageImportErrorMessage(cause: unknown): string {
  const code = reviewWorkspaceErrorCode(cause)
  if (code === 'wrong_context') return '声明不属于当前项目'
  if (code === 'unsafe_source') return '声明必须位于项目内且不能放在 .viewer 中'
  if (code === 'unsupported_protocol') return '声明依据的评审协议暂不支持'
  if (code === 'usage_invalid') return '声明内容无效或无法核验'
  if (code === 'limit_exceeded') return '声明超过安全大小限制'
  if (code === 'source_changed') return '声明在核验期间发生变化，请重新选择'
  return '无法核验声明，请检查文件后重试'
}

function reviewWorkspaceErrorCode(cause: unknown): ReviewWorkspaceError['code'] | undefined {
  return typeof cause === 'object' && cause !== null && 'code' in cause
    ? (cause as Partial<ReviewWorkspaceError>).code
    : undefined
}
