import { useMemo, useState } from 'react'
import type { PlanBridge } from '../../api/plan'
import { PlanCanvas } from './PlanCanvas'
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

  const counts = plan.snapshot
    ? `${plan.snapshot.groups.length} 组 / ${plan.snapshot.shots.length} 张`
    : '0 组 / 0 张'

  return (
    <section className="plan-workspace" data-testid="plan-workspace">
      <header className="plan-workspace__bar">
        <label className="plan-workspace__picker">
          <span>计划</span>
          <select
            aria-label="选择计划"
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
        </label>
        <span className="plan-workspace__meta">
          {plan.snapshot ? `r${plan.snapshot.revision} · ${counts}` : counts}
        </span>
        <span className="plan-workspace__stale">{stalePanelCopy(plan.marks)}</span>
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
              <h2>{selectedShot.shotId}</h2>
              <pre className="plan-drawer__prompt">{selectedShot.prompt}</pre>
              <p className="plan-drawer__refs">参考图 {selectedShot.references.length} 张</p>
              <button
                type="button"
                className="plan-drawer__mark"
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
              <ul className="plan-drawer__marks">
                {plan.marks
                  .filter(
                    (entry) =>
                      entry.mark.target.kind === 'shot' &&
                      entry.mark.target.shotId === selectedShot.shotId,
                  )
                  .map((entry) => (
                    <li key={entry.mark.id} data-status={entry.status}>
                      <span>{statusLabel(entry.status)}</span>
                      <button
                        type="button"
                        aria-label={`删除标记 ${entry.mark.id}`}
                        onClick={() => void plan.removeMark(entry.mark.id)}
                      >
                        删除
                      </button>
                    </li>
                  ))}
              </ul>
            </>
          ) : (
            <p className="plan-drawer__hint">选中一个分镜看拼装结果</p>
          )}
        </aside>
      </div>
    </section>
  )
}
