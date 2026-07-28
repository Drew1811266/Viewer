import { describe, expect, it } from 'vitest'
import {
  anchoredCompareScrollOffset,
  type CompareLayoutInput,
  type CompareLayoutPlan,
  MAX_LAYOUT_CANDIDATES,
  solveCompareLayout,
  visibleCompareIndexes,
} from './compareLayoutEngine'

const items = (count: number, ratio: number) =>
  Array.from({ length: count }, (_, index) => ({
    entityId: `image-${index}`,
    aspectRatio: ratio,
  }))

const input = (
  comparedItems: CompareLayoutInput['items'],
  overrides: Partial<CompareLayoutInput> = {},
): CompareLayoutInput => ({
  width: 1_700,
  height: 900,
  gap: 6,
  padding: 6,
  paneChromeHeight: 70,
  items: comparedItems,
  previous: null,
  ...overrides,
})

describe('solveCompareLayout', () => {
  it('puts four portrait images in one readable row', () => {
    const plan = solveCompareLayout(input(items(4, 0.75)))

    expect(plan.kind).toBe('fit-row')
    expect(plan.scrollAxis).toBe('none')
    expect(plan.rects).toHaveLength(4)
    expect(plan.candidateCount).toBeLessThanOrEqual(MAX_LAYOUT_CANDIDATES)
    expect(plan.rects[0]).toMatchObject({
      entityId: 'image-0',
      index: 0,
      left: 6,
      top: 6,
      width: 417.5,
      height: 888,
      stageWidth: 417.5,
      stageHeight: 818,
    })
    expect(plan.rects[3]?.left).toBe(1_276.5)
  })

  it('uses a readable grid for four landscapes', () => {
    const plan = solveCompareLayout(input(items(4, 1.5)))

    expect(plan.kind).toBe('fit-grid')
    expect(plan.columns).toBe(2)
    expect(plan.rows).toBe(2)
    expect(plan.score).toBeCloseTo(0.25418266842614357)
    expect(plan.rects.map(({ entityId }) => entityId)).toEqual([
      'image-0',
      'image-1',
      'image-2',
      'image-3',
    ])
  })

  it('falls back to a horizontal strip for six portraits', () => {
    const plan = solveCompareLayout(input(items(6, 0.75)))

    expect(plan.kind).toBe('horizontal-strip')
    expect(plan.scrollAxis).toBe('horizontal')
    expect(plan.totalWidth).toBeGreaterThan(1_700)
    expect(plan.rects[0]).toMatchObject({
      left: 6,
      top: 6,
      width: 613.5,
      height: 888,
      stageWidth: 613.5,
      stageHeight: 818,
    })
    expect(plan.totalWidth).toBe(3_723)
  })

  it('uses a two-column vertical flow for overflowing landscapes', () => {
    const plan = solveCompareLayout(input(items(8, 1.5)))

    expect(plan.kind).toBe('vertical-flow')
    expect(plan.columns).toBe(2)
    expect(plan.totalHeight).toBeGreaterThan(900)
    expect(plan.rects[0]).toMatchObject({
      left: 6,
      top: 6,
      width: 841,
      height: 630.6666666666666,
      stageWidth: 841,
      stageHeight: 560.6666666666666,
    })
    expect(plan.rects[1]?.left).toBe(853)
  })

  it('centers the final partial fit row without changing source order', () => {
    const plan = solveCompareLayout(input(items(5, 1.5)))

    expect(plan.kind).toBe('fit-grid')
    expect(plan.columns).toBe(3)
    expect(plan.rects.map(({ entityId }) => entityId)).toEqual([
      'image-0',
      'image-1',
      'image-2',
      'image-3',
      'image-4',
    ])
    expect(plan.rects[3]?.left).toBeCloseTo(288.3333333333333)
    expect(plan.rects[4]?.left).toBeCloseTo(853)
  })

  it('switches only after a twelve-percent score gain', () => {
    const first = solveCompareLayout(input(items(4, 0.75)))
    const retained = solveCompareLayout(
      input(items(4, 0.75), {
        width: 1_660,
        previous: first,
      }),
    )

    expect(retained.key).toBe(first.key)
    expect(retained.retainedPrevious).toBe(true)
  })

  it.each([
    [0.89, 'horizontal-strip'],
    [0.9, 'vertical-flow'],
    [1.1, 'vertical-flow'],
    [1.11, 'vertical-flow'],
  ] as const)(
    'classifies ratio %s at the portrait and landscape boundaries',
    (ratio, expectedKind) => {
      const plan = solveCompareLayout(input(items(20, ratio)))

      expect(plan.kind).toBe(expectedKind)
    },
  )

  it('accepts the exact 120,000-area boundary and rejects just below it', () => {
    const exact = solveCompareLayout(
      input(items(2, 4 / 3), {
        width: 818,
        height: 382,
      }),
    )
    const below = solveCompareLayout(
      input(items(2, 4 / 3), {
        width: 817.998,
        height: 382,
      }),
    )

    expect(exact.kind).toBe('fit-row')
    expect(exact.rects[0]).toMatchObject({
      width: 400,
      stageHeight: 300,
    })
    expect(below.kind).toBe('vertical-flow')
  })

  it('accepts the exact 360-long-edge boundary and rejects just below it', () => {
    const exact = solveCompareLayout(
      input(items(2, 1.05), {
        width: 738,
        height: 432,
      }),
    )
    const below = solveCompareLayout(
      input(items(2, 1.05), {
        width: 737.998,
        height: 432,
      }),
    )

    expect(exact.kind).toBe('fit-row')
    expect(exact.rects[0]).toMatchObject({
      width: 360,
      stageHeight: 350,
    })
    expect(below.kind).toBe('vertical-flow')
  })

  it('uses caller-supplied quarter-rotation-adjusted ratios', () => {
    const portraits = solveCompareLayout(input(items(6, 0.75)))
    const rotated = solveCompareLayout(input(items(6, 1 / 0.75)))

    expect(portraits.kind).toBe('horizontal-strip')
    expect(rotated.kind).toBe('fit-grid')
  })

  it('preserves mixed-ratio source order in the horizontal fallback', () => {
    const mixed = Array.from({ length: 20 }, (_, index) => ({
      entityId: `ordered-${index}`,
      aspectRatio: index % 2 === 0 ? 0.75 : 1.5,
    }))
    const plan = solveCompareLayout(input(mixed))

    expect(plan.kind).toBe('horizontal-strip')
    expect(plan.rects.map(({ entityId }) => entityId)).toEqual(
      mixed.map(({ entityId }) => entityId),
    )
    expect(plan.rects[1]?.left).toBe(625.5)
  })

  it('keeps finite monotonic horizontal offsets when finite operands overflow', () => {
    const plan = solveCompareLayout(
      input(items(6, 0.75), {
        width: Number.MAX_VALUE,
        gap: Number.MAX_VALUE,
      }),
    )
    const offsets = plan.rects.map(({ left }) => left)

    expect(plan.kind).toBe('horizontal-strip')
    expect(offsets.every(Number.isFinite)).toBe(true)
    expect(
      offsets.every((offset, index) => index === 0 || offset >= (offsets[index - 1] ?? 0)),
    ).toBe(true)
    expect(Number.isFinite(plan.totalWidth)).toBe(true)
    expect(plan.totalWidth).toBeGreaterThan(0)
  })

  it('uses a single-column vertical flow below 900 CSS pixels', () => {
    const plan = solveCompareLayout(
      input(items(20, 1.5), {
        width: 899,
      }),
    )

    expect(plan.kind).toBe('vertical-flow')
    expect(plan.columns).toBe(1)
    expect(plan.rects.every(({ left }) => left === 6)).toBe(true)
  })

  it('bounds candidate generation for twenty items', () => {
    const plan = solveCompareLayout(input(items(20, 1)))

    expect(plan.candidateCount).toBeLessThanOrEqual(MAX_LAYOUT_CANDIDATES)
  })

  it('switches immediately when the previous candidate becomes ineligible', () => {
    const previous = solveCompareLayout(input(items(4, 0.75)))
    const plan = solveCompareLayout(
      input(items(4, 1.5), {
        previous: {
          ...previous,
          score: Number.MAX_SAFE_INTEGER,
        },
      }),
    )

    expect(plan.key).not.toBe(previous.key)
    expect(plan.kind).toBe('fit-grid')
    expect(plan.columns).toBe(2)
    expect(plan.retainedPrevious).toBe(false)
  })

  it.each([Number.NaN, Number.POSITIVE_INFINITY, 0, -1])(
    'returns finite safe geometry for invalid dimension %s',
    (width) => {
      const plan = solveCompareLayout(input(items(2, 1), { width }))

      expect(plan.kind).toBe('safe-column')
      expect(Number.isFinite(plan.totalWidth)).toBe(true)
      expect(Number.isFinite(plan.totalHeight)).toBe(true)
      for (const rect of plan.rects) {
        expect(
          Object.entries(rect)
            .filter(([key]) => key !== 'entityId')
            .every(([, value]) => Number.isFinite(value)),
        ).toBe(true)
      }
    },
  )

  it.each([Number.NaN, Number.POSITIVE_INFINITY, 0, -1])(
    'normalizes invalid aspect ratio %s to finite square geometry',
    (aspectRatio) => {
      const plan = solveCompareLayout(
        input([
          { entityId: 'invalid-ratio', aspectRatio },
          { entityId: 'valid-ratio', aspectRatio: 1 },
        ]),
      )

      expect(plan.rects.map(({ entityId }) => entityId)).toEqual(['invalid-ratio', 'valid-ratio'])
      expect(
        plan.rects.every((rect) =>
          [rect.left, rect.top, rect.width, rect.height, rect.stageWidth, rect.stageHeight].every(
            Number.isFinite,
          ),
        ),
      ).toBe(true)
    },
  )
})

const planForAxis = (
  axis: CompareLayoutPlan['scrollAxis'],
  positions: readonly number[],
): CompareLayoutPlan => {
  const horizontal = axis === 'horizontal'
  const extent = (positions.at(-1) ?? 0) + 100

  return {
    key: `${axis}-fixture`,
    kind: horizontal ? 'horizontal-strip' : 'vertical-flow',
    score: 0,
    eligible: true,
    scrollAxis: axis,
    columns: horizontal ? positions.length : 1,
    rows: horizontal ? 1 : positions.length,
    rects: positions.map((position, index) => ({
      entityId: `fixture-${index}`,
      index,
      left: horizontal ? position : 0,
      top: horizontal ? 0 : position,
      width: 100,
      height: 100,
      stageWidth: 100,
      stageHeight: 80,
    })),
    viewportWidth: 100,
    viewportHeight: 100,
    totalWidth: horizontal ? extent : 100,
    totalHeight: horizontal ? 100 : extent,
    candidateCount: 1,
    retainedPrevious: false,
  }
}

describe('visibleCompareIndexes', () => {
  const horizontal = planForAxis('horizontal', [0, 120, 240, 360, 480])

  it.each([
    ['start', 0, [0, 1]],
    ['middle', 240, [1, 2, 3]],
    ['end', 480, [3, 4]],
  ] as const)(
    'returns horizontal items at the %s with one viewport of overscan',
    (_position, scrollLeft, expected) => {
      expect(visibleCompareIndexes(horizontal, scrollLeft, 0)).toEqual(expected)
    },
  )

  it('returns vertical items with one viewport of overscan', () => {
    const vertical = planForAxis('vertical', [0, 150, 300, 450])

    expect(visibleCompareIndexes(vertical, 0, 260)).toEqual([1, 2, 3])
  })

  it('adds at most one retained off-window entity in source order', () => {
    expect(visibleCompareIndexes(horizontal, 480, 0, 'fixture-0')).toEqual([0, 3, 4])
    expect(visibleCompareIndexes(horizontal, 480, 0, 'missing-entity')).toEqual([3, 4])
  })
})

describe('anchoredCompareScrollOffset', () => {
  it('keeps a horizontal entity at the same viewport-relative position', () => {
    const previous = planForAxis('horizontal', [0, 120, 240, 360, 480])
    const next = planForAxis('horizontal', [0, 160, 320, 480, 640])

    expect(anchoredCompareScrollOffset(previous, next, 'fixture-2', 180)).toBe(260)
  })

  it('keeps a vertical entity at the same viewport-relative position', () => {
    const previous = planForAxis('vertical', [0, 150, 300, 450])
    const next = planForAxis('vertical', [0, 200, 400, 600])

    expect(anchoredCompareScrollOffset(previous, next, 'fixture-2', 260)).toBe(360)
  })

  it('clamps the next offset to its scrollable extent', () => {
    const previous = planForAxis('horizontal', [0, 120, 240])
    const next = planForAxis('horizontal', [0, 50, 100])

    expect(anchoredCompareScrollOffset(previous, next, 'fixture-2', 0)).toBe(0)
    expect(anchoredCompareScrollOffset(previous, next, 'fixture-0', 240)).toBe(100)
  })

  it('returns the prior offset when either anchor is missing', () => {
    const previous = planForAxis('horizontal', [0, 120, 240])
    const next = planForAxis('horizontal', [0, 160])

    expect(anchoredCompareScrollOffset(previous, next, 'fixture-2', 73)).toBe(73)
    expect(anchoredCompareScrollOffset(previous, next, 'not-present', 73)).toBe(73)
  })
})
