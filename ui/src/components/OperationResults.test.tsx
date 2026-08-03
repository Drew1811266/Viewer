import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import OperationResults from './OperationResults'

describe('OperationResults', () => {
  it('summarizes partial results with safe item codes and pages bounded details', () => {
    const page = vi.fn()
    const close = vi.fn()
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
        onClose={close}
      />,
    )
    expect(screen.getByText('247 项完成')).toBeVisible()
    expect(screen.getByText('2 项失败')).toBeVisible()
    expect(screen.getByText('247 项完成')).toHaveClass('viewer-status-tag')
    expect(screen.getByText('2 项失败')).toHaveClass('viewer-status-tag')
    expect(screen.getByText('id/one.jpg')).toBeVisible()
    const inspector = screen.getByRole('complementary', { name: '文件操作结果' })
    expect(inspector).toHaveClass('viewer-inspector')
    expect(inspector).not.toHaveClass('operation-results')
    expect(screen.getByRole('button', { name: '关闭文件操作结果' })).toHaveClass(
      'viewer-icon-button',
    )
    expect(screen.getByRole('navigation', { name: '文件操作结果分页' })).toBeVisible()
    expect(screen.queryByText(/Users\//)).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: '下一页结果' }))
    expect(page).toHaveBeenCalledWith(200)
    fireEvent.click(screen.getByRole('button', { name: '关闭文件操作结果' }))
    expect(close).toHaveBeenCalledOnce()
  })

  it('keeps one result page bounded to 200 rendered rows', () => {
    render(
      <OperationResults
        batchId="batch-200"
        progress={{
          sessionId: 'session-1',
          generation: 1,
          batchId: 'batch-200',
          lifecycle: 'completed',
          requested: 201,
          completed: 200,
          failed: 0,
          skipped: 0,
          cancelled: 0,
          activeEntityId: null,
        }}
        page={{
          total: 201,
          offset: 0,
          items: Array.from({ length: 200 }, (_, index) => ({
            entityId: `item-${index}`,
            relativePath: `items/${index}.jpg`,
            status: 'completed' as const,
            code: 'copied' as const,
          })),
        }}
        onPageChange={vi.fn()}
        onClose={vi.fn()}
      />,
    )
    expect(document.querySelectorAll('.operation-results-list > li')).toHaveLength(200)
    expect(screen.getByText('1–200 / 201')).toBeVisible()
  })

  it('keeps the result inspector anchored to the formal 40px application rail', () => {
    render(
      <OperationResults
        batchId="batch-2"
        progress={{
          sessionId: 'session-1',
          generation: 1,
          batchId: 'batch-2',
          lifecycle: 'completed',
          requested: 0,
          completed: 0,
          failed: 0,
          skipped: 0,
          cancelled: 0,
          activeEntityId: null,
        }}
        page={{ total: 0, offset: 0, items: [] }}
        onPageChange={vi.fn()}
        onClose={vi.fn()}
      />,
    )
    expect(screen.getByRole('complementary', { name: '文件操作结果' })).toHaveClass(
      'viewer-inspector',
    )
  })
})
