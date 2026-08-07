import type { MutableRefObject } from 'react'
import { useEffect, useRef } from 'react'
import type { Point } from './imageGeometry'

export function useLatestPointerClientPoint(): MutableRefObject<Point | null> {
  const latest = useRef<Point | null>(null)

  useEffect(() => {
    const record = (event: PointerEvent) => {
      latest.current = { x: event.clientX, y: event.clientY }
    }

    window.addEventListener('pointermove', record, { capture: true, passive: true })
    window.addEventListener('pointerdown', record, { capture: true, passive: true })
    return () => {
      window.removeEventListener('pointermove', record, true)
      window.removeEventListener('pointerdown', record, true)
    }
  }, [])

  return latest
}
