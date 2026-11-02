import { useEffect, useRef, useState } from 'react'
import type { RenamePreview, RenameRules } from '../api/types'
import { defined } from '../defined'
import ModalSheet from './ModalSheet'
import ViewerButton from './ui/ViewerButton'
import ViewerChoiceChip from './ui/ViewerChoiceChip'
import ViewerField from './ui/ViewerField'
import ViewerLocalFeedback from './ui/ViewerLocalFeedback'
import VirtualList from './VirtualList'

interface BatchRenameDialogProps {
  entityIds: string[]
  busy: boolean
  requestPreview: (entityIds: string[], rules: RenameRules) => Promise<RenamePreview | null>
  onConfirm: (rules: RenameRules, preview: RenamePreview) => void
  onCancel: () => void
}

const initialRules: RenameRules = {
  find: '',
  replacement: '',
  prefix: '',
  suffix: '',
  sequence: null,
}

export default function BatchRenameDialog({
  entityIds,
  busy,
  requestPreview,
  onConfirm,
  onCancel,
}: BatchRenameDialogProps) {
  const [rules, setRules] = useState<RenameRules>(initialRules)
  const [preview, setPreview] = useState<RenamePreview | null>(null)
  const [loading, setLoading] = useState(false)
  const [message, setMessage] = useState<string | null>(null)
  const findRef = useRef<HTMLInputElement>(null)
  const errorRef = useRef<HTMLDivElement>(null)
  const revisionRef = useRef(0)
  const firstInvalidIndex = preview?.rows.findIndex((row) => row.errors.length > 0) ?? -1

  useEffect(() => {
    if (preview?.rows.some((row) => row.errors.length > 0)) errorRef.current?.focus()
  }, [preview])

  async function updatePreview() {
    const revision = ++revisionRef.current
    setLoading(true)
    setMessage(null)
    try {
      const result = await requestPreview(entityIds, rules)
      if (revision !== revisionRef.current) return
      setPreview(result)
      if (result === null) setMessage('无法生成重命名预览，请重试。')
    } catch {
      if (revision === revisionRef.current) setMessage('无法生成重命名预览，请重试。')
    } finally {
      if (revision === revisionRef.current) setLoading(false)
    }
  }

  function patchRules(patch: Partial<RenameRules>) {
    revisionRef.current += 1
    setRules((current) => ({ ...current, ...patch }))
    setPreview(null)
    setLoading(false)
  }

  return (
    <ModalSheet
      title={`批量重命名 ${entityIds.length} 项`}
      description={`已选择 ${entityIds.length} 个文件；执行前可以预览每个目标名称。`}
      size="large"
      onCancel={onCancel}
      initialFocusRef={findRef}
      footer={
        <div className="batch-rename-footer">
          <ViewerButton tone="secondary" disabled={busy} onClick={onCancel}>
            取消
          </ViewerButton>
          <ViewerButton
            tone="primary"
            loading={busy}
            disabled={loading || preview?.executable !== true}
            title={
              preview?.executable === true ? '执行批量重命名' : '请先生成一份可执行的重命名预览'
            }
            onClick={() => preview && onConfirm(rules, preview)}
          >
            执行批量重命名
          </ViewerButton>
        </div>
      }
    >
      <section className="batch-rename-rule-region">
        <div className="rename-rule-grid">
          <ViewerField label="查找">
            <input
              ref={findRef}
              placeholder="输入要查找的文字"
              value={rules.find}
              onChange={(event) => patchRules({ find: event.currentTarget.value })}
            />
          </ViewerField>
          <ViewerField label="替换为">
            <input
              placeholder="留空则删除"
              value={rules.replacement}
              onChange={(event) => patchRules({ replacement: event.currentTarget.value })}
            />
          </ViewerField>
        </div>
        <div className="sequence-controls">
          <ViewerField label="前缀" className="sequence-text-field">
            <input
              value={rules.prefix}
              onChange={(event) => patchRules({ prefix: event.currentTarget.value })}
            />
          </ViewerField>
          <ViewerField label="后缀" className="sequence-text-field">
            <input
              placeholder="可选"
              value={rules.suffix}
              onChange={(event) => patchRules({ suffix: event.currentTarget.value })}
            />
          </ViewerField>
          {rules.sequence && (
            <>
              <ViewerField label="起始值" className="sequence-number-field">
                <input
                  type="number"
                  min={0}
                  max={999999}
                  value={rules.sequence.start}
                  onChange={(event) =>
                    patchRules({
                      sequence: {
                        ...defined(
                          rules.sequence,
                          'Sequence start control requires sequence rules',
                        ),
                        start: Number(event.currentTarget.value),
                      },
                    })
                  }
                />
              </ViewerField>
              <ViewerField label="位数" className="sequence-number-field">
                <input
                  type="number"
                  min={1}
                  max={6}
                  value={rules.sequence.digits}
                  onChange={(event) =>
                    patchRules({
                      sequence: {
                        ...defined(
                          rules.sequence,
                          'Sequence digit control requires sequence rules',
                        ),
                        digits: Number(event.currentTarget.value),
                      },
                    })
                  }
                />
              </ViewerField>
            </>
          )}
          <ViewerChoiceChip
            checked={rules.sequence !== null}
            onCheckedChange={(checked) =>
              patchRules({ sequence: checked ? { start: 1, digits: 2 } : null })
            }
          >
            添加序号
          </ViewerChoiceChip>
          <ViewerButton
            tone="secondary"
            loading={loading}
            disabled={busy}
            onClick={() => void updatePreview()}
          >
            更新预览
          </ViewerButton>
        </div>
      </section>
      {message && (
        <ViewerLocalFeedback tone="danger" title="无法生成重命名预览">
          {message}
        </ViewerLocalFeedback>
      )}
      {preview && (
        <section className="rename-preview" aria-label="批量重命名完整预览">
          <h3>完整预览 · {preview.rows.length} 项</h3>
          <VirtualList
            items={preview.rows}
            rowHeight={48}
            height={Math.min(260, Math.max(48, preview.rows.length * 48))}
            className="rename-preview-list"
            scrollToIndex={firstInvalidIndex >= 0 ? firstInvalidIndex : undefined}
            getKey={(row) => row.entityId}
            renderItem={(row, index) => (
              <div
                ref={index === firstInvalidIndex ? errorRef : undefined}
                className="rename-preview-row"
                data-invalid={row.errors.length > 0 || undefined}
                tabIndex={row.errors.length > 0 ? -1 : undefined}
              >
                <span>{row.sourceRelativePath}</span>
                <span className="rename-preview-arrow" aria-hidden="true">
                  →
                </span>
                <span>{row.destinationRelativePath ?? row.proposedName}</span>
                {row.errors.map((error) => (
                  <code key={error}>{error}</code>
                ))}
              </div>
            )}
          />
          <p
            className="rename-preview-status"
            data-invalid={preview.rows.some((row) => row.errors.length > 0) || undefined}
          >
            {preview.rows.filter((row) => row.errors.length > 0).length}{' '}
            项无效；名称冲突会在执行前再次检查。
          </p>
        </section>
      )}
    </ModalSheet>
  )
}
