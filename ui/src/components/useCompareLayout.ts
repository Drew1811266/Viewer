import { type RefObject, useCallback, useEffect, useMemo, useRef, useState } from 'react'
import type { BrowserFile } from '../api/types'
import type { QuarterRotation } from '../state/compareModel'
import {
  type CompareLayoutPlan,
  type CompareLayoutSource,
  solveCompareLayout,
} from './compareLayoutEngine'

export const COMPARE_LAYOUT_GEOMETRY = {
  gap: 6,
  padding: 6,
  paneChromeHeight: 70,
} as const

export interface RecoveredCompareDimensions {
  sourceRevision: string
  width: number
  height: number
}

export interface UseCompareLayoutInput {
  files: readonly BrowserFile[]
  rotations: Readonly<Record<string, QuarterRotation>>
  recoveredDimensions: Readonly<Record<string, RecoveredCompareDimensions | undefined>>
}

export interface UseCompareLayoutResult {
  containerRef: RefObject<HTMLDivElement | null>
  plan: CompareLayoutPlan
}

interface WorkspaceSize {
  width: number
  height: number
}

export function compareSourceRevision(file: BrowserFile): string {
  return `${file.entityId}:${file.modifiedNs}:${file.size}`
}

export function effectiveCompareRatio(
  file: BrowserFile,
  rotation: QuarterRotation,
  recovered: RecoveredCompareDimensions | undefined,
): number {
  const sourceRevision = compareSourceRevision(file)
  const recoveredMatches = recovered !== undefined && recovered.sourceRevision === sourceRevision
  const width = file.imageMetadata?.width ?? (recoveredMatches ? recovered.width : undefined)
  const height = file.imageMetadata?.height ?? (recoveredMatches ? recovered.height : undefined)
  if (
    width === undefined ||
    height === undefined ||
    !Number.isFinite(width) ||
    !Number.isFinite(height) ||
    width <= 0 ||
    height <= 0
  ) {
    return 1
  }
  const ratio = width / height
  return rotation === 90 || rotation === 270 ? 1 / ratio : ratio
}

const planIsEqual = (left: CompareLayoutPlan, right: CompareLayoutPlan) =>
  left.kind === right.kind &&
  left.score === right.score &&
  left.totalWidth === right.totalWidth &&
  left.totalHeight === right.totalHeight &&
  left.rects.length === right.rects.length &&
  left.rects.every((rect, index) => {
    const other = right.rects[index]
    return (
      other !== undefined &&
      rect.entityId === other.entityId &&
      rect.index === other.index &&
      rect.left === other.left &&
      rect.top === other.top &&
      rect.width === other.width &&
      rect.height === other.height &&
      rect.stageWidth === other.stageWidth &&
      rect.stageHeight === other.stageHeight
    )
  })

const sourceSignature = (sources: readonly CompareLayoutSource[]) =>
  sources.map(({ entityId, aspectRatio }) => `${entityId}:${aspectRatio}`).join('|')

const finitePositiveSize = (width: number, height: number): WorkspaceSize | null =>
  Number.isFinite(width) && Number.isFinite(height) && width > 0 && height > 0
    ? { width, height }
    : null

const solvePlan = (
  size: WorkspaceSize | null,
  items: readonly CompareLayoutSource[],
  previous: CompareLayoutPlan | null,
) =>
  solveCompareLayout({
    width: size?.width ?? 0,
    height: size?.height ?? 0,
    ...COMPARE_LAYOUT_GEOMETRY,
    items,
    previous,
  })

export function useCompareLayout({
  files,
  rotations,
  recoveredDimensions,
}: UseCompareLayoutInput): UseCompareLayoutResult {
  const containerRef = useRef<HTMLDivElement | null>(null)
  const sizeRef = useRef<WorkspaceSize | null>(null)
  const frameRef = useRef<number | null>(null)
  const sources = useMemo(
    () =>
      files.map((file) => ({
        entityId: file.entityId,
        aspectRatio: effectiveCompareRatio(
          file,
          rotations[file.entityId] ?? 0,
          recoveredDimensions[compareSourceRevision(file)],
        ),
      })),
    [files, rotations, recoveredDimensions],
  )
  const sourcesRef = useRef(sources)
  sourcesRef.current = sources
  const signature = sourceSignature(sources)
  const signatureRef = useRef(signature)
  const [plan, setPlan] = useState(() => solvePlan(null, sources, null))
  const previousPlanRef = useRef(plan)

  const scheduleSolve = useCallback(() => {
    if (frameRef.current !== null) return
    const solve = () => {
      frameRef.current = null
      const next = solvePlan(sizeRef.current, sourcesRef.current, previousPlanRef.current)
      setPlan((current) => {
        if (planIsEqual(current, next)) return current
        previousPlanRef.current = next
        return next
      })
    }

    if (typeof requestAnimationFrame === 'undefined') {
      solve()
      return
    }
    frameRef.current = requestAnimationFrame(solve)
  }, [])

  const measure = useCallback(
    (width: number, height: number) => {
      const size = finitePositiveSize(width, height)
      if (size === null) return
      sizeRef.current = size
      scheduleSolve()
    },
    [scheduleSolve],
  )

  useEffect(() => {
    const node = containerRef.current
    if (node === null) return
    if (typeof ResizeObserver === 'undefined') {
      const bounds = node.getBoundingClientRect()
      measure(bounds.width, bounds.height)
      return
    }
    const observer = new ResizeObserver((entries) => {
      const bounds = entries[0]?.contentRect
      if (bounds !== undefined) measure(bounds.width, bounds.height)
    })
    observer.observe(node)
    return () => observer.disconnect()
  }, [measure, containerRef.current])

  useEffect(() => {
    if (signatureRef.current === signature) return
    signatureRef.current = signature
    if (sizeRef.current !== null) scheduleSolve()
  }, [scheduleSolve, signature])

  useEffect(
    () => () => {
      if (frameRef.current !== null && typeof cancelAnimationFrame !== 'undefined') {
        cancelAnimationFrame(frameRef.current)
      }
    },
    [],
  )

  return { containerRef, plan }
}
