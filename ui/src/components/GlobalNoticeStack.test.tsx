import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import GlobalNoticeStack from './GlobalNoticeStack'

describe('GlobalNoticeStack', () => {
  it('orders global notices and preserves action and dismissal semantics', () => {
    const showResults = vi.fn()
    render(
      <GlobalNoticeStack
        notices={[
          {
            id: 'recovery',
            title: '项目恢复完成',
            message: '已恢复 2 项操作；1 项需要检查。',
            tone: 'warning',
            action: { label: '查看结果', onAction: showResults },
          },
          {
            id: 'fatal',
            title: 'Viewer 无法继续',
            message: '项目状态不可用。',
            tone: 'danger',
          },
        ]}
      />,
    )

    expect(screen.getByRole('region', { name: '全局通知' })).toBeVisible()
    const danger = screen.getByRole('alert')
    expect(danger).toHaveTextContent('Viewer 无法继续')
    expect(danger).toHaveClass('viewer-local-feedback')
    const resultAction = screen.getByRole('button', { name: '查看结果' })
    expect(resultAction).toHaveClass('viewer-button')
    fireEvent.click(resultAction)
    expect(showResults).toHaveBeenCalledOnce()
    const dismiss = screen.getByRole('button', { name: '关闭 Viewer 无法继续' })
    expect(dismiss).toHaveClass('viewer-icon-button')
    expect(dismiss.querySelector('img')).toHaveAttribute('src', expect.stringContaining('/x.svg'))
    fireEvent.click(dismiss)
    expect(screen.queryByText('项目状态不可用。')).not.toBeInTheDocument()
  })

  it('forgets dismissed ids once their current occurrence leaves the stack', () => {
    const notice = {
      id: 'recovery',
      title: '项目恢复完成',
      message: '已恢复 1 项操作；0 项需要检查。',
      tone: 'info' as const,
    }
    const rendered = render(<GlobalNoticeStack notices={[notice]} />)

    fireEvent.click(screen.getByRole('button', { name: '关闭 项目恢复完成' }))
    expect(screen.queryByRole('region', { name: '全局通知' })).not.toBeInTheDocument()
    rendered.rerender(<GlobalNoticeStack notices={[]} />)
    rendered.rerender(<GlobalNoticeStack notices={[notice]} />)
    expect(screen.getByRole('status')).toHaveTextContent('项目恢复完成')
  })
})
