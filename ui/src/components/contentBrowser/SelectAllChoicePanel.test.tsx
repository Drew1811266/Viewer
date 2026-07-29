import { act, createEvent, fireEvent, render, screen } from '@testing-library/react'
import { StrictMode, useRef, useState } from 'react'
import { flushSync } from 'react-dom'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { SelectAllScope } from './adaptiveOtherFilePanelModel'
import SelectAllChoicePanel from './SelectAllChoicePanel'

afterEach(() => {
  vi.unstubAllGlobals()
})

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

function SynchronousUnmountHarness({ onCancel }: { onCancel: () => void }) {
  const [panelMounted, setPanelMounted] = useState(true)
  const anchorRef = useRef<HTMLButtonElement | null>(null)

  return (
    <>
      <button ref={anchorRef} type="button">
        全选当前文件夹
      </button>
      {panelMounted && (
        <SelectAllChoicePanel
          open
          anchorRef={anchorRef}
          onChoose={() => undefined}
          onCancel={() => {
            onCancel()
            flushSync(() => setPanelMounted(false))
          }}
        />
      )}
      <button type="button">外部目标</button>
    </>
  )
}

describe('SelectAllChoicePanel', () => {
  it('focuses the first command and chooses one exact scope', () => {
    const choose = vi.fn()
    render(<ChoiceHarness open onChoose={choose} />)

    expect(screen.getByRole('menuitem', { name: '全选图片' })).toHaveFocus()
    fireEvent.click(screen.getByRole('menuitem', { name: '全选其它文件' }))

    expect(choose).toHaveBeenCalledWith('other')
  })

  it('cycles menu focus with arrow keys and activates with Enter', () => {
    const choose = vi.fn()
    render(<ChoiceHarness open onChoose={choose} />)
    const menu = screen.getByRole('menu', { name: '选择全选范围' })

    fireEvent.keyDown(menu, { key: 'ArrowDown' })
    expect(screen.getByRole('menuitem', { name: '全选其它文件' })).toHaveFocus()
    fireEvent.keyDown(menu, { key: 'ArrowUp' })
    expect(screen.getByRole('menuitem', { name: '全选图片' })).toHaveFocus()
    fireEvent.keyDown(menu, { key: 'ArrowUp' })
    expect(screen.getByRole('menuitem', { name: '全部都选' })).toHaveFocus()
    fireEvent.keyDown(menu, { key: 'Enter' })

    expect(choose).toHaveBeenCalledWith('all')
  })

  it('moves to endpoints with Home and End and activates with Space', () => {
    const choose = vi.fn()
    render(<ChoiceHarness open onChoose={choose} />)
    const menu = screen.getByRole('menu', { name: '选择全选范围' })

    fireEvent.keyDown(menu, { key: 'End' })
    expect(screen.getByRole('menuitem', { name: '全部都选' })).toHaveFocus()
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

  it('restores source focus after the outside target receives its default pointer focus', () => {
    const frames = installAnimationFrameQueue()
    const choose = vi.fn()
    const cancel = vi.fn()
    render(<ChoiceHarness open onChoose={choose} onCancel={cancel} />)
    const outside = screen.getByRole('button', { name: '外部目标' })

    fireEvent.pointerDown(outside)
    outside.focus()

    expect(cancel).toHaveBeenCalledOnce()
    expect(choose).not.toHaveBeenCalled()
    expect(outside).toHaveFocus()
    expect(frames.pending()).toBe(1)

    frames.flush()

    expect(screen.getByRole('button', { name: '全选当前文件夹' })).toHaveFocus()
  })

  it('does not treat an anchor pointerdown as an outside cancellation', () => {
    const cancel = vi.fn()
    render(<ChoiceHarness open onCancel={cancel} />)

    fireEvent.pointerDown(screen.getByRole('button', { name: '全选当前文件夹' }))

    expect(cancel).not.toHaveBeenCalled()
    expect(screen.getByRole('menu', { name: '选择全选范围' })).toBeVisible()
  })

  it('does not treat a panel pointerdown as an outside cancellation', () => {
    const cancel = vi.fn()
    render(<ChoiceHarness open onCancel={cancel} />)

    fireEvent.pointerDown(screen.getByRole('menuitem', { name: '全选其它文件' }))

    expect(cancel).not.toHaveBeenCalled()
    expect(screen.getByRole('menu', { name: '选择全选范围' })).toBeVisible()
  })

  it('keeps one effective listener in StrictMode and removes it on unmount', () => {
    const frames = installAnimationFrameQueue()
    const cancel = vi.fn()
    const persistentOutside = document.createElement('button')
    document.body.append(persistentOutside)
    const rendered = render(
      <StrictMode>
        <ChoiceHarness open onCancel={cancel} />
      </StrictMode>,
    )
    const outside = screen.getByRole('button', { name: '外部目标' })

    fireEvent.pointerDown(outside)
    outside.focus()

    expect(cancel).toHaveBeenCalledOnce()
    expect(frames.pending()).toBe(1)

    rendered.unmount()

    expect(frames.pending()).toBe(0)
    fireEvent.pointerDown(persistentOutside)
    expect(cancel).toHaveBeenCalledOnce()
    persistentOutside.remove()
  })

  it('does not focus an anchor that detached before deferred restoration', () => {
    const frames = installAnimationFrameQueue()
    render(<ChoiceHarness open />)
    const anchor = screen.getByRole('button', { name: '全选当前文件夹' })
    const outside = screen.getByRole('button', { name: '外部目标' })

    fireEvent.pointerDown(outside)
    outside.focus()
    expect(frames.pending()).toBe(1)
    anchor.remove()

    frames.flush()

    expect(outside).toHaveFocus()
  })

  it('does not leak a restoration frame when cancellation synchronously unmounts the panel', () => {
    const frames = installAnimationFrameQueue()
    const cancel = vi.fn()
    render(<SynchronousUnmountHarness onCancel={cancel} />)
    const outside = screen.getByRole('button', { name: '外部目标' })

    fireEvent.pointerDown(outside)
    outside.focus()

    expect(cancel).toHaveBeenCalledOnce()
    expect(frames.pending()).toBe(0)
    frames.flush()
    expect(outside).toHaveFocus()
  })
})

function installAnimationFrameQueue() {
  let nextId = 1
  const callbacks = new Map<number, FrameRequestCallback>()
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
    const id = nextId++
    callbacks.set(id, callback)
    return id
  })
  vi.stubGlobal('cancelAnimationFrame', (id: number) => callbacks.delete(id))

  return {
    pending: () => callbacks.size,
    flush: () => {
      act(() => {
        const queued = [...callbacks.values()]
        callbacks.clear()
        queued.forEach((callback) => {
          callback(0)
        })
      })
    },
  }
}
