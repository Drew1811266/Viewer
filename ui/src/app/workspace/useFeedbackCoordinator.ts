import { useEffect, useState } from 'react'
import type { GlobalNotice } from '../../components/GlobalNoticeStack'
import type { TaskFeedback } from '../../components/TaskBar'
import type { ViewerState } from '../../state/viewerState'
import {
  buildGlobalNotices,
  type GlobalNoticeMessages,
  operationTaskFromState,
  scanTaskFromState,
  visibleTaskFeedback,
} from './feedbackModel'

export interface FeedbackCoordinatorOptions extends GlobalNoticeMessages {
  projectSessionId: string
  scan: ViewerState['scan']
  operation: ViewerState['operation']
}

export interface FeedbackCoordinator {
  visibleTasks: TaskFeedback[]
  globalNotices: GlobalNotice[]
  setThumbnailTask(task: TaskFeedback | null): void
  setTextTask(task: TaskFeedback | null): void
  dismissTask(taskId: string): void
}

export function useFeedbackCoordinator({
  projectSessionId,
  scan,
  operation,
  projectError,
  finderDragMessage,
  workspaceActionError,
}: FeedbackCoordinatorOptions): FeedbackCoordinator {
  const [thumbnailTask, setThumbnailTask] = useState<TaskFeedback | null>(null)
  const [textTask, setTextTask] = useState<TaskFeedback | null>(null)
  const [dismissedTasks, setDismissedTasks] = useState<Set<string>>(() => new Set())

  useEffect(() => {
    setThumbnailTask(null)
    setTextTask(null)
    setDismissedTasks(new Set())
  }, [projectSessionId])

  return {
    visibleTasks: visibleTaskFeedback(
      [scanTaskFromState(scan), thumbnailTask, textTask, operationTaskFromState(operation)],
      dismissedTasks,
    ),
    globalNotices: buildGlobalNotices({
      projectError,
      finderDragMessage,
      workspaceActionError,
    }),
    setThumbnailTask,
    setTextTask,
    dismissTask(taskId) {
      setDismissedTasks((current) => new Set([...current, taskId]))
    },
  }
}
