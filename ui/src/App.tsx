import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import type { ViewerBridge } from './api/viewer'
import { tauriViewerBridge } from './api/viewer'
import EmptyProject from './components/EmptyProject'
import BatchRenameDialog from './components/BatchRenameDialog'
import ContentBrowser from './components/ContentBrowser'
import CompareWorkspace from './components/CompareWorkspace'
import DestinationDialog from './components/DestinationDialog'
import FileActionToolbar from './components/FileActionToolbar'
import FolderOverview from './components/FolderOverview'
import FolderTree from './components/FolderTree'
import ImagePreview from './components/ImagePreview'
import InfoOverlay from './components/InfoOverlay'
import MarkerControls from './components/MarkerControls'
import OperationResults from './components/OperationResults'
import RenameDialog from './components/RenameDialog'
import SearchResults from './components/SearchResults'
import SearchToolbar from './components/SearchToolbar'
import TaskBar from './components/TaskBar'
import type { TaskFeedback } from './components/TaskBar'
import TextPreview from './components/TextPreview'
import TrashConfirmation from './components/TrashConfirmation'
import { useViewerController } from './state/useViewerController'
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

export default function App({ bridge = tauriViewerBridge }: AppProps) {
  const {
    state,
    openProject,
    closeProject,
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
  } = useViewerController(bridge)
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false)
  const [sidebarWidth, setSidebarWidth] = useState(260)
  const [thumbnailTask, setThumbnailTask] = useState<TaskFeedback | null>(null)
  const [textTask, setTextTask] = useState<TaskFeedback | null>(null)
  const [dismissedTasks, setDismissedTasks] = useState<Set<string>>(() => new Set())
  const [activePreview, setActivePreview] = useState<BrowserFile | null>(null)
  const [selectedFiles, setSelectedFiles] = useState<BrowserFile[]>([])
  const [draggedEntityIds, setDraggedEntityIds] = useState<string[]>([])
  const draggedEntityIdsRef = useRef<string[]>([])
  const finderExportStartedRef = useRef(false)
  const [finderDragMessage, setFinderDragMessage] = useState<string | null>(null)
  const [compareStatus, setCompareStatus] = useState<string | null>(null)
  const [operationDialog, setOperationDialog] = useState<OperationDialog | null>(null)
  const [operationSubmitting, setOperationSubmitting] = useState(false)
  const [resultsBatchId, setResultsBatchId] = useState<string | null>(null)
  const [infoOpen, setInfoOpen] = useState(false)
  const [dimensions, setDimensions] = useState<
    Record<string, { width: number; height: number } | undefined>
  >({})
  useEffect(() => {
    setThumbnailTask(null)
    setTextTask(null)
    setDismissedTasks(new Set())
    setActivePreview(null)
    setSelectedFiles([])
    setDraggedEntityIds([])
    draggedEntityIdsRef.current = []
    finderExportStartedRef.current = false
    setFinderDragMessage(null)
    setCompareStatus(null)
    setOperationDialog(null)
    setOperationSubmitting(false)
    setResultsBatchId(null)
    setInfoOpen(false)
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
  useEffect(() => {
    function toggleInfo(event: KeyboardEvent) {
      const target = event.target
      if (
        !(
          event.metaKey &&
          !event.ctrlKey &&
          !event.altKey &&
          !event.shiftKey &&
          event.key.toLowerCase() === 'i'
        ) ||
        isOrganizationShortcutTargetBlocked(target) ||
        operationDialog !== null
      ) {
        return
      }
      event.preventDefault()
      setInfoOpen((open) => !open)
    }
    window.addEventListener('keydown', toggleInfo)
    return () => window.removeEventListener('keydown', toggleInfo)
  }, [operationDialog])
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
    (state.operation.active !== null && state.operation.active.lifecycle !== 'completed')
  const canMutateSelection =
    selectedFiles.length > 0 && state.project?.access === 'read_write' && !operationBusy

  const exportDraggedSelection = useCallback(
    (clientX: number, clientY: number) => {
      const project = state.project
      const entityIds = draggedEntityIdsRef.current
      const outside =
        clientX <= 0 ||
        clientY <= 0 ||
        clientX >= window.innerWidth ||
        clientY >= window.innerHeight
      if (
        project === null ||
        state.status !== 'active' ||
        entityIds.length === 0 ||
        !outside ||
        finderExportStartedRef.current
      ) {
        return
      }
      finderExportStartedRef.current = true
      void bridge
        .beginFinderDrag({
          sessionId: project.sessionId,
          generation: project.generation,
          entityIds: [...entityIds],
        })
        .catch(() => {
          finderExportStartedRef.current = false
          setFinderDragMessage('无法拖到 Finder，请重新拖动。')
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

  const openDestinationDialog = useCallback(
    (mode: 'copy' | 'move') => {
      if (!canMutateSelection) return
      setOperationDialog({ kind: 'destination', mode, files: selectedFiles })
    },
    [canMutateSelection, selectedFiles],
  )

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
      setDraggedEntityIds([])
      draggedEntityIdsRef.current = []
      finderExportStartedRef.current = false
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
    [operationBusy, preflightFileCommand, state.project?.access, state.workspace, submitFileCommand],
  )

  const isDropTargetValid = useCallback(
    (destinationId: string, mode: 'move' | 'copy') => {
      if (state.workspace?.workspace !== 'content') return false
      const destination = state.folders.find((folder) => folder.entityId === destinationId)
      if (!destination) return false
      const currentFiles = [...state.workspace.images, ...state.workspace.textFiles]
      const byId = new Map(currentFiles.map((file) => [file.entityId, file]))
      const files = draggedEntityIds.map((entityId) => byId.get(entityId))
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
    [draggedEntityIds, state.folders, state.workspace],
  )

  const openPreview = useCallback(
    (file: BrowserFile) => {
      setActivePreview(file)
      setPreviewEntityId(file.entityId)
    },
    [setPreviewEntityId],
  )

  const closePreview = useCallback(() => {
    setActivePreview(null)
    setPreviewEntityId(null)
  }, [setPreviewEntityId])

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
    state.workspace?.workspace === 'content' &&
    !state.search.showResults &&
    !operationBusy

  const openComparison = useCallback(() => {
    if (!compareEntryAvailable) {
      setCompareStatus(
        operationBusy
          ? '请等待当前文件操作完成后再开始对比。'
          : '请先返回文件夹内容，再选择图片进行对比。',
      )
      return
    }
    if (
      selectedFiles.length < 2 ||
      selectedFiles.length > 4 ||
      !selectedFiles.every(matchesImage)
    ) {
      setCompareStatus('请选择 2–4 张 JPG 或 PNG 图片进行对比。')
      return
    }
    setCompareStatus(null)
    setActivePreview(null)
    setPreviewEntityId(null)
    setCompareEntityIds(selectedFiles.map((file) => file.entityId))
  }, [
    compareEntryAvailable,
    operationBusy,
    selectedFiles,
    setCompareEntityIds,
    setPreviewEntityId,
  ])

  const changeComparedEntities = useCallback(
    (entityIds: string[]) => {
      setCompareStatus(null)
      if (entityIds.length >= 2) {
        setCompareEntityIds(entityIds)
        return
      }
      setCompareEntityIds([])
      if (entityIds.length !== 1 || state.workspace?.workspace !== 'content') return
      const survivor = state.workspace.images.find(
        (file) => file.entityId === entityIds[0],
      )
      if (survivor !== undefined) openPreview(survivor)
    },
    [openPreview, setCompareEntityIds, state.workspace],
  )

  useEffect(() => {
    if (state.compareEntityIds.length === 0) return
    const liveIds = compareFiles.map((file) => file.entityId)
    if (
      liveIds.length !== state.compareEntityIds.length ||
      liveIds.length < 2
    ) {
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
      state.contextRepair?.removedEntityIds.includes(activePreview.entityId)
    ) {
      setActivePreview(null)
    }
  }, [activePreview, state.contextRepair])

  useEffect(() => {
    function handleOrganizationShortcut(event: KeyboardEvent) {
      if (
        event.defaultPrevented ||
        isOrganizationShortcutTargetBlocked(event.target) ||
        operationDialog !== null ||
        activePreview !== null ||
        compareOpen ||
        infoOpen ||
        resultsBatchId !== null ||
        hasTextSelection()
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
        if (operationBusy) return
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
    undoLastOperation,
  ])

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

  return (
    <main
      className="viewer-shell"
      onDragOverCapture={(event) => exportDraggedSelection(event.clientX, event.clientY)}
      onDragLeaveCapture={(event) => exportDraggedSelection(event.clientX, event.clientY)}
    >
      <header>
        <h1>{state.project.displayName}</h1>
        <button
          type="button"
          disabled={state.status === 'closing'}
          onClick={() => void closeProject()}
        >
          {state.status === 'closing' ? '正在关闭…' : '关闭项目'}
        </button>
      </header>
      {state.project.access === 'read_only' && (
        <p className="read-only-banner" role="status">
          只读项目
        </p>
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
      <MarkerControls
        selectedCount={state.selectedEntityIds.length}
        selectionInfo={state.selectionInfo}
        readOnly={state.project.access === 'read_only'}
        shortcutsDisabled={compareOpen}
        onSetReview={(reviewState) => void setReviewState(reviewState)}
        onToggleFavorite={() => void toggleFavorite()}
      />
      <FileActionToolbar
        selectedCount={selectedFiles.length}
        selectedImageCount={selectedFiles.filter(matchesImage).length}
        readOnly={state.project.access === 'read_only'}
        busy={operationBusy || compareOpen}
        compareContextAvailable={compareEntryAvailable}
        onRename={openRenameDialog}
        onCopy={() => openDestinationDialog('copy')}
        onMove={() => openDestinationDialog('move')}
        onTrash={openTrashDialog}
        onCompare={openComparison}
        onInfo={() => setInfoOpen(true)}
      />
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
                draggedEntityIds={draggedEntityIds}
                readOnly={state.project.access !== 'read_write' || operationBusy}
                isDropTargetValid={isDropTargetValid}
                onDropFiles={(entityIds, destinationId, mode) =>
                  void dropFiles(entityIds, destinationId, mode)
                }
              />
              <label className="sidebar-resize">
                文件夹栏宽度
                <input
                  type="range"
                  min="200"
                  max="420"
                  value={sidebarWidth}
                  onChange={(event) => setSidebarWidth(Number(event.currentTarget.value))}
                />
              </label>
            </>
          )}
        </aside>
        <section className="workspace" aria-label="项目内容">
          {state.search.showResults &&
            state.search.page === null &&
            state.search.status === 'searching' && (
            <p role="status">正在搜索…</p>
          )}
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
              folders={state.workspace.folders}
              currentPath={state.selectedFolderPath || state.project.displayName}
              requestThumbnail={requestThumbnail}
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
                  requestThumbnail={requestContentThumbnail}
                  onThumbnailTaskChange={setThumbnailTask}
                  onPreview={openPreview}
                  onSelectionChange={selectFiles}
                  onDragSelectionStart={(entityIds) => {
                    finderExportStartedRef.current = false
                    setFinderDragMessage(null)
                    draggedEntityIdsRef.current = entityIds
                    setDraggedEntityIds(entityIds)
                  }}
                  onDragSelectionEnd={() => {
                    finderExportStartedRef.current = false
                    draggedEntityIdsRef.current = []
                    setDraggedEntityIds([])
                  }}
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
      <TaskBar
        tasks={visibleTasks}
        onCancel={(taskId) => {
          if (taskId === state.operation.active?.batchId) void cancelOperation()
          else void cancelTask(taskId)
        }}
        onDismiss={(taskId) =>
          setDismissedTasks((current) => new Set([...current, taskId]))
        }
        onShowResults={(taskId) => setResultsBatchId(taskId)}
      />
      {activePreview && matchesImage(activePreview) && state.workspace?.workspace === 'content' && (
        <ImagePreview
          file={
            state.workspace.images.find(
              (file) => file.entityId === activePreview.entityId,
            ) ?? activePreview
          }
          files={state.workspace.images}
          requestImage={requestPreviewImage}
          onNavigate={openPreview}
          onClose={closePreview}
          onDimensions={rememberDimensions}
        />
      )}
      {activePreview && !matchesImage(activePreview) && (
        <TextPreview
          file={activePreview}
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

function isEditableTarget(target: EventTarget | null): boolean {
  return (
    target instanceof HTMLInputElement ||
    target instanceof HTMLTextAreaElement ||
    target instanceof HTMLSelectElement ||
    (target instanceof HTMLElement && target.isContentEditable)
  )
}

function isOrganizationShortcutTargetBlocked(target: EventTarget | null): boolean {
  if (isEditableTarget(target)) return true
  return (
    target instanceof HTMLElement &&
    target.closest('button, a, summary, [role="button"], [role="dialog"], [aria-modal="true"]') !== null
  )
}

function hasTextSelection(): boolean {
  const selection = window.getSelection()
  return selection !== null && !selection.isCollapsed && selection.toString().length > 0
}
