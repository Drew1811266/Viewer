import { useId, useMemo, useRef, useState } from 'react'
import type {
  ConflictPolicy,
  ConflictResolution,
  FileCommandItem,
  FileCommandPreflight,
  FolderTreeItem,
} from '../api/types'
import ModalSheet from './ModalSheet'
import ViewerButton from './ui/ViewerButton'
import ViewerChoiceChip from './ui/ViewerChoiceChip'
import ViewerField from './ui/ViewerField'
import ViewerIcon from './ui/ViewerIcon'
import ViewerLocalFeedback from './ui/ViewerLocalFeedback'
import ViewerStatusTag from './ui/ViewerStatusTag'

interface DestinationDialogProps {
  mode: 'copy' | 'move'
  entityIds: string[]
  folders: FolderTreeItem[]
  busy: boolean
  initialDestinationId?: string
  initialPreflight?: FileCommandPreflight
  requestPreflight: (items: FileCommandItem[]) => Promise<FileCommandPreflight | null>
  onConfirm: (items: FileCommandItem[], conflicts: ConflictResolution[]) => void
  onCancel: () => void
}

type DecisionMap = Record<string, ConflictPolicy | ''>

export default function DestinationDialog({
  mode,
  entityIds,
  folders,
  busy,
  initialDestinationId,
  initialPreflight,
  requestPreflight,
  onConfirm,
  onCancel,
}: DestinationDialogProps) {
  const [destinationId, setDestinationId] = useState<string | null>(initialDestinationId ?? null)
  const [preflight, setPreflight] = useState<FileCommandPreflight | null>(initialPreflight ?? null)
  const [decisions, setDecisions] = useState<DecisionMap>({})
  const [applyRemainingId, setApplyRemainingId] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)
  const [message, setMessage] = useState<string | null>(null)
  const firstFolderRef = useRef<HTMLInputElement>(null)
  const preflightRevisionRef = useRef(0)
  const controlLabelId = useId()
  const items = useMemo<FileCommandItem[]>(
    () =>
      destinationId === null
        ? []
        : entityIds.map((entityId) => ({
            entityId,
            action:
              mode === 'copy'
                ? { kind: 'copy', destinationFolderId: destinationId }
                : { kind: 'move', destinationFolderId: destinationId },
          })),
    [destinationId, entityIds, mode],
  )
  const conflictRows = preflight?.rows.filter((row) => row.state === 'conflict') ?? []
  const blockedRows = preflight?.rows.filter((row) => row.state === 'blocked') ?? []
  const decisionsComplete = conflictsAreResolved(
    conflictRows.map((row) => row.entityId),
    decisions,
    applyRemainingId,
  )
  const canExecute =
    preflight?.executable === true && blockedRows.length === 0 && decisionsComplete && !busy

  function chooseDestination(entityId: string) {
    preflightRevisionRef.current += 1
    setDestinationId(entityId)
    setPreflight(null)
    setDecisions({})
    setApplyRemainingId(null)
    setMessage(null)
    setLoading(false)
  }

  async function checkConflicts() {
    if (items.length === 0) return
    const revision = ++preflightRevisionRef.current
    setLoading(true)
    setMessage(null)
    try {
      const result = await requestPreflight(items)
      if (revision !== preflightRevisionRef.current) return
      setPreflight(result)
      if (result === null) setMessage('无法检查目标文件夹，请重试。')
    } catch {
      if (revision === preflightRevisionRef.current) {
        setMessage('无法检查目标文件夹，请重试。')
      }
    } finally {
      if (revision === preflightRevisionRef.current) setLoading(false)
    }
  }

  function submit() {
    if (!canExecute) return
    const conflicts: ConflictResolution[] = []
    for (const row of conflictRows) {
      const policy = decisions[row.entityId]
      if (!policy) continue
      const applyToRemaining = applyRemainingId === row.entityId
      conflicts.push({ entityId: row.entityId, policy, applyToRemaining })
      if (applyToRemaining) break
    }
    onConfirm(items, conflicts)
  }

  return (
    <ModalSheet
      title={mode === 'copy' ? '复制到' : '移动到'}
      description={`已选择 ${entityIds.length} 个文件；目标位置必须在当前项目内。`}
      size="large"
      onCancel={onCancel}
      initialFocusRef={firstFolderRef}
      footer={
        <div className="destination-dialog-footer">
          <ViewerButton tone="secondary" disabled={busy} onClick={onCancel}>
            取消
          </ViewerButton>
          <ViewerButton
            tone="primary"
            loading={busy}
            disabled={!canExecute}
            title={canExecute ? undefined : '请完成目标检查并处理所有冲突'}
            onClick={submit}
          >
            {mode === 'copy' ? `复制 ${entityIds.length} 项` : `移动 ${entityIds.length} 项`}
          </ViewerButton>
        </div>
      }
    >
      <div className="destination-dialog-layout">
        <fieldset className="destination-list">
          <legend className="visually-hidden">项目内文件夹</legend>
          {folders.map((folder, index) => (
            <label
              key={folder.entityId}
              style={{ paddingLeft: `${12 + folderDepth(folder, folders) * 14}px` }}
            >
              <input
                ref={index === 0 ? firstFolderRef : undefined}
                className="visually-hidden"
                type="radio"
                name="destination"
                checked={destinationId === folder.entityId}
                onChange={() => chooseDestination(folder.entityId)}
              />
              <ViewerIcon name="folder" />
              <span>{folder.name}</span>
              <small>{folder.relativePath}</small>
            </label>
          ))}
        </fieldset>
        <div className="destination-dialog-main">
          <div className="destination-dialog-check">
            <p>目标中已有同名文件时，会逐项要求选择跳过、保留两者或替换。</p>
            <ViewerButton
              tone="secondary"
              loading={loading}
              disabled={destinationId === null || busy}
              onClick={() => void checkConflicts()}
            >
              检查冲突
            </ViewerButton>
          </div>
          {message && (
            <ViewerLocalFeedback tone="danger" title="无法检查目标文件夹">
              {message}
            </ViewerLocalFeedback>
          )}
          {preflight && (
            <section className="conflict-list" aria-label="目标检查结果">
              <span id={`${controlLabelId}-policy`} className="visually-hidden">
                冲突处理
              </span>
              <span id={`${controlLabelId}-remaining`} className="visually-hidden">
                应用到剩余冲突
              </span>
              {conflictRows.length > 0 && <p>选择“替换”时，现有目标文件会移到 macOS 废纸篓。</p>}
              {preflight.rows.map((row, index) => {
                const pathLabelId = `${controlLabelId}-path-${index}`
                return (
                  <div key={row.entityId} className="conflict-row" data-state={row.state}>
                    <span id={pathLabelId}>{row.relativePath}</span>
                    {row.state === 'ready' && (
                      <ViewerStatusTag tone="success">可执行</ViewerStatusTag>
                    )}
                    {row.state === 'blocked' && (
                      <>
                        <ViewerStatusTag tone="danger">已阻止</ViewerStatusTag>
                        <code>{row.code ?? 'invalid_target'}</code>
                      </>
                    )}
                    {row.state === 'conflict' && (
                      <>
                        <ViewerStatusTag tone="warning">需要处理</ViewerStatusTag>
                        <ViewerField label="冲突处理" className="conflict-policy-field">
                          <select
                            aria-labelledby={`${controlLabelId}-policy ${pathLabelId}`}
                            value={decisions[row.entityId] ?? ''}
                            onChange={(event) => {
                              const policy = event.currentTarget.value as ConflictPolicy
                              setDecisions((current) => ({
                                ...current,
                                [row.entityId]: policy,
                              }))
                            }}
                          >
                            <option value="">请选择</option>
                            <option value="skip">跳过</option>
                            <option value="keep_both">两者都保留</option>
                            <option value="replace">替换现有文件</option>
                          </select>
                        </ViewerField>
                        <ViewerChoiceChip
                          aria-labelledby={`${controlLabelId}-remaining ${pathLabelId}`}
                          checked={applyRemainingId === row.entityId}
                          onCheckedChange={(checked) =>
                            setApplyRemainingId(checked ? row.entityId : null)
                          }
                        >
                          应用到剩余冲突
                        </ViewerChoiceChip>
                      </>
                    )}
                  </div>
                )
              })}
            </section>
          )}
        </div>
      </div>
    </ModalSheet>
  )
}

function conflictsAreResolved(
  entityIds: string[],
  decisions: DecisionMap,
  applyRemainingId: string | null,
): boolean {
  let carried = false
  for (const entityId of entityIds) {
    if (carried) continue
    if (!decisions[entityId]) return false
    if (applyRemainingId === entityId) carried = true
  }
  return true
}

function folderDepth(folder: FolderTreeItem, folders: FolderTreeItem[]): number {
  const byId = new Map(folders.map((item) => [item.entityId, item]))
  let current = folder
  let depth = 0
  const visited = new Set<string>()
  while (current.parentEntityId && !visited.has(current.entityId)) {
    visited.add(current.entityId)
    const parent = byId.get(current.parentEntityId)
    if (!parent) break
    depth += 1
    current = parent
  }
  return depth
}
