import { describe, expect, it } from 'vitest'
import { solveCompareLayout } from './compareLayoutEngine'

const twenty = (ratio: number) =>
  Array.from({ length: 20 }, (_, index) => ({
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

describe('compare layout performance', () => {
  it.each([0.75, 1, 1.5])('keeps candidate work bounded for ratio %s', (ratio) => {
    const plan = solve(twenty(ratio))
    expect(plan.candidateCount).toBeLessThanOrEqual(6)
    expect(plan.rects).toHaveLength(20)
  })

  it.runIf(process.env.VIEWER_COMPARE_BENCH === '1')(
    'solves a mixed 20-image set below the current-machine 2 ms target',
    () => {
      const mixed = Array.from({ length: 20 }, (_, index) => ({
        entityId: `image-${index}`,
        aspectRatio: index % 2 === 0 ? 0.75 : 1.5,
      }))
      for (let index = 0; index < 100; index += 1) solve(mixed)
      const start = performance.now()
      for (let index = 0; index < 1_000; index += 1) solve(mixed)
      const average = (performance.now() - start) / 1_000
      console.info(`compare layout average: ${average.toFixed(4)} ms`)
      expect(average).toBeLessThan(2)
    },
  )
})
