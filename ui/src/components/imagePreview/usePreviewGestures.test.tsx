import { fireEvent, render, screen } from '@testing-library/react'
import { useRef } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { Point } from './imageGeometry'
import { normalizeWheelDelta, usePreviewGestures } from './usePreviewGestures'

interface Actions {
  panBy: (delta: Point) => void
  zoomBy: (factor: number, anchor: Point) => void
}

function Harness({
  actions,
  disabled = false,
  bounds = { x: 100, y: 100 },
}: {
  actions: Actions
  disabled?: boolean
  bounds?: Point
}) {
  const stage = useRef<HTMLDivElement>(null)
  const pointer = usePreviewGestures({ stage, disabled, panBounds: bounds, ...actions })
  return (
    <div ref={stage} data-testid="stage" {...pointer}>
      <button type="button" role="toolbar" data-testid="toolbar">
        工具
      </button>
    </div>
  )
}

describe('normalizeWheelDelta', () => {
  it.each([
    [WheelEvent.DOM_DELTA_PIXEL, 2, -3, { x: 2, y: -3 }],
    [WheelEvent.DOM_DELTA_LINE, 2, -3, { x: 32, y: -48 }],
    [WheelEvent.DOM_DELTA_PAGE, 2, -3, { x: 800, y: -1200 }],
  ])('normalizes delta mode %d', (deltaMode, deltaX, deltaY, expected) => {
    const event = new WheelEvent('wheel', { deltaMode, deltaX, deltaY })
    expect(normalizeWheelDelta(event, 400)).toEqual(expected)
  })
})

describe('usePreviewGestures', () => {
  let frames: FrameRequestCallback[]
  let canceled: number[]
  let nextFrame: number

  beforeEach(() => {
    frames = []
    canceled = []
    nextFrame = 0
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
      frames.push(callback)
      nextFrame += 1
      return nextFrame
    })
    vi.stubGlobal('cancelAnimationFrame', (frame: number) => canceled.push(frame))
  })

  afterEach(() => vi.unstubAllGlobals())

  it('owns pinch and pan events only on the enabled image stage', () => {
    const actions = { panBy: vi.fn(), zoomBy: vi.fn() }
    render(<Harness actions={actions} />)
    const stage = screen.getByTestId('stage')
    vi.spyOn(stage, 'getBoundingClientRect').mockReturnValue({
      x: 100,
      y: 50,
      left: 100,
      top: 50,
      right: 600,
      bottom: 450,
      width: 500,
      height: 400,
      toJSON: () => undefined,
    })

    const pinch = new WheelEvent('wheel', {
      bubbles: true,
      cancelable: true,
      ctrlKey: true,
      clientX: 320,
      clientY: 180,
      deltaY: -20,
      deltaMode: WheelEvent.DOM_DELTA_PIXEL,
    })
    stage.dispatchEvent(pinch)
    expect(pinch.defaultPrevented).toBe(true)
    expect(actions.zoomBy).not.toHaveBeenCalled()
    frames.shift()?.(0)
    expect(actions.zoomBy).toHaveBeenCalledWith(Math.exp(0.08), { x: 220, y: 130 })

    const pan = new WheelEvent('wheel', {
      bubbles: true,
      cancelable: true,
      deltaX: 12,
      deltaY: -18,
    })
    stage.dispatchEvent(pan)
    expect(pan.defaultPrevented).toBe(true)
    frames.shift()?.(16)
    expect(actions.panBy).toHaveBeenCalledWith({ x: -12, y: 18 })

    const toolbarWheel = new WheelEvent('wheel', { bubbles: true, cancelable: true, deltaY: 10 })
    screen.getByTestId('toolbar').dispatchEvent(toolbarWheel)
    expect(toolbarWheel.defaultPrevented).toBe(false)

    const outside = document.createElement('div')
    document.body.append(outside)
    const outsideWheel = new WheelEvent('wheel', { bubbles: true, cancelable: true, deltaY: 10 })
    outside.dispatchEvent(outsideWheel)
    expect(outsideWheel.defaultPrevented).toBe(false)
    outside.remove()
  })

  it('preserves fine pinch deltas while coalescing them into one responsive frame', () => {
    const actions = { panBy: vi.fn(), zoomBy: vi.fn() }
    render(<Harness actions={actions} />)
    const stage = screen.getByTestId('stage')
    vi.spyOn(stage, 'getBoundingClientRect').mockReturnValue({
      x: 100,
      y: 50,
      left: 100,
      top: 50,
      right: 600,
      bottom: 450,
      width: 500,
      height: 400,
      toJSON: () => undefined,
    })

    for (const deltaY of [-2, -3, -5]) {
      stage.dispatchEvent(
        new WheelEvent('wheel', {
          bubbles: true,
          cancelable: true,
          ctrlKey: true,
          clientX: 320,
          clientY: 180,
          deltaY,
        }),
      )
    }

    expect(frames).toHaveLength(1)
    frames.shift()?.(16)
    expect(actions.zoomBy).toHaveBeenCalledTimes(1)
    expect(actions.zoomBy).toHaveBeenCalledWith(Math.exp(0.04), { x: 220, y: 130 })
  })

  it('coalesces wheel work into one frame and cancels pending work on unmount', () => {
    const actions = { panBy: vi.fn(), zoomBy: vi.fn() }
    const view = render(<Harness actions={actions} />)
    const stage = screen.getByTestId('stage')

    for (const [deltaX, deltaY] of [
      [3, 4],
      [5, -2],
      [-1, 6],
    ]) {
      stage.dispatchEvent(
        new WheelEvent('wheel', { bubbles: true, cancelable: true, deltaX, deltaY }),
      )
    }
    expect(frames).toHaveLength(1)
    frames.shift()?.(0)
    expect(actions.panBy).toHaveBeenCalledTimes(1)
    expect(actions.panBy).toHaveBeenCalledWith({ x: -7, y: -8 })

    stage.dispatchEvent(
      new WheelEvent('wheel', { bubbles: true, cancelable: true, ctrlKey: true, deltaY: -5 }),
    )
    stage.dispatchEvent(
      new WheelEvent('wheel', { bubbles: true, cancelable: true, ctrlKey: true, deltaY: -15 }),
    )
    expect(frames).toHaveLength(1)
    view.unmount()
    expect(canceled).toEqual([2])
    frames.shift()?.(16)
    expect(actions.zoomBy).not.toHaveBeenCalled()
  })

  it('shares the bounded pan action with primary-button pointer dragging', () => {
    const actions = { panBy: vi.fn(), zoomBy: vi.fn() }
    render(<Harness actions={actions} />)
    const stage = screen.getByTestId('stage')
    const setPointerCapture = vi.fn()
    const releasePointerCapture = vi.fn()
    Object.defineProperties(stage, {
      setPointerCapture: { configurable: true, value: setPointerCapture },
      releasePointerCapture: { configurable: true, value: releasePointerCapture },
    })

    fireEvent.pointerDown(stage, { button: 0, pointerId: 7, clientX: 100, clientY: 80 })
    fireEvent.pointerMove(stage, { pointerId: 7, clientX: 126, clientY: 65 })
    expect(setPointerCapture).toHaveBeenCalledWith(7)
    expect(actions.panBy).toHaveBeenCalledWith({ x: 26, y: -15 })
    fireEvent.pointerUp(stage, { pointerId: 7 })
    expect(releasePointerCapture).toHaveBeenCalledWith(7)

    fireEvent.pointerDown(stage, { button: 2, pointerId: 8, clientX: 0, clientY: 0 })
    expect(setPointerCapture).toHaveBeenCalledTimes(1)
  })

  it('ignores disabled, non-cancelable, and unpannable input', () => {
    const actions = { panBy: vi.fn(), zoomBy: vi.fn() }
    const view = render(<Harness actions={actions} disabled />)
    const stage = screen.getByTestId('stage')
    const disabledWheel = new WheelEvent('wheel', { bubbles: true, cancelable: true, deltaY: 10 })
    stage.dispatchEvent(disabledWheel)
    expect(disabledWheel.defaultPrevented).toBe(false)

    view.rerender(<Harness actions={actions} />)
    const fixedWheel = new WheelEvent('wheel', { bubbles: true, cancelable: false, deltaY: 10 })
    stage.dispatchEvent(fixedWheel)
    expect(actions.panBy).not.toHaveBeenCalled()

    view.rerender(<Harness actions={actions} bounds={{ x: 0, y: 0 }} />)
    const setPointerCapture = vi.fn()
    Object.defineProperty(stage, 'setPointerCapture', {
      configurable: true,
      value: setPointerCapture,
    })
    fireEvent.pointerDown(stage, { button: 0, pointerId: 9, clientX: 0, clientY: 0 })
    expect(setPointerCapture).not.toHaveBeenCalled()
  })
})
