import { act, renderHook } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { sourcePointAtStagePoint } from './imageGeometry'
import { useImageViewport } from './useImageViewport'

const STAGE = { width: 500, height: 400 }
const SOURCE = { width: 1000, height: 800 }

describe('useImageViewport', () => {
  it('derives fit geometry from the current stage instead of retaining an earlier measurement', () => {
    const hook = renderHook(
      ({ stage, source }) => useImageViewport({ stage, source, fitInset: 0.9 }),
      {
        initialProps: {
          stage: { width: 625, height: 416 },
          source: { width: 1_500, height: 1_000 },
        },
      },
    )
    expect(hook.result.current.scale * 1_500).toBeCloseTo(561.6)
    expect(hook.result.current.scale * 1_000).toBeCloseTo(374.4)

    hook.rerender({
      stage: { width: 960, height: 600 },
      source: { width: 6_300, height: 4_200 },
    })

    expect(hook.result.current.geometry.stage).toEqual({ width: 960, height: 600 })
    expect(hook.result.current.scale * 6_300).toBeCloseTo(810)
    expect(hook.result.current.scale * 4_200).toBeCloseTo(540)
  })

  it('uses fit as the sole reset mode and resets the viewport for a new entity', () => {
    const hook = renderHook(() => useImageViewport({ stage: STAGE, source: SOURCE, fitInset: 0.9 }))
    expect(hook.result.current).not.toHaveProperty('setOriginal')

    act(() => hook.result.current.zoomBy(2, { x: 400, y: 300 }))
    expect(hook.result.current.state.mode).toBe('free')
    expect(hook.result.current.state.zoom).toBe(2)

    act(() => hook.result.current.setFit())
    expect(hook.result.current.state).toEqual({
      mode: 'fit',
      zoom: 1,
      rotation: 0,
      offset: { x: 0, y: 0 },
    })

    act(() => {
      hook.result.current.panBy({ x: 50, y: 50 })
      hook.result.current.rotateClockwise()
      hook.result.current.zoomBy(2, { x: 400, y: 300 })
      hook.result.current.resetForEntity()
    })
    expect(hook.result.current.state).toEqual({
      mode: 'fit',
      zoom: 1,
      rotation: 0,
      offset: { x: 0, y: 0 },
    })
  })

  it('preserves an off-center source anchor and exposes the derived CSS transform', () => {
    const hook = renderHook(() => useImageViewport({ stage: STAGE, source: SOURCE, fitInset: 0.9 }))
    const anchor = { x: 400, y: 300 }
    const sourceBefore = sourcePointAtStagePoint(
      anchor,
      hook.result.current.state,
      hook.result.current.geometry,
    )

    act(() => hook.result.current.zoomBy(2, anchor))

    expect(hook.result.current.state.mode).toBe('free')
    expect(hook.result.current.state.zoom).toBe(2)
    expect(
      sourcePointAtStagePoint(anchor, hook.result.current.state, hook.result.current.geometry),
    ).toEqual(
      expect.objectContaining({
        x: expect.closeTo(sourceBefore?.x ?? 0, 6),
        y: expect.closeTo(sourceBefore?.y ?? 0, 6),
      }),
    )
    expect(hook.result.current.transform).toBe('translate(-150px, -100px) rotate(0deg) scale(0.9)')
  })

  it('uses one bounded pan action for deltas, rotation, and measurement changes', () => {
    const hook = renderHook(
      ({ stage, source }) => useImageViewport({ stage, source, fitInset: 0.9 }),
      {
        initialProps: { stage: STAGE, source: SOURCE },
      },
    )

    act(() => hook.result.current.zoomBy(2, { x: 250, y: 200 }))
    act(() => hook.result.current.panBy({ x: 200, y: 150 }))
    act(() => hook.result.current.panBy({ x: 100, y: 100 }))
    expect(hook.result.current.state.offset).toEqual({ x: 200, y: 160 })

    act(() => hook.result.current.rotateClockwise())
    expect(hook.result.current.state.rotation).toBe(90)
    expect(hook.result.current.state.offset).toEqual({ x: 38, y: 160 })

    act(() => hook.result.current.rotateClockwise())
    act(() => hook.result.current.rotateClockwise())
    act(() => hook.result.current.rotateClockwise())
    expect(hook.result.current.state.rotation).toBe(0)

    hook.rerender({ stage: { width: 900, height: 700 }, source: SOURCE })
    expect(hook.result.current.geometry.stage).toEqual({ width: 900, height: 700 })
    expect(hook.result.current.state.offset).toEqual({ x: 38, y: 160 })

    hook.rerender({ stage: STAGE, source: { width: 0, height: 0 } })
    expect(hook.result.current.state.offset).toEqual({ x: 0, y: 0 })
  })
})
