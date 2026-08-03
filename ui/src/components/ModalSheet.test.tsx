import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import ModalSheet from './ModalSheet'
import ViewerButton from './ui/ViewerButton'

describe('ModalSheet', () => {
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
    expect(dialog.querySelector('.modal-sheet-header')).not.toBeNull()
    expect(dialog.querySelector('.modal-sheet-body')).not.toBeNull()
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
