export function PlanEmpty({ onRetry }: { onRetry: () => void }) {
  return (
    <section className="plan-state" data-testid="plan-empty">
      <span className="plan-state__icon" aria-hidden="true">
        ⌘
      </span>
      <h2 className="plan-state__title">项目里还没有生成计划</h2>
      <p className="plan-state__body">Agent 写完会自动出现，通常 2 秒内。</p>
      <div className="plan-state__slot" aria-hidden="true">
        <span className="plan-state__slot-hint">等待第一次 plan_write</span>
      </div>
      <code className="plan-state__code">
        .viewer/plans/incoming/&lt;planId&gt;/&lt;revision&gt;.json
      </code>
      <button type="button" className="plan-state__action" onClick={onRetry}>
        重新读取
      </button>
    </section>
  )
}

export function PlanLoading() {
  return (
    <section className="plan-state" data-testid="plan-loading" aria-busy="true">
      <h2 className="plan-state__title">正在读取计划…</h2>
      <p className="plan-state__body">首次打开读一遍，之后每 2 秒检查一次。</p>
      <div className="plan-state__skeleton" aria-hidden="true">
        <span />
        <span />
        <span />
      </div>
    </section>
  )
}

export function PlanError({
  message,
  warnings,
  onRetry,
}: {
  message: string
  warnings: { path: string; code: string; message: string }[]
  onRetry: () => void
}) {
  return (
    <section className="plan-state" data-testid="plan-error" role="alert">
      <h2 className="plan-state__title">这个计划读不了</h2>
      <p className="plan-state__body">{message}</p>
      {warnings.length > 0 ? (
        <ul className="plan-state__warnings">
          {warnings.slice(0, 4).map((warning) => (
            <li key={`${warning.code}-${warning.path}`}>
              <code>{warning.path}</code>
              <span className="plan-node__chip plan-node__chip--warn">{warning.code}</span>
              <span className="plan-state__body">{warning.message}</span>
            </li>
          ))}
        </ul>
      ) : null}
      <button type="button" className="plan-state__action" onClick={onRetry}>
        重新读取
      </button>
    </section>
  )
}
