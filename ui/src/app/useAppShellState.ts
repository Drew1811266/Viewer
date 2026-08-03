import type { KeyboardEvent as ReactKeyboardEvent, PointerEvent as ReactPointerEvent } from 'react'
import { useCallback, useEffect, useRef, useState } from 'react'

export interface AppShellState {
  sidebarCollapsed: boolean
  sidebarWidth: number
  projectMenuOpen: boolean
  setProjectMenuOpen(open: boolean): void
  toggleSidebar(): void
  startSidebarResize(event: ReactPointerEvent<HTMLButtonElement>): void
}

interface AppShellStateInternals {
  resizeSidebarFromKeyboard(event: ReactKeyboardEvent<HTMLButtonElement>): void
}

const internalsByShellState = new WeakMap<AppShellState, AppShellStateInternals>()

/** @internal Connects shell-owned keyboard resizing to App's separator element. */
export function getAppShellStateInternals(state: AppShellState): AppShellStateInternals {
  const internals = internalsByShellState.get(state)
  if (internals === undefined) throw new Error('App shell state internals are unavailable')
  return internals
}

export function useAppShellState(projectSessionId: string): AppShellState {
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false)
  const [sidebarWidth, setSidebarWidth] = useState(220)
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
        setSidebarWidth(Math.max(200, Math.min(420, startWidth + next.clientX - startX)))
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
    setSidebarWidth((width) => Math.max(200, Math.min(420, width + delta)))
  }, [])

  const state: AppShellState = {
    sidebarCollapsed,
    sidebarWidth,
    projectMenuOpen,
    setProjectMenuOpen,
    toggleSidebar,
    startSidebarResize,
  }
  internalsByShellState.set(state, { resizeSidebarFromKeyboard })

  return state
}
