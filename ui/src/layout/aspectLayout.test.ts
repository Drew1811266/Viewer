import { describe, expect, it } from 'vitest'
import {
  type AspectGeometry,
  anchoredScrollOffset,
  buildFilmstripGeometry,
  buildFlowGeometry,
  directionalNeighbor,
  horizontalVisibleIndexes,
  intersectingAspectIndexes,
  proportionalWidth,
  validDimensions,
  verticalVisibleRows,
} from './aspectLayout'

const sources = [
  { key: 'portrait', dimensions: { width: 2, height: 3 } },
  { key: 'square', dimensions: { width: 1, height: 1 } },
  { key: 'landscape', dimensions: { width: 3, height: 2 } },
  { key: 'panorama', dimensions: { width: 10, height: 1 } },
  { key: 'extreme-portrait', dimensions: { width: 1, height: 10 } },
] as const

function expectFiniteGeometry(geometry: AspectGeometry) {
  expect(Number.isFinite(geometry.totalWidth)).toBe(true)
  expect(Number.isFinite(geometry.totalHeight)).toBe(true)

  for (const item of geometry.items) {
    for (const value of [
      item.index,
      item.row,
      item.left,
      item.top,
      item.width,
      item.height,
      item.imageWidth,
      item.imageHeight,
      item.left + item.width,
      item.top + item.height,
    ]) {
      expect(Number.isFinite(value)).toBe(true)
    }
    expect(item.left + item.width).toBeLessThanOrEqual(geometry.totalWidth)
    expect(item.top + item.height).toBeLessThanOrEqual(geometry.totalHeight)
  }

  for (const row of geometry.rows) {
    for (const value of [
      row.index,
      row.start,
      row.end,
      row.top,
      row.height,
      row.top + row.height,
    ]) {
      expect(Number.isFinite(value)).toBe(true)
    }
    expect(row.top + row.height).toBeLessThanOrEqual(geometry.totalHeight)
  }
}

describe('aspect layout geometry', () => {
  it('keeps proportional widths for every supported orientation', () => {
    expect(proportionalWidth(132, { width: 2, height: 3 })).toBe(88)
    expect(proportionalWidth(132, { width: 1, height: 1 })).toBe(132)
    expect(proportionalWidth(132, { width: 3, height: 2 })).toBe(198)
    expect(proportionalWidth(132, { width: 10, height: 1 })).toBe(1320)
    expect(proportionalWidth(132, { width: 1, height: 10 })).toBe(13.2)
  })

  it('uses a square skeleton only for invalid or overflowing dimensions', () => {
    const invalid = [
      null,
      { width: 0, height: 1 },
      { width: 1, height: 0 },
      { width: -1, height: 1 },
      { width: 1, height: -1 },
      { width: Number.POSITIVE_INFINITY, height: 1 },
      { width: 1, height: Number.NaN },
      { width: Number.MAX_VALUE, height: Number.MIN_VALUE },
    ]

    for (const dimensions of invalid) {
      expect(validDimensions(dimensions)).toBe(false)
      expect(proportionalWidth(132, dimensions)).toBe(132)
    }
  })

  it('keeps an exact 1:100 portrait proportional and finite', () => {
    expect(proportionalWidth(132, { width: 1, height: 100 })).toBe(1.32)

    const strip = buildFilmstripGeometry(
      [{ key: 'one-to-one-hundred', dimensions: { width: 1, height: 100 } }],
      132,
      8,
      12,
    )

    expect(strip.items[0]).toMatchObject({
      key: 'one-to-one-hundred',
      left: 12,
      width: 1.32,
      imageWidth: 1.32,
      imageHeight: 132,
    })
    expect(strip.totalWidth).toBe(25.32)
    expectFiniteGeometry(strip)
  })

  it('builds a fractional filmstrip without rounding cumulative offsets', () => {
    const strip = buildFilmstripGeometry(
      [
        { key: 'portrait', dimensions: { width: 2, height: 3 } },
        { key: 'landscape', dimensions: { width: 3, height: 2 } },
      ],
      132,
      8,
      12,
    )

    expect(strip.items.map(({ left, width }) => [left, width])).toEqual([
      [12, 88],
      [108, 198],
    ])
    expect(strip.totalWidth).toBe(318)

    const fractional = buildFilmstripGeometry(
      [
        { key: 'first', dimensions: { width: 1, height: 10 } },
        { key: 'second', dimensions: { width: 1, height: 10 } },
      ],
      132,
      1,
      0,
    )
    expect(fractional.items.map((item) => item.left)).toEqual([0, 14.2])
    expect(fractional.totalWidth).toBeCloseTo(27.4)
  })

  it('keeps cumulative filmstrip geometry finite when valid widths would overflow offsets', () => {
    const geometry = buildFilmstripGeometry(
      [
        { key: 'first', dimensions: { width: 1, height: 1 } },
        { key: 'second', dimensions: { width: 1, height: 1 } },
      ],
      Number.MAX_VALUE,
      Number.MAX_VALUE,
      Number.MAX_VALUE,
    )

    expectFiniteGeometry(geometry)
  })

  it('finds filmstrip windows with intersection-aware binary-search boundaries', () => {
    const geometry = buildFilmstripGeometry(sources, 10, 2, 4)

    expect(horizontalVisibleIndexes(geometry, 15, 10, 0)).toEqual({ start: 1, end: 3 })
    expect(horizontalVisibleIndexes(geometry, 15, 10, 8)).toEqual({ start: 0, end: 3 })
    expect(horizontalVisibleIndexes(geometry, 1_000, 20, 0)).toEqual({ start: 5, end: 5 })
    expect(geometry.items[4]).toMatchObject({ key: 'extreme-portrait', index: 4 })
  })

  it('wraps fixed-height flow rows in source order and leaves the final row left aligned', () => {
    const geometry = buildFlowGeometry(sources.slice(0, 3), 31, 10, 4, 2)

    expect(geometry.rows).toEqual([
      { index: 0, start: 0, end: 2, top: 0, height: 14 },
      { index: 1, start: 2, end: 3, top: 16, height: 14 },
    ])
    expect(
      geometry.items.map(({ key, index, row, top, height }) => ({
        key,
        index,
        row,
        top,
        height,
      })),
    ).toEqual([
      { key: 'portrait', index: 0, row: 0, top: 0, height: 14 },
      { key: 'square', index: 1, row: 0, top: 0, height: 14 },
      { key: 'landscape', index: 2, row: 1, top: 16, height: 14 },
    ])
    expect(geometry.items[0]?.left).toBe(0)
    expect(geometry.items[1]?.left).toBeCloseTo(20 / 3 + 2)
    expect(geometry.items[2]?.left).toBe(0)
    expect(geometry.items[0]?.width).toBeCloseTo(20 / 3)
    expect(geometry.items[1]?.width).toBe(10)
    expect(geometry.items[2]?.width).toBe(15)
    expect(geometry.indexByKey).toEqual(
      new Map([
        ['portrait', 0],
        ['square', 1],
        ['landscape', 2],
      ]),
    )
  })

  it('keeps a natural-width item wider than the viewport on its own row', () => {
    const geometry = buildFlowGeometry(
      [
        { key: 'panorama', dimensions: { width: 10, height: 1 } },
        { key: 'portrait', dimensions: { width: 2, height: 3 } },
      ],
      100,
      20,
      0,
      6,
    )

    expect(geometry.items.map(({ key, row, left }) => ({ key, row, left }))).toEqual([
      { key: 'panorama', row: 0, left: 0 },
      { key: 'portrait', row: 1, left: 0 },
    ])
    expect(geometry.items[0]?.width).toBe(200)
    expect(geometry.items[1]?.width).toBeCloseTo(40 / 3)
    expect(geometry.totalWidth).toBe(200)
  })

  it('keeps derived flow row heights, offsets, and extents finite', () => {
    const geometry = buildFlowGeometry(
      [
        { key: 'first', dimensions: { width: 1, height: 1 } },
        { key: 'second', dimensions: { width: 1, height: 1 } },
      ],
      Number.MAX_VALUE,
      Number.MAX_VALUE,
      Number.MAX_VALUE,
      Number.MAX_VALUE,
    )

    expectFiniteGeometry(geometry)
  })

  it('returns vertical row windows with row overscan', () => {
    const geometry = buildFlowGeometry(sources.slice(0, 4), 17, 10, 5, 2)

    expect(geometry.rows).toHaveLength(4)
    expect(verticalVisibleRows(geometry, 17, 12, 1)).toEqual({ start: 0, end: 3 })
    expect(verticalVisibleRows(geometry, 1_000, 20, 2)).toEqual({ start: 4, end: 4 })
  })

  it('hit tests every aspect rectangle including unmounted geometry in source order', () => {
    const geometry = buildFlowGeometry(sources.slice(0, 4), 17, 10, 5, 2)

    expect(
      intersectingAspectIndexes(geometry, {
        left: 0,
        top: 50,
        right: 100,
        bottom: 64,
        width: 100,
        height: 15,
      }),
    ).toEqual([3])
  })

  it('uses source order horizontally and nearest horizontal centers vertically', () => {
    const geometry = buildFlowGeometry(
      [
        { key: 'wide', dimensions: { width: 3, height: 2 } },
        { key: 'narrow', dimensions: { width: 1, height: 2 } },
        { key: 'upper-right', dimensions: { width: 1, height: 1 } },
        { key: 'lower-left', dimensions: { width: 1, height: 1 } },
        { key: 'lower-right', dimensions: { width: 3, height: 2 } },
      ],
      22,
      10,
      0,
      2,
    )

    expect(directionalNeighbor(geometry, 1, 'left')).toBe(0)
    expect(directionalNeighbor(geometry, 1, 'right')).toBe(2)
    expect(directionalNeighbor(geometry, 2, 'right')).toBe(3)
    expect(directionalNeighbor(geometry, 2, 'down')).toBe(4)
    expect(directionalNeighbor(geometry, 4, 'up')).toBe(2)
    expect(directionalNeighbor(geometry, 0, 'up')).toBe(0)
  })

  it('reconstructs the first-visible anchor after density and viewport geometry changes', () => {
    const previous = buildFlowGeometry(sources.slice(0, 4), 31, 10, 4, 2)
    const next = buildFlowGeometry(sources.slice(0, 4), 21, 20, 4, 2)
    const horizontalPrevious = buildFilmstripGeometry(sources.slice(0, 3), 10, 2, 4)
    const horizontalNext = buildFilmstripGeometry(sources.slice(0, 3), 20, 2, 4)

    expect(anchoredScrollOffset(previous, next, 'landscape', 17, 'vertical')).toBe(53)
    expect(
      anchoredScrollOffset(horizontalPrevious, horizontalNext, 'square', 13, 'horizontal'),
    ).toBeCloseTo(59 / 3)
    expect(anchoredScrollOffset(previous, next, 'missing', 17, 'vertical')).toBe(17)
  })

  it('keeps level-five flow and filmstrip geometry finite and source ordered', () => {
    const flow = buildFlowGeometry(sources, 720, 240, 48, 12)
    const filmstrip = buildFilmstripGeometry(sources, 240, 12, 16)

    expect(flow.items.map(({ key }) => key)).toEqual(sources.map(({ key }) => key))
    expect(filmstrip.items.map(({ key }) => key)).toEqual(sources.map(({ key }) => key))
    expect(flow.items.every(({ imageHeight }) => imageHeight === 240)).toBe(true)
    expect(filmstrip.items.every(({ imageHeight }) => imageHeight === 240)).toBe(true)
    expectFiniteGeometry(flow)
    expectFiniteGeometry(filmstrip)
  })

  it('keeps generated geometry finite, monotonic, source-indexed, and within its scroll extent', () => {
    const geometry = buildFlowGeometry(
      [
        ...sources,
        { key: 'missing', dimensions: null },
        { key: 'bad', dimensions: { width: Number.MAX_VALUE, height: Number.MIN_VALUE } },
      ],
      31,
      13.2,
      4.5,
      1.25,
    )

    for (const [index, item] of geometry.items.entries()) {
      expect(item.index).toBe(index)
      expect(geometry.indexByKey.get(item.key)).toBe(index)
      expect(item.left).toBeGreaterThanOrEqual(0)
      expect(item.top).toBeGreaterThanOrEqual(0)
      expect(item.width).toBeGreaterThan(0)
      expect(item.height).toBeGreaterThan(0)
      expect(item.imageWidth).toBeGreaterThan(0)
      expect(item.imageHeight).toBeGreaterThan(0)
      const previousItem = index === 0 ? undefined : geometry.items[index - 1]
      if (previousItem !== undefined) expect(item.top).toBeGreaterThanOrEqual(previousItem.top)
    }
    expectFiniteGeometry(geometry)
  })
})
