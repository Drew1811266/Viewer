import { useCallback, useEffect, useRef, useState } from 'react'
import type { RadialMenuRequest } from '../components/RadialFileMenu'
import type { ViewerStatus } from '../state/viewerState'

export type RadialMenuSession = RadialMenuRequest & {
  requestId: number
  projectIdentity: string
}

export interface UseRadialMenuSessionOptions {
  projectIdentity: string
  projectStatus: ViewerStatus
  contextKey: string
}

export interface RadialMenuSessionState {
  radialMenu: RadialMenuSession | null
  activeRadialMenu: RadialMenuSession | null
  beginRadialSession(request: RadialMenuRequest): void
  finishRadialSession(reportedReturnTarget?: HTMLElement | null): void
}

export function useRadialMenuSession({
  projectIdentity,
  projectStatus,
  contextKey,
}: UseRadialMenuSessionOptions): RadialMenuSessionState {
  const [radialMenu, setRadialMenu] = useState<RadialMenuSession | null>(null)
  const radialReturnFocusTarget = useRef<HTMLElement | null>(null)
  const radialRequestSequence = useRef(0)

  const beginRadialSession = useCallback(
    (request: RadialMenuRequest) => {
      if (projectStatus !== 'active' || projectIdentity === 'no-project') return
      const returnFocusTarget = radialReturnFocusTarget.current ?? request.returnFocusTarget
      radialReturnFocusTarget.current = returnFocusTarget
      radialRequestSequence.current += 1
      setRadialMenu({
        ...request,
        requestId: radialRequestSequence.current,
        projectIdentity,
        returnFocusTarget,
      })
    },
    [projectIdentity, projectStatus],
  )

  const finishRadialSession = useCallback((reportedReturnTarget: HTMLElement | null = null) => {
    setRadialMenu(null)
    const returnFocusTarget = radialReturnFocusTarget.current ?? reportedReturnTarget
    radialReturnFocusTarget.current = null
    if (returnFocusTarget?.isConnected) returnFocusTarget.focus()
  }, [])

  useEffect(() => {
    if (radialMenu !== null) finishRadialSession()
    // Deliberately exclude radialMenu: only a menu present when context changed is stale.
  }, [contextKey, finishRadialSession, projectIdentity, projectStatus])

  const activeRadialMenu =
    projectStatus === 'active' && radialMenu?.projectIdentity === projectIdentity
      ? radialMenu
      : null

  return {
    radialMenu,
    activeRadialMenu,
    beginRadialSession,
    finishRadialSession,
  }
}
