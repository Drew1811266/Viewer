import type { CSSProperties, PointerEvent as ReactPointerEvent } from 'react'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import type {
  BrowserFile,
  ConflictResolution,
  FileCommandItem,
  FileCommandPreflight,
  ImageRepresentationRequest,
  RenamePreview,
  RenameRules,
  TextEncoding,
} from './api/types'
import type { ViewerBridge } from './api/viewer'
import { tauriViewerBridge } from './api/viewer'
import BatchRenameDialog from './components/BatchRenameDialog'
import CloseOperationDialog from './components/CloseOperationDialog'
import CompareWorkspace from './components/CompareWorkspace'
import ContentBrowser from './components/ContentBrowser'
import DestinationDialog from './components/DestinationDialog'
import EmptyProject from './components/EmptyProject'
import FolderOverview from './components/FolderOverview'
import FolderTree from './components/FolderTree'
import ImagePreview from './components/ImagePreview'
import InfoOverlay from './components/InfoOverlay'
import OperationResults from './components/OperationResults'
import type { RadialMenuRequest } from './components/RadialFileMenu'
import RadialFileMenu from './components/RadialFileMenu'
import ReadOnlyBanner from './components/ReadOnlyBanner'
import RenameDialog from './components/RenameDialog'
import type { RadialLeafAction } from './components/radialMenuModel'
import { buildRadialMenuModel } from './components/radialMenuModel'
import SearchResults from './components/SearchResults'
import SearchToolbar from './components/SearchToolbar'
import type { TaskFeedback } from './components/TaskBar'
import TaskBar from './components/TaskBar'
import TextPreview from './components/TextPreview'
import TrashConfirmation from './components/TrashConfirmation'
import { organizationShortcutIsOwned } from './state/organizationShortcutOwnership'
import type { OrganizationDragMode } from './state/useOrganizationPointerDrag'
import { useOrganizationPointerDrag } from './state/useOrganizationPointerDrag'
import useReviewShortcuts from './state/useReviewShortcuts'
import { useViewerController } from './state/useViewerController'

interface AppProps {
  bridge?: ViewerBridge
}

type OperationDialog =
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

type RadialMenuSession = RadialMenuRequest & {
  requestId: number
  projectIdentity: string
}

interface PreviewSession {
  file: BrowserFile
  files: BrowserFile[] | null
  folderOverviewIdentity: string | null
}

export default function App({ bridge = tauriViewerBridge }: AppProps) {
  const {
    state,
    openProject,
    closeProject,
    reselectProject,
    selectFolder,
    showAllDescendants,
    cancelTask,
    setSearchText,
    setSearchScope,
    setSearchFilters,
    setSearchSort,
    setSearchLayout,
    removeSearchFilter,
    clearSearchFilters,
    setVisibleSearchHits,
    setSearchPage,
    returnToFolderContext,
    setSelectedEntityIds,
    setReviewState,
    toggleFavorite,
    previewRename,
    preflightFileCommand,
    executeFileCommand,
    cancelOperation,
    loadOperationResults,
    undoLastOperation,
    setPreviewEntityId,
    setCompareEntityIds,
    consumeContextRepair,
    clearCloseBlocked,
    openPermissionSettings,
  } = useViewerController(bridge)
  const radialProjectIdentity = state.project
    ? `${state.project.sessionId}:${state.project.generation}`
    : 'no-project'
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false)
  const [sidebarWidth, setSidebarWidth] = useState(260)
  const [projectMenuOpen, setProjectMenuOpen] = useState(false)
  const startSidebarResize = useCallback(
    (event: ReactPointerEvent<HTMLButtonElement>) => {
      if (sidebarCollapsed || event.button !== 0) return
      event.preventDefault()
      const startX = event.clientX
      const startWidth = sidebarWidth
      const move = (next: PointerEvent) => {
        setSidebarWidth(Math.max(200, Math.min(420, startWidth + next.clientX - startX)))
      }
      const stop = () => {
        window.removeEventListener('pointermove', move)
        window.removeEventListener('pointerup', stop)
        window.removeEventListener('pointercancel', stop)
      }
      window.addEventListener('pointermove', move)
      window.addEventListener('pointerup', stop)
      window.addEventListener('pointercancel', stop)
    },
    [sidebarCollapsed, sidebarWidth],
  )
  const [thumbnailTask, setThumbnailTask] = useState<TaskFeedback | null>(null)
  const [textTask, setTextTask] = useState<TaskFeedback | null>(null)
  const [dismissedTasks, setDismissedTasks] = useState<Set<string>>(() => new Set())
  const [activePreview, setActivePreview] = useState<PreviewSession | null>(null)
  const [selectedFiles, setSelectedFiles] = useState<BrowserFile[]>([])
  const [finderDragMessage, setFinderDragMessage] = useState<string | null>(null)
  const [compareStatus, setCompareStatus] = useState<string | null>(null)
  const [operationDialog, setOperationDialog] = useState<OperationDialog | null>(null)
  const [operationSubmitting, setOperationSubmitting] = useState(false)
  const [resultsBatchId, setResultsBatchId] = useState<string | null>(null)
  const [infoOpen, setInfoOpen] = useState(false)
  const [radialMenu, setRadialMenu] = useState<RadialMenuSession | null>(null)
  const radialReturnFocusTarget = useRef<HTMLElement | null>(null)
  const radialRequestSequence = useRef(0)
  const beginRadialSession = useCallback(
    (request: RadialMenuRequest) => {
      if (state.status !== 'active' || state.project === null) return
      const returnFocusTarget = radialReturnFocusTarget.current ?? request.returnFocusTarget
      radialReturnFocusTarget.current = returnFocusTarget
      radialRequestSequence.current += 1
      setRadialMenu({
        ...request,
        requestId: radialRequestSequence.current,
        projectIdentity: radialProjectIdentity,
        returnFocusTarget,
      })
    },
    [radialProjectIdentity, state.project, state.status],
  )
  const finishRadialSession = useCallback((reportedReturnTarget: HTMLElement | null = null) => {
    setRadialMenu(null)
    const returnFocusTarget = radialReturnFocusTarget.current ?? reportedReturnTarget
    radialReturnFocusTarget.current = null
    if (returnFocusTarget?.isConnected) returnFocusTarget.focus()
  }, [])
  const [dimensions, setDimensions] = useState<
    Record<string, { width: number; height: number } | undefined>
  >({})
  useEffect(() => {
    setThumbnailTask(null)
    setTextTask(null)
    setDismissedTasks(new Set())
    setActivePreview(null)
    setSelectedFiles([])
    setFinderDragMessage(null)
    setCompareStatus(null)
    setOperationDialog(null)
    setOperationSubmitting(false)
    setResultsBatchId(null)
    setInfoOpen(false)
    setProjectMenuOpen(false)
    setDimensions({})
  }, [state.project?.sessionId])
  const requestThumbnail = useCallback(
    (file: BrowserFile) =>
      bridge
        .requestImage({
          entityId: file.entityId,
          representation: { kind: 'thumbnail', maxPixels: 320, scaleMilli: 1_000 },
        })
        .then((image) => image.url),
    [bridge],
  )
  const requestFolderImages = useCallback(
    async (entityId: string) => {
      const workspace = await bridge.queryFolder(entityId, false)
      return workspace.workspace === 'content' ? workspace.images : []
    },
    [bridge],
  )
  const requestContentThumbnail = useCallback(
    (file: BrowserFile, maxPixels: number, scaleMilli: number) =>
      bridge
        .requestImage({
          entityId: file.entityId,
          representation: { kind: 'thumbnail', maxPixels, scaleMilli },
        })
        .then((image) => image.url),
    [bridge],
  )
  const requestPreviewImage = useCallback(
    (file: BrowserFile, representation: ImageRepresentationRequest) =>
      bridge.requestImage({ entityId: file.entityId, representation }),
    [bridge],
  )
  const requestTextPreview = useCallback(
    (file: BrowserFile, encoding?: TextEncoding) =>
      bridge.previewText({ entityId: file.entityId, encoding }),
    [bridge],
  )
  const rememberDimensions = useCallback((entityId: string, width: number, height: number) => {
    setDimensions((current) => ({ ...current, [entityId]: { width, height } }))
  }, [])
  const scanTask = useMemo<TaskFeedback | null>(() => {
    if (state.scan === null) return null
    const published = state.scan.publishedFolders + state.scan.publishedFiles
    const failed = state.scan.totals?.failed ?? state.scan.failedItems.length
    const requested = state.scan.totals
      ? state.scan.totals.folders + state.scan.totals.files + failed
      : published + failed + 1
    return {
      id: state.scan.taskId,
      label: '扫描项目',
      status:
        state.scan.phase === 'running'
          ? 'running'
          : state.scan.phase === 'cancelled'
            ? 'cancelled'
            : failed > 0
              ? 'failed'
              : 'complete',
      requested,
      completed: state.scan.totals
        ? state.scan.totals.folders + state.scan.totals.files
        : published,
      failed,
      cancellable: state.scan.phase === 'running',
      failures: state.scan.failedItems.map((failure) => ({
        item: failure.relativePath,
        code: failure.code,
      })),
    }
  }, [state.scan])
  const operationTask = useMemo<TaskFeedback | null>(() => {
    const progress = state.operation.active
    if (progress === null) return null
    const running = progress.lifecycle !== 'completed'
    const failures =
      state.operation.results?.items
        .filter((item) => item.status === 'failed')
        .map((item) => ({ item: item.relativePath, code: item.code })) ?? []
    return {
      id: progress.batchId,
      label: operationLabel(state.operation.kind),
      status: running
        ? 'running'
        : progress.failed > 0
          ? 'failed'
          : progress.cancelled === progress.requested && progress.requested > 0
            ? 'cancelled'
            : 'complete',
      requested: progress.requested,
      completed: progress.completed,
      failed: progress.failed,
      skipped: progress.skipped,
      cancelled: progress.cancelled,
      cancellable: progress.lifecycle === 'queued' || progress.lifecycle === 'running',
      failures,
      hasResults: state.operation.results !== null,
    }
  }, [state.operation])
  const visibleTasks = useMemo(
    () =>
      [scanTask, thumbnailTask, textTask, operationTask].filter(
        (task): task is TaskFeedback =>
          task !== null && (!dismissedTasks.has(task.id) || task.status === 'running'),
      ),
    [dismissedTasks, operationTask, scanTask, textTask, thumbnailTask],
  )
  const selectFolderTarget = useCallback(
    (entityId: string | null) => {
      setSelectedFiles([])
      setSelectedEntityIds(entityId === null ? [] : [entityId])
      void selectFolder(entityId)
    },
    [selectFolder, setSelectedEntityIds],
  )
  const selectFiles = useCallback(
    (files: BrowserFile[]) => {
      setSelectedFiles(files)
      setSelectedEntityIds(files.map((file) => file.entityId))
    },
    [setSelectedEntityIds],
  )
  const operationBusy =
    operationSubmitting ||
    state.status !== 'active' ||
    state.operation.finishing ||
    state.operation.pending ||
    (state.operation.active !== null && state.operation.active.lifecycle !== 'completed')
  const canMutateSelection =
    selectedFiles.length > 0 && state.project?.access === 'read_write' && !operationBusy
  const compareFiles = useMemo(() => {
    if (state.workspace?.workspace !== 'content') return []
    const byId = new Map(state.workspace.images.map((file) => [file.entityId, file]))
    return state.compareEntityIds.flatMap((entityId) => {
      const file = byId.get(entityId)
      return file === undefined ? [] : [file]
    })
  }, [state.compareEntityIds, state.workspace])
  const compareOpen = state.compareEntityIds.length >= 2 && compareFiles.length >= 2
  const compareEntryAvailable =
    state.workspace?.workspace === 'content' && !state.search.showResults && !operationBusy
  const activeRadialMenu =
    state.status === 'active' && radialMenu?.projectIdentity === radialProjectIdentity
      ? radialMenu
      : null
  const radialModel = useMemo(() => {
    const files = activeRadialMenu?.files ?? []
    const reviews = new Set(files.map((file) => file.marker.reviewState))
    const favorites = new Set(files.map((file) => file.marker.favorite))
    return buildRadialMenuModel({
      selectedCount: files.length,
      selectedImageCount: files.filter(matchesImage).length,
      readOnly: state.project?.access === 'read_only',
      busy: operationBusy,
      compareContextAvailable: compareEntryAvailable,
      commonReview: reviews.size === 1 ? (files[0]?.marker.reviewState ?? null) : 'mixed',
      commonFavorite: favorites.size === 1 ? (files[0]?.marker.favorite ?? false) : 'mixed',
    })
  }, [activeRadialMenu, compareEntryAvailable, operationBusy, state.project?.access])

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
            activePreview !== null ||
            compareOpen ||
            resultsBatchId !== null ||
            operationBusy ||
            state.closeBlocked !== null,
        )
      ) {
        return
      }
      event.preventDefault()
      setInfoOpen((open) => !open)
    }
    window.addEventListener('keydown', toggleInfo)
    return () => window.removeEventListener('keydown', toggleInfo)
  }, [
    activePreview,
    compareOpen,
    operationBusy,
    operationDialog,
    resultsBatchId,
    state.closeBlocked,
  ])

  const exportToFinder = useCallback(
    (entityIds: string[]) => {
      const project = state.project
      setFinderDragMessage(null)
      if (project === null || state.status !== 'active' || entityIds.length === 0) return
      void bridge
        .beginFinderDrag({
          sessionId: project.sessionId,
          generation: project.generation,
          entityIds: [...entityIds],
        })
        .catch((error: unknown) => {
          const code =
            typeof error === 'object' && error !== null && 'code' in error
              ? String(error.code)
              : null
          setFinderDragMessage(
            code === 'finder_drag_selection_stale' || code === 'stale_project_session'
              ? '部分文件已发生变化，请刷新后重试。'
              : '无法拖到 Finder，请重新拖动。',
          )
        })
    },
    [bridge, state.project, state.status],
  )

  useEffect(() => {
    if (
      operationDialog !== null &&
      (state.status !== 'active' ||
        state.project?.access !== 'read_write' ||
        (state.operation.active !== null && state.operation.active.lifecycle !== 'completed'))
    ) {
      setOperationDialog(null)
    }
  }, [operationDialog, state.operation.active, state.project?.access, state.status])

  const openRenameDialog = useCallback(() => {
    if (!canMutateSelection) return
    setOperationDialog(
      selectedFiles.length === 1
        ? { kind: 'rename', file: selectedFiles[0]! }
        : { kind: 'batch_rename', files: selectedFiles },
    )
  }, [canMutateSelection, selectedFiles])

  const openTrashDialog = useCallback(() => {
    if (!canMutateSelection) return
    setOperationDialog({ kind: 'trash', files: selectedFiles })
  }, [canMutateSelection, selectedFiles])

  const submitFileCommand = useCallback(
    async (
      kind: 'rename' | 'copy' | 'move' | 'trash',
      items: FileCommandItem[],
      conflicts: ConflictResolution[] = [],
    ) => {
      setOperationSubmitting(true)
      try {
        const started = await executeFileCommand(kind, items, conflicts)
        if (started) setOperationDialog(null)
        return started
      } finally {
        setOperationSubmitting(false)
      }
    },
    [executeFileCommand],
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
      const currentFiles = [...state.workspace.images, ...state.workspace.textFiles]
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
      setOperationDialog({
        kind: 'destination',
        mode,
        files: files as BrowserFile[],
        initialDestinationId: destinationId,
        initialPreflight: preflight,
      })
    },
    [
      operationBusy,
      preflightFileCommand,
      state.project?.access,
      state.workspace,
      submitFileCommand,
    ],
  )

  const isOrganizationDropTargetValid = useCallback(
    (entityIds: readonly string[], destinationId: string, mode: OrganizationDragMode) => {
      if (state.workspace?.workspace !== 'content') return false
      const destination = state.folders.find((folder) => folder.entityId === destinationId)
      if (!destination) return false
      const currentFiles = [...state.workspace.images, ...state.workspace.textFiles]
      const byId = new Map(currentFiles.map((file) => [file.entityId, file]))
      const files = entityIds.map((entityId) => byId.get(entityId))
      if (files.some((file) => file === undefined)) return false
      return (
        mode === 'copy' ||
        files.every(
          (file) =>
            file !== undefined &&
            parentRelativePath(file.relativePath) !== destination.relativePath,
        )
      )
    },
    [state.folders, state.workspace],
  )

  const organizationWorkspaceIdentity = [
    state.workspace?.workspace ?? 'none',
    state.selectedFolderId ?? 'root',
    state.showingAggregate ? 'aggregate' : 'folder',
    state.search.showResults ? 'search' : 'browser',
  ].join(':')
  const organizationDragResetKey = [
    state.project?.sessionId ?? 'no-session',
    state.project?.generation ?? 'no-generation',
    organizationWorkspaceIdentity,
  ].join(':')

  useEffect(() => {
    if (radialMenu !== null) finishRadialSession()
    // Deliberately exclude radialMenu: only a menu present when context changed is stale.
  }, [
    activePreview,
    compareOpen,
    finishRadialSession,
    infoOpen,
    operationDialog,
    organizationWorkspaceIdentity,
    resultsBatchId,
    state.closeBlocked,
    state.contextRepair,
    state.status,
    radialProjectIdentity,
  ])
  const {
    dragView: organizationDragView,
    dropTarget: organizationDropTarget,
    handlePointerInput: handleOrganizationPointerInput,
    cancel: cancelOrganizationPointerDrag,
  } = useOrganizationPointerDrag({
    disabled: state.project?.access !== 'read_write' || operationBusy || compareOpen,
    resetKey: organizationDragResetKey,
    isDropTargetValid: isOrganizationDropTargetValid,
    onDrop: dropFiles,
  })

  const [folderOverviewProjectionState, setFolderOverviewProjectionState] = useState(() => ({
    projection: state.workspace,
    sequence: 0,
  }))
  let folderOverviewSequence = folderOverviewProjectionState.sequence
  if (folderOverviewProjectionState.projection !== state.workspace) {
    folderOverviewSequence += 1
    setFolderOverviewProjectionState({
      projection: state.workspace,
      sequence: folderOverviewSequence,
    })
  }
  const folderOverviewIdentity = [
    state.project?.sessionId ?? 'no-session',
    state.project?.generation ?? 0,
    state.selectedFolderId ?? 'root',
    folderOverviewSequence,
  ].join(':')

  useEffect(() => {
    cancelOrganizationPointerDrag()
  }, [cancelOrganizationPointerDrag, state.workspace])

  const openPreview = useCallback(
    (file: BrowserFile) => {
      setActivePreview({ file, files: null, folderOverviewIdentity: null })
      setPreviewEntityId(file.entityId)
    },
    [setPreviewEntityId],
  )

  const openFilmstripPreview = useCallback(
    (file: BrowserFile, files: BrowserFile[]) => {
      setActivePreview({ file, files, folderOverviewIdentity })
      setPreviewEntityId(file.entityId)
    },
    [folderOverviewIdentity, setPreviewEntityId],
  )

  const navigatePreview = useCallback(
    (file: BrowserFile) => {
      setActivePreview((current) => (current === null ? null : { ...current, file }))
      setPreviewEntityId(file.entityId)
    },
    [setPreviewEntityId],
  )

  const closePreview = useCallback(() => {
    setActivePreview(null)
    setPreviewEntityId(null)
  }, [setPreviewEntityId])

  const openComparison = useCallback(
    (files: BrowserFile[] = selectedFiles) => {
      if (!compareEntryAvailable) {
        setCompareStatus(
          operationBusy
            ? '请等待当前文件操作完成后再开始对比。'
            : '请先返回文件夹内容，再选择图片进行对比。',
        )
        return
      }
      if (files.length < 2 || files.length > 4 || !files.every(matchesImage)) {
        setCompareStatus('请选择 2–4 张 JPG 或 PNG 图片进行对比。')
        return
      }
      setCompareStatus(null)
      setActivePreview(null)
      setPreviewEntityId(null)
      setCompareEntityIds(files.map((file) => file.entityId))
    },
    [compareEntryAvailable, operationBusy, selectedFiles, setCompareEntityIds, setPreviewEntityId],
  )

  const runRadialAction = useCallback(
    (action: RadialLeafAction) => {
      const files =
        state.status === 'active' && radialMenu?.projectIdentity === radialProjectIdentity
          ? radialMenu.files
          : []
      if (files.length === 0) {
        finishRadialSession()
        return
      }
      finishRadialSession()
      const ids = files.map((file) => file.entityId)
      if (action === 'preview' && files.length === 1) openPreview(files[0]!)
      else if (action === 'mark.keep') void setReviewState('keep', ids)
      else if (action === 'mark.pending') void setReviewState('pending', ids)
      else if (action === 'mark.reject') void setReviewState('reject', ids)
      else if (action === 'mark.clear') void setReviewState(null, ids)
      else if (action === 'mark.favorite') void toggleFavorite(ids)
      else if (action === 'organize.rename') {
        setOperationDialog(
          files.length === 1
            ? { kind: 'rename', file: files[0]! }
            : { kind: 'batch_rename', files },
        )
      } else if (action === 'organize.copy') {
        setOperationDialog({ kind: 'destination', mode: 'copy', files })
      } else if (action === 'organize.move') {
        setOperationDialog({ kind: 'destination', mode: 'move', files })
      } else if (action === 'trash') setOperationDialog({ kind: 'trash', files })
      else if (action === 'compare') openComparison(files)
      else if (action === 'info') setInfoOpen(true)
    },
    [
      openComparison,
      openPreview,
      radialMenu,
      radialProjectIdentity,
      finishRadialSession,
      setReviewState,
      state.status,
      toggleFavorite,
    ],
  )

  const changeComparedEntities = useCallback(
    (entityIds: string[]) => {
      setCompareStatus(null)
      if (entityIds.length >= 2) {
        setCompareEntityIds(entityIds)
        return
      }
      setCompareEntityIds([])
      if (entityIds.length !== 1 || state.workspace?.workspace !== 'content') return
      const survivor = state.workspace.images.find((file) => file.entityId === entityIds[0])
      if (survivor !== undefined) openPreview(survivor)
    },
    [openPreview, setCompareEntityIds, state.workspace],
  )

  useEffect(() => {
    if (state.compareEntityIds.length === 0) return
    const liveIds = compareFiles.map((file) => file.entityId)
    if (liveIds.length !== state.compareEntityIds.length || liveIds.length < 2) {
      changeComparedEntities(liveIds)
    }
  }, [changeComparedEntities, compareFiles, state.compareEntityIds])

  useEffect(() => {
    if (compareOpen && state.search.showResults) changeComparedEntities([])
  }, [changeComparedEntities, compareOpen, state.search.showResults])

  useEffect(() => {
    const active = state.operation.active
    if (active?.lifecycle === 'completed' && state.operation.results !== null) {
      setResultsBatchId(active.batchId)
    }
  }, [state.operation.active, state.operation.results])

  useEffect(() => {
    if (
      activePreview &&
      state.contextRepair?.removedEntityIds.includes(activePreview.file.entityId)
    ) {
      setActivePreview(null)
    }
  }, [activePreview, state.contextRepair])

  useEffect(() => {
    if (
      activePreview !== null &&
      activePreview.folderOverviewIdentity !== null &&
      activePreview.folderOverviewIdentity !== folderOverviewIdentity
    ) {
      setActivePreview(null)
      setPreviewEntityId(null)
    }
  }, [activePreview, folderOverviewIdentity, setPreviewEntityId])

  useEffect(() => {
    function handleOrganizationShortcut(event: KeyboardEvent) {
      if (
        organizationShortcutIsOwned(
          event,
          operationDialog !== null ||
            activePreview !== null ||
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
      if (event.key.toLowerCase() === 'c') {
        event.preventDefault()
        openComparison()
      } else if (event.key === 'Enter') {
        if (!canMutateSelection) return
        event.preventDefault()
        openRenameDialog()
      } else if (event.key === 'Delete' || event.key === 'Backspace') {
        if (!canMutateSelection) return
        event.preventDefault()
        openTrashDialog()
      }
    }
    window.addEventListener('keydown', handleOrganizationShortcut)
    return () => window.removeEventListener('keydown', handleOrganizationShortcut)
  }, [
    activePreview,
    canMutateSelection,
    compareOpen,
    infoOpen,
    openRenameDialog,
    openComparison,
    openTrashDialog,
    operationDialog,
    operationBusy,
    resultsBatchId,
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
      activePreview !== null ||
      compareOpen ||
      infoOpen ||
      resultsBatchId !== null ||
      state.closeBlocked !== null,
    onSetReview: (reviewState) => void setReviewState(reviewState),
    onToggleFavorite: () => void toggleFavorite(),
  })

  if (state.project === null) {
    return (
      <EmptyProject
        bridge={bridge}
        busy={state.status === 'opening'}
        errorMessage={state.errorMessage}
        onOpenProject={openProject}
      />
    )
  }

  const activePreviewFiles =
    activePreview?.files ?? (state.workspace?.workspace === 'content' ? state.workspace.images : [])
  const activePreviewFile =
    activePreview === null
      ? null
      : (activePreviewFiles.find(
          (candidate) => candidate.entityId === activePreview.file.entityId,
        ) ?? activePreview.file)

  return (
    <main
      className="viewer-shell"
      data-organization-drag-active={organizationDragView ? true : undefined}
    >
      <header className="workspace-header">
        <h1>{state.project.displayName}</h1>
        <SearchToolbar
          query={state.search.query}
          folders={state.folders}
          focusRequest={state.search.focusRequest}
          onTextChange={setSearchText}
          onScopeChange={setSearchScope}
          onFiltersChange={setSearchFilters}
          onSortChange={setSearchSort}
          onLayoutChange={setSearchLayout}
          onRemoveFilter={removeSearchFilter}
          onClearFilters={clearSearchFilters}
        />
        <details className="project-menu" open={projectMenuOpen}>
          <summary
            aria-label="项目菜单"
            role="button"
            aria-expanded={projectMenuOpen}
            onClick={(event) => {
              event.preventDefault()
              setProjectMenuOpen((open) => !open)
            }}
            onKeyDown={(event) => {
              if (event.key !== 'Enter' && event.key !== ' ' && event.key !== 'Spacebar') return
              event.preventDefault()
              setProjectMenuOpen((open) => !open)
            }}
          >
            •••
          </summary>
          <div hidden={!projectMenuOpen}>
            {state.project.access === 'read_only' && (
              <p className="project-access-status">访问权限：只读</p>
            )}
            <button
              type="button"
              disabled={state.status === 'closing'}
              onClick={() => void closeProject()}
            >
              {state.status === 'closing' ? '正在关闭…' : '关闭项目'}
            </button>
          </div>
        </details>
      </header>
      {state.project.access === 'read_only' && (
        <ReadOnlyBanner
          busy={state.status === 'closing'}
          onOpenSettings={() => void openPermissionSettings()}
          onReselect={() => void reselectProject()}
        />
      )}
      {state.recoveryReport &&
        (state.recoveryReport.recovered > 0 || state.recoveryReport.needsUserReview > 0) && (
          <p className="recovery-banner" role="status">
            已恢复 {state.recoveryReport.recovered} 项操作；
            {state.recoveryReport.needsUserReview} 项需要检查。
          </p>
        )}
      {state.contextRepair && (
        <p className="context-repair-banner" role="status">
          {state.contextRepair.message}
        </p>
      )}
      {state.errorMessage && <p role="alert">{state.errorMessage}</p>}
      {finderDragMessage && (
        <p className="finder-drag-error" role="alert">
          {finderDragMessage}
        </p>
      )}
      {compareStatus && (
        <p className="compare-status" role="status">
          {compareStatus}
        </p>
      )}
      <div className="viewer-columns">
        <aside
          className="folder-sidebar"
          aria-label="文件夹栏"
          style={{ width: sidebarCollapsed ? 44 : sidebarWidth }}
        >
          <button
            type="button"
            aria-label={sidebarCollapsed ? '展开文件夹栏' : '折叠文件夹栏'}
            onClick={() => setSidebarCollapsed((collapsed) => !collapsed)}
          >
            {sidebarCollapsed ? '›' : '‹'}
          </button>
          {!sidebarCollapsed && (
            <>
              <button
                type="button"
                className="project-root-button"
                aria-pressed={state.selectedFolderId === null}
                onClick={() => selectFolderTarget(null)}
              >
                项目根目录
              </button>
              <FolderTree
                folders={state.folders}
                selectedId={state.selectedFolderId}
                onSelect={selectFolderTarget}
                organizationDropTarget={organizationDropTarget}
              />
              <button
                type="button"
                className="sidebar-separator"
                role="separator"
                aria-label="调整文件夹栏宽度"
                aria-orientation="vertical"
                aria-valuemin={200}
                aria-valuemax={420}
                aria-valuenow={sidebarWidth}
                onPointerDown={startSidebarResize}
                onKeyDown={(event) => {
                  if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return
                  event.preventDefault()
                  const delta = event.key === 'ArrowLeft' ? -16 : 16
                  setSidebarWidth((width) => Math.max(200, Math.min(420, width + delta)))
                }}
              />
            </>
          )}
        </aside>
        <section className="workspace" aria-label="项目内容">
          {state.search.showResults &&
            state.search.page === null &&
            state.search.status === 'searching' && <p role="status">正在搜索…</p>}
          {state.search.showResults &&
            state.search.page === null &&
            state.search.status === 'error' && <p>搜索未完成，请调整条件或重试。</p>}
          {state.search.showResults && state.search.page !== null && (
            <SearchResults
              page={state.search.page}
              query={state.search.query}
              snippets={state.search.snippets}
              offset={state.search.offset}
              limit={200}
              onPageChange={setSearchPage}
              onVisibleHits={setVisibleSearchHits}
              onClearFilters={clearSearchFilters}
              onSearchProject={() => setSearchScope(null)}
              onReturnToFolder={returnToFolderContext}
            />
          )}
          {!state.search.showResults && state.workspace === null && <p>正在读取项目…</p>}
          {!state.search.showResults && state.workspace?.workspace === 'empty' && (
            <p>此文件夹中没有支持的文件。</p>
          )}
          {!state.search.showResults && state.workspace?.workspace === 'category' && (
            <FolderOverview
              key={folderOverviewIdentity}
              folders={state.workspace.folders}
              currentPath={state.selectedFolderPath || state.project.displayName}
              requestFolderImages={requestFolderImages}
              requestThumbnail={requestThumbnail}
              onPreview={openFilmstripPreview}
              onSelect={selectFolderTarget}
              onShowAll={() => void showAllDescendants()}
            />
          )}
          {!state.search.showResults && state.workspace?.workspace === 'content' && (
            <>
              <div className="content-workspace-surface" hidden={compareOpen}>
                {state.showingAggregate && <p className="aggregate-label">全部后代文件</p>}
                <ContentBrowser
                  workspace={state.workspace}
                  currentPath={state.selectedFolderPath || state.project.displayName}
                  requestThumbnail={requestContentThumbnail}
                  onThumbnailTaskChange={setThumbnailTask}
                  onPreview={openPreview}
                  onSelectionChange={selectFiles}
                  organizationDragDisabled={
                    state.project.access !== 'read_write' || operationBusy || compareOpen
                  }
                  onFinderDragStart={exportToFinder}
                  onOrganizationPointerInput={handleOrganizationPointerInput}
                  repairSelectionId={state.contextRepair?.suggestedEntityId ?? null}
                  onRepairSelectionApplied={consumeContextRepair}
                  onRadialMenuRequest={beginRadialSession}
                />
              </div>
              {compareOpen && (
                <CompareWorkspace
                  files={compareFiles}
                  readOnly={state.project.access === 'read_only'}
                  requestImage={requestPreviewImage}
                  onEntityIdsChange={changeComparedEntities}
                  onSetReview={(entityId, reviewState) =>
                    void setReviewState(reviewState, [entityId])
                  }
                  onToggleFavorite={(entityId) => void toggleFavorite([entityId])}
                  onStatus={setCompareStatus}
                />
              )}
            </>
          )}
        </section>
      </div>
      {organizationDragView && (
        <div
          className="organization-drag-preview"
          style={
            {
              '--organization-drag-x': `${organizationDragView.clientX}px`,
              '--organization-drag-y': `${organizationDragView.clientY}px`,
            } as CSSProperties
          }
        >
          {organizationDragView.mode === 'copy' ? '复制' : '移动'} {organizationDragView.itemCount}{' '}
          项
        </div>
      )}
      <TaskBar
        tasks={visibleTasks}
        onCancel={(taskId) => {
          if (taskId === state.operation.active?.batchId) void cancelOperation()
          else void cancelTask(taskId)
        }}
        onDismiss={(taskId) => setDismissedTasks((current) => new Set([...current, taskId]))}
        onShowResults={(taskId) => setResultsBatchId(taskId)}
      />
      {activeRadialMenu && (
        <RadialFileMenu
          key={activeRadialMenu.requestId}
          origin={activeRadialMenu.origin}
          pointerId={activeRadialMenu.pointerId}
          selectionCount={activeRadialMenu.files.length}
          readOnly={state.project.access === 'read_only'}
          returnFocusTarget={activeRadialMenu.returnFocusTarget}
          model={radialModel}
          onAction={runRadialAction}
          onClose={finishRadialSession}
        />
      )}
      {activePreviewFile && matchesImage(activePreviewFile) && activePreviewFiles.length > 0 && (
        <ImagePreview
          file={activePreviewFile}
          files={activePreviewFiles}
          requestImage={requestPreviewImage}
          onNavigate={navigatePreview}
          onClose={closePreview}
          onDimensions={rememberDimensions}
        />
      )}
      {activePreviewFile && !matchesImage(activePreviewFile) && (
        <TextPreview
          file={activePreviewFile}
          requestPreview={requestTextPreview}
          openExternalLink={bridge.openExternalLink}
          onClose={closePreview}
          onTaskChange={setTextTask}
        />
      )}
      {infoOpen && (
        <InfoOverlay
          files={selectedFiles}
          selectionInfo={state.selectionInfo}
          dimensions={dimensions}
          onClose={() => setInfoOpen(false)}
        />
      )}
      {operationDialog?.kind === 'rename' && (
        <RenameDialog
          currentName={operationDialog.file.name}
          busy={operationBusy}
          onCancel={() => setOperationDialog(null)}
          onConfirm={(proposedName, editExtension) =>
            void submitFileCommand('rename', [
              {
                entityId: operationDialog.file.entityId,
                action: { kind: 'rename', proposedName, editExtension },
              },
            ])
          }
        />
      )}
      {operationDialog?.kind === 'batch_rename' && (
        <BatchRenameDialog
          entityIds={operationDialog.files.map((file) => file.entityId)}
          busy={operationBusy}
          requestPreview={(entityIds, rules) => previewRename(entityIds, rules)}
          onCancel={() => setOperationDialog(null)}
          onConfirm={(_rules: RenameRules, preview: RenamePreview) =>
            void submitFileCommand(
              'rename',
              preview.rows.map((row) => ({
                entityId: row.entityId,
                action: {
                  kind: 'rename',
                  proposedName: row.proposedName,
                  editExtension: true,
                },
              })),
            )
          }
        />
      )}
      {operationDialog?.kind === 'destination' && (
        <DestinationDialog
          mode={operationDialog.mode}
          entityIds={operationDialog.files.map((file) => file.entityId)}
          folders={state.folders}
          busy={operationBusy}
          initialDestinationId={operationDialog.initialDestinationId}
          initialPreflight={operationDialog.initialPreflight}
          requestPreflight={(items) => preflightFileCommand(operationDialog.mode, items)}
          onCancel={() => setOperationDialog(null)}
          onConfirm={(items, conflicts) =>
            void submitFileCommand(operationDialog.mode, items, conflicts)
          }
        />
      )}
      {operationDialog?.kind === 'trash' && (
        <TrashConfirmation
          count={operationDialog.files.length}
          busy={operationBusy}
          onCancel={() => setOperationDialog(null)}
          onConfirm={() =>
            void submitFileCommand(
              'trash',
              operationDialog.files.map((file) => ({
                entityId: file.entityId,
                action: { kind: 'trash' },
              })),
            )
          }
        />
      )}
      {resultsBatchId !== null &&
        state.operation.active?.batchId === resultsBatchId &&
        state.operation.results !== null && (
          <OperationResults
            batchId={resultsBatchId}
            progress={state.operation.active}
            page={state.operation.results}
            onPageChange={(offset) => void loadOperationResults(offset)}
            onClose={() => setResultsBatchId(null)}
          />
        )}
      {state.closeBlocked !== null && (
        <CloseOperationDialog
          busy={state.status === 'closing'}
          onWait={() => void closeProject('wait', state.closeBlocked?.target ?? 'project')}
          onCancelPending={() =>
            void closeProject('cancel_pending', state.closeBlocked?.target ?? 'project')
          }
          onStay={clearCloseBlocked}
        />
      )}
    </main>
  )
}

function matchesImage(file: BrowserFile): boolean {
  return file.kind === 'jpeg' || file.kind === 'png'
}

function parentRelativePath(relativePath: string): string {
  const separator = relativePath.lastIndexOf('/')
  return separator === -1 ? '' : relativePath.slice(0, separator)
}

function operationLabel(kind: 'rename' | 'copy' | 'move' | 'trash' | null): string {
  if (kind === 'rename') return '重命名文件'
  if (kind === 'copy') return '复制文件'
  if (kind === 'move') return '移动文件'
  if (kind === 'trash') return '移到废纸篓'
  return '文件操作'
}
