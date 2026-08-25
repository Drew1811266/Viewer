import { describe, expect, it } from 'vitest'
import type { TaskFeedback } from '../../components/TaskBar'
import type { ViewerState } from '../../state/viewerState'
import {
  buildGlobalNotices,
  operationTaskFromState,
  scanTaskFromState,
  visibleTaskFeedback,
} from './feedbackModel'

const running: TaskFeedback = {
  id: 'running',
  label: '生成缩略图',
  status: 'running',
  requested: 2,
  completed: 1,
  failed: 0,
  cancellable: false,
  failures: [],
}

const complete: TaskFeedback = {
  ...running,
  id: 'complete',
  status: 'complete',
  completed: 2,
}

describe('feedbackModel', () => {
  it('maps scan progress and cancellation to the existing task contract', () => {
    const scan: NonNullable<ViewerState['scan']> = {
      taskId: 'scan-1',
      phase: 'cancelled',
      publishedFolders: 2,
      publishedFiles: 3,
      failedItems: [{ relativePath: 'bad.jpg', code: 'unreadable' }],
      totals: { folders: 2, files: 3, failed: 1 },
    }

    expect(scanTaskFromState(scan)).toEqual({
      id: 'scan-1',
      label: '扫描项目',
      status: 'cancelled',
      requested: 6,
      completed: 5,
      failed: 1,
      cancellable: false,
      failures: [{ item: 'bad.jpg', code: 'unreadable' }],
    })
  })

  it('maps operation completion, cancellation, results, and labels without changing precedence', () => {
    const completedProgress: NonNullable<ViewerState['operation']['active']> = {
      sessionId: 'session-1',
      generation: 1,
      batchId: 'batch-1',
      lifecycle: 'completed',
      requested: 3,
      completed: 0,
      failed: 0,
      skipped: 0,
      cancelled: 3,
      activeEntityId: null,
    }
    const operation: ViewerState['operation'] = {
      kind: 'move',
      active: completedProgress,
      results: { total: 0, offset: 0, items: [] },
      finishing: false,
      pending: false,
    }

    expect(operationTaskFromState(operation)).toEqual({
      id: 'batch-1',
      label: '移动文件',
      status: 'cancelled',
      requested: 3,
      completed: 0,
      failed: 0,
      skipped: 0,
      cancelled: 3,
      cancellable: false,
      failures: [],
      hasResults: true,
    })

    expect(
      operationTaskFromState({
        ...operation,
        kind: 'trash',
        active: { ...completedProgress, failed: 1, cancelled: 2 },
      }),
    ).toMatchObject({ label: '移到废纸篓', status: 'failed' })
  })

  it('keeps running and result-bearing tasks visible while hiding dismissed or superseded success', () => {
    expect(visibleTaskFeedback([running, complete], new Set())).toEqual([running])
    expect(visibleTaskFeedback([running], new Set(['running']))).toEqual([running])
    expect(
      visibleTaskFeedback([{ ...complete, hasResults: true }], new Set(['unrelated'])),
    ).toEqual([{ ...complete, hasResults: true }])
    expect(visibleTaskFeedback([complete], new Set(['complete']))).toEqual([])
  })

  it('builds the exact existing global notices in stable precedence order', () => {
    expect(
      buildGlobalNotices({
        projectError: '项目错误',
        finderDragMessage: '拖动错误',
        workspaceActionError: '显示错误',
      }),
    ).toEqual([
      {
        id: 'application:项目错误',
        title: 'Viewer 出现问题',
        message: '项目错误',
        tone: 'danger',
      },
      {
        id: 'finder-drag:拖动错误',
        title: '无法拖出文件',
        message: '拖动错误',
        tone: 'danger',
      },
      {
        id: 'workspace:显示错误',
        title: '无法在文件管理器中显示项目',
        message: '显示错误',
        tone: 'danger',
      },
    ])
  })
})
