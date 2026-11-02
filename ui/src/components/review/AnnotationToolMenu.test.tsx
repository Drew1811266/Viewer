import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { ImageReviewWorkbenchController } from '../../app/review/useImageReviewWorkbench'
import AnnotationToolbar from './AnnotationToolbar'
import AnnotationToolMenu from './AnnotationToolMenu'

function controller(
  overrides: Partial<ImageReviewWorkbenchController> = {},
): ImageReviewWorkbenchController {
  return {
    protocol: 'legacy',
    tool: 'browse',
    annotationStyle: {
      tool: 'rectangle',
      color: 'red',
      width: 'medium',
      opacityPercent: 100,
      dashed: false,
    },
    styleBusy: false,
    canUndo: false,
    canRedo: false,
    editor: {
      activeTool: 'browse',
      temporarilyPanning: false,
      selectedItemId: null,
      phase: { status: 'idle' },
    },
    dirty: false,
    feedback: [],
    selectedItemId: null,
    redrawItemId: null,
    railOpen: true,
    readOnlyReason: null,
    restorableItemId: null,
    statusMessage: null,
    preparedImage: null,
    leaveConfirmation: null,
    setTool: vi.fn(),
    setAnnotationStyle: vi.fn(),
    undo: vi.fn(),
    redo: vi.fn(),
    setTemporaryPan: vi.fn(),
    beginAnnotation: vi.fn(),
    beginDrawing: vi.fn(() => true),
    finishDrawing: vi.fn(async () => undefined),
    beginFeedbackTextEdit: vi.fn(),
    beginRedraw: vi.fn(),
    stageFeedbackAnchor: vi.fn(() => true),
    updateDraftAnchor: vi.fn(),
    updateDraftText: vi.fn(),
    saveDraft: vi.fn(async () => undefined),
    cancelDraft: vi.fn(),
    selectFeedback: vi.fn(),
    replaceFeedbackAnchor: vi.fn(async () => undefined),
    deleteFeedback: vi.fn(async () => undefined),
    restoreDeletedFeedback: vi.fn(async () => undefined),
    setRailOpen: vi.fn(),
    requestLeave: vi.fn(async () => 'proceeded' as const),
    cancelLeave: vi.fn(),
    discardUnsavedAndProceed: vi.fn(async () => undefined),
    ...overrides,
  }
}

describe('AnnotationToolMenu', () => {
  it('presents every markup tool in a fixed order and selects one from a single trigger', () => {
    const select = vi.fn()
    render(<AnnotationToolMenu activeTool="rectangle" onSelect={select} />)

    const trigger = screen.getByRole('button', { name: '标记' })
    fireEvent.click(trigger)
    expect(screen.getByRole('menu')).toHaveAttribute('data-native-input-exclusion', 'true')
    const items = screen.getAllByRole('menuitemradio')
    expect(items.map((item) => item.textContent)).toEqual([
      expect.stringContaining('选择'),
      expect.stringContaining('点'),
      expect.stringContaining('箭头'),
      expect.stringContaining('直线'),
      expect.stringContaining('画笔'),
      expect.stringContaining('荧光笔'),
      expect.stringContaining('矩形'),
      expect.stringContaining('椭圆'),
      expect.stringContaining('多边形'),
      expect.stringContaining('文字'),
      expect.stringContaining('标尺'),
    ])
    expect(screen.getByRole('menuitemradio', { name: /矩形/ })).toHaveAttribute(
      'aria-checked',
      'true',
    )

    fireEvent.click(screen.getByRole('menuitemradio', { name: /椭圆/ }))
    expect(select).toHaveBeenCalledWith('ellipse')
    expect(screen.queryByRole('menu')).not.toBeInTheDocument()
    expect(trigger).toHaveFocus()
  })

  it('supports roving focus, closes on Escape, and dismisses outside without changing tools', () => {
    const select = vi.fn()
    const escapeFromParent = vi.fn()
    render(
      <div onKeyDown={(event) => event.key === 'Escape' && escapeFromParent()}>
        <AnnotationToolMenu activeTool="arrow" onSelect={select} />
        <button type="button">菜单外</button>
      </div>,
    )

    const trigger = screen.getByRole('button', { name: '标记' })
    fireEvent.click(trigger)
    expect(screen.getByRole('menuitemradio', { name: /箭头/ })).toHaveFocus()
    fireEvent.keyDown(screen.getByRole('menu'), { key: 'ArrowDown' })
    expect(screen.getByRole('menuitemradio', { name: /矩形/ })).toHaveFocus()
    fireEvent.keyDown(screen.getByRole('menu'), { key: 'ArrowRight' })
    expect(screen.getByRole('menuitemradio', { name: /椭圆/ })).toHaveFocus()
    fireEvent.keyDown(screen.getByRole('menu'), { key: 'End' })
    expect(screen.getByRole('menuitemradio', { name: /标尺/ })).toHaveFocus()
    fireEvent.keyDown(screen.getByRole('menu'), { key: 'Escape' })
    expect(screen.queryByRole('menu')).not.toBeInTheDocument()
    expect(trigger).toHaveFocus()
    expect(escapeFromParent).not.toHaveBeenCalled()

    fireEvent.click(trigger)
    fireEvent.pointerDown(screen.getByRole('button', { name: '菜单外' }))
    expect(screen.queryByRole('menu')).not.toBeInTheDocument()
    expect(select).not.toHaveBeenCalled()
  })

  it('keeps tools discoverable in read-only mode while preventing selection', () => {
    const select = vi.fn()
    render(
      <AnnotationToolMenu
        activeTool="browse"
        disabledReason="当前素材版本不可写"
        onSelect={select}
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: '标记' }))
    const ellipse = screen.getByRole('menuitemradio', { name: /椭圆/ })
    expect(ellipse).toHaveAttribute('aria-disabled', 'true')
    fireEvent.click(ellipse)
    expect(select).not.toHaveBeenCalled()
    expect(screen.getByRole('menu')).toBeVisible()
  })
})

describe('AnnotationToolbar shortcuts', () => {
  it('routes P/A/B/R/O/V while suppressing text entry, modifiers, and IME composition', () => {
    const review = controller()
    render(
      <div>
        <AnnotationToolbar controller={review} />
        <input aria-label="输入框" />
        <textarea aria-label="文本框" />
        <select aria-label="选择框" />
        <div aria-label="可编辑区域" contentEditable />
      </div>,
    )

    for (const [key, tool] of [
      ['p', 'point'],
      ['A', 'arrow'],
      ['b', 'brush'],
      ['R', 'rectangle'],
      ['o', 'ellipse'],
      ['v', 'browse'],
    ] as const) {
      fireEvent.keyDown(window, { key })
      expect(review.setTool).toHaveBeenLastCalledWith(tool)
    }

    vi.mocked(review.setTool).mockClear()
    fireEvent.keyDown(screen.getByRole('textbox', { name: '输入框' }), { key: 'p' })
    fireEvent.keyDown(screen.getByRole('textbox', { name: '文本框' }), { key: 'a' })
    fireEvent.keyDown(screen.getByRole('combobox', { name: '选择框' }), { key: 'b' })
    const editable = screen.getByLabelText('可编辑区域')
    Object.defineProperty(editable, 'isContentEditable', { configurable: true, value: true })
    fireEvent.keyDown(editable, { key: 'r' })
    fireEvent.keyDown(window, { key: 'o', metaKey: true })
    fireEvent.keyDown(window, { key: 'o', ctrlKey: true })
    fireEvent.keyDown(window, { key: 'o', altKey: true })
    fireEvent.keyDown(window, { key: 'o', shiftKey: true })
    fireEvent.keyDown(window, { key: 'o', isComposing: true })
    expect(review.setTool).not.toHaveBeenCalled()
  })

  it('routes T/L/H/G/M only when continuous review offers the extended toolset', () => {
    const legacy = controller({ protocol: 'legacy' })
    const legacyRender = render(<AnnotationToolbar controller={legacy} />)
    for (const key of ['t', 'l', 'h', 'g', 'm']) fireEvent.keyDown(window, { key })
    expect(legacy.setTool).not.toHaveBeenCalled()
    legacyRender.unmount()

    const continuous = controller({ protocol: 'continuous' })
    render(<AnnotationToolbar controller={continuous} />)
    for (const [key, tool] of [
      ['t', 'text'],
      ['l', 'line'],
      ['h', 'highlighter'],
      ['g', 'polygon'],
      ['m', 'ruler'],
    ] as const) {
      fireEvent.keyDown(window, { key })
      expect(continuous.setTool).toHaveBeenLastCalledWith(tool)
    }
  })

  it('deletes a selected annotation with Delete or Backspace but not while typing', () => {
    const review = controller({ selectedItemId: 'target-1' })
    render(
      <div>
        <AnnotationToolbar controller={review} />
        <input aria-label="输入框" />
      </div>,
    )

    fireEvent.keyDown(window, { key: 'Delete' })
    expect(review.deleteFeedback).toHaveBeenCalledExactlyOnceWith('target-1')
    fireEvent.keyDown(window, { key: 'Backspace' })
    expect(review.deleteFeedback).toHaveBeenCalledTimes(2)
    fireEvent.keyDown(screen.getByRole('textbox', { name: '输入框' }), { key: 'Backspace' })
    expect(review.deleteFeedback).toHaveBeenCalledTimes(2)
  })
  it('routes undo and redo commands without hijacking text entry', () => {
    const review = controller({ protocol: 'continuous' })
    render(
      <div>
        <AnnotationToolbar controller={review} />
        <input aria-label="输入框" />
      </div>,
    )

    fireEvent.keyDown(window, { key: 'z', metaKey: true })
    expect(review.undo).toHaveBeenCalledOnce()
    fireEvent.keyDown(window, { key: 'z', ctrlKey: true, shiftKey: true })
    expect(review.redo).toHaveBeenCalledTimes(1)
    fireEvent.keyDown(window, { key: 'y', ctrlKey: true })
    expect(review.redo).toHaveBeenCalledTimes(2)
    fireEvent.keyDown(screen.getByRole('textbox', { name: '输入框' }), {
      key: 'z',
      metaKey: true,
    })
    expect(review.undo).toHaveBeenCalledOnce()
  })
})
