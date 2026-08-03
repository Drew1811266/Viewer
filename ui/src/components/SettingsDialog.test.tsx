import { fireEvent, render, screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import SettingsDialog from './SettingsDialog'

describe('SettingsDialog', () => {
  it('shows one density radio group and reports a choice immediately', () => {
    const onDensityChange = vi.fn()
    render(
      <SettingsDialog
        density="standard"
        error={null}
        onDensityChange={onDensityChange}
        onClose={vi.fn()}
      />,
    )

    const dialog = screen.getByRole('dialog', { name: '软件设置' })
    expect(within(dialog).getByRole('heading', { name: '显示与外观' })).toBeVisible()
    const navigation = within(dialog).getByRole('navigation', { name: '设置分类' })
    expect(within(navigation).getAllByRole('button')).toHaveLength(1)
    expect(within(navigation).getByRole('button', { name: '显示与外观' })).toBeVisible()
    for (const absent of ['浏览', '文件操作', '快捷键']) {
      expect(within(navigation).queryByRole('button', { name: absent })).not.toBeInTheDocument()
    }
    const group = within(dialog).getByRole('group', { name: '缩略图密度' })
    expect(within(group).getByRole('radio', { name: '紧凑' })).not.toBeChecked()
    expect(within(group).getByRole('radio', { name: '标准' })).toBeChecked()
    const large = within(group).getByRole('radio', { name: '大图' })
    expect(large).not.toBeChecked()

    fireEvent.click(large)

    expect(onDensityChange).toHaveBeenCalledWith('large')
    expect(dialog.querySelector('.viewer-dialog__footer')).not.toBeNull()
    expect(within(dialog).getByRole('button', { name: '关闭' })).toHaveAttribute(
      'data-tone',
      'secondary',
    )
  })

  it('shows the latest save error', () => {
    render(
      <SettingsDialog
        density="standard"
        error="设置未能保存"
        onDensityChange={vi.fn()}
        onClose={vi.fn()}
      />,
    )

    expect(screen.getByRole('alert')).toHaveTextContent('设置未能保存')
  })

  it('closes by button or Escape, traps focus, and restores the previous focus', () => {
    const trigger = document.createElement('button')
    document.body.append(trigger)
    trigger.focus()
    const onClose = vi.fn()
    const rendered = render(
      <SettingsDialog
        density="standard"
        error={null}
        onDensityChange={vi.fn()}
        onClose={onClose}
      />,
    )
    const first = screen.getByRole('radio', { name: '紧凑' })
    const navigationItem = screen.getByRole('button', { name: '显示与外观' })
    const close = screen.getByRole('button', { name: '关闭' })
    expect(first).toHaveFocus()

    close.focus()
    fireEvent.keyDown(close, { key: 'Tab' })
    expect(navigationItem).toHaveFocus()
    fireEvent.keyDown(navigationItem, { key: 'Tab', shiftKey: true })
    expect(close).toHaveFocus()
    fireEvent.keyDown(close, { key: 'Escape' })
    expect(onClose).toHaveBeenCalledOnce()
    fireEvent.click(close)
    expect(onClose).toHaveBeenCalledTimes(2)

    rendered.unmount()
    expect(trigger).toHaveFocus()
    trigger.remove()
  })
})
