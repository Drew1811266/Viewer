import type { GlobalNotice } from '../../components/GlobalNoticeStack'
import type { TaskFeedback } from '../../components/TaskBar'
import type { ViewerState } from '../../state/viewerState'

export interface GlobalNoticeMessages {
  projectError: string | null
  finderDragMessage: string | null
  workspaceActionError: string | null
}

export function scanTaskFromState(scan: ViewerState['scan']): TaskFeedback | null {
  if (scan === null) return null
  const published = scan.publishedFolders + scan.publishedFiles
  const failed = scan.totals?.failed ?? scan.failedItems.length
  const requested = scan.totals
    ? scan.totals.folders + scan.totals.files + failed
    : published + failed + 1

  return {
    id: scan.taskId,
    label: '扫描项目',
    status:
      scan.phase === 'running'
        ? 'running'
        : scan.phase === 'cancelled'
          ? 'cancelled'
          : failed > 0
            ? 'failed'
            : 'complete',
    requested,
    completed: scan.totals ? scan.totals.folders + scan.totals.files : published,
    failed,
    cancellable: scan.phase === 'running',
    failures: scan.failedItems.map((failure) => ({
      item: failure.relativePath,
      code: failure.code,
    })),
  }
}

export function operationTaskFromState(operation: ViewerState['operation']): TaskFeedback | null {
  const progress = operation.active
  if (progress === null) return null
  const running = progress.lifecycle !== 'completed'
  const failures =
    operation.results?.items
      .filter((item) => item.status === 'failed')
      .map((item) => ({ item: item.relativePath, code: item.code })) ?? []

  return {
    id: progress.batchId,
    label: operationLabel(operation.kind),
    status: running
      ? 'running'
      : progress.failed > 0
        ? 'failed'
        : progress.cancelled === progress.requested && progress.requested > 0
          ? 'cancelled'
          : 'complete',
    requested: progress.requested,
    completed: progress.completed,
    failed: progress.failed,
    skipped: progress.skipped,
    cancelled: progress.cancelled,
    cancellable: progress.lifecycle === 'queued' || progress.lifecycle === 'running',
    failures,
    hasResults: operation.results !== null,
  }
}

export function visibleTaskFeedback(
  candidates: readonly (TaskFeedback | null)[],
  dismissedTasks: ReadonlySet<string>,
): TaskFeedback[] {
  const tasks = candidates.filter((task): task is TaskFeedback => task !== null)
  const running = tasks.some((task) => task.status === 'running')

  return tasks.filter(
    (task) =>
      (!dismissedTasks.has(task.id) || task.status === 'running') &&
      !(running && task.status === 'complete' && !task.hasResults),
  )
}

export function buildGlobalNotices({
  projectError,
  finderDragMessage,
  workspaceActionError,
}: GlobalNoticeMessages): GlobalNotice[] {
  const notices: GlobalNotice[] = []
  if (projectError) {
    notices.push({
      id: `application:${projectError}`,
      title: 'Viewer 出现问题',
      message: projectError,
      tone: 'danger',
    })
  }
  if (finderDragMessage) {
    notices.push({
      id: `finder-drag:${finderDragMessage}`,
      title: '无法拖出文件',
      message: finderDragMessage,
      tone: 'danger',
    })
  }
  if (workspaceActionError) {
    notices.push({
      id: `workspace:${workspaceActionError}`,
      title: '无法在文件管理器中显示项目',
      message: workspaceActionError,
      tone: 'danger',
    })
  }
  return notices
}

function operationLabel(kind: ViewerState['operation']['kind']): string {
  if (kind === 'rename') return '重命名文件'
  if (kind === 'copy') return '复制文件'
  if (kind === 'move') return '移动文件'
  if (kind === 'trash') return '移到废纸篓'
  return '文件操作'
}
