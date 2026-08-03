import { useEffect, useRef, useState } from 'react'
import ViewerButton, { ViewerIconButton } from './ui/ViewerButton'
import ViewerStatusTag from './ui/ViewerStatusTag'
import ViewerTaskSurface from './ui/ViewerTaskSurface'

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
  const [surfaceExpanded, setSurfaceExpanded] = useState(false)
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
      if (!cleanTaskIds.has(id) || hiddenTaskIds.has(id) || delay !== successDismissMs) {
        window.clearTimeout(timer)
        dismissTimers.current.delete(id)
      }
    })
    candidates
      .filter((candidate) => cleanTaskIds.has(candidate.id) && !hiddenTaskIds.has(candidate.id))
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
      dismissTimers.current.forEach(({ timer }) => {
        window.clearTimeout(timer)
      })
      dismissTimers.current.clear()
    },
    [],
  )

  if (visibleTasks.length === 0) return null

  const primaryTask =
    visibleTasks.find((candidate) => candidate.status === 'failed' || candidate.failed > 0) ??
    visibleTasks.find((candidate) => candidate.status === 'running') ??
    visibleTasks.find((candidate) => candidate.hasResults) ??
    visibleTasks[0]
  if (primaryTask === undefined) return null
  const aggregateRequested = visibleTasks.reduce((total, current) => total + current.requested, 0)
  const aggregateFinished = visibleTasks.reduce(
    (total, current) => total + finishedCount(current),
    0,
  )
  const summaryLabel = visibleTasks.length === 1 ? primaryTask.label : liveTaskSummary(visibleTasks)
  const summaryToggleLabel =
    visibleTasks.length === 1
      ? surfaceExpanded
        ? '收起任务详情'
        : '展开任务详情'
      : surfaceExpanded
        ? '收起后台任务'
        : '展开后台任务'

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
      <div className="task-surface">
        <div className="task-surface-summary">
          <ViewerIconButton
            icon={surfaceExpanded ? 'chevron-down' : 'chevron-right'}
            className="task-summary-toggle"
            label={summaryToggleLabel}
            tone="quiet"
            aria-expanded={surfaceExpanded}
            onClick={() => setSurfaceExpanded((current) => !current)}
          />
          <ViewerTaskSurface
            label={visibleTasks.length === 1 ? `${primaryTask.label}进度` : '后台任务总进度'}
            current={aggregateFinished}
            total={aggregateRequested}
            indeterminate={aggregateRequested === 0 && primaryTask.status === 'running'}
          >
            {summaryLabel}
          </ViewerTaskSurface>
          {visibleTasks.length === 1 && (
            <TaskActions
              task={primaryTask}
              onCancel={onCancel}
              onDismiss={onDismiss}
              onShowResults={onShowResults}
            />
          )}
          <TaskOutcome task={primaryTask} />
        </div>
        {surfaceExpanded && (
          <div className="task-list">
            {visibleTasks.map((currentTask) => (
              <div className="task-row" key={currentTask.id}>
                <ViewerTaskSurface
                  label={`${currentTask.label}进度`}
                  current={finishedCount(currentTask)}
                  total={currentTask.requested}
                  indeterminate={currentTask.requested === 0 && currentTask.status === 'running'}
                >
                  {currentTask.label}
                </ViewerTaskSurface>
                <TaskActions
                  task={currentTask}
                  onCancel={onCancel}
                  onDismiss={onDismiss}
                  onShowResults={onShowResults}
                />
                <TaskOutcome task={currentTask} />
                <div className="task-details">
                  {currentTask.failures.length === 0 ? (
                    <p>没有失败项目。</p>
                  ) : (
                    <ul>
                      {currentTask.failures.map((failure) => (
                        <li key={`${failure.item}:${failure.code}`}>
                          <span>{failure.item}</span> <span>{failure.code}</span>
                        </li>
                      ))}
                    </ul>
                  )}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </aside>
  )
}

function TaskActions({
  task,
  onCancel,
  onDismiss,
  onShowResults,
}: {
  task: TaskFeedback
  onCancel?: (taskId: string) => void
  onDismiss?: (taskId: string) => void
  onShowResults?: (taskId: string) => void
}) {
  return (
    <div className="task-row-actions">
      {task.cancellable && task.status === 'running' && onCancel && (
        <ViewerButton tone="quiet" onClick={() => onCancel(task.id)}>
          取消任务
        </ViewerButton>
      )}
      {task.status !== 'running' && !isCleanSuccess(task) && onDismiss && (
        <ViewerButton tone="quiet" onClick={() => onDismiss(task.id)}>
          关闭任务
        </ViewerButton>
      )}
      {task.hasResults && onShowResults && (
        <ViewerButton
          tone="secondary"
          aria-label={`查看${task.label}结果`}
          onClick={() => onShowResults(task.id)}
        >
          查看结果
        </ViewerButton>
      )}
    </div>
  )
}

function TaskOutcome({ task }: { task: TaskFeedback }) {
  if (task.failed === 0 && (task.skipped ?? 0) === 0 && (task.cancelled ?? 0) === 0) {
    return null
  }
  return (
    <p className="task-row-outcome">
      {task.failed > 0 && <ViewerStatusTag tone="danger">{task.failed} 项失败</ViewerStatusTag>}
      {(task.skipped ?? 0) > 0 && (
        <ViewerStatusTag tone="warning">{task.skipped} 项跳过</ViewerStatusTag>
      )}
      {(task.cancelled ?? 0) > 0 && (
        <ViewerStatusTag tone="neutral">{task.cancelled} 项取消</ViewerStatusTag>
      )}
    </p>
  )
}

function finishedCount(task: TaskFeedback): number {
  return Math.min(
    task.requested,
    task.completed + task.failed + (task.skipped ?? 0) + (task.cancelled ?? 0),
  )
}

function isCleanSuccess(task: TaskFeedback): boolean {
  return (
    task.status === 'complete' &&
    task.failed === 0 &&
    (task.cancelled ?? 0) === 0 &&
    !task.hasResults
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
