import { type RefObject, useCallback, useLayoutEffect, useRef, useState } from 'react'

export interface UseMeasuredElementHeightResult {
  ref: RefObject<HTMLDivElement | null>
  height: number
}

const validHeight = (height: number): number | null =>
  Number.isFinite(height) && height > 0 ? Math.max(1, Math.round(height)) : null

export function useMeasuredElementHeight(fallbackHeight = 520): UseMeasuredElementHeightResult {
  const ref = useRef<HTMLDivElement | null>(null)
  const observedNode = useRef<HTMLDivElement | null>(null)
  const observer = useRef<ResizeObserver | null>(null)
  const latest = useRef(validHeight(fallbackHeight) ?? 520)
  const queued = useRef<number | null>(null)
  const pending = useRef<number | null>(null)
  const [height, setHeight] = useState(latest.current)

  const publish = useCallback((candidate: number) => {
    const next = validHeight(candidate)
    if (next === null) return
    if (queued.current === null && next === latest.current) return
    pending.current = next
    if (queued.current !== null) return
    const flush = () => {
      queued.current = null
      const value = pending.current
      pending.current = null
      if (value === null || value === latest.current) return
      latest.current = value
      setHeight(value)
    }
    if (typeof requestAnimationFrame === 'undefined') {
      flush()
    } else {
      queued.current = requestAnimationFrame(flush)
    }
  }, [])

  const detach = useCallback(() => {
    observer.current?.disconnect()
    observer.current = null
    observedNode.current = null
    if (queued.current !== null && typeof cancelAnimationFrame !== 'undefined') {
      cancelAnimationFrame(queued.current)
    }
    queued.current = null
    pending.current = null
  }, [])

  useLayoutEffect(() => {
    const node = ref.current
    if (node === observedNode.current) return
    detach()
    if (node === null) return
    observedNode.current = node
    if (typeof ResizeObserver === 'undefined') {
      const bounds = node.getBoundingClientRect()
      const next = validHeight(bounds.height)
      if (next !== null && next !== latest.current) {
        latest.current = next
        setHeight(next)
      }
      return
    }
    observer.current = new ResizeObserver((entries) => {
      const bounds = entries[0]?.contentRect
      if (bounds !== undefined) publish(bounds.height)
    })
    observer.current.observe(node)
  })

  useLayoutEffect(() => detach, [detach])

  return { ref, height }
}
