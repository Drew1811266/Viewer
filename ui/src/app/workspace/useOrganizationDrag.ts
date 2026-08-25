import { useCallback, useEffect, useState } from 'react'
import type {
  OrganizationDragView,
  OrganizationDropTarget,
  OrganizationPointerInput,
} from '../../state/useOrganizationPointerDrag'
import { useOrganizationPointerDrag } from '../../state/useOrganizationPointerDrag'
import type { ViewerState } from '../../state/viewerState'
import { finderDragFailureMessage, isOrganizationDropTargetValid } from './organizationModel'
import type { OrganizationCommands } from './organizationTypes'
import type { OrganizationOperations } from './useOrganizationOperations'

export interface OrganizationDrag {
  finderDragMessage: string | null
  clearFinderDragMessage(): void
  exportToFinder(entityIds: string[]): void
  organizationDragView: OrganizationDragView | null
  organizationDropTarget: OrganizationDropTarget | null
  handleOrganizationPointerInput(input: OrganizationPointerInput): void
}

export function useOrganizationDrag({
  state,
  projectSessionId,
  workspaceIdentity,
  compareOpen,
  operationBusy,
  beginFinderDrag,
  dropFiles,
}: {
  state: ViewerState
  projectSessionId: string
  workspaceIdentity: string
  compareOpen: boolean
  operationBusy: boolean
  beginFinderDrag: OrganizationCommands['beginFinderDrag']
  dropFiles: OrganizationOperations['dropFiles']
}): OrganizationDrag {
  const [finderDragMessage, setFinderDragMessage] = useState<string | null>(null)

  useEffect(() => setFinderDragMessage(null), [projectSessionId])

  const clearFinderDragMessage = useCallback(() => setFinderDragMessage(null), [])
  const exportToFinder = useCallback(
    (entityIds: string[]) => {
      setFinderDragMessage(null)
      void beginFinderDrag(entityIds).catch((error: unknown) =>
        setFinderDragMessage(finderDragFailureMessage(error)),
      )
    },
    [beginFinderDrag],
  )
  const validateDropTarget = useCallback(
    (entityIds: readonly string[], destinationId: string, mode: 'move' | 'copy') =>
      isOrganizationDropTargetValid({
        workspace: state.workspace,
        folders: state.folders,
        entityIds,
        destinationId,
        mode,
      }),
    [state.folders, state.workspace],
  )
  const resetKey = [
    projectSessionId,
    state.project?.generation ?? 'no-generation',
    workspaceIdentity,
  ].join(':')
  const {
    dragView: organizationDragView,
    dropTarget: organizationDropTarget,
    handlePointerInput: handleOrganizationPointerInput,
    cancel: cancelOrganizationPointerDrag,
  } = useOrganizationPointerDrag({
    disabled: state.project?.access !== 'read_write' || operationBusy || compareOpen,
    resetKey,
    isDropTargetValid: validateDropTarget,
    onDrop: dropFiles,
  })

  useEffect(() => cancelOrganizationPointerDrag(), [cancelOrganizationPointerDrag, state.workspace])

  return {
    finderDragMessage,
    clearFinderDragMessage,
    exportToFinder,
    organizationDragView,
    organizationDropTarget,
    handleOrganizationPointerInput,
  }
}
