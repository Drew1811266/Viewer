import { useMemo, useState } from 'react'
import type { PlanBridge } from '../../api/plan'
import { PlanCanvas } from './PlanCanvas'
import { stalePanelCopy, usePlanWorkspace } from './usePlanWorkspace'

export interface PlanWorkspaceProps {
  bridge: PlanBridge | null
  projectRoot: string | null
}

export function PlanWorkspace({ bridge, projectRoot }: PlanWorkspaceProps) {
  const plan = usePlanWorkspace(bridge, projectRoot)
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
            </>
          ) : (
            <p className="plan-drawer__hint">选中一个分镜看拼装结果</p>
          )}
        </aside>
      </div>
    </section>
  )
}
