import { useCallback, useEffect, useState } from 'react'
import type {
  BrowserFile,
  ConflictResolution,
  FileCommandItem,
  FileCommandPreflight,
} from '../../api/types'
import type { ViewerController } from '../../state/useViewerController'
import type { ViewerState } from '../../state/viewerState'
import { type OperationDialog, useOperationDialogs } from '../useOperationDialogs'
import {
  canMutateOrganizationSelection,
  finderDragFailureMessage,
  organizationOperationBusy,
} from './organizationModel'

export type OrganizationCommands = Pick<
  ViewerController,
  | 'setSelectedEntityIds'
  | 'setReviewState'
  | 'toggleFavorite'
  | 'previewRename'
  | 'preflightFileCommand'
  | 'executeFileCommand'
  | 'cancelOperation'
  | 'loadOperationResults'
  | 'undoLastOperation'
  | 'consumeContextRepair'
  | 'beginFinderDrag'
>

export interface OrganizationOptions {
  state: ViewerState
  commands: OrganizationCommands
}

export type OrganizationFileCommandKind = 'rename' | 'copy' | 'move' | 'trash'

export interface OrganizationCoordinator {
  selectedFiles: BrowserFile[]
  selectFiles(files: BrowserFile[]): void
  selectFolderTarget(entityId: string | null): void
  operationDialog: OperationDialog | null
  operationSubmitting: boolean
  operationBusy: boolean
  canMutateSelection: boolean
  finderDragMessage: string | null
  clearFinderDragMessage(): void
  exportToFinder(entityIds: string[]): void
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
  resultsBatchId: string | null
  showResults(batchId: string): void
  closeResults(): void
  previewRename: OrganizationCommands['previewRename']
  preflightFileCommand: OrganizationCommands['preflightFileCommand']
  cancelOperation: OrganizationCommands['cancelOperation']
  loadOperationResults: OrganizationCommands['loadOperationResults']
  undoLastOperation: OrganizationCommands['undoLastOperation']
  consumeContextRepair: OrganizationCommands['consumeContextRepair']
}

export function useOrganizationCoordinator({
  state,
  commands,
}: OrganizationOptions): OrganizationCoordinator {
  const {
    setSelectedEntityIds,
    previewRename,
    preflightFileCommand,
    executeFileCommand,
    cancelOperation,
    loadOperationResults,
    undoLastOperation,
    consumeContextRepair,
    beginFinderDrag,
  } = commands
  const projectSessionId = state.project?.sessionId ?? 'no-session'
  const { operationDialog, operationSubmitting, setOperationDialog, setOperationSubmitting } =
    useOperationDialogs({
      projectSessionId,
      projectStatus: state.status,
      projectAccess: state.project?.access ?? null,
      activeOperationLifecycle: state.operation.active?.lifecycle ?? null,
    })
  const [selectedFiles, setSelectedFiles] = useState<BrowserFile[]>([])
  const [finderDragMessage, setFinderDragMessage] = useState<string | null>(null)
  const [resultsBatchId, setResultsBatchId] = useState<string | null>(null)
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

  useEffect(() => {
    setSelectedFiles([])
    setFinderDragMessage(null)
    setResultsBatchId(null)
  }, [projectSessionId])

  useEffect(() => {
    const active = state.operation.active
    if (active?.lifecycle === 'completed' && state.operation.results !== null) {
      setResultsBatchId(active.batchId)
    }
  }, [state.operation.active, state.operation.results])

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

  const filesCanMutate = useCallback(
    (files: readonly BrowserFile[]) =>
      canMutateOrganizationSelection({
        selectionCount: files.length,
        projectAccess: state.project?.access ?? null,
        operationBusy,
      }),
    [operationBusy, state.project?.access],
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
        const started = await executeFileCommand(kind, items, conflicts)
        if (started) setOperationDialog(null)
        return started !== null
      } finally {
        setOperationSubmitting(false)
      }
    },
    [executeFileCommand, setOperationDialog, setOperationSubmitting],
  )

  const showResults = useCallback((batchId: string) => setResultsBatchId(batchId), [])
  const closeResults = useCallback(() => setResultsBatchId(null), [])

  return {
    selectedFiles,
    selectFiles,
    selectFolderTarget,
    operationDialog,
    operationSubmitting,
    operationBusy,
    canMutateSelection,
    finderDragMessage,
    clearFinderDragMessage,
    exportToFinder,
    openRenameDialog,
    openDestinationDialog,
    openTrashDialog,
    closeOperationDialog,
    submitFileCommand,
    resultsBatchId,
    showResults,
    closeResults,
    previewRename,
    preflightFileCommand,
    cancelOperation,
    loadOperationResults,
    undoLastOperation,
    consumeContextRepair,
  }
}
