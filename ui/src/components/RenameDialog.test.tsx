import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import RenameDialog from './RenameDialog'

describe('RenameDialog', () => {
  it('blocks invalid and unchanged names, then submits an explicit safe name', () => {
    const confirm = vi.fn()
    render(
      <RenameDialog currentName="front.jpg" busy={false} onConfirm={confirm} onCancel={vi.fn()} />,
    )
    const input = screen.getByRole('textbox', { name: '新文件名' })
    expect(input).toHaveFocus()
    expect(input).toHaveProperty('selectionStart', 0)
    expect(input).toHaveProperty('selectionEnd', 5)
    expect(screen.getByRole('button', { name: '重命名' })).toBeDisabled()
    fireEvent.change(input, { target: { value: '../secret.jpg' } })
    expect(screen.getByRole('alert')).toHaveTextContent('不能包含')
    expect(screen.getByRole('button', { name: '重命名' })).toBeDisabled()
    fireEvent.change(input, { target: { value: 'hero.jpg' } })
    fireEvent.click(screen.getByRole('button', { name: '重命名' }))
    expect(confirm).toHaveBeenCalledWith('hero', false)
  })

  it('keeps extension editing opt-in and restores focus when cancelled', () => {
    const cancel = vi.fn()
    const trigger = document.createElement('button')
    document.body.append(trigger)
    trigger.focus()
    const rendered = render(
      <RenameDialog currentName="front.jpg" busy={false} onConfirm={vi.fn()} onCancel={cancel} />,
    )
    expect(screen.getByRole('textbox', { name: '新文件名' })).toHaveValue('front.jpg')
    fireEvent.change(screen.getByRole('textbox', { name: '新文件名' }), {
      target: { value: 'front.png' },
    })
    expect(screen.getByRole('button', { name: '重命名' })).toBeDisabled()
    expect(screen.getByRole('alert')).toHaveTextContent('扩展名')
    fireEvent.click(screen.getByRole('checkbox', { name: '允许修改扩展名' }))
    expect(screen.getByRole('button', { name: '重命名' })).toBeEnabled()
    fireEvent.click(screen.getByRole('button', { name: '取消' }))
    expect(cancel).toHaveBeenCalledOnce()
    rendered.unmount()
    expect(trigger).toHaveFocus()
    trigger.remove()
  })
})
