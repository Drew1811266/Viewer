import type { CSSProperties, MutableRefObject } from 'react'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import type { BrowserFile, RenamePreview, RenameRules } from './api/types'
import type { ViewerBridge } from './api/viewer'
import { tauriViewerBridge } from './api/viewer'
import { useDelayedProjectionProgress } from './app/useDelayedProjectionProgress'
import type { WorkspaceIntentSink } from './app/workspace/intents'
import { createWorkspacePorts } from './app/workspace/ports'
import { useFeedbackCoordinator } from './app/workspace/useFeedbackCoordinator'
import { useOrganizationCoordinator } from './app/workspace/useOrganizationCoordinator'
import { useViewingCoordinator } from './app/workspace/useViewingCoordinator'
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
import SearchResults from './components/SearchResults'
import SearchToolbar from './components/SearchToolbar'
import SettingsDialog from './components/SettingsDialog'
import TaskBar from './components/TaskBar'
import TextPreview from './components/TextPreview'
import TrashConfirmation from './components/TrashConfirmation'
import UnsupportedFilePreview from './components/UnsupportedFilePreview'
import ViewerButton, { ViewerIconButton } from './components/ui/ViewerButton'
import ViewerEmptyState from './components/ui/ViewerEmptyState'
import ViewerStatusTag from './components/ui/ViewerStatusTag'
import VideoPreview from './components/VideoPreview'
import WorkspaceLoadingState from './components/WorkspaceLoadingState'
import WorkspaceMoreMenu from './components/WorkspaceMoreMenu'
import WorkspaceViewMenu, { type WorkspaceViewContext } from './components/WorkspaceViewMenu'
import { isImageFile, isVideoFile } from './fileKinds'
import { useViewerSettings, ViewerSettingsProvider } from './settings/ViewerSettingsProvider'
import {
  compareEntryAvailability,
  compareValidationMessage,
  validateCompareCandidates,
} from './state/comparePolicy'
import { useViewerController } from './state/useViewerController'

interface AppProps {
  bridge?: ViewerBridge
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
    beginFinderDrag,
    setPreviewEntityId,
    setCompareEntityIds,
    consumeContextRepair,
    clearCloseBlocked,
    openPermissionSettings,
  } = useViewerController(bridge)
  const projectSessionId = state.project?.sessionId ?? 'no-session'
  const intentTargetRef = useRef<WorkspaceIntentSink>(() => undefined)
  const emitIntent = useCallback<WorkspaceIntentSink>((intent) => {
    intentTargetRef.current(intent)
  }, [])
  const ports = useMemo(() => createWorkspacePorts(bridge), [bridge])
  const shell = useWorkspaceShellCoordinator({
    projectSessionId,
    videoProjectSessionId: state.project?.sessionId ?? null,
    port: ports.shell,
  })
  const compareFiles = useMemo(() => {
    if (state.workspace?.workspace !== 'content') return []
    const byId = new Map(state.workspace.images.map((file) => [file.entityId, file]))
    return state.compareEntityIds.flatMap((entityId) => {
      const file = byId.get(entityId)
      return file === undefined ? [] : [file]
    })
  }, [state.compareEntityIds, state.workspace])
  const compareOpen = state.compareEntityIds.length >= 2 && compareFiles.length >= 2
  const viewing = useViewingCoordinator({
    state,
    projectSessionId,
    port: ports.preview,
    playbackPort: ports.playback,
    commands: { setPreviewEntityId, setCompareEntityIds },
  })
  const organization = useOrganizationCoordinator({
    state,
    commands: {
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
    },
    emitIntent,
    compareOpen,
    activePreviewOpen: viewing.activePreview !== null,
    infoOpen: shell.infoOpen,
  })
  const feedback = useFeedbackCoordinator({
    projectSessionId,
    scan: state.scan,
    operation: state.operation,
    projectError: state.errorMessage,
    finderDragMessage: organization.finderDragMessage,
    workspaceActionError: shell.workspaceActionError,
  })
  const [compareStatus, setCompareStatus] = useState<string | null>(null)
  const moreMenuTriggerRef = useRef<HTMLElement>(null)
  useEffect(() => {
    setCompareStatus(null)
  }, [projectSessionId])
  const displayedFolderId = state.projectionTransition?.selectedFolderId ?? state.selectedFolderId
  const projectionProgressVisible = useDelayedProjectionProgress(
    state.projectionTransition,
    state.workspace !== null,
  )
  const selectFolderTarget = useCallback(
    (entityId: string | null) => {
      organization.selectFolderTarget(entityId)
      void selectFolder(entityId)
    },
    [organization.selectFolderTarget, selectFolder],
  )
  const compareAvailability = compareEntryAvailability({
    workspace: state.workspace,
    searchResultsOpen: state.search.showResults,
    operationBusy: organization.operationBusy,
  })

  const openComparison = useCallback(
    (files: BrowserFile[] = organization.selectedFiles) => {
      if (compareAvailability !== 'available') {
        setCompareStatus(
          compareAvailability === 'busy'
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
      viewing.closePreview()
      setCompareEntityIds(files.map((file) => file.entityId))
    },
    [compareAvailability, organization.selectedFiles, setCompareEntityIds, viewing.closePreview],
  )

  intentTargetRef.current = (intent) => {
    switch (intent.kind) {
      case 'open-preview':
        viewing.openIntentPreview(intent)
        return
      case 'enter-compare':
        openComparison([...intent.files])
        return
      case 'start-rename':
        organization.openRenameDialog(intent.files)
        return
      case 'show-operation-results':
        organization.showResults(intent.batchId)
        return
      case 'open-settings':
        shell.openSettings()
        return
      case 'open-info':
        if (shell.infoOpen) shell.closeInfo()
        else shell.openInfo()
        return
      case 'close-project':
        void closeProject()
    }
  }

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
      if (survivor !== undefined) viewing.openPreview(survivor)
    },
    [setCompareEntityIds, state.workspace, viewing.openPreview],
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

  const operationDialogSnapshot = organization.operationDialog

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

  const contentWorkspaceActive =
    !state.search.showResults && state.workspace?.workspace === 'content'
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
        viewing.activePreviewFile !== null && isVideoFile(viewing.activePreviewFile)
          ? true
          : undefined
      }
      data-organization-drag-active={organization.organizationDragView ? true : undefined}
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
            onOpenSettings={() => emitIntent({ kind: 'open-settings' })}
            onOpenPermissionSettings={() => void openPermissionSettings()}
            onReselectProject={() => void reselectProject()}
            onCloseProject={() => emitIntent({ kind: 'close-project' })}
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
            organizationDropTarget={organization.organizationDropTarget}
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
              {viewing.contextRepairMessage && (
                <p className="context-repair-banner local-error" role="status">
                  {viewing.contextRepairMessage}
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
                  key={viewing.folderOverviewIdentity}
                  folders={state.workspace.folders}
                  density={thumbnailDensity}
                  requestFolderImages={viewing.requestFolderImages}
                  requestThumbnail={viewing.requestThumbnail}
                  onPreview={viewing.openFilmstripPreview}
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
                      requestThumbnail={viewing.requestThumbnail}
                      onThumbnailTaskChange={feedback.setThumbnailTask}
                      onPreview={viewing.openPreview}
                      onOpenVideo={viewing.openVideoPreview}
                      requestVideoCover={viewing.requestVideoCover}
                      onSelectionChange={organization.selectFiles}
                      organizationDragDisabled={
                        state.project.access !== 'read_write' ||
                        organization.operationBusy ||
                        compareOpen
                      }
                      onFinderDragStart={organization.exportToFinder}
                      onOrganizationPointerInput={organization.handleOrganizationPointerInput}
                      repairSelectionId={state.contextRepair?.suggestedEntityId ?? null}
                      onRepairSelectionApplied={organization.consumeContextRepair}
                      onRadialMenuRequest={organization.beginRadialSession}
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
                        requestImage={viewing.requestPreviewImage}
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
      {organization.organizationDragView && (
        <OrganizationDragPreview {...organization.organizationDragView} />
      )}
      <TaskBar
        tasks={feedback.visibleTasks}
        onCancel={(taskId) => {
          if (taskId === state.operation.active?.batchId) void organization.cancelOperation()
          else void cancelTask(taskId)
        }}
        onDismiss={feedback.dismissTask}
        onShowResults={(batchId) => emitIntent({ kind: 'show-operation-results', batchId })}
      />
      {organization.activeRadialMenu !== null && (
        <RadialFileMenu
          key={organization.activeRadialMenu.requestId}
          origin={organization.activeRadialMenu.origin}
          pointerId={organization.activeRadialMenu.pointerId}
          selectionCount={organization.activeRadialMenu.files.length}
          readOnly={state.project.access === 'read_only'}
          returnFocusTarget={organization.activeRadialMenu.returnFocusTarget}
          model={organization.radialModel}
          onAction={organization.runRadialAction}
          onClose={organization.finishRadialSession}
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
      {viewing.activePreviewFile &&
        isImageFile(viewing.activePreviewFile) &&
        viewing.activePreviewFiles.length > 0 && (
          <ImagePreview
            file={viewing.activePreviewFile}
            files={viewing.activePreviewFiles}
            magnifier={magnifier}
            pointerClientPoint={pointerClientPoint}
            requestImage={viewing.requestPreviewImage}
            onNavigate={viewing.navigatePreview}
            onClose={viewing.closePreview}
            onDimensions={viewing.recordDimensions}
            unavailableEntityIds={viewing.unavailablePreviewEntityIds}
          />
        )}
      {viewing.activePreviewFile &&
        isVideoFile(viewing.activePreviewFile) &&
        viewing.activePreviewFiles.length > 0 && (
          <VideoPreview
            key={viewing.activePreviewFile.entityId}
            file={viewing.activePreviewFile}
            files={viewing.activePreviewFiles.filter(isVideoFile)}
            bridge={viewing.playbackPort}
            onNavigate={viewing.navigatePreview}
            onClose={viewing.closePreview}
          />
        )}
      {viewing.activeTextPreviewFiles !== null && (
        <TextPreview
          files={viewing.activeTextPreviewFiles}
          unavailableEntityIds={viewing.unavailablePreviewEntityIds}
          requestPreview={viewing.requestTextPreview}
          openExternalLink={viewing.openExternalLink}
          onClose={viewing.closePreview}
          onTaskChange={feedback.setTextTask}
        />
      )}
      {viewing.activePreviewFile?.kind === 'other' && (
        <UnsupportedFilePreview
          file={viewing.activePreviewFile}
          unavailable={viewing.unavailablePreviewEntityIds.has(viewing.activePreviewFile.entityId)}
          onClose={viewing.closePreview}
        />
      )}
      {shell.infoOpen && (
        <InfoOverlay
          files={organization.selectedFiles}
          selectionInfo={state.selectionInfo}
          dimensions={viewing.dimensions}
          onClose={shell.closeInfo}
        />
      )}
      {operationDialogSnapshot?.kind === 'rename' && (
        <RenameDialog
          currentName={operationDialogSnapshot.file.name}
          busy={organization.operationBusy}
          onCancel={organization.closeOperationDialog}
          onConfirm={(proposedName, editExtension) =>
            void organization.submitFileCommand('rename', [
              {
                entityId: operationDialogSnapshot.file.entityId,
                action: { kind: 'rename', proposedName, editExtension },
              },
            ])
          }
        />
      )}
      {operationDialogSnapshot?.kind === 'batch_rename' && (
        <BatchRenameDialog
          entityIds={operationDialogSnapshot.files.map((file) => file.entityId)}
          busy={organization.operationBusy}
          requestPreview={(entityIds, rules) => organization.previewRename(entityIds, rules)}
          onCancel={organization.closeOperationDialog}
          onConfirm={(_rules: RenameRules, preview: RenamePreview) =>
            void organization.submitFileCommand(
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
      {operationDialogSnapshot?.kind === 'destination' && (
        <DestinationDialog
          mode={operationDialogSnapshot.mode}
          entityIds={operationDialogSnapshot.files.map((file) => file.entityId)}
          folders={state.folders}
          busy={organization.operationBusy}
          initialDestinationId={operationDialogSnapshot.initialDestinationId}
          initialPreflight={operationDialogSnapshot.initialPreflight}
          requestPreflight={(items) =>
            organization.preflightFileCommand(operationDialogSnapshot.mode, items)
          }
          onCancel={organization.closeOperationDialog}
          onConfirm={(items, conflicts) =>
            void organization.submitFileCommand(operationDialogSnapshot.mode, items, conflicts)
          }
        />
      )}
      {operationDialogSnapshot?.kind === 'trash' && (
        <TrashConfirmation
          count={operationDialogSnapshot.files.length}
          busy={organization.operationBusy}
          onCancel={organization.closeOperationDialog}
          onConfirm={() =>
            void organization.submitFileCommand(
              'trash',
              operationDialogSnapshot.files.map((file) => ({
                entityId: file.entityId,
                action: { kind: 'trash' },
              })),
            )
          }
        />
      )}
      {organization.resultsBatchId !== null &&
        state.operation.active?.batchId === organization.resultsBatchId &&
        state.operation.results !== null && (
          <OperationResults
            batchId={organization.resultsBatchId}
            progress={state.operation.active}
            page={state.operation.results}
            onPageChange={(offset) => void organization.loadOperationResults(offset)}
            onClose={organization.closeResults}
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
