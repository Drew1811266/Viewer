import { fireEvent, render, screen } from '@testing-library/react'
import { useRef } from 'react'
import { describe, expect, it, vi } from 'vitest'
import { useVideoShortcuts } from './useVideoShortcuts'

describe('useVideoShortcuts', () => {
  it('owns the approved shortcuts and prevents Space from scrolling', () => {
    const actions = shortcutActions()
    render(<ShortcutHarness {...actions} playing fullscreen={false} />)

    const space = new KeyboardEvent('keydown', { key: ' ', bubbles: true, cancelable: true })
    window.dispatchEvent(space)
    fireEvent.keyDown(window, { key: 'ArrowLeft' })
    fireEvent.keyDown(window, { key: 'ArrowRight' })
    fireEvent.keyDown(window, { key: 'm' })
    fireEvent.keyDown(window, { key: 'F' })

    expect(space.defaultPrevented).toBe(true)
    expect(actions.onTogglePlayback).toHaveBeenCalledOnce()
    expect(actions.onStep.mock.calls).toEqual([['backward'], ['forward']])
    expect(actions.onToggleMuted).toHaveBeenCalledOnce()
    expect(actions.onToggleFullscreen).toHaveBeenCalledOnce()
    expect(actions.onOwnedShortcut).toHaveBeenCalledTimes(5)
  })

  it('ignores text controls, contenteditable targets, and unrelated dialogs', () => {
    const actions = shortcutActions()
    render(<ShortcutHarness {...actions} playing={false} fullscreen={false} />)

    fireEvent.keyDown(screen.getByRole('textbox', { name: '标题' }), { key: ' ' })
    fireEvent.keyDown(screen.getByRole('combobox', { name: '选项' }), { key: 'm' })
    fireEvent.keyDown(screen.getByTestId('editable'), { key: 'ArrowRight' })
    fireEvent.keyDown(screen.getByRole('dialog', { name: '其他对话框' }), { key: 'f' })

    expect(actions.onTogglePlayback).not.toHaveBeenCalled()
    expect(actions.onStep).not.toHaveBeenCalled()
    expect(actions.onToggleMuted).not.toHaveBeenCalled()
    expect(actions.onToggleFullscreen).not.toHaveBeenCalled()
    expect(actions.onOwnedShortcut).not.toHaveBeenCalled()
  })

  it('does not capture Command, Control, Option, or Shift key combinations', () => {
    const actions = shortcutActions()
    render(<ShortcutHarness {...actions} playing fullscreen={false} />)

    fireEvent.keyDown(window, { key: 'f', metaKey: true })
    fireEvent.keyDown(window, { key: 'm', ctrlKey: true })
    fireEvent.keyDown(window, { key: 'ArrowRight', altKey: true })
    fireEvent.keyDown(window, { key: 'M', shiftKey: true })

    expect(actions.onToggleFullscreen).not.toHaveBeenCalled()
    expect(actions.onToggleMuted).not.toHaveBeenCalled()
    expect(actions.onStep).not.toHaveBeenCalled()
    expect(actions.onOwnedShortcut).not.toHaveBeenCalled()
  })

  it('exits fullscreen before allowing a subsequent Escape to close preview', () => {
    const actions = shortcutActions()
    const rendered = render(<ShortcutHarness {...actions} playing fullscreen />)

    fireEvent.keyDown(window, { key: 'Escape' })
    expect(actions.onExitFullscreen).toHaveBeenCalledOnce()
    expect(actions.onClose).not.toHaveBeenCalled()

    rendered.rerender(<ShortcutHarness {...actions} playing fullscreen={false} />)
    fireEvent.keyDown(window, { key: 'Escape' })
    expect(actions.onExitFullscreen).toHaveBeenCalledOnce()
    expect(actions.onClose).toHaveBeenCalledOnce()
  })
})

function ShortcutHarness({
  playing,
  fullscreen,
  ...actions
}: ReturnType<typeof shortcutActions> & { playing: boolean; fullscreen: boolean }) {
  const root = useRef<HTMLElement>(null)
  useVideoShortcuts({ active: true, root, playing, fullscreen, ...actions })
  return (
    <>
      <section ref={root} role="dialog" aria-label="视频预览">
        <input aria-label="标题" />
        <select aria-label="选项" defaultValue="a">
          <option value="a">A</option>
        </select>
        <div data-testid="editable" contentEditable suppressContentEditableWarning>
          可编辑
        </div>
      </section>
      <section role="dialog" aria-label="其他对话框" tabIndex={-1} />
    </>
  )
}

function shortcutActions() {
  return {
    onTogglePlayback: vi.fn(),
    onStep: vi.fn(),
    onToggleMuted: vi.fn(),
    onToggleFullscreen: vi.fn(),
    onExitFullscreen: vi.fn(),
    onClose: vi.fn(),
    onOwnedShortcut: vi.fn(),
  }
}
