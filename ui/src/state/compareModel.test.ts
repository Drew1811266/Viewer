import { describe, expect, it } from 'vitest'
import { defined } from '../defined'
import {
  compareLayout,
  createCompareState,
  MAX_COMPARE_SCALE,
  MIN_COMPARE_SCALE,
  reconcileComparePanes,
  reduceCompare,
} from './compareModel'

const images = [
  { entityId: 'a', kind: 'jpeg' as const },
  { entityId: 'b', kind: 'png' as const },
  { entityId: 'c', kind: 'jpeg' as const },
  { entityId: 'd', kind: 'png' as const },
]

const wide = {
  imageWidth: 4_000,
  imageHeight: 2_000,
  viewportWidth: 1_000,
  viewportHeight: 1_000,
}

const tall = {
  imageWidth: 1_000,
  imageHeight: 4_000,
  viewportWidth: 1_000,
  viewportHeight: 1_000,
}

function stateWithMetrics() {
  const created = createCompareState(images.slice(0, 2))
  if (!created.ok) throw new Error('fixture must be valid')
  return reduceCompare(
    reduceCompare(created.state, { type: 'metrics_changed', entityId: 'a', metrics: wide }),
    { type: 'metrics_changed', entityId: 'b', metrics: tall },
  )
}

describe('compareModel', () => {
  it('accepts exactly two to four unique JPG/PNG entities', () => {
    expect(createCompareState(images.slice(0, 1))).toEqual({
      ok: false,
      reason: 'invalid_cardinality',
    })
    expect(createCompareState([...images, { entityId: 'e', kind: 'jpeg' }])).toEqual({
      ok: false,
      reason: 'invalid_cardinality',
    })
    const firstImage = defined(images[0], 'Expected first comparison image')
    expect(createCompareState([firstImage, firstImage])).toEqual({
      ok: false,
      reason: 'duplicate_entity',
    })
    expect(createCompareState([firstImage, { entityId: 'note', kind: 'markdown' }])).toEqual({
      ok: false,
      reason: 'unsupported_type',
    })

    const created = createCompareState(images)
    expect(created.ok).toBe(true)
    if (!created.ok) return
    expect(created.state.mode).toBe('synchronized')
    expect(created.state.transforms.a).toEqual({
      scale: 1,
      centerX: 0.5,
      centerY: 0.5,
      rotation: 0,
    })
  })

  it('selects deterministic layouts for two, three and four panes', () => {
    for (const [count, layout] of [
      [2, 'two_columns'],
      [3, 'three_asymmetric'],
      [4, 'four_grid'],
    ] as const) {
      const created = createCompareState(images.slice(0, count))
      if (!created.ok) throw new Error('fixture must be valid')
      expect(compareLayout(created.state)).toBe(layout)
    }
  })

  it('supports fit, 100 percent and bounded zoom', () => {
    let state = stateWithMetrics()
    state = reduceCompare(state, { type: 'actual_size', entityId: 'a' })
    expect(state.shared.scale).toBe(4)
    expect(defined(state.transforms.a, 'Expected transform for image a').scale).toBe(4)
    expect(defined(state.transforms.b, 'Expected transform for image b').scale).toBe(4)

    state = reduceCompare(state, { type: 'zoom', entityId: 'a', factor: 100 })
    expect(state.shared.scale).toBe(MAX_COMPARE_SCALE)
    state = reduceCompare(state, { type: 'zoom', entityId: 'a', factor: 0.0001 })
    expect(state.shared.scale).toBe(MIN_COMPARE_SCALE)

    state = reduceCompare(state, { type: 'fit', entityId: 'a' })
    expect(state.shared).toEqual({ scale: 1, centerX: 0.5, centerY: 0.5 })
  })

  it('propagates normalized pan in synchronized mode with per-pane clamping', () => {
    let state = stateWithMetrics()
    state = reduceCompare(state, { type: 'zoom', entityId: 'a', factor: 2 })
    state = reduceCompare(state, {
      type: 'pan',
      entityId: 'a',
      deltaX: 0.4,
      deltaY: 0.4,
    })

    expect(state.shared).toEqual({ scale: 2, centerX: 0.9, centerY: 0.9 })
    expect(state.transforms.a).toMatchObject({ centerX: 0.75, centerY: 0.5 })
    expect(state.transforms.b).toMatchObject({ centerX: 0.5, centerY: 0.75 })
  })

  it('retains independent transforms and derives synchronized state from the active pane', () => {
    let state = stateWithMetrics()
    state = reduceCompare(state, { type: 'mode_changed', mode: 'independent' })
    state = reduceCompare(state, { type: 'zoom', entityId: 'a', factor: 2 })
    state = reduceCompare(state, { type: 'active_changed', entityId: 'b' })
    state = reduceCompare(state, { type: 'zoom', entityId: 'b', factor: 3 })
    expect(defined(state.transforms.a, 'Expected transform for image a').scale).toBe(2)
    expect(defined(state.transforms.b, 'Expected transform for image b').scale).toBe(3)

    state = reduceCompare(state, { type: 'mode_changed', mode: 'synchronized' })
    expect(state.shared.scale).toBe(3)
    expect(defined(state.transforms.a, 'Expected transform for image a').scale).toBe(3)
    expect(defined(state.transforms.b, 'Expected transform for image b').scale).toBe(3)
    state = reduceCompare(state, { type: 'mode_changed', mode: 'independent' })
    expect(defined(state.transforms.a, 'Expected transform for image a').scale).toBe(3)
    expect(defined(state.transforms.b, 'Expected transform for image b').scale).toBe(3)
  })

  it('keeps rotation pane-local even while pan and zoom are synchronized', () => {
    let state = stateWithMetrics()
    state = reduceCompare(state, { type: 'rotate_clockwise', entityId: 'a' })
    expect(defined(state.transforms.a, 'Expected transform for image a').rotation).toBe(90)
    expect(defined(state.transforms.b, 'Expected transform for image b').rotation).toBe(0)
    expect(state.shared).toEqual({ scale: 1, centerX: 0.5, centerY: 0.5 })
  })

  it('preserves surviving transforms, reflows, and exits to preview or grid', () => {
    const created = createCompareState(images)
    if (!created.ok) throw new Error('fixture must be valid')
    let state = reduceCompare(created.state, { type: 'mode_changed', mode: 'independent' })
    state = reduceCompare(state, { type: 'zoom', entityId: 'b', factor: 2 })

    const three = reconcileComparePanes(state, ['a', 'b', 'c'])
    expect(three.kind).toBe('compare')
    if (three.kind !== 'compare') return
    expect(compareLayout(three.state)).toBe('three_asymmetric')
    expect(defined(three.state.transforms.b, 'Expected retained transform for image b').scale).toBe(
      2,
    )

    const one = reconcileComparePanes(three.state, ['b'])
    expect(one).toEqual({ kind: 'single_preview', entityId: 'b' })
    expect(reconcileComparePanes(three.state, [])).toEqual({ kind: 'grid' })
  })
})
