import { useCallback, useEffect, useState } from 'react'
import type {
  BrowserFile,
  ConflictResolution,
  FileCommandItem,
  FileCommandPreflight,
} from '../../api/types'
import type { ViewerState } from '../../state/viewerState'
import { type OperationDialog, useOperationDialogs } from '../useOperationDialogs'
import { canMutateOrganizationSelection, organizationOperationBusy } from './organizationModel'
import type { OrganizationCommands, OrganizationFileCommandKind } from './organizationTypes'

type OperationCommands = Pick<
  OrganizationCommands,
  | 'previewRename'
  | 'preflightFileCommand'
  | 'executeFileCommand'
  | 'cancelOperation'
  | 'loadOperationResults'
  | 'undoLastOperation'
  | 'consumeContextRepair'
>

export interface OrganizationOperations {
  operationDialog: OperationDialog | null
  operationSubmitting: boolean
  operationBusy: boolean
  canMutateSelection: boolean
  openRenameDialog(files?: readonly BrowserFile[]): void
  openDestinationDialog(
    mode: 'copy' | 'move',
    files?: readonly BrowserFile[],
    initialDestinationId?: string,
    initialPreflight?: FileCommandPreflight,
  ): void
  openTrashDialog(files?: readonly BrowserFile[]): void
  closeOperationDialog(): void
  submitFileCommand(
    kind: OrganizationFileCommandKind,
    items: FileCommandItem[],
    conflicts?: ConflictResolution[],
  ): Promise<boolean>
  dropFiles(entityIds: string[], destinationId: string, mode: 'move' | 'copy'): Promise<void>
  resultsBatchId: string | null
  showResults(batchId: string): void
  closeResults(): void
  previewRename: OperationCommands['previewRename']
  preflightFileCommand: OperationCommands['preflightFileCommand']
  cancelOperation: OperationCommands['cancelOperation']
  loadOperationResults: OperationCommands['loadOperationResults']
  undoLastOperation: OperationCommands['undoLastOperation']
  consumeContextRepair: OperationCommands['consumeContextRepair']
}

export function useOrganizationOperations({
  state,
  selectedFiles,
  commands,
}: {
  state: ViewerState
  selectedFiles: BrowserFile[]
  commands: OperationCommands
}): OrganizationOperations {
  const projectSessionId = state.project?.sessionId ?? 'no-session'
  const { operationDialog, operationSubmitting, setOperationDialog, setOperationSubmitting } =
    useOperationDialogs({
      projectSessionId,
      projectStatus: state.status,
      projectAccess: state.project?.access ?? null,
      activeOperationLifecycle: state.operation.active?.lifecycle ?? null,
    })
  const operationBusy = organizationOperationBusy({
    operationSubmitting,
    projectStatus: state.status,
    operationFinishing: state.operation.finishing,
    operationPending: state.operation.pending,
    activeOperation: state.operation.active,
  })
  const canMutateSelection = canMutateOrganizationSelection({
    selectionCount: selectedFiles.length,
    projectAccess: state.project?.access ?? null,
    operationBusy,
  })

  const dialogActions = useOrganizationDialogActions({
    selectedFiles,
    projectAccess: state.project?.access ?? null,
    operationBusy,
    commands,
    setOperationDialog,
    setOperationSubmitting,
  })
  const dropFiles = useOrganizationDropFiles({
    state,
    operationBusy,
    preflightFileCommand: commands.preflightFileCommand,
    openDestinationDialog: dialogActions.openDestinationDialog,
    submitFileCommand: dialogActions.submitFileCommand,
  })
  const results = useOperationResults(projectSessionId, state.operation)

  return {
    operationDialog,
    operationSubmitting,
    operationBusy,
    canMutateSelection,
    ...dialogActions,
    dropFiles,
    ...results,
    previewRename: commands.previewRename,
    preflightFileCommand: commands.preflightFileCommand,
    cancelOperation: commands.cancelOperation,
    loadOperationResults: commands.loadOperationResults,
    undoLastOperation: commands.undoLastOperation,
    consumeContextRepair: commands.consumeContextRepair,
  }
}

type DialogActions = Pick<
  OrganizationOperations,
  | 'openRenameDialog'
  | 'openDestinationDialog'
  | 'openTrashDialog'
  | 'closeOperationDialog'
  | 'submitFileCommand'
>

function useOrganizationDialogActions({
  selectedFiles,
  projectAccess,
  operationBusy,
  commands,
  setOperationDialog,
  setOperationSubmitting,
}: {
  selectedFiles: BrowserFile[]
  projectAccess: NonNullable<ViewerState['project']>['access'] | null
  operationBusy: boolean
  commands: Pick<OperationCommands, 'executeFileCommand'>
  setOperationDialog: ReturnType<typeof useOperationDialogs>['setOperationDialog']
  setOperationSubmitting: ReturnType<typeof useOperationDialogs>['setOperationSubmitting']
}): DialogActions {
  const filesCanMutate = useCallback(
    (files: readonly BrowserFile[]) =>
      canMutateOrganizationSelection({
        selectionCount: files.length,
        projectAccess,
        operationBusy,
      }),
    [operationBusy, projectAccess],
  )
  const openRenameDialog = useCallback(
    (files: readonly BrowserFile[] = selectedFiles) => {
      if (!filesCanMutate(files)) return
      if (files.length === 1) {
        const [file] = files
        if (file === undefined) throw new Error('Single-file rename selection is missing its file')
        setOperationDialog({ kind: 'rename', file })
        return
      }
      setOperationDialog({ kind: 'batch_rename', files: [...files] })
    },
    [filesCanMutate, selectedFiles, setOperationDialog],
  )
  const openDestinationDialog = useCallback(
    (
      mode: 'copy' | 'move',
      files: readonly BrowserFile[] = selectedFiles,
      initialDestinationId?: string,
      initialPreflight?: FileCommandPreflight,
    ) => {
      if (!filesCanMutate(files)) return
      setOperationDialog({
        kind: 'destination',
        mode,
        files: [...files],
        initialDestinationId,
        initialPreflight,
      })
    },
    [filesCanMutate, selectedFiles, setOperationDialog],
  )
  const openTrashDialog = useCallback(
    (files: readonly BrowserFile[] = selectedFiles) => {
      if (!filesCanMutate(files)) return
      setOperationDialog({ kind: 'trash', files: [...files] })
    },
    [filesCanMutate, selectedFiles, setOperationDialog],
  )
  const closeOperationDialog = useCallback(() => setOperationDialog(null), [setOperationDialog])
  const submitFileCommand = useCallback(
    async (
      kind: OrganizationFileCommandKind,
      items: FileCommandItem[],
      conflicts: ConflictResolution[] = [],
    ) => {
      setOperationSubmitting(true)
      try {
        const started = await commands.executeFileCommand(kind, items, conflicts)
        if (started) setOperationDialog(null)
        return started !== null
      } finally {
        setOperationSubmitting(false)
      }
    },
    [commands.executeFileCommand, setOperationDialog, setOperationSubmitting],
  )

  return {
    openRenameDialog,
    openDestinationDialog,
    openTrashDialog,
    closeOperationDialog,
    submitFileCommand,
  }
}

function useOrganizationDropFiles({
  state,
  operationBusy,
  preflightFileCommand,
  openDestinationDialog,
  submitFileCommand,
}: {
  state: ViewerState
  operationBusy: boolean
  preflightFileCommand: OperationCommands['preflightFileCommand']
  openDestinationDialog: DialogActions['openDestinationDialog']
  submitFileCommand: DialogActions['submitFileCommand']
}): OrganizationOperations['dropFiles'] {
  return useCallback(
    async (entityIds: string[], destinationId: string, mode: 'move' | 'copy') => {
      if (
        operationBusy ||
        state.project?.access !== 'read_write' ||
        state.workspace?.workspace !== 'content'
      ) {
        return
      }
      const currentFiles = [
        ...state.workspace.images,
        ...state.workspace.videos,
        ...state.workspace.otherFiles,
      ]
      const byId = new Map(currentFiles.map((file) => [file.entityId, file]))
      const files = entityIds.map((entityId) => byId.get(entityId))
      if (files.some((file) => file === undefined)) return
      const items: FileCommandItem[] = entityIds.map((entityId) => ({
        entityId,
        action:
          mode === 'copy'
            ? { kind: 'copy', destinationFolderId: destinationId }
            : { kind: 'move', destinationFolderId: destinationId },
      }))
      const preflight = await preflightFileCommand(mode, items)
      if (preflight === null) return
      if (preflight.executable && preflight.rows.every((row) => row.state === 'ready')) {
        await submitFileCommand(mode, items)
        return
      }
      openDestinationDialog(mode, files as BrowserFile[], destinationId, preflight)
    },
    [
      openDestinationDialog,
      operationBusy,
      preflightFileCommand,
      state.project?.access,
      state.workspace,
      submitFileCommand,
    ],
  )
}

function useOperationResults(
  projectSessionId: string,
  operation: ViewerState['operation'],
): Pick<OrganizationOperations, 'resultsBatchId' | 'showResults' | 'closeResults'> {
  const [resultsBatchId, setResultsBatchId] = useState<string | null>(null)
  useEffect(() => setResultsBatchId(null), [projectSessionId])
  useEffect(() => {
    const active = operation.active
    if (active?.lifecycle === 'completed' && operation.results !== null) {
      setResultsBatchId(active.batchId)
    }
  }, [operation.active, operation.results])
  const showResults = useCallback((batchId: string) => setResultsBatchId(batchId), [])
  const closeResults = useCallback(() => setResultsBatchId(null), [])
  return { resultsBatchId, showResults, closeResults }
}
