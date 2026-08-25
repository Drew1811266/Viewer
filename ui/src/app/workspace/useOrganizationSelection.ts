import { useCallback, useEffect, useState } from 'react'
import type { BrowserFile } from '../../api/types'
import type { OrganizationCommands } from './organizationTypes'

export interface OrganizationSelection {
  selectedFiles: BrowserFile[]
  selectFiles(files: BrowserFile[]): void
  selectFolderTarget(entityId: string | null): void
}

export function useOrganizationSelection(
  projectSessionId: string,
  setSelectedEntityIds: OrganizationCommands['setSelectedEntityIds'],
): OrganizationSelection {
  const [selectedFiles, setSelectedFiles] = useState<BrowserFile[]>([])

  useEffect(() => setSelectedFiles([]), [projectSessionId])

  const selectFiles = useCallback(
    (files: BrowserFile[]) => {
      setSelectedFiles(files)
      setSelectedEntityIds(files.map((file) => file.entityId))
    },
    [setSelectedEntityIds],
  )
  const selectFolderTarget = useCallback(
    (entityId: string | null) => {
      setSelectedFiles([])
      setSelectedEntityIds(entityId === null ? [] : [entityId])
    },
    [setSelectedEntityIds],
  )

  return { selectedFiles, selectFiles, selectFolderTarget }
}
