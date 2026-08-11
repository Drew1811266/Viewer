import { useCallback, useLayoutEffect, useState } from 'react'
import type { BrowserFile } from '../api/types'

export interface PreviewSession {
  file: BrowserFile
  files: BrowserFile[] | null
  folderOverviewIdentity: string | null
}

export interface PreviewSessionState {
  activePreview: PreviewSession | null
  dimensions: Record<string, { width: number; height: number } | undefined>
  openPreview(session: PreviewSession): void
  closePreview(): void
  recordDimensions(entityId: string, width: number, height: number): void
}

interface PreviewSessionInternals {
  navigatePreview(file: BrowserFile): void
}

const internalsByPreviewState = new WeakMap<PreviewSessionState, PreviewSessionInternals>()

/** @internal Connects preview-owned functional navigation to App's controller transition. */
export function getPreviewSessionInternals(state: PreviewSessionState): PreviewSessionInternals {
  const internals = internalsByPreviewState.get(state)
  if (internals === undefined) throw new Error('Preview session internals are unavailable')
  return internals
}

export function usePreviewSession(projectSessionId: string): PreviewSessionState {
  const [activePreview, setActivePreview] = useState<PreviewSession | null>(null)
  const [dimensions, setDimensions] = useState<
    Record<string, { width: number; height: number } | undefined>
  >({})

  const openPreview = useCallback((session: PreviewSession) => {
    setActivePreview(session)
  }, [])

  const closePreview = useCallback(() => {
    setActivePreview(null)
  }, [])

  const navigatePreview = useCallback((file: BrowserFile) => {
    setActivePreview((current) => (current === null ? null : { ...current, file }))
  }, [])

  const recordDimensions = useCallback((entityId: string, width: number, height: number) => {
    setDimensions((current) => ({ ...current, [entityId]: { width, height } }))
  }, [])

  useLayoutEffect(() => {
    setActivePreview(null)
    setDimensions({})
  }, [projectSessionId])

  const state: PreviewSessionState = {
    activePreview,
    dimensions,
    openPreview,
    closePreview,
    recordDimensions,
  }
  internalsByPreviewState.set(state, { navigatePreview })
  return state
}
