import { useEffect, useState } from 'react'

export const VIDEO_COMPACT_QUERY = '(max-width: 1099px)'

export function useVideoResponsiveLayout(): { compact: boolean } {
  const [compact, setCompact] = useState(() =>
    typeof window.matchMedia === 'function'
      ? window.matchMedia(VIDEO_COMPACT_QUERY).matches
      : false,
  )

  useEffect(() => {
    if (typeof window.matchMedia !== 'function') return
    const query = window.matchMedia(VIDEO_COMPACT_QUERY)
    const changed = (event: MediaQueryListEvent) => setCompact(event.matches)
    setCompact(query.matches)
    query.addEventListener('change', changed)
    return () => query.removeEventListener('change', changed)
  }, [])

  return { compact }
}
