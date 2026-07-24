import { useCallback, useEffect, useState } from 'react'
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

  const recordDimensions = useCallback((entityId: string, width: number, height: number) => {
    setDimensions((current) => ({ ...current, [entityId]: { width, height } }))
  }, [])

  useEffect(() => {
    setActivePreview(null)
    setDimensions({})
  }, [projectSessionId])

  return {
    activePreview,
    dimensions,
    openPreview,
    closePreview,
    recordDimensions,
  }
}
