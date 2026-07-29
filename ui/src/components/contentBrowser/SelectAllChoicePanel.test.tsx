import { createEvent, fireEvent, render, screen } from '@testing-library/react'
import { useRef, useState } from 'react'
import { describe, expect, it, vi } from 'vitest'
import type { SelectAllScope } from './adaptiveTextPanelModel'
import SelectAllChoicePanel from './SelectAllChoicePanel'

interface ChoiceHarnessProps {
  open?: boolean
  onChoose?: (scope: SelectAllScope) => void
  onCancel?: () => void
}

function ChoiceHarness({
  open = false,
  onChoose = () => undefined,
  onCancel = () => undefined,
}: ChoiceHarnessProps) {
  const [panelOpen, setPanelOpen] = useState(open)
  const anchorRef = useRef<HTMLButtonElement | null>(null)

  function cancel() {
    setPanelOpen(false)
    onCancel()
  }

  return (
    <>
      <div>
        <button
          ref={anchorRef}
          type="button"
          onClick={() => {
            if (panelOpen) {
              cancel()
              anchorRef.current?.focus()
            } else {
              setPanelOpen(true)
            }
          }}
        >
          全选当前文件夹
        </button>
        <SelectAllChoicePanel
          open={panelOpen}
          anchorRef={anchorRef}
          onChoose={onChoose}
          onCancel={cancel}
        />
      </div>
      <button type="button">外部目标</button>
    </>
  )
}

describe('SelectAllChoicePanel', () => {
  it('focuses the first command and chooses one exact scope', () => {
    const choose = vi.fn()
    render(<ChoiceHarness open onChoose={choose} />)

    expect(screen.getByRole('menuitem', { name: '全选图片' })).toHaveFocus()
    fireEvent.click(screen.getByRole('menuitem', { name: '全选文本文件' }))

    expect(choose).toHaveBeenCalledWith('text')
  })

  it('cycles menu focus with arrow keys and activates with Enter', () => {
    const choose = vi.fn()
    render(<ChoiceHarness open onChoose={choose} />)
    const menu = screen.getByRole('menu', { name: '选择全选范围' })

    fireEvent.keyDown(menu, { key: 'ArrowDown' })
    expect(screen.getByRole('menuitem', { name: '全选文本文件' })).toHaveFocus()
    fireEvent.keyDown(menu, { key: 'ArrowUp' })
    expect(screen.getByRole('menuitem', { name: '全选图片' })).toHaveFocus()
    fireEvent.keyDown(menu, { key: 'ArrowUp' })
    expect(screen.getByRole('menuitem', { name: '全部选择' })).toHaveFocus()
    fireEvent.keyDown(menu, { key: 'Enter' })

    expect(choose).toHaveBeenCalledWith('all')
  })

  it('moves to endpoints with Home and End and activates with Space', () => {
    const choose = vi.fn()
    render(<ChoiceHarness open onChoose={choose} />)
    const menu = screen.getByRole('menu', { name: '选择全选范围' })

    fireEvent.keyDown(menu, { key: 'End' })
    expect(screen.getByRole('menuitem', { name: '全部选择' })).toHaveFocus()
    fireEvent.keyDown(menu, { key: 'Home' })
    expect(screen.getByRole('menuitem', { name: '全选图片' })).toHaveFocus()
    fireEvent.keyDown(menu, { key: ' ' })

    expect(choose).toHaveBeenCalledWith('images')
  })

  it('keeps every command in the natural Tab order without trapping Tab', () => {
    render(<ChoiceHarness open />)
    const items = screen.getAllByRole('menuitem')

    expect(items).toHaveLength(3)
    expect(items.every((item) => item.getAttribute('tabindex') !== '-1')).toBe(true)
    const tab = createEvent.keyDown(items[0] as HTMLElement, { key: 'Tab' })
    fireEvent(items[0] as HTMLElement, tab)

    expect(tab.defaultPrevented).toBe(false)
  })

  it('cancels with Escape, preserves selection ownership, and restores source focus', () => {
    const choose = vi.fn()
    const cancel = vi.fn()
    render(<ChoiceHarness open onChoose={choose} onCancel={cancel} />)

    fireEvent.keyDown(screen.getByRole('menu', { name: '选择全选范围' }), {
      key: 'Escape',
    })

    expect(cancel).toHaveBeenCalledOnce()
    expect(choose).not.toHaveBeenCalled()
    expect(screen.getByRole('button', { name: '全选当前文件夹' })).toHaveFocus()
  })

  it('cancels when the source is reactivated and restores source focus', () => {
    const choose = vi.fn()
    const cancel = vi.fn()
    render(<ChoiceHarness open onChoose={choose} onCancel={cancel} />)

    fireEvent.click(screen.getByRole('button', { name: '全选当前文件夹' }))

    expect(cancel).toHaveBeenCalledOnce()
    expect(choose).not.toHaveBeenCalled()
    expect(screen.getByRole('button', { name: '全选当前文件夹' })).toHaveFocus()
  })

  it('cancels on an outside pointer and restores source focus', () => {
    const choose = vi.fn()
    const cancel = vi.fn()
    render(<ChoiceHarness open onChoose={choose} onCancel={cancel} />)

    fireEvent.pointerDown(screen.getByRole('button', { name: '外部目标' }))

    expect(cancel).toHaveBeenCalledOnce()
    expect(choose).not.toHaveBeenCalled()
    expect(screen.getByRole('button', { name: '全选当前文件夹' })).toHaveFocus()
  })
})
