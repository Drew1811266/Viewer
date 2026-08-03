import { fireEvent, render, screen } from '@testing-library/react'
import { createRef } from 'react'
import { describe, expect, it, vi } from 'vitest'
import ViewerMenuRow from './ViewerMenuRow'
import ViewerPopover from './ViewerPopover'

function Harness({ onOpenChange = vi.fn() }: { onOpenChange?: (open: boolean) => void }) {
  const triggerRef = createRef<HTMLButtonElement>()
  return (
    <div>
      <button ref={triggerRef} type="button">
        筛选
      </button>
      <ViewerPopover
        open
        label="筛选条件"
        triggerRef={triggerRef}
        onOpenChange={onOpenChange}
        align="start"
      >
        <button type="button">筛选内容</button>
      </ViewerPopover>
      <button type="button">外部</button>
    </div>
  )
}

describe('ViewerPopover', () => {
  it('dismisses on Escape and restores focus to its trigger', () => {
    const onOpenChange = vi.fn()
    render(<Harness onOpenChange={onOpenChange} />)
    const content = screen.getByRole('button', { name: '筛选内容' })
    content.focus()
    fireEvent.keyDown(document, { key: 'Escape' })

    expect(onOpenChange).toHaveBeenCalledWith(false)
    expect(screen.getByRole('button', { name: '筛选' })).toHaveFocus()
  })

  it('dismisses outside pointer input but ignores its trigger and content', () => {
    const onOpenChange = vi.fn()
    render(<Harness onOpenChange={onOpenChange} />)
    fireEvent.pointerDown(screen.getByRole('button', { name: '筛选内容' }))
    fireEvent.pointerDown(screen.getByRole('button', { name: '筛选' }))
    expect(onOpenChange).not.toHaveBeenCalled()

    fireEvent.pointerDown(screen.getByRole('button', { name: '外部' }))
    expect(onOpenChange).toHaveBeenCalledWith(false)
    expect(screen.getByRole('button', { name: '筛选' })).toHaveFocus()
  })

  it('exposes a labelled popover surface and alignment hook', () => {
    render(<Harness />)
    expect(screen.getByRole('region', { name: '筛选条件' })).toHaveAttribute('data-align', 'start')
    expect(screen.getAllByRole('button').map((button) => button.textContent)).toEqual([
      '筛选',
      '筛选内容',
      '外部',
    ])
  })

  it('keeps current and disabled menu states readable without color-only meaning', () => {
    const select = vi.fn()
    render(
      <>
        <ViewerMenuRow current onSelect={select}>
          按文件夹分组
        </ViewerMenuRow>
        <ViewerMenuRow disabledReason="项目为只读" tone="danger" onSelect={select}>
          移到废纸篓
        </ViewerMenuRow>
      </>,
    )

    expect(screen.getByRole('button', { name: /按文件夹分组.*当前/ })).toHaveAttribute(
      'data-current',
      'true',
    )
    expect(screen.getByRole('button', { name: /移到废纸篓.*项目为只读/ })).toBeDisabled()
    expect(screen.getByText('项目为只读')).toBeVisible()
  })
})
