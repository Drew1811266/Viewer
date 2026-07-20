import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import TaskBar from './TaskBar'
import type { TaskFeedback } from './TaskBar'

const failedScanTask: TaskFeedback = {
  id: 'scan-1',
  label: '扫描项目',
  status: 'failed',
  requested: 12,
  completed: 10,
  failed: 2,
  cancellable: false,
  failures: [
    { item: 'catalog/a.jpg', code: 'unreadable' },
    { item: 'catalog/b.jpg', code: 'unreadable' },
  ],
}

describe('TaskBar', () => {
  it('shows progress compactly and retains expandable safe failure details', () => {
    render(<TaskBar task={failedScanTask} />)

    expect(screen.getByText('扫描项目')).toBeVisible()
    expect(screen.getByText('2 项失败')).toBeVisible()
    fireEvent.click(screen.getByRole('button', { name: '展开任务详情' }))
    expect(screen.getByText('catalog/a.jpg')).toBeVisible()
  })

  it('only offers cancellation for cancellable work', () => {
    const cancel = vi.fn()
    const { rerender } = render(<TaskBar task={failedScanTask} onCancel={cancel} />)
    expect(screen.queryByRole('button', { name: '取消任务' })).not.toBeInTheDocument()

    rerender(
      <TaskBar
        task={{ ...failedScanTask, status: 'running', cancellable: true }}
        onCancel={cancel}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: '取消任务' }))
    expect(cancel).toHaveBeenCalledWith('scan-1')
  })

  it('counts skipped/cancelled operation items and opens completed results', () => {
    const showResults = vi.fn()
    render(
      <TaskBar
        task={{
          id: 'batch-1',
          label: '移动文件',
          status: 'complete',
          requested: 10,
          completed: 6,
          failed: 1,
          skipped: 2,
          cancelled: 1,
          cancellable: false,
          failures: [],
          hasResults: true,
        }}
        onShowResults={showResults}
      />,
    )
    expect(screen.getByText('10/10')).toBeVisible()
    expect(screen.getByText('2 项跳过')).toBeVisible()
    expect(screen.getByText('1 项取消')).toBeVisible()
    fireEvent.click(screen.getByRole('button', { name: '查看移动文件结果' }))
    expect(showResults).toHaveBeenCalledWith('batch-1')
  })
})
