import { useEffect, useState } from 'react'

const VIEWPORT_GUTTER = 16

function viewportPopoverMaxWidth() {
  return Math.max(0, window.innerWidth - VIEWPORT_GUTTER)
}

export default function useViewportPopoverMaxWidth() {
  const [maxWidth, setMaxWidth] = useState(viewportPopoverMaxWidth)

  useEffect(() => {
    const updateMaxWidth = () => setMaxWidth(viewportPopoverMaxWidth())
    window.addEventListener('resize', updateMaxWidth)
    return () => window.removeEventListener('resize', updateMaxWidth)
  }, [])

  return `${maxWidth}px`
}
