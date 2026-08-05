import { useEffect, useState } from 'react'
import type { ProjectionTransition } from '../state/viewerState'

export const PROJECTION_PROGRESS_DELAY_MS = 120

export function useDelayedProjectionProgress(
  transition: ProjectionTransition | null,
  workspaceAvailable: boolean,
  delayMs = PROJECTION_PROGRESS_DELAY_MS,
) {
  const [visible, setVisible] = useState(false)

  useEffect(() => {
    setVisible(false)
    if (transition === null || !workspaceAvailable) return
    const timer = window.setTimeout(() => setVisible(true), delayMs)
    return () => window.clearTimeout(timer)
  }, [delayMs, transition, workspaceAvailable])

  return visible
}
