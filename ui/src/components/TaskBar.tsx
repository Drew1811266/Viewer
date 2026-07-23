import { useEffect, useRef, useState } from 'react'

export interface TaskFeedback {
  id: string
  label: string
  status: 'running' | 'complete' | 'failed' | 'cancelled'
  requested: number
  completed: number
  failed: number
  skipped?: number
  cancelled?: number
  cancellable: boolean
  failures: Array<{ item: string; code: string }>
  hasResults?: boolean
}

interface TaskBarProps {
  task?: TaskFeedback | null
  tasks?: TaskFeedback[]
  onCancel?: (taskId: string) => void
  onDismiss?: (taskId: string) => void
  onShowResults?: (taskId: string) => void
  successDismissMs?: number
}

export default function TaskBar({
  task = null,
  tasks,
  onCancel,
  onDismiss,
  onShowResults,
  successDismissMs = 2000,
}: TaskBarProps) {
  const candidates = tasks ?? (task ? [task] : [])
  const [hiddenTaskIds, setHiddenTaskIds] = useState<Set<string>>(() => new Set())
  const visibleTasks = candidates.filter((candidate) => !hiddenTaskIds.has(candidate.id))
  const [expandedTaskId, setExpandedTaskId] = useState<string | null>(null)
  const dismissTimers = useRef(new Map<string, { delay: number; timer: number }>())

  useEffect(() => {
    const mustReappear = new Set(
      candidates
        .filter(
          (candidate) =>
            candidate.status !== 'complete' ||
            candidate.failed > 0 ||
            (candidate.cancelled ?? 0) > 0 ||
            Boolean(candidate.hasResults),
        )
        .map((candidate) => candidate.id),
    )
    setHiddenTaskIds((current) => {
      const next = new Set([...current].filter((id) => !mustReappear.has(id)))
      return next.size === current.size ? current : next
    })
    const cleanTaskIds = new Set(
      candidates
        .filter(
          (candidate) =>
            candidate.status === 'complete' &&
            candidate.failed === 0 &&
            (candidate.cancelled ?? 0) === 0 &&
            !candidate.hasResults,
        )
        .map((candidate) => candidate.id),
    )
    dismissTimers.current.forEach(({ delay, timer }, id) => {
      if (
        !cleanTaskIds.has(id) ||
        hiddenTaskIds.has(id) ||
        delay !== successDismissMs
      ) {
        window.clearTimeout(timer)
        dismissTimers.current.delete(id)
      }
    })
    candidates
      .filter(
        (candidate) =>
          cleanTaskIds.has(candidate.id) && !hiddenTaskIds.has(candidate.id),
      )
      .forEach((candidate) => {
        if (dismissTimers.current.has(candidate.id)) return
        const timer = window.setTimeout(() => {
          dismissTimers.current.delete(candidate.id)
          setHiddenTaskIds((current) =>
            current.has(candidate.id) ? current : new Set([...current, candidate.id]),
          )
        }, successDismissMs)
        dismissTimers.current.set(candidate.id, { delay: successDismissMs, timer })
      })
  }, [candidates, hiddenTaskIds, successDismissMs])

  useEffect(
    () => () => {
      dismissTimers.current.forEach(({ timer }) => window.clearTimeout(timer))
      dismissTimers.current.clear()
    },
    [],
  )

  useEffect(() => {
    const expanded = visibleTasks.find((candidate) => candidate.id === expandedTaskId)
    if (expanded?.status === 'complete' && expanded.failed === 0) setExpandedTaskId(null)
  }, [expandedTaskId, visibleTasks])

  if (visibleTasks.length === 0) return null

  return (
    <aside className="task-bar" aria-label="后台任务">
      <p
        className="visually-hidden task-live-status"
        role="status"
        aria-label="后台任务状态"
        aria-live="polite"
        aria-atomic="true"
      >
        {liveTaskSummary(visibleTasks)}
      </p>
      {visibleTasks.map((currentTask) => {
        const expanded = expandedTaskId === currentTask.id
        const finished = Math.min(
          currentTask.requested,
          currentTask.completed +
            currentTask.failed +
            (currentTask.skipped ?? 0) +
            (currentTask.cancelled ?? 0),
        )
        const detailLabel =
          visibleTasks.length === 1
            ? expanded
              ? '收起任务详情'
              : '展开任务详情'
            : `${expanded ? '收起' : '展开'}${currentTask.label}详情`
        return (
          <div className="task-row" key={currentTask.id}>
            <button
              type="button"
              aria-label={detailLabel}
              onClick={() => setExpandedTaskId(expanded ? null : currentTask.id)}
            >
              {expanded ? '▾' : '▸'}
            </button>
            <strong>{currentTask.label}</strong>
            <span>
              {finished}/{currentTask.requested}
            </span>
            {currentTask.failed > 0 && <span>{currentTask.failed} 项失败</span>}
            {(currentTask.skipped ?? 0) > 0 && <span>{currentTask.skipped} 项跳过</span>}
            {(currentTask.cancelled ?? 0) > 0 && (
              <span>{currentTask.cancelled} 项取消</span>
            )}
            {currentTask.cancellable && currentTask.status === 'running' && onCancel && (
              <button type="button" onClick={() => onCancel(currentTask.id)}>
                取消任务
              </button>
            )}
            {currentTask.status !== 'running' && onDismiss && (
              <button type="button" onClick={() => onDismiss(currentTask.id)}>
                关闭任务
              </button>
            )}
            {currentTask.hasResults && onShowResults && (
              <button
                type="button"
                aria-label={`查看${currentTask.label}结果`}
                onClick={() => onShowResults(currentTask.id)}
              >
                查看结果
              </button>
            )}
            {expanded && (
              <div className="task-details">
                {currentTask.failures.length === 0 ? (
                  <p>没有失败项目。</p>
                ) : (
                  <ul>
                    {currentTask.failures.map((failure, index) => (
                      <li key={`${failure.item}:${failure.code}:${index}`}>
                        <span>{failure.item}</span> <span>{failure.code}</span>
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            )}
          </div>
        )
      })}
    </aside>
  )
}

function liveTaskSummary(tasks: TaskFeedback[]): string {
  const running = tasks.filter((task) => task.status === 'running').length
  if (running > 0) return `${running} 个任务进行中`
  const failed = tasks.filter((task) => task.status === 'failed' || task.failed > 0).length
  if (failed > 0) return `${failed} 个任务失败`
  const cancelled = tasks.filter((task) => task.status === 'cancelled').length
  if (cancelled > 0) return `${cancelled} 个任务已取消`
  return `${tasks.length} 个任务已完成`
}
