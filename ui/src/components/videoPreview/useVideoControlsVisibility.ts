import { useCallback, useEffect, useState } from 'react'
import type { VideoError } from '../../api/types'

export const CONTROL_IDLE_MS = 2_500

export interface VideoControlsVisibilityOptions {
  playing: boolean
  seeking: boolean
  adjusting: boolean
  focusedWithin: boolean
  error: VideoError | null
  reducedMotion: boolean
}

export interface VideoControlsVisibility {
  visible: boolean
  reducedMotion: boolean
  reveal(): void
}

export function useVideoControlsVisibility({
  playing,
  seeking,
  adjusting,
  focusedWithin,
  error,
  reducedMotion,
}: VideoControlsVisibilityOptions): VideoControlsVisibility {
  const [visible, setVisible] = useState(true)
  const [activity, setActivity] = useState(0)
  const mustRemainVisible = !playing || seeking || adjusting || focusedWithin || error !== null

  const reveal = useCallback(() => {
    setVisible(true)
    setActivity((current) => current + 1)
  }, [])

  useEffect(() => {
    if (mustRemainVisible) {
      setVisible(true)
      return
    }
    const timeout = window.setTimeout(() => setVisible(false), CONTROL_IDLE_MS)
    return () => window.clearTimeout(timeout)
  }, [activity, mustRemainVisible])

  return { visible, reducedMotion, reveal }
}

export function useReducedMotionPreference(): boolean {
  const [reducedMotion, setReducedMotion] = useState(() =>
    typeof window.matchMedia === 'function'
      ? window.matchMedia('(prefers-reduced-motion: reduce)').matches
      : false,
  )

  useEffect(() => {
    if (typeof window.matchMedia !== 'function') return
    const query = window.matchMedia('(prefers-reduced-motion: reduce)')
    const changed = (event: MediaQueryListEvent) => setReducedMotion(event.matches)
    setReducedMotion(query.matches)
    query.addEventListener?.('change', changed)
    return () => query.removeEventListener?.('change', changed)
  }, [])

  return reducedMotion
}
