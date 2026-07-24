import { useEffect, useRef, useState } from 'react'
import type { RenamePreview, RenameRules } from '../api/types'
import { defined } from '../defined'
import ModalSheet from './ModalSheet'
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
      onCancel={onCancel}
      initialFocusRef={findRef}
    >
      <div className="rename-rule-grid">
        <label className="form-field">
          <span>查找</span>
          <input
            ref={findRef}
            value={rules.find}
            onChange={(event) => patchRules({ find: event.currentTarget.value })}
          />
        </label>
        <label className="form-field">
          <span>替换为</span>
          <input
            value={rules.replacement}
            onChange={(event) => patchRules({ replacement: event.currentTarget.value })}
          />
        </label>
        <label className="form-field">
          <span>前缀</span>
          <input
            value={rules.prefix}
            onChange={(event) => patchRules({ prefix: event.currentTarget.value })}
          />
        </label>
        <label className="form-field">
          <span>后缀</span>
          <input
            value={rules.suffix}
            onChange={(event) => patchRules({ suffix: event.currentTarget.value })}
          />
        </label>
      </div>
      <div className="sequence-controls">
        <label className="checkbox-field">
          <input
            type="checkbox"
            checked={rules.sequence !== null}
            onChange={(event) =>
              patchRules({
                sequence: event.currentTarget.checked ? { start: 1, digits: 2 } : null,
              })
            }
          />
          添加序号
        </label>
        {rules.sequence && (
          <>
            <label>
              起始值
              <input
                type="number"
                min={0}
                max={999999}
                value={rules.sequence.start}
                onChange={(event) =>
                  patchRules({
                    sequence: {
                      ...defined(rules.sequence, 'Sequence start control requires sequence rules'),
                      start: Number(event.currentTarget.value),
                    },
                  })
                }
              />
            </label>
            <label>
              位数
              <input
                type="number"
                min={1}
                max={6}
                value={rules.sequence.digits}
                onChange={(event) =>
                  patchRules({
                    sequence: {
                      ...defined(rules.sequence, 'Sequence digit control requires sequence rules'),
                      digits: Number(event.currentTarget.value),
                    },
                  })
                }
              />
            </label>
          </>
        )}
        <button type="button" disabled={loading || busy} onClick={() => void updatePreview()}>
          {loading ? '正在生成…' : '更新预览'}
        </button>
      </div>
      {message && <p role="alert">{message}</p>}
      {preview && (
        <section className="rename-preview" aria-label="批量重命名完整预览">
          <h3>完整预览：{preview.rows.length} 项</h3>
          <VirtualList
            items={preview.rows}
            rowHeight={52}
            height={260}
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
                <span>{row.destinationRelativePath ?? row.proposedName}</span>
                {row.errors.map((error) => (
                  <code key={error}>{error}</code>
                ))}
              </div>
            )}
          />
        </section>
      )}
      <div className="modal-actions">
        <button type="button" onClick={onCancel}>
          取消
        </button>
        <button
          type="button"
          disabled={busy || loading || preview?.executable !== true}
          onClick={() => preview && onConfirm(rules, preview)}
        >
          执行批量重命名
        </button>
      </div>
    </ModalSheet>
  )
}
