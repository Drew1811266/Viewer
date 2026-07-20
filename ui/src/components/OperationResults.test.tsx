import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import OperationResults from './OperationResults'

describe('OperationResults', () => {
  it('summarizes partial results with safe item codes and pages bounded details', () => {
    const page = vi.fn()
    render(
      <OperationResults
        batchId="batch-1"
        progress={{
          sessionId: 'session-1',
          generation: 1,
          batchId: 'batch-1',
          lifecycle: 'completed',
          requested: 250,
          completed: 247,
          failed: 2,
          skipped: 1,
          cancelled: 0,
          activeEntityId: null,
        }}
        page={{
          total: 250,
          offset: 0,
          items: [
            {
              entityId: 'one',
              relativePath: 'id/one.jpg',
              status: 'failed',
              code: 'permission_denied',
            },
          ],
        }}
        onPageChange={page}
        onClose={vi.fn()}
      />,
    )
    expect(screen.getByText('247 项完成')).toBeVisible()
    expect(screen.getByText('2 项失败')).toBeVisible()
    expect(screen.getByText('id/one.jpg')).toBeVisible()
    expect(screen.queryByText(/Users\//)).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: '下一页结果' }))
    expect(page).toHaveBeenCalledWith(200)
  })
})
