import { describe, expect, it } from 'vitest'
import { defined } from '../defined'
import {
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

const twentyImages = Array.from({ length: 20 }, (_, index) => ({
  entityId: `image-${index}`,
  kind: 'jpeg' as const,
}))

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
  it('accepts exactly two to twenty unique image entities', () => {
    expect(createCompareState(images.slice(0, 1))).toEqual({
      ok: false,
      reason: 'invalid_cardinality',
    })
    expect(createCompareState([...twentyImages, { entityId: 'image-20', kind: 'jpeg' }])).toEqual({
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

    const created = createCompareState(twentyImages)
    expect(created.ok).toBe(true)
    if (!created.ok) return
    expect(created.state.mode).toBe('synchronized')
    expect(created.state.transforms['image-0']).toEqual({
      scale: 1,
      centerX: 0.5,
      centerY: 0.5,
      rotation: 0,
    })

    const withUnsupported = createCompareState([
      defined(images[0], 'Expected first comparison image'),
      { entityId: 'raw', kind: 'unsupported_image' },
    ])
    expect(withUnsupported.ok).toBe(true)
    if (!withUnsupported.ok) return
    expect(withUnsupported.state.entityIds).toEqual(['a', 'raw'])
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

  it('preserves surviving transforms and exits to preview or grid', () => {
    const created = createCompareState(twentyImages)
    if (!created.ok) throw new Error('fixture must be valid')
    let state = reduceCompare(created.state, { type: 'mode_changed', mode: 'independent' })
    state = reduceCompare(state, { type: 'zoom', entityId: 'image-10', factor: 2 })

    const survivors = twentyImages
      .map(({ entityId }) => entityId)
      .filter((entityId) => !['image-0', 'image-5', 'image-15'].includes(entityId))
    const remaining = reconcileComparePanes(state, survivors)
    expect(remaining.kind).toBe('compare')
    if (remaining.kind !== 'compare') return
    expect(remaining.state.entityIds).toEqual(survivors)
    expect(
      defined(remaining.state.transforms['image-10'], 'Expected retained transform for image 10')
        .scale,
    ).toBe(2)

    const one = reconcileComparePanes(remaining.state, ['image-10'])
    expect(one).toEqual({ kind: 'single_preview', entityId: 'image-10' })
    expect(reconcileComparePanes(remaining.state, [])).toEqual({ kind: 'grid' })
  })

  it('chooses the nearest surviving active pane with next before previous on a tie', () => {
    const created = createCompareState(twentyImages.slice(0, 5))
    if (!created.ok) throw new Error('fixture must be valid')
    const centered = reduceCompare(created.state, {
      type: 'active_changed',
      entityId: 'image-2',
    })

    const tied = reconcileComparePanes(centered, ['image-0', 'image-1', 'image-3', 'image-4'])
    expect(tied.kind).toBe('compare')
    if (tied.kind !== 'compare') return
    expect(tied.state.activeEntityId).toBe('image-3')

    const nearEnd = reduceCompare(created.state, {
      type: 'active_changed',
      entityId: 'image-4',
    })
    const preceding = reconcileComparePanes(nearEnd, ['image-0', 'image-1'])
    expect(preceding.kind).toBe('compare')
    if (preceding.kind !== 'compare') return
    expect(preceding.state.activeEntityId).toBe('image-1')
  })
})
