import { fireEvent, render, screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import SettingsDialog from './SettingsDialog'

describe('SettingsDialog', () => {
  it.each([
    [1024, 720],
    [720, 450],
  ])(
    'keeps the real settings category and footer reachable at %d×%d CSS pixels',
    (width, height) => {
      vi.stubGlobal('innerWidth', width)
      vi.stubGlobal('innerHeight', height)
      render(
        <SettingsDialog
          density="standard"
          error={null}
          onDensityChange={vi.fn()}
          onClose={vi.fn()}
        />,
      )
      expect(screen.getByRole('navigation', { name: '设置分类' })).toBeVisible()
      expect(screen.getByRole('slider', { name: '缩略图大小' })).toBeVisible()
      expect(screen.getByRole('button', { name: '关闭' })).toBeVisible()
    },
  )

  it('shows one five-stop size slider and reports a choice immediately', () => {
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
    const group = within(dialog).getByRole('group', { name: '缩略图大小' })
    const slider = within(group).getByRole('slider', { name: '缩略图大小' })
    expect(slider).toHaveAttribute('min', '1')
    expect(slider).toHaveAttribute('max', '5')
    expect(slider).toHaveAttribute('step', '1')
    expect(slider).toHaveValue('2')
    expect(slider).toHaveAttribute('aria-valuetext', '档位 2，132 像素')
    expect(within(group).queryAllByRole('radio')).toHaveLength(0)
    for (const oldLabel of ['紧凑', '标准', '大图']) {
      expect(within(group).queryByText(oldLabel)).not.toBeInTheDocument()
    }
    const levels = [...group.querySelectorAll('.thumbnail-size-levels span')]
    expect(levels.map((level) => level.textContent)).toEqual(['1', '2', '3', '4', '5'])
    expect(levels[1]).toHaveAttribute('data-current', 'true')
    expect(levels[0]).not.toHaveAttribute('data-current')

    fireEvent.change(slider, { target: { value: '5' } })

    expect(onDensityChange).toHaveBeenCalledWith('maximum')
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
    const first = screen.getByRole('slider', { name: '缩略图大小' })
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
