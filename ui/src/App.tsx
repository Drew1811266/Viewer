import type { CSSProperties, MutableRefObject } from 'react'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import type {
  BrowserFile,
  ConflictResolution,
  FileCommandItem,
  ImageRepresentationRequest,
  RenamePreview,
  RenameRules,
  TextEncoding,
  VideoFile,
} from './api/types'
import type { ViewerBridge } from './api/viewer'
import { tauriViewerBridge } from './api/viewer'
import { createProjectThumbnailCache, type ThumbnailLoader } from './app/projectThumbnailCache'
import { useDelayedProjectionProgress } from './app/useDelayedProjectionProgress'
import { useOperationDialogs } from './app/useOperationDialogs'
import { getPreviewSessionInternals, usePreviewSession } from './app/usePreviewSession'
import { useRadialMenuContextToken, useRadialMenuSession } from './app/useRadialMenuSession'
import { createWorkspacePorts } from './app/workspace/ports'
import { useFeedbackCoordinator } from './app/workspace/useFeedbackCoordinator'
import { useWorkspaceShellCoordinator } from './app/workspace/useWorkspaceShellCoordinator'
import BatchRenameDialog from './components/BatchRenameDialog'
import CloseOperationDialog from './components/CloseOperationDialog'
import CompareWorkspace from './components/CompareWorkspace'
import ContentBrowser from './components/ContentBrowser'
import DestinationDialog from './components/DestinationDialog'
import EmptyProject from './components/EmptyProject'
import FolderOverview from './components/FolderOverview'
import FolderTree from './components/FolderTree'
import GlobalNoticeStack from './components/GlobalNoticeStack'
import ImagePreview from './components/ImagePreview'
import InfoOverlay from './components/InfoOverlay'
import type { Point } from './components/imagePreview/imageGeometry'
import { useLatestPointerClientPoint } from './components/imagePreview/useLatestPointerClientPoint'
import OperationResults from './components/OperationResults'
import OrganizationDragPreview from './components/OrganizationDragPreview'
import RadialFileMenu from './components/RadialFileMenu'
import ReadOnlyBanner from './components/ReadOnlyBanner'
import RenameDialog from './components/RenameDialog'
import type { RadialLeafAction } from './components/radialMenuModel'
import { buildRadialMenuModel } from './components/radialMenuModel'
import SearchResults from './components/SearchResults'
import SearchToolbar from './components/SearchToolbar'
import SettingsDialog from './components/SettingsDialog'
import TaskBar from './components/TaskBar'
import TextPreview, { type TextPreviewFiles } from './components/TextPreview'
import TrashConfirmation from './components/TrashConfirmation'
import UnsupportedFilePreview from './components/UnsupportedFilePreview'
import ViewerButton, { ViewerIconButton } from './components/ui/ViewerButton'
import ViewerEmptyState from './components/ui/ViewerEmptyState'
import ViewerStatusTag from './components/ui/ViewerStatusTag'
import VideoPreview from './components/VideoPreview'
import WorkspaceLoadingState from './components/WorkspaceLoadingState'
import WorkspaceMoreMenu from './components/WorkspaceMoreMenu'
import WorkspaceViewMenu, { type WorkspaceViewContext } from './components/WorkspaceViewMenu'
import { defined } from './defined'
import { isImageFile, isPreviewableText, isVideoFile } from './fileKinds'
import { useViewerSettings, ViewerSettingsProvider } from './settings/ViewerSettingsProvider'
import { compareValidationMessage, validateCompareCandidates } from './state/comparePolicy'
import { organizationShortcutIsOwned } from './state/organizationShortcutOwnership'
import { validatePreviewSelection, videoPreviewNeighbors } from './state/previewPolicy'
import type { OrganizationDragMode } from './state/useOrganizationPointerDrag'
import { useOrganizationPointerDrag } from './state/useOrganizationPointerDrag'
import useReviewShortcuts from './state/useReviewShortcuts'
import { useViewerController } from './state/useViewerController'

interface AppProps {
  bridge?: ViewerBridge
}

interface PreviewRepairMemory {
  unavailableEntityIds: ReadonlySet<string>
  message: string | null
}

const EMPTY_PREVIEW_REPAIR: PreviewRepairMemory = {
  unavailableEntityIds: new Set(),
  message: null,
}

export default function App({ bridge = tauriViewerBridge }: AppProps) {
  const pointerClientPoint = useLatestPointerClientPoint()
  return (
    <ViewerSettingsProvider bridge={bridge}>
      <ViewerWorkspace bridge={bridge} pointerClientPoint={pointerClientPoint} />
    </ViewerSettingsProvider>
  )
}

function ViewerWorkspace({
  bridge,
  pointerClientPoint,
}: {
  bridge: ViewerBridge
  pointerClientPoint: MutableRefObject<Point | null>
}) {
  const {
    thumbnailDensity,
    magnifier,
    settingsError,
    setThumbnailDensity,
    setMagnifierShape,
    setMagnifierMagnification,
    setMagnifierArea,
  } = useViewerSettings()
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
  const projectSessionId = state.project?.sessionId ?? 'no-session'
  const radialProjectIdentity = state.project
    ? `${state.project.sessionId}:${state.project.generation}`
    : 'no-project'
  const organizationWorkspaceIdentity = [
    state.workspace?.workspace ?? 'none',
    state.selectedFolderId ?? 'root',
    state.showingAggregate ? 'aggregate' : 'folder',
    state.search.showResults ? 'search' : 'browser',
  ].join(':')
  const ports = useMemo(() => createWorkspacePorts(bridge), [bridge])
  const shell = useWorkspaceShellCoordinator({
    projectSessionId,
    videoProjectSessionId: state.project?.sessionId ?? null,
    port: ports.shell,
  })
  const previewSession = usePreviewSession(projectSessionId)
  const {
    activePreview,
    dimensions,
    openPreview: openPreviewSession,
    openVideoPreview: openVideoPreviewSession,
    closePreview: closePreviewSession,
    recordDimensions,
  } = previewSession
  const { navigatePreview: navigatePreviewSession } = getPreviewSessionInternals(previewSession)
  const { operationDialog, operationSubmitting, setOperationDialog, setOperationSubmitting } =
    useOperationDialogs({
      projectSessionId,
      projectStatus: state.status,
      projectAccess: state.project?.access ?? null,
      activeOperationLifecycle: state.operation.active?.lifecycle ?? null,
    })
  const [selectedFiles, setSelectedFiles] = useState<BrowserFile[]>([])
  const [finderDragMessage, setFinderDragMessage] = useState<string | null>(null)
  const feedback = useFeedbackCoordinator({
    projectSessionId,
    scan: state.scan,
    operation: state.operation,
    projectError: state.errorMessage,
    finderDragMessage,
    workspaceActionError: shell.workspaceActionError,
  })
  const [compareStatus, setCompareStatus] = useState<string | null>(null)
  const [resultsBatchId, setResultsBatchId] = useState<string | null>(null)
  const [previewRepair, setPreviewRepair] = useState<PreviewRepairMemory>(EMPTY_PREVIEW_REPAIR)
  const moreMenuTriggerRef = useRef<HTMLElement>(null)
  useEffect(() => {
    setSelectedFiles([])
    setFinderDragMessage(null)
    setCompareStatus(null)
    setResultsBatchId(null)
    setPreviewRepair(EMPTY_PREVIEW_REPAIR)
  }, [projectSessionId])
  const loadThumbnail = useCallback<ThumbnailLoader>(
    (file: BrowserFile, maxPixels: number, scaleMilli: number) =>
      bridge
        .requestImage({
          entityId: file.entityId,
          representation: { kind: 'thumbnail', maxPixels, scaleMilli },
        })
        .then((image) => image.url),
    [bridge],
  )
  const thumbnailCache = useMemo(
    () => createProjectThumbnailCache(projectSessionId, loadThumbnail),
    [loadThumbnail, projectSessionId],
  )
  useEffect(() => () => thumbnailCache.clear(), [thumbnailCache])
  const requestThumbnail = thumbnailCache.request
  const requestFolderImages = useCallback(
    async (entityId: string) => {
      const workspace = await bridge.queryFolder(entityId, false)
      return workspace.workspace === 'content' ? workspace.images : []
    },
    [bridge],
  )
  const requestPreviewImage = useCallback(
    (file: BrowserFile, representation: ImageRepresentationRequest, signal?: AbortSignal) =>
      bridge.requestImage({ entityId: file.entityId, representation }, signal),
    [bridge],
  )
  const requestTextPreview = useCallback(
    (file: BrowserFile, encoding?: TextEncoding) =>
      bridge.previewText({ entityId: file.entityId, encoding }),
    [bridge],
  )
  const displayedFolderId = state.projectionTransition?.selectedFolderId ?? state.selectedFolderId
  const projectionProgressVisible = useDelayedProjectionProgress(
    state.projectionTransition,
    state.workspace !== null,
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
  const radialContextKey = useRadialMenuContextToken({
    activePreview,
    compareOpen,
    infoOpen: shell.infoOpen,
    operationDialog,
    organizationWorkspaceIdentity,
    resultsBatchId,
    closeBlocked: state.closeBlocked,
    contextRepair: state.contextRepair,
  })
  const { radialMenu, activeRadialMenu, beginRadialSession, finishRadialSession } =
    useRadialMenuSession({
      projectIdentity: radialProjectIdentity,
      projectStatus: state.status,
      contextKey: radialContextKey,
    })
  const radialModel = useMemo(() => {
    const files = activeRadialMenu?.files ?? []
    const reviews = new Set(files.map((file) => file.marker.reviewState))
    const favorites = new Set(files.map((file) => file.marker.favorite))
    const previewValidation = validatePreviewSelection(files)
    return buildRadialMenuModel({
      selectedCount: files.length,
      selectedImageCount: files.filter(isImageFile).length,
      previewEnabled: previewValidation.ok,
      previewDisabledReason: previewValidation.ok ? undefined : previewValidation.reason,
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
      if (shell.infoOpen) shell.closeInfo()
      else shell.openInfo()
    }
    window.addEventListener('keydown', toggleInfo)
    return () => window.removeEventListener('keydown', toggleInfo)
  }, [
    activePreview,
    compareOpen,
    operationBusy,
    operationDialog,
    resultsBatchId,
    shell.closeInfo,
    shell.infoOpen,
    shell.openInfo,
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

  const openRenameDialog = useCallback(() => {
    if (!canMutateSelection) return
    if (selectedFiles.length === 1) {
      const [file] = selectedFiles
      if (file === undefined) throw new Error('Single-file rename selection is missing its file')
      setOperationDialog({ kind: 'rename', file })
      return
    }
    setOperationDialog({ kind: 'batch_rename', files: selectedFiles })
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
      const currentFiles = [
        ...state.workspace.images,
        ...state.workspace.videos,
        ...state.workspace.otherFiles,
      ]
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

  const organizationDragResetKey = [
    state.project?.sessionId ?? 'no-session',
    state.project?.generation ?? 'no-generation',
    organizationWorkspaceIdentity,
  ].join(':')

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

  const currentVideoPreviewFiles = useMemo<VideoFile[]>(() => {
    if (state.workspace?.workspace !== 'content') return []
    const searchHits = state.search.showResults ? (state.search.page?.hits ?? []) : null
    return videoPreviewNeighbors(state.workspace.videos, searchHits)
  }, [state.search.page?.hits, state.search.showResults, state.workspace])

  const openVideoPreview = useCallback(
    (entityId: string) => {
      const file = currentVideoPreviewFiles.find((candidate) => candidate.entityId === entityId)
      if (file === undefined) return
      setPreviewRepair(EMPTY_PREVIEW_REPAIR)
      openVideoPreviewSession(file, currentVideoPreviewFiles)
      setPreviewEntityId(file.entityId)
    },
    [currentVideoPreviewFiles, openVideoPreviewSession, setPreviewEntityId],
  )

  const requestVideoCover = useCallback(
    (entityId: string) => bridge.videoRequestCover(entityId),
    [bridge],
  )

  const openPreview = useCallback(
    (file: BrowserFile) => {
      if (isVideoFile(file)) {
        openVideoPreview(file.entityId)
        return
      }
      setPreviewRepair(EMPTY_PREVIEW_REPAIR)
      openPreviewSession({ file, files: null, folderOverviewIdentity: null })
      setPreviewEntityId(file.entityId)
    },
    [openPreviewSession, openVideoPreview, setPreviewEntityId],
  )

  const openFilmstripPreview = useCallback(
    (file: BrowserFile, files: BrowserFile[]) => {
      setPreviewRepair(EMPTY_PREVIEW_REPAIR)
      openPreviewSession({ file, files, folderOverviewIdentity })
      setPreviewEntityId(file.entityId)
    },
    [folderOverviewIdentity, openPreviewSession, setPreviewEntityId],
  )

  const navigatePreview = useCallback(
    (file: BrowserFile) => {
      navigatePreviewSession(file)
      setPreviewEntityId(file.entityId)
    },
    [navigatePreviewSession, setPreviewEntityId],
  )

  const closePreview = useCallback(() => {
    setPreviewRepair(EMPTY_PREVIEW_REPAIR)
    closePreviewSession()
    setPreviewEntityId(null)
  }, [closePreviewSession, setPreviewEntityId])

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
      const validation = validateCompareCandidates(files)
      if (!validation.ok) {
        setCompareStatus(compareValidationMessage(validation.reason))
        return
      }
      setCompareStatus(null)
      setPreviewRepair(EMPTY_PREVIEW_REPAIR)
      closePreviewSession()
      setPreviewEntityId(null)
      setCompareEntityIds(files.map((file) => file.entityId))
    },
    [
      closePreviewSession,
      compareEntryAvailable,
      operationBusy,
      selectedFiles,
      setCompareEntityIds,
      setPreviewEntityId,
    ],
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
      if (action === 'preview') {
        const validation = validatePreviewSelection(files)
        if (!validation.ok) return
        if (validation.mode === 'single') {
          openPreview(defined(files[0], 'Single preview requires one file'))
        } else {
          const first = defined(files[0], 'Split text preview requires a left file')
          const second = defined(files[1], 'Split text preview requires a right file')
          setPreviewRepair(EMPTY_PREVIEW_REPAIR)
          openPreviewSession({
            file: first,
            files: [first, second],
            folderOverviewIdentity: null,
          })
          setPreviewEntityId(first.entityId)
        }
      } else if (action === 'mark.keep') void setReviewState('keep', ids)
      else if (action === 'mark.pending') void setReviewState('pending', ids)
      else if (action === 'mark.reject') void setReviewState('reject', ids)
      else if (action === 'mark.clear') void setReviewState(null, ids)
      else if (action === 'mark.favorite') void toggleFavorite(ids)
      else if (action === 'organize.rename') {
        if (files.length === 1) {
          const [file] = files
          if (file === undefined)
            throw new Error('Single-file radial selection is missing its file')
          setOperationDialog({ kind: 'rename', file })
        } else {
          setOperationDialog({ kind: 'batch_rename', files })
        }
      } else if (action === 'organize.copy') {
        setOperationDialog({ kind: 'destination', mode: 'copy', files })
      } else if (action === 'organize.move') {
        setOperationDialog({ kind: 'destination', mode: 'move', files })
      } else if (action === 'trash') setOperationDialog({ kind: 'trash', files })
      else if (action === 'compare') openComparison(files)
      else if (action === 'info') shell.openInfo()
    },
    [
      openComparison,
      openPreview,
      openPreviewSession,
      radialMenu,
      radialProjectIdentity,
      finishRadialSession,
      setPreviewEntityId,
      setReviewState,
      shell.openInfo,
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
    if (activePreview === null || state.contextRepair === null) return
    const sessionEntityIds = new Set(
      (activePreview.files ?? [activePreview.file]).map((file) => file.entityId),
    )
    const removedSessionEntityIds = state.contextRepair.removedEntityIds.filter((entityId) =>
      sessionEntityIds.has(entityId),
    )
    if (removedSessionEntityIds.length === 0) return
    setPreviewRepair((current) => {
      const unavailableEntityIds = new Set(current.unavailableEntityIds)
      let changed = current.message !== state.contextRepair?.message
      for (const entityId of removedSessionEntityIds) {
        if (!unavailableEntityIds.has(entityId)) {
          unavailableEntityIds.add(entityId)
          changed = true
        }
      }
      return changed
        ? { unavailableEntityIds, message: state.contextRepair?.message ?? null }
        : current
    })
  }, [activePreview, state.contextRepair])

  const unavailablePreviewEntityIds = useMemo(
    () =>
      new Set([
        ...previewRepair.unavailableEntityIds,
        ...(state.contextRepair?.removedEntityIds ?? []),
      ]),
    [previewRepair.unavailableEntityIds, state.contextRepair?.removedEntityIds],
  )

  useEffect(() => {
    if (
      activePreview !== null &&
      activePreview.folderOverviewIdentity !== null &&
      activePreview.folderOverviewIdentity !== folderOverviewIdentity
    ) {
      setPreviewRepair(EMPTY_PREVIEW_REPAIR)
      closePreviewSession()
      setPreviewEntityId(null)
    }
  }, [activePreview, closePreviewSession, folderOverviewIdentity, setPreviewEntityId])

  useEffect(() => {
    function handleOrganizationShortcut(event: KeyboardEvent) {
      if (
        organizationShortcutIsOwned(
          event,
          operationDialog !== null ||
            activePreview !== null ||
            compareOpen ||
            shell.infoOpen ||
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
        openPreview(defined(selectedFiles[0], 'Missing selected preview file'))
      } else if (event.key.toLowerCase() === 'c') {
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
    shell.infoOpen,
    openPreview,
    openRenameDialog,
    openComparison,
    openTrashDialog,
    operationDialog,
    operationBusy,
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
      activePreview !== null ||
      compareOpen ||
      shell.infoOpen ||
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
        fatalError={state.status === 'error'}
        onOpenProject={openProject}
      />
    )
  }

  const sessionPreviewFiles =
    activePreview === null ? [] : (activePreview.files ?? [activePreview.file])
  const activeTextPreviewFiles: TextPreviewFiles | null =
    sessionPreviewFiles.length === 1 &&
    isPreviewableText(defined(sessionPreviewFiles[0], 'Missing single preview file'))
      ? [defined(sessionPreviewFiles[0], 'Missing single preview file')]
      : sessionPreviewFiles.length === 2 && sessionPreviewFiles.every(isPreviewableText)
        ? [
            defined(sessionPreviewFiles[0], 'Missing left text preview file'),
            defined(sessionPreviewFiles[1], 'Missing right text preview file'),
          ]
        : null
  const activePreviewFiles =
    activePreview?.file.kind === 'video'
      ? (activePreview.files ?? currentVideoPreviewFiles)
          .filter(isVideoFile)
          .map(
            (file) =>
              (state.workspace?.workspace === 'content'
                ? state.workspace.videos.find(
                    (workspaceVideo) => workspaceVideo.entityId === file.entityId,
                  )
                : undefined) ?? file,
          )
      : (activePreview?.files ??
        (state.workspace?.workspace === 'content' ? state.workspace.images : []))
  const activePreviewFile =
    activePreview === null
      ? null
      : (activePreviewFiles.find(
          (candidate) => candidate.entityId === activePreview.file.entityId,
        ) ?? activePreview.file)
  const contentWorkspaceActive =
    !state.search.showResults && state.workspace?.workspace === 'content'
  const contextRepairMessage =
    state.contextRepair?.message ?? (activePreview === null ? null : previewRepair.message)
  const pendingRecoveryReport =
    state.recoveryReport !== null &&
    (state.recoveryReport.recovered > 0 || state.recoveryReport.needsUserReview > 0) &&
    shell.recoveryAcknowledgedSessionId !== projectSessionId
      ? state.recoveryReport
      : null
  const viewContext: WorkspaceViewContext = state.search.showResults
    ? {
        kind: 'search',
        layout: state.search.query.layout,
        onLayoutChange: setSearchLayout,
      }
    : state.workspace?.workspace === 'category'
      ? { kind: 'category', onShowAllDescendants: () => void showAllDescendants() }
      : state.workspace?.workspace === 'content'
        ? {
            kind: 'content',
            showingAggregate: state.showingAggregate,
            selectAllRequest: shell.selectAllRequest,
            onSelectAll: shell.requestSelectAll,
            onShowAllDescendants: () => void showAllDescendants(),
            onReturnToFolder: returnToFolderContext,
          }
        : { kind: 'none' }

  return (
    <main
      className="viewer-shell"
      data-video-preview-open={
        activePreviewFile !== null && isVideoFile(activePreviewFile) ? true : undefined
      }
      data-organization-drag-active={organizationDragView ? true : undefined}
      style={
        {
          '--viewer-sidebar-width': `${shell.effectiveSidebarCollapsed ? 52 : shell.sidebarWidth}px`,
        } as CSSProperties
      }
    >
      <header className="workspace-header">
        <div className="project-identity" data-collapsed={shell.effectiveSidebarCollapsed}>
          {shell.effectiveSidebarCollapsed ? (
            <ViewerIconButton
              icon="chevron-right"
              label={shell.narrowViewport ? '窄窗口中已折叠文件夹栏' : '展开文件夹栏'}
              tone="quiet"
              onClick={shell.toggleSidebar}
              disabled={shell.narrowViewport}
            />
          ) : (
            <>
              <h1>{state.project.displayName}</h1>
              <ViewerIconButton
                icon="chevron-left"
                label="折叠文件夹栏"
                tone="quiet"
                onClick={shell.toggleSidebar}
              />
            </>
          )}
        </div>
        <div className="workspace-header-main" role="toolbar" aria-label="Viewer 工具栏">
          <SearchToolbar
            filterOpen={shell.toolbarPopover.openPopover === 'filter'}
            onFilterOpenChange={(open) => shell.toolbarPopover.setPopoverOpen('filter', open)}
            query={state.search.query}
            folders={state.folders}
            focusRequest={state.search.focusRequest}
            onTextChange={setSearchText}
            onScopeChange={setSearchScope}
            onFiltersChange={setSearchFilters}
            onSortChange={setSearchSort}
            onRemoveFilter={removeSearchFilter}
            onClearFilters={clearSearchFilters}
          />
          <WorkspaceViewMenu
            context={viewContext}
            open={shell.toolbarPopover.openPopover === 'view'}
            onOpenChange={(open) => shell.toolbarPopover.setPopoverOpen('view', open)}
          />
          <WorkspaceMoreMenu
            ref={moreMenuTriggerRef}
            open={shell.toolbarPopover.openPopover === 'more'}
            onOpenChange={(open) => shell.toolbarPopover.setPopoverOpen('more', open)}
            access={state.project.access}
            closing={state.status === 'closing'}
            onOpenSettings={shell.openSettings}
            onOpenPermissionSettings={() => void openPermissionSettings()}
            onReselectProject={() => void reselectProject()}
            onCloseProject={() => void closeProject()}
          />
        </div>
      </header>
      {state.project.access === 'read_only' && (
        <ReadOnlyBanner
          busy={state.status === 'closing'}
          onOpenSettings={() => void openPermissionSettings()}
          onReselect={() => void reselectProject()}
        />
      )}
      <div className="viewer-columns">
        <aside
          className="folder-sidebar"
          aria-label="文件夹栏"
          data-collapsed={shell.effectiveSidebarCollapsed}
          style={{ width: shell.effectiveSidebarCollapsed ? 52 : shell.sidebarWidth }}
        >
          <p className="folder-tree-label">项目目录</p>
          {state.workspace !== null && (
            <button
              type="button"
              className="project-root-button"
              aria-pressed={displayedFolderId === null}
              onClick={() => selectFolderTarget(null)}
            >
              {state.project.displayName}
            </button>
          )}
          <FolderTree
            folders={state.folders}
            loading={state.workspace === null}
            selectedId={displayedFolderId}
            onSelect={selectFolderTarget}
            organizationDropTarget={organizationDropTarget}
          />
          {!shell.effectiveSidebarCollapsed && (
            <button
              type="button"
              className="sidebar-separator"
              role="separator"
              aria-label="调整文件夹栏宽度"
              aria-orientation="vertical"
              aria-valuemin={200}
              aria-valuemax={420}
              aria-valuenow={shell.sidebarWidth}
              onPointerDown={shell.startSidebarResize}
              onKeyDown={shell.resizeSidebarFromKeyboard}
            />
          )}
        </aside>
        <section
          className={contentWorkspaceActive ? 'workspace workspace--content' : 'workspace'}
          aria-label="项目内容"
        >
          {projectionProgressVisible && (
            <div className="projection-progress" role="progressbar" aria-label="正在切换文件夹" />
          )}
          {pendingRecoveryReport !== null ? (
            <ViewerEmptyState
              appearance="plain"
              title="项目状态已恢复"
              description={`${pendingRecoveryReport.recovered} 项操作已经恢复，${pendingRecoveryReport.needsUserReview} 项需要检查。`}
              action={
                <ViewerButton tone="primary" onClick={shell.acknowledgeRecovery}>
                  继续浏览项目
                </ViewerButton>
              }
            />
          ) : (
            <>
              {contextRepairMessage && (
                <p className="context-repair-banner local-error" role="status">
                  {contextRepairMessage}
                </p>
              )}
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
                  searching={
                    state.search.status === 'searching' || !state.search.page.progress.complete
                  }
                />
              )}
              {!state.search.showResults && state.workspace === null && <WorkspaceLoadingState />}
              {!state.search.showResults && state.workspace?.workspace === 'empty' && (
                <ViewerEmptyState
                  appearance="plain"
                  title="这个项目中还没有可显示的文件"
                  description="Viewer 会显示支持的图片、视频、Markdown 与文本文件。"
                  action={
                    <ViewerButton onClick={shell.revealProject}>在文件管理器中显示</ViewerButton>
                  }
                />
              )}
              {!state.search.showResults && state.workspace?.workspace === 'category' && (
                <FolderOverview
                  key={folderOverviewIdentity}
                  folders={state.workspace.folders}
                  density={thumbnailDensity}
                  requestFolderImages={requestFolderImages}
                  requestThumbnail={requestThumbnail}
                  onPreview={openFilmstripPreview}
                  onSelect={selectFolderTarget}
                />
              )}
              {!state.search.showResults && state.workspace?.workspace === 'content' && (
                <>
                  <div
                    className="content-workspace-surface"
                    data-testid="content-workspace-surface"
                    hidden={compareOpen}
                  >
                    {state.showingAggregate && (
                      <ViewerStatusTag className="aggregate-label" tone="info">
                        全部后代文件
                      </ViewerStatusTag>
                    )}
                    <ContentBrowser
                      workspace={state.workspace}
                      density={thumbnailDensity}
                      viewCommand={shell.contentViewCommand}
                      onViewStateChange={shell.setSelectAllRequest}
                      onRequestViewMenu={() => shell.toolbarPopover.setPopoverOpen('view', true)}
                      requestThumbnail={requestThumbnail}
                      onThumbnailTaskChange={feedback.setThumbnailTask}
                      onPreview={openPreview}
                      onOpenVideo={openVideoPreview}
                      requestVideoCover={requestVideoCover}
                      onSelectionChange={selectFiles}
                      organizationDragDisabled={
                        state.project.access !== 'read_write' || operationBusy || compareOpen
                      }
                      onFinderDragStart={exportToFinder}
                      onOrganizationPointerInput={handleOrganizationPointerInput}
                      repairSelectionId={state.contextRepair?.suggestedEntityId ?? null}
                      onRepairSelectionApplied={consumeContextRepair}
                      onRadialMenuRequest={beginRadialSession}
                      otherFilePanelExpanded={shell.otherFilePanel.expanded}
                      onOtherFilePanelExpandedChange={shell.otherFilePanel.setExpanded}
                      videoPanelExpanded={shell.videoPanel.expanded}
                      onVideoPanelExpandedChange={shell.videoPanel.setExpanded}
                    />
                  </div>
                  {!compareOpen && compareStatus && (
                    <p className="compare-status" role="status">
                      {compareStatus}
                    </p>
                  )}
                  {compareOpen && (
                    <>
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
                      {compareStatus && (
                        <p className="compare-status" role="status">
                          {compareStatus}
                        </p>
                      )}
                    </>
                  )}
                </>
              )}
            </>
          )}
        </section>
      </div>
      <GlobalNoticeStack
        notices={feedback.globalNotices}
        belowReadOnly={state.project.access === 'read_only'}
      />
      {organizationDragView && <OrganizationDragPreview {...organizationDragView} />}
      <TaskBar
        tasks={feedback.visibleTasks}
        onCancel={(taskId) => {
          if (taskId === state.operation.active?.batchId) void cancelOperation()
          else void cancelTask(taskId)
        }}
        onDismiss={feedback.dismissTask}
        onShowResults={(taskId) => setResultsBatchId(taskId)}
      />
      {activeRadialMenu !== null && (
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
      {shell.settingsOpen && (
        <SettingsDialog
          bridge={bridge}
          density={thumbnailDensity}
          magnifier={magnifier}
          error={settingsError}
          onDensityChange={setThumbnailDensity}
          onMagnifierShapeChange={setMagnifierShape}
          onMagnifierMagnificationChange={setMagnifierMagnification}
          onMagnifierAreaChange={setMagnifierArea}
          onClose={shell.closeSettings}
          returnFocusRef={moreMenuTriggerRef}
        />
      )}
      {activePreviewFile && isImageFile(activePreviewFile) && activePreviewFiles.length > 0 && (
        <ImagePreview
          file={activePreviewFile}
          files={activePreviewFiles}
          magnifier={magnifier}
          pointerClientPoint={pointerClientPoint}
          requestImage={requestPreviewImage}
          onNavigate={navigatePreview}
          onClose={closePreview}
          onDimensions={recordDimensions}
          unavailableEntityIds={unavailablePreviewEntityIds}
        />
      )}
      {activePreviewFile && isVideoFile(activePreviewFile) && activePreviewFiles.length > 0 && (
        <VideoPreview
          key={activePreviewFile.entityId}
          file={activePreviewFile}
          files={activePreviewFiles.filter(isVideoFile)}
          bridge={bridge}
          onNavigate={navigatePreview}
          onClose={closePreview}
        />
      )}
      {activeTextPreviewFiles !== null && (
        <TextPreview
          files={activeTextPreviewFiles}
          unavailableEntityIds={unavailablePreviewEntityIds}
          requestPreview={requestTextPreview}
          openExternalLink={bridge.openExternalLink}
          onClose={closePreview}
          onTaskChange={feedback.setTextTask}
        />
      )}
      {activePreviewFile?.kind === 'other' && (
        <UnsupportedFilePreview
          file={activePreviewFile}
          unavailable={unavailablePreviewEntityIds.has(activePreviewFile.entityId)}
          onClose={closePreview}
        />
      )}
      {shell.infoOpen && (
        <InfoOverlay
          files={selectedFiles}
          selectionInfo={state.selectionInfo}
          dimensions={dimensions}
          onClose={shell.closeInfo}
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

function parentRelativePath(relativePath: string): string {
  const separator = relativePath.lastIndexOf('/')
  return separator === -1 ? '' : relativePath.slice(0, separator)
}
