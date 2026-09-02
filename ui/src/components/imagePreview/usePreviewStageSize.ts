import { type RefObject, useLayoutEffect, useState } from 'react'
import type { Size } from './imageGeometry'

const EMPTY_STAGE: Size = { width: 0, height: 0 }
const MAX_STABILIZATION_FRAMES = 8
const REQUIRED_STABLE_FRAMES = 2

export function usePreviewStageSize(stage: RefObject<HTMLElement | null>): Size {
  const [size, setSize] = useState<Size>(EMPTY_STAGE)

  useLayoutEffect(() => {
    const node = stage.current
    if (node === null) return
    let frame: number | null = null
    let framesRemaining = 0
    let stableFrames = 0
    let lastMeasurement: Size | null = null

    const publish = (candidate: Size) => {
      if (!isPositiveSize(candidate)) return
      const next = {
        width: Math.round(candidate.width),
        height: Math.round(candidate.height),
      }
      stableFrames = sameSize(lastMeasurement, next) ? stableFrames + 1 : 0
      lastMeasurement = next
      setSize((current) =>
        current.width === next.width && current.height === next.height ? current : next,
      )
    }

    const measure = (fallback: Size = EMPTY_STAGE) => {
      const bounds = node.getBoundingClientRect()
      publish({
        width: bounds.width || fallback.width || node.clientWidth,
        height: bounds.height || fallback.height || node.clientHeight,
      })
    }

    const scheduleFrame = () => {
      if (frame !== null || typeof requestAnimationFrame !== 'function') return
      frame = requestAnimationFrame(() => {
        frame = null
        measure()
        framesRemaining -= 1
        if (framesRemaining > 0 && stableFrames < REQUIRED_STABLE_FRAMES) scheduleFrame()
      })
    }

    const stabilize = () => {
      framesRemaining = MAX_STABILIZATION_FRAMES
      stableFrames = 0
      scheduleFrame()
    }

    const remeasure = () => {
      measure()
      stabilize()
    }

    measure()
    stabilize()
    const visualViewport = window.visualViewport
    window.addEventListener('resize', remeasure)
    visualViewport?.addEventListener('resize', remeasure)

    const observer =
      typeof ResizeObserver === 'undefined'
        ? null
        : new ResizeObserver((entries) => {
            const content = entries.find((entry) => entry.target === node)?.contentRect
            measure(content ?? EMPTY_STAGE)
            stabilize()
          })
    observer?.observe(node)
    if (node.parentElement !== null) observer?.observe(node.parentElement)
    return () => {
      observer?.disconnect()
      if (frame !== null && typeof cancelAnimationFrame === 'function') cancelAnimationFrame(frame)
      window.removeEventListener('resize', remeasure)
      visualViewport?.removeEventListener('resize', remeasure)
    }
  }, [stage])

  return size
}

function sameSize(left: Size | null, right: Size): boolean {
  return left?.width === right.width && left.height === right.height
}

export function isPositiveSize(size: Size): boolean {
  return (
    Number.isFinite(size.width) && Number.isFinite(size.height) && size.width > 0 && size.height > 0
  )
}
