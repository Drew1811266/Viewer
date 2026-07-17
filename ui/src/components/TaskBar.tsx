import { useEffect, useState } from 'react'

export interface TaskFeedback {
  id: string
  label: string
  status: 'running' | 'complete' | 'failed' | 'cancelled'
  requested: number
  completed: number
  failed: number
  cancellable: boolean
  failures: Array<{ item: string; code: string }>
}

interface TaskBarProps {
  task?: TaskFeedback | null
  tasks?: TaskFeedback[]
  onCancel?: (taskId: string) => void
  onDismiss?: (taskId: string) => void
}

export default function TaskBar({ task = null, tasks, onCancel, onDismiss }: TaskBarProps) {
  const visibleTasks = tasks ?? (task ? [task] : [])
  const [expandedTaskId, setExpandedTaskId] = useState<string | null>(null)

  useEffect(() => {
    const expanded = visibleTasks.find((candidate) => candidate.id === expandedTaskId)
    if (expanded?.status === 'complete' && expanded.failed === 0) setExpandedTaskId(null)
  }, [expandedTaskId, visibleTasks])

  if (visibleTasks.length === 0) return null

  return (
    <aside className="task-bar" aria-label="后台任务">
      {visibleTasks.map((currentTask) => {
        const expanded = expandedTaskId === currentTask.id
        const finished = Math.min(
          currentTask.requested,
          currentTask.completed + currentTask.failed,
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
