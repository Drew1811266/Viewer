import { type RefObject, useEffect } from 'react'

export function useVideoPreviewScrollLock(root: RefObject<HTMLElement | null>): void {
  useEffect(() => {
    const node = root.current
    if (node === null) return
    const preventScroll = (event: WheelEvent) => event.preventDefault()
    node.addEventListener('wheel', preventScroll, { passive: false })
    return () => node.removeEventListener('wheel', preventScroll)
  }, [root])
}
