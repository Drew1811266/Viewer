import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import CloseOperationDialog from './CloseOperationDialog'

describe('CloseOperationDialog', () => {
  it('offers wait, cancel-pending and stay without treating committed work as reversible', () => {
    const wait = vi.fn()
    const cancelPending = vi.fn()
    const stay = vi.fn()
    const rendered = render(
      <CloseOperationDialog
        busy={false}
        onWait={wait}
        onCancelPending={cancelPending}
        onStay={stay}
      />,
    )

    const dialog = screen.getByRole('dialog', { name: '文件操作尚未完成' })
    expect(dialog).toHaveTextContent('已经完成的项目不会撤销')
    expect(screen.getAllByRole('button').map((button) => button.textContent)).toEqual([
      '停留在当前项目',
      '取消待处理项目并关闭',
      '等待完成后关闭',
    ])
    expect(dialog.querySelector('.viewer-dialog__footer')).not.toBeNull()
    expect(screen.getByRole('button', { name: '等待完成后关闭' })).toHaveAttribute(
      'data-tone',
      'primary',
    )
    expect(screen.getByRole('button', { name: '取消待处理项目并关闭' })).toHaveAttribute(
      'data-tone',
      'secondary',
    )
    fireEvent.click(screen.getByRole('button', { name: '等待完成后关闭' }))
    fireEvent.click(screen.getByRole('button', { name: '取消待处理项目并关闭' }))
    fireEvent.click(screen.getByRole('button', { name: '停留在当前项目' }))
    expect(wait).toHaveBeenCalledOnce()
    expect(cancelPending).toHaveBeenCalledOnce()
    expect(stay).toHaveBeenCalledOnce()

    rendered.rerender(
      <CloseOperationDialog busy onWait={wait} onCancelPending={cancelPending} onStay={stay} />,
    )
    for (const button of screen.getAllByRole('button')) expect(button).toBeDisabled()
    expect(screen.getByRole('status')).toHaveTextContent('正在安全结束文件操作')
    fireEvent.keyDown(dialog, { key: 'Escape' })
    expect(stay).toHaveBeenCalledOnce()
  })
})
