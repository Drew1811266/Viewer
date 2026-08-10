import { type RefObject, useLayoutEffect, useState } from 'react'
import type { Size } from './imageGeometry'

const EMPTY_STAGE: Size = { width: 0, height: 0 }

export function usePreviewStageSize(stage: RefObject<HTMLElement | null>): Size {
  const [size, setSize] = useState<Size>(EMPTY_STAGE)

  useLayoutEffect(() => {
    const node = stage.current
    if (node === null) return

    const publish = (candidate: Size) => {
      if (!isPositiveSize(candidate)) return
      const next = {
        width: Math.round(candidate.width),
        height: Math.round(candidate.height),
      }
      setSize((current) =>
        current.width === next.width && current.height === next.height ? current : next,
      )
    }

    const bounds = node.getBoundingClientRect()
    publish({
      width: bounds.width || node.clientWidth,
      height: bounds.height || node.clientHeight,
    })

    if (typeof ResizeObserver === 'undefined') return
    const observer = new ResizeObserver((entries) => {
      const content = entries[0]?.contentRect
      if (content !== undefined) publish(content)
    })
    observer.observe(node)
    return () => observer.disconnect()
  }, [stage])

  return size
}

export function isPositiveSize(size: Size): boolean {
  return (
    Number.isFinite(size.width) && Number.isFinite(size.height) && size.width > 0 && size.height > 0
  )
}
