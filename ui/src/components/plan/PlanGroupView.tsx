import type { PlanSnapshot } from './planGraph'
import { STATE_LABELS } from './planLabels'

export interface PlanGroupViewProps {
  snapshot: PlanSnapshot
  groupId: string
  onBack: () => void
  onSelectShot: (shotId: string) => void
}

export function PlanGroupView({ snapshot, groupId, onBack, onSelectShot }: PlanGroupViewProps) {
  const group = snapshot.groups.find((candidate) => candidate.id === groupId)
  const shots = snapshot.shots.filter((shot) => shot.groupId === groupId)
  if (!group) return null

  const generated = shots.filter((shot) => shot.generation === 'current').length
  const mismatch = group.declaredCount !== group.actualCount

  return (
    <section className="plan-group" data-testid="plan-group">
      <header className="plan-group__bar">
        <button type="button" className="plan-group__back" onClick={onBack}>
          ‹ 返回画布
        </button>
        <span className="plan-group__breadcrumb">
          {snapshot.title || snapshot.planId} / {group.label}
        </span>
        <span className="plan-workspace__spacer" />
        <span className="plan-workspace__counts">
          {shots.length} 张 · 已生成 {generated}/{shots.length}
        </span>
        {mismatch ? (
          <span className="plan-node__chip plan-node__chip--warn">
            ⚠ 声明 {group.declaredCount}
          </span>
        ) : null}
      </header>

      <div className="plan-group__grid">
        {shots.map((shot) => {
          const state = shot.generation ?? 'ungenerated'
          return (
            <button
              type="button"
              key={shot.shotId}
              className={`plan-group__card plan-shot--${state}`}
              onClick={() => onSelectShot(shot.shotId)}
            >
              <span className="plan-group__preview" aria-hidden="true" />
              <span className="plan-group__id">{shot.shotId}</span>
              <span className="plan-group__intent">{shot.prompt.split('\n').at(-1)}</span>
              <span className={`plan-node__state plan-node__state--${state}`}>
                {STATE_LABELS[state]}
              </span>
            </button>
          )
        })}
      </div>
    </section>
  )
}
