import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import ModalSheet from './ModalSheet'
import ViewerButton from './ui/ViewerButton'

describe('ModalSheet', () => {
  it('uses the atlas 430 px shell by default and reserves the large shell for complex flows', () => {
    const rendered = render(
      <ModalSheet title="单项操作" onCancel={vi.fn()}>
        <input aria-label="单项字段" />
      </ModalSheet>,
    )

    expect(screen.getByRole('dialog', { name: '单项操作' })).toHaveAttribute('data-size', 'small')

    rendered.rerender(
      <ModalSheet title="复杂操作" size="large" onCancel={vi.fn()}>
        <input aria-label="复杂字段" />
      </ModalSheet>,
    )

    expect(screen.getByRole('dialog', { name: '复杂操作' })).toHaveAttribute('data-size', 'large')
  })

  it.each([
    [1024, 720],
    [720, 450],
  ])('keeps its body and footer actions mounted at %d×%d CSS pixels', (width, height) => {
    vi.stubGlobal('innerWidth', width)
    vi.stubGlobal('innerHeight', height)
    render(
      <ModalSheet
        title="视口检查"
        onCancel={vi.fn()}
        footer={
          <>
            <ViewerButton tone="secondary">取消</ViewerButton>
            <ViewerButton tone="primary">确认</ViewerButton>
          </>
        }
      >
        <input aria-label="对话框字段" />
      </ModalSheet>,
    )
    expect(screen.getByRole('dialog', { name: '视口检查' })).toBeVisible()
    expect(screen.getByRole('textbox', { name: '对话框字段' })).toBeVisible()
    expect(screen.getByRole('button', { name: '取消' })).toBeVisible()
    expect(screen.getByRole('button', { name: '确认' })).toBeVisible()
  })

  it('traps Tab, handles Escape and restores the invoking focus', () => {
    const trigger = document.createElement('button')
    document.body.append(trigger)
    trigger.focus()
    const cancel = vi.fn()
    const rendered = render(
      <ModalSheet
        title="安全操作"
        onCancel={cancel}
        footer={<ViewerButton tone="primary">最后一个</ViewerButton>}
      >
        <button type="button">第一个</button>
      </ModalSheet>,
    )
    const first = screen.getByRole('button', { name: '第一个' })
    const last = screen.getByRole('button', { name: '最后一个' })
    const dialog = screen.getByRole('dialog', { name: '安全操作' })
    expect(dialog.querySelector('.viewer-dialog__header')).not.toBeNull()
    expect(dialog.querySelector('.viewer-dialog__body')).not.toBeNull()
    expect(dialog.querySelector('.viewer-dialog__footer')).not.toBeNull()
    expect(first).toHaveFocus()

    last.focus()
    fireEvent.keyDown(last, { key: 'Tab' })
    expect(first).toHaveFocus()
    fireEvent.keyDown(first, { key: 'Tab', shiftKey: true })
    expect(last).toHaveFocus()
    fireEvent.keyDown(last, { key: 'Escape' })
    expect(cancel).toHaveBeenCalledOnce()

    rendered.unmount()
    expect(trigger).toHaveFocus()
    trigger.remove()
  })
})
