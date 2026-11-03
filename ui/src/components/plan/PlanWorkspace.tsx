import { useMemo, useState } from 'react'
import type { PlanBridge } from '../../api/plan'
import { PlanCanvas } from './PlanCanvas'
import { PlanGroupView } from './PlanGroupView'
import { buildRedoList, type PlanFilter, type PlanSnapshot, redoListDocument } from './planGraph'
import { STATE_LABELS, statusLabel } from './planLabels'
import { stalePanelCopy, usePlanWorkspace } from './usePlanWorkspace'

const CATEGORY_LABELS: Record<string, string> = {
  wrong: '错误',
  missing: '缺失',
  extra: '多余',
  style: '风格',
  order: '顺序',
  other: '其它',
}

export interface PlanWorkspaceProps {
  bridge: PlanBridge | null
}

export function PlanWorkspace({ bridge }: PlanWorkspaceProps) {
  const plan = usePlanWorkspace(bridge)
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null)
  const [filter, setFilter] = useState<PlanFilter>('all')
  const [drillGroupId, setDrillGroupId] = useState<string | null>(null)
  const [category, setCategory] = useState('wrong')
  const [note, setNote] = useState('')

  const selectedShot = useMemo(() => {
    if (!plan.snapshot || !selectedNodeId?.startsWith('shot:')) return null
    const shotId = selectedNodeId.slice('shot:'.length)
    return plan.snapshot.shots.find((shot) => shot.shotId === shotId) ?? null
  }, [plan.snapshot, selectedNodeId])

  const shotMarks = useMemo(
    () =>
      selectedShot
        ? plan.marks.filter(
            (entry) =>
              entry.mark.target.kind === 'shot' && entry.mark.target.shotId === selectedShot.shotId,
          )
        : [],
    [plan.marks, selectedShot],
  )

  const counts = plan.snapshot
    ? `${plan.snapshot.groups.length} 组 · ${plan.snapshot.shots.length} 张`
    : '0 组 · 0 张'
  const mismatch =
    plan.snapshot?.warnings.filter((warning) => warning.code === 'count_mismatch') ?? []

  return (
    <section className="plan-workspace" data-testid="plan-workspace">
      <header className="plan-workspace__bar">
        <div className="plan-workspace__switcher">
          <span className="plan-workspace__switcher-icon" aria-hidden="true">
            ⌘
          </span>
          <div className="plan-workspace__switcher-body">
            <span className="plan-workspace__title">
              {plan.snapshot?.title || plan.selectedPlanId || '暂无计划'}
            </span>
            <span className="plan-workspace__subtitle">
              {plan.snapshot ? `${plan.selectedPlanId ?? ''} · r${plan.snapshot.revision}` : '—'}
            </span>
          </div>
          <select
            aria-label="选择计划"
            className="plan-workspace__picker"
            value={plan.selectedPlanId ?? ''}
            onChange={(event) => plan.selectPlan(event.target.value || null)}
          >
            {plan.plans.length === 0 ? <option value="">暂无计划</option> : null}
            {plan.plans.map((entry) => (
              <option key={entry.planId} value={entry.planId}>
                {entry.planId} · r{entry.revision}
              </option>
            ))}
          </select>
        </div>

        <div className="plan-workspace__stats">
          <span className="plan-workspace__counts">{counts}</span>
          <span className="plan-workspace__producer">{stalePanelCopy(plan.marks)}</span>
        </div>

        {mismatch.length > 0 ? (
          <span className="plan-node__chip plan-node__chip--warn">⚠ 声明与实际不符</span>
        ) : null}

        <span className="plan-workspace__spacer" />
        {(
          [
            ['all', '全部', plan.snapshot?.shots.length ?? 0],
            ['ungenerated', '待生成', ungeneratedCount(plan.snapshot)],
            ['stale', '计划已变', staleCount(plan.snapshot)],
          ] as const
        ).map(([value, label, count]) => (
          <button
            key={value}
            type="button"
            className="plan-filter"
            aria-pressed={filter === value}
            onClick={() => setFilter(value)}
          >
            {label} <span className="plan-filter__count">{count}</span>
          </button>
        ))}
      </header>

      {plan.snapshot?.diff && plan.snapshot.diff.affectedShots.length > 0 ? (
        <div className="plan-diff" data-testid="plan-diff">
          <span className="plan-diff__label">本版改动</span>
          <div className="plan-diff__card plan-diff__card--requirement">
            <span className="plan-diff__title">
              需求层 · {plan.snapshot.diff.requirement.length}
            </span>
            <span className="plan-diff__body">
              {plan.snapshot.diff.requirement[0]?.message ?? '需求没有变化'}
            </span>
          </div>
          <div className="plan-diff__card plan-diff__card--material">
            <span className="plan-diff__title">素材层 · {plan.snapshot.diff.material.length}</span>
            <span className="plan-diff__body">
              {plan.snapshot.diff.material[0]?.message ?? '素材没有变化'}
            </span>
          </div>
          <span className="plan-workspace__spacer" />
          <span className="plan-diff__count">
            受影响 {plan.snapshot.diff.affectedShots.length} 张
          </span>
        </div>
      ) : null}

      {plan.status === 'error' ? (
        <p className="plan-workspace__error" role="alert">
          {plan.error}
        </p>
      ) : null}

      <div className="plan-workspace__body">
        {plan.snapshot && drillGroupId ? (
          <PlanGroupView
            snapshot={plan.snapshot}
            groupId={drillGroupId}
            onBack={() => setDrillGroupId(null)}
            onSelectShot={(shotId) => {
              setSelectedNodeId(`shot:${shotId}`)
              setDrillGroupId(null)
            }}
          />
        ) : plan.snapshot ? (
          <PlanCanvas
            snapshot={plan.snapshot}
            filter={filter}
            selectedNodeId={selectedNodeId}
            onSelectNode={setSelectedNodeId}
            onDrillGroup={setDrillGroupId}
          />
        ) : (
          <p className="plan-workspace__empty">
            {plan.status === 'loading' ? '正在读取计划…' : '这个项目还没有生成计划'}
          </p>
        )}

        <aside className="plan-drawer" data-testid="plan-drawer">
          {selectedShot ? (
            <>
              <div className="plan-drawer__head">
                <span className="plan-drawer__id">{selectedShot.shotId}</span>
                <span
                  className={`plan-node__state plan-node__state--${selectedShot.generation ?? 'ungenerated'}`}
                >
                  {STATE_LABELS[selectedShot.generation ?? 'ungenerated']}
                </span>
              </div>
              <span className="plan-drawer__intent">{selectedShot.prompt.split('\n').at(-1)}</span>

              <span className="plan-drawer__section">拼装后的提示词</span>
              <div className="plan-prompt">
                {(selectedShot.segments ?? []).map((segment) => (
                  <div className="plan-segment" key={segment.path}>
                    <span className={`plan-segment__tag plan-segment__tag--${segment.kind}`}>
                      {segment.label}
                    </span>
                    <span className="plan-segment__text">{segment.text}</span>
                  </div>
                ))}
              </div>

              <span className="plan-drawer__section">
                参考图 · {selectedShot.references.length} 张
              </span>
              <div className="plan-refs">
                {selectedShot.references.map((reference) => (
                  <div className="plan-ref" key={reference}>
                    <img
                      className="plan-ref__img"
                      src={`plan-ref://localhost/${reference}`}
                      alt={`参考图 ${reference.slice(0, 6)}`}
                    />
                    <span className="plan-ref__row">
                      <span>{reference.slice(0, 6)}</span>
                    </span>
                  </div>
                ))}
              </div>

              <span className="plan-drawer__section">规格与模型</span>
              <div className="plan-kv">
                <span className="plan-kv__key">哈希</span>
                <span className="plan-kv__value">{selectedShot.hash.slice(0, 12)}</span>
              </div>

              <span className="plan-drawer__section">标记 · {shotMarks.length}</span>
              {shotMarks.map((entry) => (
                <div className="plan-mark" key={entry.mark.id} data-status={entry.status}>
                  <span className="plan-mark__dot" aria-hidden="true" />
                  <span className="plan-mark__body">
                    <span className="plan-mark__note">
                      {statusLabel(entry.status)} ·{' '}
                      {CATEGORY_LABELS[entry.mark.category] ?? entry.mark.category}
                    </span>
                    <span className="plan-mark__meta">{entry.mark.note ?? '没有备注'}</span>
                  </span>
                  <button
                    type="button"
                    aria-label={`删除标记 ${entry.mark.id}`}
                    onClick={() => void plan.removeMark(entry.mark.id)}
                  >
                    删除
                  </button>
                </div>
              ))}

              <form
                className="plan-mark-form"
                onSubmit={(event) => {
                  event.preventDefault()
                  if (!selectedShot) return
                  void plan.addMark({
                    target: { kind: 'shot', shotId: selectedShot.shotId },
                    category,
                    note: note.trim() || undefined,
                    observedHash: selectedShot.hash,
                  })
                  setNote('')
                }}
              >
                <span className="plan-drawer__section">对这张说一句</span>
                <div className="plan-mark-form__row">
                  {Object.entries(CATEGORY_LABELS).map(([value, label]) => (
                    <button
                      key={value}
                      type="button"
                      className="plan-filter"
                      aria-pressed={category === value}
                      onClick={() => setCategory(value)}
                    >
                      {label}
                    </button>
                  ))}
                </div>
                <input
                  className="plan-mark-form__input"
                  aria-label="标记备注"
                  placeholder="这张的光太硬了…"
                  value={note}
                  onChange={(event) => setNote(event.target.value)}
                />
                <div className="plan-mark-form__row">
                  <span className="plan-mark__meta">⌘ Enter 保存</span>
                  <span className="plan-workspace__spacer" />
                  <button type="submit" className="plan-mark-form__save">
                    保存标记
                  </button>
                </div>
              </form>
              <p className="plan-drawer__hint">标记写到 .viewer/plan-marks/，不改计划文件</p>
            </>
          ) : (
            <>
              <span className="plan-drawer__section">
                受影响 · {buildRedoList(plan.snapshot).length}
              </span>
              {buildRedoList(plan.snapshot).length === 0 ? (
                <p className="plan-drawer__hint">这一版没有受影响的分镜</p>
              ) : (
                buildRedoList(plan.snapshot).map((entry) => (
                  <button
                    type="button"
                    key={entry.shotId}
                    className="plan-affected"
                    onClick={() => setSelectedNodeId(`shot:${entry.shotId}`)}
                  >
                    <span className="plan-affected__dot" data-state={entry.state} />
                    <span className="plan-affected__body">
                      <span className="plan-affected__id">{entry.shotId}</span>
                      <span className="plan-affected__meta">
                        {entry.state === 'removed'
                          ? '已删除'
                          : entry.state === 'stale'
                            ? '计划已变'
                            : '未生成'}
                      </span>
                    </span>
                    <span className="plan-affected__chevron">›</span>
                  </button>
                ))
              )}
              <div className="plan-drawer__actions">
                <button
                  type="button"
                  className="plan-button"
                  disabled={!plan.snapshot}
                  onClick={() => downloadRedoList(plan.snapshot)}
                >
                  导出重做清单
                </button>
              </div>
              <p className="plan-drawer__hint">选中一个分镜看拼装结果</p>
            </>
          )}
        </aside>
      </div>
    </section>
  )
}

function downloadRedoList(snapshot: PlanSnapshot | null) {
  if (!snapshot) return
  const blob = new Blob([redoListDocument(snapshot)], { type: 'application/json' })
  const url = URL.createObjectURL(blob)
  const anchor = document.createElement('a')
  anchor.href = url
  anchor.download = `${snapshot.planId}-r${snapshot.revision}-redo.json`
  anchor.click()
  URL.revokeObjectURL(url)
}

function ungeneratedCount(snapshot: PlanSnapshot | null): number {
  return (
    snapshot?.shots.filter((shot) => (shot.generation ?? 'ungenerated') === 'ungenerated').length ??
    0
  )
}

function staleCount(snapshot: PlanSnapshot | null): number {
  return snapshot?.shots.filter((shot) => shot.generation === 'stale').length ?? 0
}
