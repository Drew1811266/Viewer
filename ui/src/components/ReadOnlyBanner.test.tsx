import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import ReadOnlyBanner from './ReadOnlyBanner'

describe('ReadOnlyBanner', () => {
  it('explains the capability boundary and exposes only direct recovery actions', () => {
    const openSettings = vi.fn()
    const reselect = vi.fn()
    const rendered = render(
      <ReadOnlyBanner busy={false} onOpenSettings={openSettings} onReselect={reselect} />,
    )

    expect(screen.getByRole('status', { name: '只读模式' })).toHaveTextContent(
      '可以浏览、搜索和预览，但不能修改文件或标记',
    )
    expect(openSettings).not.toHaveBeenCalled()
    expect(reselect).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: '权限设置' }))
    fireEvent.click(screen.getByRole('button', { name: '重新选择目录' }))
    expect(openSettings).toHaveBeenCalledOnce()
    expect(reselect).toHaveBeenCalledOnce()

    rendered.rerender(<ReadOnlyBanner busy onOpenSettings={openSettings} onReselect={reselect} />)
    expect(screen.getByRole('button', { name: '权限设置' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '重新选择目录' })).toBeDisabled()
  })
})
