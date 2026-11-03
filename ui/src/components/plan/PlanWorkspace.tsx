import { useMemo, useState } from 'react'
import type { PlanBridge } from '../../api/plan'
import { PlanCanvas } from './PlanCanvas'
import type { PlanSnapshot } from './planGraph'
import { stalePanelCopy, usePlanWorkspace } from './usePlanWorkspace'

export interface PlanWorkspaceProps {
  bridge: PlanBridge | null
}

const STATUS_LABELS: Record<string, string> = {
  pending: '待处理',
  changed: '已改',
  replaced: '已换',
  orphaned: '已失效',
}

const STATE_CHIP: Record<string, string> = {
  current: '已生成 · 一致',
  stale: '计划已变',
  ungenerated: '未生成',
}

export function statusLabel(status: string): string {
  return STATUS_LABELS[status] ?? status
}

export function PlanWorkspace({ bridge }: PlanWorkspaceProps) {
  const plan = usePlanWorkspace(bridge)
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null)

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
            <span className="plan-workspace__title">{plan.selectedPlanId ?? '暂无计划'}</span>
            <span className="plan-workspace__subtitle">
              {plan.snapshot ? `r${plan.snapshot.revision}` : '—'}
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
        <button type="button" className="plan-filter" aria-pressed="true">
          全部 <span className="plan-filter__count">{plan.snapshot?.shots.length ?? 0}</span>
        </button>
        <button type="button" className="plan-filter" aria-pressed="false">
          待生成 <span className="plan-filter__count">{ungeneratedCount(plan.snapshot)}</span>
        </button>
        <button type="button" className="plan-filter" aria-pressed="false">
          计划已变 <span className="plan-filter__count">{staleCount(plan.snapshot)}</span>
        </button>
      </header>

      {plan.status === 'error' ? (
        <p className="plan-workspace__error" role="alert">
          {plan.error}
        </p>
      ) : null}

      <div className="plan-workspace__body">
        {plan.snapshot ? (
          <PlanCanvas
            snapshot={plan.snapshot}
            selectedNodeId={selectedNodeId}
            onSelectNode={setSelectedNodeId}
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
                  {STATE_CHIP[selectedShot.generation ?? 'ungenerated']}
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
                    <span className="plan-ref__img" aria-hidden="true" />
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
                      {statusLabel(entry.status)} · {entry.mark.category}
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

              <div className="plan-drawer__actions">
                <button
                  type="button"
                  className="plan-button"
                  onClick={() =>
                    void plan.addMark({
                      target: { kind: 'shot', shotId: selectedShot.shotId },
                      category: 'wrong',
                      observedHash: selectedShot.hash,
                    })
                  }
                >
                  标记这个分镜
                </button>
              </div>
              <p className="plan-drawer__hint">标记写到 .viewer/plan-marks/，不改计划文件</p>
            </>
          ) : (
            <p className="plan-drawer__hint">选中一个分镜看拼装结果</p>
          )}
        </aside>
      </div>
    </section>
  )
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
