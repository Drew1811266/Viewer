import { fireEvent, render, screen } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import useReviewShortcuts from './useReviewShortcuts'

function Harness({ disabled = false }: { disabled?: boolean }) {
  useReviewShortcuts({
    disabled,
    onSetReview: review,
    onToggleFavorite: favorite,
  })
  return (
    <>
      <input aria-label="编辑名称" />
      <textarea aria-label="编辑备注" />
      <div aria-label="富文本" contentEditable />
    </>
  )
}

const review = vi.fn()
const favorite = vi.fn()

describe('useReviewShortcuts', () => {
  beforeEach(() => {
    review.mockClear()
    favorite.mockClear()
  })

  it('routes 1/2/3/0/F while the workspace owns the event', () => {
    render(<Harness />)
    for (const [key, value] of [
      ['1', 'keep'],
      ['2', 'pending'],
      ['3', 'reject'],
      ['0', null],
    ] as const) {
      fireEvent.keyDown(window, { key })
      expect(review).toHaveBeenLastCalledWith(value)
    }
    fireEvent.keyDown(window, { key: 'f' })
    expect(favorite).toHaveBeenCalledOnce()
  })

  it('does not steal editable, modal, selected-text, prevented, or disabled events', () => {
    const rendered = render(<Harness />)
    for (const target of [
      screen.getByLabelText('编辑名称'),
      screen.getByLabelText('编辑备注'),
      screen.getByLabelText('富文本'),
    ]) {
      fireEvent.keyDown(target, { key: '1' })
      fireEvent.keyDown(target, { key: 'f' })
    }
    const prevented = new KeyboardEvent('keydown', {
      key: '1',
      bubbles: true,
      cancelable: true,
    })
    prevented.preventDefault()
    window.dispatchEvent(prevented)
    const modal = document.createElement('div')
    modal.setAttribute('aria-modal', 'true')
    document.body.append(modal)
    fireEvent.keyDown(window, { key: '2' })
    modal.remove()
    const selection = vi.spyOn(window, 'getSelection').mockReturnValue({
      isCollapsed: false,
      toString: () => 'selected text',
    } as Selection)
    fireEvent.keyDown(window, { key: '3' })
    selection.mockRestore()
    for (const modifier of ['metaKey', 'ctrlKey', 'altKey', 'shiftKey'] as const) {
      fireEvent.keyDown(window, { key: 'f', [modifier]: true })
    }
    rendered.rerender(<Harness disabled />)
    fireEvent.keyDown(window, { key: 'f' })
    expect(review).not.toHaveBeenCalled()
    expect(favorite).not.toHaveBeenCalled()
  })
})
