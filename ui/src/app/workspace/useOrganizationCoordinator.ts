import { useCallback, useEffect, useMemo, useState } from 'react'
import type {
  BrowserFile,
  ConflictResolution,
  FileCommandItem,
  FileCommandPreflight,
} from '../../api/types'
import type { RadialLeafAction } from '../../components/radialMenuModel'
import { defined } from '../../defined'
import { compareEntryAvailability } from '../../state/comparePolicy'
import { organizationShortcutIsOwned } from '../../state/organizationShortcutOwnership'
import { validatePreviewSelection } from '../../state/previewPolicy'
import type {
  OrganizationDragView,
  OrganizationDropTarget,
  OrganizationPointerInput,
} from '../../state/useOrganizationPointerDrag'
import { useOrganizationPointerDrag } from '../../state/useOrganizationPointerDrag'
import useReviewShortcuts from '../../state/useReviewShortcuts'
import type { ViewerController } from '../../state/useViewerController'
import type { ViewerState } from '../../state/viewerState'
import { type OperationDialog, useOperationDialogs } from '../useOperationDialogs'
import { useRadialMenuContextToken, useRadialMenuSession } from '../useRadialMenuSession'
import type { WorkspaceIntentSink } from './intents'
import {
  buildOrganizationRadialModel,
  canMutateOrganizationSelection,
  finderDragFailureMessage,
  isOrganizationDropTargetValid,
  organizationOperationBusy,
  organizationProjectIdentity,
  organizationWorkspaceIdentity,
} from './organizationModel'

const ACTIVE_PREVIEW_CONTEXT = {}

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
  emitIntent: WorkspaceIntentSink
  compareOpen: boolean
  activePreviewOpen: boolean
  infoOpen: boolean
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
  radialMenu: ReturnType<typeof useRadialMenuSession>['radialMenu']
  activeRadialMenu: ReturnType<typeof useRadialMenuSession>['activeRadialMenu']
  radialModel: ReturnType<typeof buildOrganizationRadialModel>
  beginRadialSession: ReturnType<typeof useRadialMenuSession>['beginRadialSession']
  finishRadialSession: ReturnType<typeof useRadialMenuSession>['finishRadialSession']
  runRadialAction(action: RadialLeafAction): void
  organizationDragView: OrganizationDragView | null
  organizationDropTarget: OrganizationDropTarget | null
  handleOrganizationPointerInput(input: OrganizationPointerInput): void
}

export function useOrganizationCoordinator({
  state,
  commands,
  emitIntent,
  compareOpen,
  activePreviewOpen,
  infoOpen,
}: OrganizationOptions): OrganizationCoordinator {
  const {
    setSelectedEntityIds,
    setReviewState,
    toggleFavorite,
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
  const projectIdentity = organizationProjectIdentity(state)
  const workspaceIdentity = organizationWorkspaceIdentity(state)
  const radialContextKey = useRadialMenuContextToken({
    activePreview: activePreviewOpen ? ACTIVE_PREVIEW_CONTEXT : null,
    compareOpen,
    infoOpen,
    operationDialog,
    organizationWorkspaceIdentity: workspaceIdentity,
    resultsBatchId,
    closeBlocked: state.closeBlocked,
    contextRepair: state.contextRepair,
  })
  const { radialMenu, activeRadialMenu, beginRadialSession, finishRadialSession } =
    useRadialMenuSession({
      projectIdentity,
      projectStatus: state.status,
      contextKey: radialContextKey,
    })
  const compareContextAvailable =
    compareEntryAvailability({
      workspace: state.workspace,
      searchResultsOpen: state.search.showResults,
      operationBusy,
    }) === 'available'
  const radialModel = useMemo(
    () =>
      buildOrganizationRadialModel({
        files: activeRadialMenu?.files ?? [],
        projectAccess: state.project?.access ?? null,
        operationBusy,
        compareContextAvailable,
      }),
    [activeRadialMenu, compareContextAvailable, operationBusy, state.project?.access],
  )

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

  const dropFiles = useCallback(
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

  const validateOrganizationDropTarget = useCallback(
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
  const organizationDragResetKey = [
    state.project?.sessionId ?? 'no-session',
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
    resetKey: organizationDragResetKey,
    isDropTargetValid: validateOrganizationDropTarget,
    onDrop: dropFiles,
  })

  useEffect(() => {
    cancelOrganizationPointerDrag()
  }, [cancelOrganizationPointerDrag, state.workspace])

  const runRadialAction = useCallback(
    (action: RadialLeafAction) => {
      const files =
        state.status === 'active' && radialMenu?.projectIdentity === projectIdentity
          ? radialMenu.files
          : []
      if (files.length === 0) {
        finishRadialSession()
        return
      }
      finishRadialSession()
      const ids = files.map((file) => file.entityId)
      if (action === 'preview') {
        const validation = validatePreviewSelection(files)
        if (!validation.ok) return
        if (validation.mode === 'single') {
          emitIntent({
            kind: 'open-preview',
            file: defined(files[0], 'Single preview requires one file'),
            files: null,
            folderOverviewIdentity: null,
          })
        } else {
          const first = defined(files[0], 'Split text preview requires a left file')
          const second = defined(files[1], 'Split text preview requires a right file')
          emitIntent({
            kind: 'open-preview',
            file: first,
            files: [first, second],
            folderOverviewIdentity: null,
          })
        }
      } else if (action === 'mark.keep') void setReviewState('keep', ids)
      else if (action === 'mark.pending') void setReviewState('pending', ids)
      else if (action === 'mark.reject') void setReviewState('reject', ids)
      else if (action === 'mark.clear') void setReviewState(null, ids)
      else if (action === 'mark.favorite') void toggleFavorite(ids)
      else if (action === 'organize.rename') emitIntent({ kind: 'start-rename', files })
      else if (action === 'organize.copy') openDestinationDialog('copy', files)
      else if (action === 'organize.move') openDestinationDialog('move', files)
      else if (action === 'trash') openTrashDialog(files)
      else if (action === 'compare') emitIntent({ kind: 'enter-compare', files })
      else if (action === 'info') emitIntent({ kind: 'open-info' })
    },
    [
      emitIntent,
      finishRadialSession,
      openDestinationDialog,
      openTrashDialog,
      projectIdentity,
      radialMenu,
      setReviewState,
      state.status,
      toggleFavorite,
    ],
  )

  useEffect(() => {
    function toggleInfo(event: KeyboardEvent) {
      if (
        !(
          event.metaKey &&
          !event.ctrlKey &&
          !event.altKey &&
          !event.shiftKey &&
          event.key.toLowerCase() === 'i'
        ) ||
        organizationShortcutIsOwned(
          event,
          operationDialog !== null ||
            activePreviewOpen ||
            compareOpen ||
            resultsBatchId !== null ||
            operationBusy ||
            state.closeBlocked !== null,
        )
      ) {
        return
      }
      event.preventDefault()
      emitIntent({ kind: 'open-info' })
    }
    window.addEventListener('keydown', toggleInfo)
    return () => window.removeEventListener('keydown', toggleInfo)
  }, [
    activePreviewOpen,
    compareOpen,
    emitIntent,
    operationBusy,
    operationDialog,
    resultsBatchId,
    state.closeBlocked,
  ])

  useEffect(() => {
    function handleOrganizationShortcut(event: KeyboardEvent) {
      if (
        organizationShortcutIsOwned(
          event,
          operationDialog !== null ||
            activePreviewOpen ||
            compareOpen ||
            infoOpen ||
            resultsBatchId !== null ||
            operationBusy ||
            state.status !== 'active' ||
            state.closeBlocked !== null,
        )
      ) {
        return
      }
      if (
        event.metaKey &&
        !event.ctrlKey &&
        !event.altKey &&
        !event.shiftKey &&
        event.key.toLowerCase() === 'z'
      ) {
        if (operationBusy || event.repeat) return
        event.preventDefault()
        void undoLastOperation()
        return
      }
      if (event.metaKey || event.ctrlKey || event.altKey || event.shiftKey) return
      if (
        (event.key === ' ' || event.key === 'Spacebar' || event.code === 'Space') &&
        selectedFiles.length === 1
      ) {
        event.preventDefault()
        emitIntent({
          kind: 'open-preview',
          file: defined(selectedFiles[0], 'Missing selected preview file'),
          files: null,
          folderOverviewIdentity: null,
        })
      } else if (event.key.toLowerCase() === 'c') {
        event.preventDefault()
        emitIntent({ kind: 'enter-compare', files: selectedFiles })
      } else if (event.key === 'Enter') {
        if (!canMutateSelection) return
        event.preventDefault()
        emitIntent({ kind: 'start-rename', files: selectedFiles })
      } else if (event.key === 'Delete' || event.key === 'Backspace') {
        if (!canMutateSelection) return
        event.preventDefault()
        openTrashDialog()
      }
    }
    window.addEventListener('keydown', handleOrganizationShortcut)
    return () => window.removeEventListener('keydown', handleOrganizationShortcut)
  }, [
    activePreviewOpen,
    canMutateSelection,
    compareOpen,
    emitIntent,
    infoOpen,
    openTrashDialog,
    operationBusy,
    operationDialog,
    resultsBatchId,
    selectedFiles,
    state.closeBlocked,
    state.status,
    undoLastOperation,
  ])

  useReviewShortcuts({
    disabled:
      state.project?.access === 'read_only' ||
      state.selectedEntityIds.length === 0 ||
      operationBusy ||
      state.status !== 'active' ||
      operationDialog !== null ||
      activePreviewOpen ||
      compareOpen ||
      infoOpen ||
      resultsBatchId !== null ||
      state.closeBlocked !== null,
    onSetReview: (reviewState) => void setReviewState(reviewState),
    onToggleFavorite: () => void toggleFavorite(),
  })

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
    radialMenu,
    activeRadialMenu,
    radialModel,
    beginRadialSession,
    finishRadialSession,
    runRadialAction,
    organizationDragView,
    organizationDropTarget,
    handleOrganizationPointerInput,
  }
}
