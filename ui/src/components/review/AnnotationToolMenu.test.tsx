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
    const items = screen.getAllByRole('menuitemradio')
    expect(items.map((item) => item.textContent)).toEqual([
      expect.stringContaining('点'),
      expect.stringContaining('箭头'),
      expect.stringContaining('画笔'),
      expect.stringContaining('矩形'),
      expect.stringContaining('椭圆'),
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
    render(
      <div>
        <AnnotationToolMenu activeTool="arrow" onSelect={select} />
        <button type="button">菜单外</button>
      </div>,
    )

    const trigger = screen.getByRole('button', { name: '标记' })
    fireEvent.click(trigger)
    expect(screen.getByRole('menuitemradio', { name: /箭头/ })).toHaveFocus()
    fireEvent.keyDown(screen.getByRole('menu'), { key: 'ArrowDown' })
    expect(screen.getByRole('menuitemradio', { name: /画笔/ })).toHaveFocus()
    fireEvent.keyDown(screen.getByRole('menu'), { key: 'End' })
    expect(screen.getByRole('menuitemradio', { name: /椭圆/ })).toHaveFocus()
    fireEvent.keyDown(screen.getByRole('menu'), { key: 'Escape' })
    expect(screen.queryByRole('menu')).not.toBeInTheDocument()
    expect(trigger).toHaveFocus()

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
})
