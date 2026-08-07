import { describe, expect, it } from 'vitest'
import { type CompareLayoutPlan, solveCompareLayout } from './compareLayoutEngine'

const eight = (ratio: number) =>
  Array.from({ length: 8 }, (_, index) => ({
    entityId: `image-${index}`,
    aspectRatio: ratio,
  }))

const solve = (items: ReadonlyArray<{ entityId: string; aspectRatio: number }>) =>
  solveCompareLayout({
    width: 1_700,
    height: 900,
    gap: 6,
    padding: 6,
    paneChromeHeight: 70,
    items,
    previous: null,
  })

const checksum = (plan: CompareLayoutPlan) => {
  if (plan.rects.length !== 8) return Number.NaN
  return plan.rects.reduce((sum, rect, index) => {
    const dimensions = [
      rect.left,
      rect.top,
      rect.width,
      rect.height,
      rect.stageWidth,
      rect.stageHeight,
    ]
    const valid =
      rect.index === index &&
      rect.entityId === `image-${index}` &&
      dimensions.every(Number.isFinite) &&
      rect.left >= 0 &&
      rect.top >= 0 &&
      rect.width > 0 &&
      rect.height > 0 &&
      rect.stageWidth > 0 &&
      rect.stageHeight > 0
    return valid ? sum + index + 1 : Number.NaN
  }, plan.rects.length)
}

describe('compare layout performance', () => {
  it.each([0.75, 1, 1.5])('keeps candidate work bounded for ratio %s', (ratio) => {
    const plan = solve(eight(ratio))
    expect(plan.candidateCount).toBeLessThanOrEqual(6)
    expect(plan.rects).toHaveLength(8)
  })

  it.runIf(process.env.VIEWER_COMPARE_BENCH === '1')(
    'solves a mixed 8-image set below the current-machine 2 ms target',
    () => {
      const mixed = Array.from({ length: 8 }, (_, index) => ({
        entityId: `image-${index}`,
        aspectRatio: index % 2 === 0 ? 0.75 : 1.5,
      }))
      for (let index = 0; index < 100; index += 1) solve(mixed)
      let accumulatedChecksum = 0
      const start = performance.now()
      for (let index = 0; index < 1_000; index += 1) {
        accumulatedChecksum += checksum(solve(mixed))
      }
      const average = (performance.now() - start) / 1_000
      console.info(`compare layout average: ${average.toFixed(4)} ms`)
      expect(accumulatedChecksum).toBe(44_000)
      expect(average).toBeLessThan(2)
    },
  )
})
