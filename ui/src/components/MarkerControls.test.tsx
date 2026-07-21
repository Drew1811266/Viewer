import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { SelectionInfo } from '../api/types'
import MarkerControls from './MarkerControls'

const mixed: SelectionInfo = {
  relativePaths: ['id/1.jpg', 'id/2.jpg'],
  totalSize: 30,
  types: { folders: 0, images: 2, textFiles: 0 },
  commonReview: { state: 'mixed' },
  commonFavorite: { state: 'common', value: false },
}

describe('MarkerControls', () => {
  it('applies buttons and 1/2/3/0/F to every selected target independently', () => {
    const review = vi.fn()
    const favorite = vi.fn()
    render(
      <MarkerControls
        selectedCount={2}
        selectionInfo={mixed}
        readOnly={false}
        onSetReview={review}
        onToggleFavorite={favorite}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: '标记为保留' }))
    fireEvent.click(screen.getByRole('button', { name: '切换收藏' }))
    expect(review).toHaveBeenCalledWith('keep')
    expect(favorite).toHaveBeenCalledOnce()
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
    expect(favorite).toHaveBeenCalledTimes(2)
    expect(screen.getByText('已选 2 项')).toBeVisible()
    expect(screen.getByText('审阅状态：混合')).toBeVisible()
    expect(screen.getByText('收藏：否')).toBeVisible()
  })

  it('does not steal shortcuts from editable controls and disables writes when read-only', () => {
    const review = vi.fn()
    const favorite = vi.fn()
    const rendered = render(
      <>
        <input aria-label="编辑名称" />
        <textarea aria-label="编辑备注" />
        <div aria-label="富文本" contentEditable />
        <MarkerControls
          selectedCount={1}
          selectionInfo={mixed}
          readOnly={false}
          onSetReview={review}
          onToggleFavorite={favorite}
        />
      </>,
    )
    for (const target of [
      screen.getByLabelText('编辑名称'),
      screen.getByLabelText('编辑备注'),
      screen.getByLabelText('富文本'),
    ]) {
      fireEvent.keyDown(target, { key: '1' })
      fireEvent.keyDown(target, { key: 'f' })
    }
    expect(review).not.toHaveBeenCalled()
    expect(favorite).not.toHaveBeenCalled()

    rendered.rerender(
      <MarkerControls
        selectedCount={1}
        selectionInfo={mixed}
        readOnly
        onSetReview={review}
        onToggleFavorite={favorite}
      />,
    )
    expect(screen.getByRole('button', { name: '标记为保留' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '切换收藏' })).toBeDisabled()
    fireEvent.keyDown(window, { key: '2' })
    fireEvent.keyDown(window, { key: 'f' })
    expect(review).not.toHaveBeenCalled()
    expect(favorite).not.toHaveBeenCalled()
  })

  it('honors shared shortcut ownership for prevented, modal, selected-text, busy, and interactive contexts', () => {
    const review = vi.fn()
    const favorite = vi.fn()
    const rendered = render(
      <>
        <button type="button">其他操作</button>
        <MarkerControls
          selectedCount={1}
          selectionInfo={mixed}
          readOnly={false}
          onSetReview={review}
          onToggleFavorite={favorite}
        />
      </>,
    )

    const prevented = new KeyboardEvent('keydown', {
      key: '1',
      bubbles: true,
      cancelable: true,
    })
    prevented.preventDefault()
    window.dispatchEvent(prevented)
    expect(review).not.toHaveBeenCalled()

    const interactive = screen.getByRole('button', { name: '其他操作' })
    interactive.focus()
    fireEvent.keyDown(interactive, { key: '2' })
    expect(review).not.toHaveBeenCalled()
    interactive.blur()

    const modal = document.createElement('div')
    modal.setAttribute('aria-modal', 'true')
    document.body.append(modal)
    fireEvent.keyDown(window, { key: '3' })
    expect(review).not.toHaveBeenCalled()
    modal.remove()

    const selection = vi.spyOn(window, 'getSelection').mockReturnValue({
      isCollapsed: false,
      toString: () => 'selected text',
    } as Selection)
    fireEvent.keyDown(window, { key: '0' })
    expect(review).not.toHaveBeenCalled()
    selection.mockRestore()

    rendered.rerender(
      <MarkerControls
        selectedCount={1}
        selectionInfo={mixed}
        readOnly={false}
        shortcutsDisabled
        onSetReview={review}
        onToggleFavorite={favorite}
      />,
    )
    fireEvent.keyDown(window, { key: 'f' })

    expect(review).not.toHaveBeenCalled()
    expect(favorite).not.toHaveBeenCalled()
    expect(screen.getByRole('button', { name: '标记为保留' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '切换收藏' })).toBeDisabled()
  })
})
