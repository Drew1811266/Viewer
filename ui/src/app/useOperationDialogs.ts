import { useEffect, useState } from 'react'
import type { BatchLifecycle, BrowserFile, FileCommandPreflight, ProjectAccess } from '../api/types'
import type { ViewerStatus } from '../state/viewerState'

export type OperationDialog =
  | { kind: 'rename'; file: BrowserFile }
  | { kind: 'batch_rename'; files: BrowserFile[] }
  | {
      kind: 'destination'
      mode: 'copy' | 'move'
      files: BrowserFile[]
      initialDestinationId?: string
      initialPreflight?: FileCommandPreflight
    }
  | { kind: 'trash'; files: BrowserFile[] }

export interface UseOperationDialogsOptions {
  projectSessionId: string
  projectStatus: ViewerStatus
  projectAccess: ProjectAccess | null
  activeOperationLifecycle: BatchLifecycle | null
}

export interface OperationDialogsState {
  operationDialog: OperationDialog | null
  operationSubmitting: boolean
  setOperationDialog(dialog: OperationDialog | null): void
  setOperationSubmitting(submitting: boolean): void
}

export function useOperationDialogs({
  projectSessionId,
  projectStatus,
  projectAccess,
  activeOperationLifecycle,
}: UseOperationDialogsOptions): OperationDialogsState {
  const [operationDialog, setOperationDialog] = useState<OperationDialog | null>(null)
  const [operationSubmitting, setOperationSubmitting] = useState(false)

  useEffect(() => {
    setOperationDialog(null)
    setOperationSubmitting(false)
  }, [projectSessionId])

  useEffect(() => {
    if (
      operationDialog !== null &&
      (projectStatus !== 'active' ||
        projectAccess !== 'read_write' ||
        (activeOperationLifecycle !== null && activeOperationLifecycle !== 'completed'))
    ) {
      setOperationDialog(null)
    }
  }, [activeOperationLifecycle, operationDialog, projectAccess, projectStatus])

  return {
    operationDialog,
    operationSubmitting,
    setOperationDialog,
    setOperationSubmitting,
  }
}
