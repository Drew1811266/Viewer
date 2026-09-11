import type { KeyboardEvent as ReactKeyboardEvent, PointerEvent as ReactPointerEvent } from 'react'
import { useCallback, useEffect, useRef, useState } from 'react'

export interface AppShellState {
  sidebarCollapsed: boolean
  sidebarWidth: number
  projectMenuOpen: boolean
  setProjectMenuOpen(open: boolean): void
  toggleSidebar(): void
  startSidebarResize(event: ReactPointerEvent<HTMLButtonElement>): void
  resizeSidebarFromKeyboard(event: ReactKeyboardEvent<HTMLButtonElement>): void
}

export function useAppShellState(projectSessionId: string): AppShellState {
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false)
  const [sidebarWidth, setSidebarWidth] = useState(152)
  const [projectMenuOpen, setProjectMenuOpen] = useState(false)
  const stopSidebarResize = useRef<(() => void) | null>(null)

  const toggleSidebar = useCallback(() => {
    setSidebarCollapsed((collapsed) => !collapsed)
  }, [])

  const startSidebarResize = useCallback(
    (event: ReactPointerEvent<HTMLButtonElement>) => {
      if (sidebarCollapsed || event.button !== 0) return
      event.preventDefault()
      const startX = event.clientX
      const startWidth = sidebarWidth
      const move = (next: PointerEvent) => {
        setSidebarWidth(Math.max(120, Math.min(420, startWidth + next.clientX - startX)))
      }
      const stop = () => {
        window.removeEventListener('pointermove', move)
        window.removeEventListener('pointerup', stop)
        window.removeEventListener('pointercancel', stop)
        if (stopSidebarResize.current === stop) stopSidebarResize.current = null
      }
      stopSidebarResize.current?.()
      stopSidebarResize.current = stop
      window.addEventListener('pointermove', move)
      window.addEventListener('pointerup', stop)
      window.addEventListener('pointercancel', stop)
    },
    [sidebarCollapsed, sidebarWidth],
  )

  useEffect(() => {
    setProjectMenuOpen(false)
    return () => stopSidebarResize.current?.()
  }, [projectSessionId])

  const resizeSidebarFromKeyboard = useCallback((event: ReactKeyboardEvent<HTMLButtonElement>) => {
    if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return
    event.preventDefault()
    const delta = event.key === 'ArrowLeft' ? -16 : 16
    setSidebarWidth((width) => Math.max(120, Math.min(420, width + delta)))
  }, [])

  return {
    sidebarCollapsed,
    sidebarWidth,
    projectMenuOpen,
    setProjectMenuOpen,
    toggleSidebar,
    startSidebarResize,
    resizeSidebarFromKeyboard,
  }
}
