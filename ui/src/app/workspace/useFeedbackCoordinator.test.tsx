import { act, renderHook } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { TaskFeedback } from '../../components/TaskBar'
import type { ViewerState } from '../../state/viewerState'
import { initialOperationState } from '../../state/viewerState'
import { useFeedbackCoordinator } from './useFeedbackCoordinator'

const complete: TaskFeedback = {
  id: 'thumbnail-1',
  label: '生成缩略图',
  status: 'complete',
  requested: 1,
  completed: 1,
  failed: 0,
  cancellable: false,
  failures: [],
}

function options(projectSessionId: string, scan: ViewerState['scan'] = null) {
  return {
    projectSessionId,
    scan,
    operation: initialOperationState(),
    projectError: null,
    finderDragMessage: null,
    workspaceActionError: null,
  }
}

describe('useFeedbackCoordinator', () => {
  it('resets local tasks and dismissed ids when the project session changes', () => {
    const scan: NonNullable<ViewerState['scan']> = {
      taskId: 'scan-1',
      phase: 'finished',
      publishedFolders: 1,
      publishedFiles: 0,
      failedItems: [],
      totals: { folders: 1, files: 0, failed: 0 },
    }
    const hook = renderHook(({ sessionId }) => useFeedbackCoordinator(options(sessionId, scan)), {
      initialProps: { sessionId: 'session-1' },
    })

    act(() => {
      hook.result.current.setThumbnailTask(complete)
      hook.result.current.setTextTask({ ...complete, id: 'text-1', label: '读取文本' })
      hook.result.current.dismissTask('scan-1')
      hook.result.current.dismissTask('thumbnail-1')
      hook.result.current.dismissTask('text-1')
    })
    expect(hook.result.current.visibleTasks).toEqual([])

    hook.rerender({ sessionId: 'session-2' })

    expect(hook.result.current.visibleTasks).toEqual([
      expect.objectContaining({ id: 'scan-1', status: 'complete' }),
    ])
  })

  it('shows a running task even when it was dismissed and suppresses unrelated success', () => {
    const hook = renderHook(() => useFeedbackCoordinator(options('session-1')))

    act(() => {
      hook.result.current.setThumbnailTask({ ...complete, status: 'running', completed: 0 })
      hook.result.current.setTextTask({ ...complete, id: 'text-1', label: '读取文本' })
      hook.result.current.dismissTask('thumbnail-1')
    })

    expect(hook.result.current.visibleTasks).toEqual([
      expect.objectContaining({ id: 'thumbnail-1', status: 'running' }),
    ])
  })
})
