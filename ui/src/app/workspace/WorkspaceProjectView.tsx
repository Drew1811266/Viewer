import type { CSSProperties, MutableRefObject, ReactNode } from 'react'
import { useCallback, useRef } from 'react'
import type { FolderTreeItem, RenamePreview, RenameRules } from '../../api/types'
import BatchRenameDialog from '../../components/BatchRenameDialog'
import CloseOperationDialog from '../../components/CloseOperationDialog'
import CompareWorkspace from '../../components/CompareWorkspace'
import ContentBrowser from '../../components/ContentBrowser'
import DestinationDialog from '../../components/DestinationDialog'
import FolderOverview from '../../components/FolderOverview'
import FolderTree from '../../components/FolderTree'
import GlobalNoticeStack from '../../components/GlobalNoticeStack'
import ImagePreview from '../../components/ImagePreview'
import InfoOverlay from '../../components/InfoOverlay'
import type { Point } from '../../components/imagePreview/imageGeometry'
import OperationResults from '../../components/OperationResults'
import OrganizationDragPreview from '../../components/OrganizationDragPreview'
import RadialFileMenu from '../../components/RadialFileMenu'
import ReadOnlyBanner from '../../components/ReadOnlyBanner'
import RenameDialog from '../../components/RenameDialog'
import SearchResults from '../../components/SearchResults'
import SearchToolbar from '../../components/SearchToolbar'
import SettingsDialog from '../../components/SettingsDialog'
import TaskBar from '../../components/TaskBar'
import TextPreview from '../../components/TextPreview'
import TrashConfirmation from '../../components/TrashConfirmation'
import UnsupportedFilePreview from '../../components/UnsupportedFilePreview'
import ViewerButton, { ViewerIconButton } from '../../components/ui/ViewerButton'
import ViewerEmptyState from '../../components/ui/ViewerEmptyState'
import ViewerStatusTag from '../../components/ui/ViewerStatusTag'
import VideoPreview from '../../components/VideoPreview'
import WorkspaceLoadingState from '../../components/WorkspaceLoadingState'
import WorkspaceMoreMenu from '../../components/WorkspaceMoreMenu'
import WorkspaceViewMenu, { type WorkspaceViewContext } from '../../components/WorkspaceViewMenu'
import { isImageFile, isVideoFile } from '../../fileKinds'
import type { ViewerSettingsContextValue } from '../../settings/ViewerSettingsProvider'
import type { ViewerController } from '../../state/useViewerController'
import type { ViewerState } from '../../state/viewerState'
import { useDelayedProjectionProgress } from '../useDelayedProjectionProgress'
import type { WorkspaceIntentSink } from './intents'
import type { WorkspacePorts } from './ports'
import type { FeedbackCoordinator } from './useFeedbackCoordinator'
import type { OrganizationCoordinator } from './useOrganizationCoordinator'
import type { ViewingCoordinator } from './useViewingCoordinator'
import type { WorkspaceShellCoordinator } from './useWorkspaceShellCoordinator'

type Project = NonNullable<ViewerState['project']>

export type WorkspaceViewCommands = Pick<
  ViewerController,
  | 'cancelTask'
  | 'clearCloseBlocked'
  | 'clearSearchFilters'
  | 'closeProject'
  | 'openPermissionSettings'
  | 'removeSearchFilter'
  | 'returnToFolderContext'
  | 'selectFolder'
  | 'setReviewState'
  | 'setSearchFilters'
  | 'setSearchLayout'
  | 'setSearchPage'
  | 'setSearchScope'
  | 'setSearchSort'
  | 'setSearchText'
  | 'setSelectedEntityIds'
  | 'setVisibleSearchHits'
  | 'showAllDescendants'
  | 'toggleFavorite'
>

export interface WorkspaceProjectViewProps {
  state: ViewerState
  project: Project
  projectSessionId: string
  commands: WorkspaceViewCommands
  ports: Pick<WorkspacePorts, 'settings'>
  settings: ViewerSettingsContextValue
  shell: WorkspaceShellCoordinator
  viewing: ViewingCoordinator
  organization: OrganizationCoordinator
  feedback: FeedbackCoordinator
  emitIntent: WorkspaceIntentSink
  pointerClientPoint: MutableRefObject<Point | null>
  reviewToolbarAction: ReactNode
  reviewLayer: ReactNode
  onReselectProject(): void
}

export default function WorkspaceProjectView(props: WorkspaceProjectViewProps) {
  const { state, project, projectSessionId, commands, shell, viewing, organization } = props
  const moreMenuTriggerRef = useRef<HTMLElement>(null)
  const displayedFolderId = state.projectionTransition?.selectedFolderId ?? state.selectedFolderId
  const projectionProgressVisible = useDelayedProjectionProgress(
    state.projectionTransition,
    state.workspace !== null,
  )
  const selectFolderTarget = useCallback(
    (entityId: string | null) => {
      organization.selectFolderTarget(entityId)
      void commands.selectFolder(entityId)
    },
    [commands.selectFolder, organization.selectFolderTarget],
  )
  const viewContext = buildWorkspaceViewContext(state, shell, commands)
  const pendingRecoveryReport =
    state.recoveryReport !== null &&
    (state.recoveryReport.recovered > 0 || state.recoveryReport.needsUserReview > 0) &&
    shell.recoveryAcknowledgedSessionId !== projectSessionId
      ? state.recoveryReport
      : null

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
      <WorkspaceHeader
        {...props}
        viewContext={viewContext}
        moreMenuTriggerRef={moreMenuTriggerRef}
      />
      {project.access === 'read_only' && (
        <ReadOnlyBanner
          busy={state.status === 'closing'}
          onOpenSettings={() => void commands.openPermissionSettings()}
          onReselect={props.onReselectProject}
        />
      )}
      {props.reviewLayer}
      <WorkspaceColumns
        {...props}
        displayedFolderId={displayedFolderId}
        projectionProgressVisible={projectionProgressVisible}
        pendingRecoveryReport={pendingRecoveryReport}
        selectFolderTarget={selectFolderTarget}
      />
      <WorkspaceFeedbackLayers {...props} />
      <WorkspaceOverlays {...props} moreMenuTriggerRef={moreMenuTriggerRef} />
    </main>
  )
}

function buildWorkspaceViewContext(
  state: ViewerState,
  shell: WorkspaceShellCoordinator,
  commands: WorkspaceViewCommands,
): WorkspaceViewContext {
  if (state.search.showResults) {
    return {
      kind: 'search',
      layout: state.search.query.layout,
      onLayoutChange: commands.setSearchLayout,
    }
  }
  if (state.workspace?.workspace === 'category') {
    return { kind: 'category', onShowAllDescendants: () => void commands.showAllDescendants() }
  }
  if (state.workspace?.workspace === 'content') {
    return {
      kind: 'content',
      showingAggregate: state.showingAggregate,
      selectAllRequest: shell.selectAllRequest,
      onSelectAll: shell.requestSelectAll,
      onShowAllDescendants: () => void commands.showAllDescendants(),
      onReturnToFolder: commands.returnToFolderContext,
    }
  }
  return { kind: 'none' }
}

function WorkspaceHeader({
  state,
  project,
  commands,
  shell,
  emitIntent,
  viewContext,
  moreMenuTriggerRef,
  reviewToolbarAction,
  onReselectProject,
}: WorkspaceProjectViewProps & {
  viewContext: WorkspaceViewContext
  moreMenuTriggerRef: MutableRefObject<HTMLElement | null>
}) {
  return (
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
            <h1>{project.displayName}</h1>
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
          onTextChange={commands.setSearchText}
          onScopeChange={commands.setSearchScope}
          onFiltersChange={commands.setSearchFilters}
          onSortChange={commands.setSearchSort}
          onRemoveFilter={commands.removeSearchFilter}
          onClearFilters={commands.clearSearchFilters}
        />
        {reviewToolbarAction}
        <WorkspaceViewMenu
          context={viewContext}
          open={shell.toolbarPopover.openPopover === 'view'}
          onOpenChange={(open) => shell.toolbarPopover.setPopoverOpen('view', open)}
        />
        <WorkspaceMoreMenu
          ref={moreMenuTriggerRef}
          open={shell.toolbarPopover.openPopover === 'more'}
          onOpenChange={(open) => shell.toolbarPopover.setPopoverOpen('more', open)}
          access={project.access}
          closing={state.status === 'closing'}
          onOpenSettings={() => emitIntent({ kind: 'open-settings' })}
          onOpenPermissionSettings={() => void commands.openPermissionSettings()}
          onReselectProject={onReselectProject}
          onCloseProject={() => emitIntent({ kind: 'close-project' })}
        />
      </div>
    </header>
  )
}

function WorkspaceColumns({
  state,
  project,
  settings,
  shell,
  viewing,
  organization,
  feedback,
  commands,
  displayedFolderId,
  projectionProgressVisible,
  pendingRecoveryReport,
  selectFolderTarget,
}: WorkspaceProjectViewProps & {
  displayedFolderId: string | null
  projectionProgressVisible: boolean
  pendingRecoveryReport: ViewerState['recoveryReport']
  selectFolderTarget(entityId: string | null): void
}) {
  return (
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
            {project.displayName}
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
      <WorkspaceContent
        state={state}
        project={project}
        commands={commands}
        shell={shell}
        viewing={viewing}
        organization={organization}
        feedback={feedback}
        thumbnailDensity={settings.thumbnailDensity}
        projectionProgressVisible={projectionProgressVisible}
        pendingRecoveryReport={pendingRecoveryReport}
        selectFolderTarget={selectFolderTarget}
      />
    </div>
  )
}

function WorkspaceContent({
  state,
  project,
  commands,
  shell,
  viewing,
  organization,
  feedback,
  thumbnailDensity,
  projectionProgressVisible,
  pendingRecoveryReport,
  selectFolderTarget,
}: Pick<
  WorkspaceProjectViewProps,
  'state' | 'project' | 'commands' | 'shell' | 'viewing' | 'organization' | 'feedback'
> & {
  thumbnailDensity: ViewerSettingsContextValue['thumbnailDensity']
  projectionProgressVisible: boolean
  pendingRecoveryReport: ViewerState['recoveryReport']
  selectFolderTarget(entityId: string | null): void
}) {
  const contentWorkspaceActive =
    !state.search.showResults && state.workspace?.workspace === 'content'

  return (
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
        <WorkspaceContentBody
          state={state}
          project={project}
          commands={commands}
          shell={shell}
          viewing={viewing}
          organization={organization}
          feedback={feedback}
          thumbnailDensity={thumbnailDensity}
          selectFolderTarget={selectFolderTarget}
        />
      )}
    </section>
  )
}

function WorkspaceContentBody({
  state,
  project,
  commands,
  shell,
  viewing,
  organization,
  feedback,
  thumbnailDensity,
  selectFolderTarget,
}: Pick<
  WorkspaceProjectViewProps,
  'state' | 'project' | 'commands' | 'shell' | 'viewing' | 'organization' | 'feedback'
> & {
  thumbnailDensity: ViewerSettingsContextValue['thumbnailDensity']
  selectFolderTarget(entityId: string | null): void
}) {
  return (
    <>
      {viewing.contextRepairMessage && (
        <p className="context-repair-banner local-error" role="status">
          {viewing.contextRepairMessage}
        </p>
      )}
      <WorkspaceSearchContent state={state} commands={commands} />
      <WorkspaceBrowseContent
        state={state}
        project={project}
        commands={commands}
        shell={shell}
        viewing={viewing}
        organization={organization}
        feedback={feedback}
        thumbnailDensity={thumbnailDensity}
        selectFolderTarget={selectFolderTarget}
      />
    </>
  )
}

function WorkspaceSearchContent({
  state,
  commands,
}: Pick<WorkspaceProjectViewProps, 'state' | 'commands'>) {
  if (!state.search.showResults) return null
  if (state.search.page === null) {
    if (state.search.status === 'searching') return <p role="status">正在搜索…</p>
    if (state.search.status === 'error') return <p>搜索未完成，请调整条件或重试。</p>
    return null
  }
  return (
    <SearchResults
      page={state.search.page}
      query={state.search.query}
      snippets={state.search.snippets}
      offset={state.search.offset}
      limit={200}
      onPageChange={commands.setSearchPage}
      onVisibleHits={commands.setVisibleSearchHits}
      onClearFilters={commands.clearSearchFilters}
      onSearchProject={() => commands.setSearchScope(null)}
      onReturnToFolder={commands.returnToFolderContext}
      selectedEntityIds={state.selectedEntityIds}
      onSelectionChange={commands.setSelectedEntityIds}
      searching={state.search.status === 'searching' || !state.search.page.progress.complete}
    />
  )
}

function WorkspaceBrowseContent(
  props: Pick<
    WorkspaceProjectViewProps,
    'state' | 'project' | 'commands' | 'shell' | 'viewing' | 'organization' | 'feedback'
  > & {
    thumbnailDensity: ViewerSettingsContextValue['thumbnailDensity']
    selectFolderTarget(entityId: string | null): void
  },
) {
  const { state, shell, viewing, thumbnailDensity, selectFolderTarget } = props
  if (state.search.showResults) return null
  if (state.workspace === null) return <WorkspaceLoadingState />
  if (state.workspace.workspace === 'empty') {
    return (
      <ViewerEmptyState
        appearance="plain"
        title="这个项目中还没有可显示的文件"
        description="Viewer 会显示支持的图片、视频、Markdown 与文本文件。"
        action={<ViewerButton onClick={shell.revealProject}>在文件管理器中显示</ViewerButton>}
      />
    )
  }
  if (state.workspace.workspace === 'category') {
    return (
      <FolderOverview
        key={viewing.folderOverviewIdentity}
        folders={state.workspace.folders}
        density={thumbnailDensity}
        requestFolderImages={viewing.requestFolderImages}
        requestThumbnail={viewing.requestThumbnail}
        onPreview={viewing.openFilmstripPreview}
        onSelect={selectFolderTarget}
      />
    )
  }
  return <ContentWorkspace {...props} />
}

function ContentWorkspace({
  state,
  project,
  commands,
  shell,
  viewing,
  organization,
  feedback,
  thumbnailDensity,
}: Pick<
  WorkspaceProjectViewProps,
  'state' | 'project' | 'commands' | 'shell' | 'viewing' | 'organization' | 'feedback'
> & { thumbnailDensity: ViewerSettingsContextValue['thumbnailDensity'] }) {
  if (state.workspace?.workspace !== 'content') return null

  return (
    <>
      <div
        className="content-workspace-surface"
        data-testid="content-workspace-surface"
        hidden={viewing.compareOpen}
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
          selectedEntityIds={state.selectedEntityIds}
          organizationDragDisabled={
            project.access !== 'read_write' || organization.operationBusy || viewing.compareOpen
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
      {!viewing.compareOpen && viewing.compareStatus && (
        <p className="compare-status" role="status">
          {viewing.compareStatus}
        </p>
      )}
      {viewing.compareOpen && (
        <>
          <CompareWorkspace
            files={viewing.compareFiles}
            readOnly={project.access === 'read_only'}
            requestImage={viewing.requestPreviewImage}
            onEntityIdsChange={viewing.changeComparedEntities}
            onSetReview={(entityId, reviewState) =>
              void commands.setReviewState(reviewState, [entityId])
            }
            onToggleFavorite={(entityId) => void commands.toggleFavorite([entityId])}
            onStatus={viewing.setCompareStatus}
          />
          {viewing.compareStatus && (
            <p className="compare-status" role="status">
              {viewing.compareStatus}
            </p>
          )}
        </>
      )}
    </>
  )
}

function WorkspaceFeedbackLayers({
  state,
  project,
  commands,
  organization,
  feedback,
  emitIntent,
}: WorkspaceProjectViewProps) {
  return (
    <>
      <GlobalNoticeStack
        notices={feedback.globalNotices}
        belowReadOnly={project.access === 'read_only'}
      />
      {organization.organizationDragView && (
        <OrganizationDragPreview {...organization.organizationDragView} />
      )}
      <TaskBar
        tasks={feedback.visibleTasks}
        onCancel={(taskId) => {
          if (taskId === state.operation.active?.batchId) void organization.cancelOperation()
          else void commands.cancelTask(taskId)
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
          readOnly={project.access === 'read_only'}
          returnFocusTarget={organization.activeRadialMenu.returnFocusTarget}
          model={organization.radialModel}
          onAction={organization.runRadialAction}
          onClose={organization.finishRadialSession}
        />
      )}
    </>
  )
}

function WorkspaceOverlays({
  state,
  settings,
  ports,
  commands,
  shell,
  viewing,
  organization,
  feedback,
  pointerClientPoint,
  moreMenuTriggerRef,
}: WorkspaceProjectViewProps & { moreMenuTriggerRef: MutableRefObject<HTMLElement | null> }) {
  return (
    <>
      {shell.settingsOpen && (
        <SettingsDialog
          bridge={ports.settings}
          density={settings.thumbnailDensity}
          magnifier={settings.magnifier}
          error={settings.settingsError}
          onDensityChange={settings.setThumbnailDensity}
          onMagnifierShapeChange={settings.setMagnifierShape}
          onMagnifierMagnificationChange={settings.setMagnifierMagnification}
          onMagnifierAreaChange={settings.setMagnifierArea}
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
            magnifier={settings.magnifier}
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
      <WorkspaceOperationDialogs organization={organization} folders={state.folders} />
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
          onWait={() => void commands.closeProject('wait', state.closeBlocked?.target ?? 'project')}
          onCancelPending={() =>
            void commands.closeProject('cancel_pending', state.closeBlocked?.target ?? 'project')
          }
          onStay={commands.clearCloseBlocked}
        />
      )}
    </>
  )
}

function WorkspaceOperationDialogs({
  organization,
  folders,
}: {
  organization: OrganizationCoordinator
  folders: FolderTreeItem[]
}) {
  const dialog = organization.operationDialog

  if (dialog?.kind === 'rename') {
    return (
      <RenameDialog
        currentName={dialog.file.name}
        busy={organization.operationBusy}
        onCancel={organization.closeOperationDialog}
        onConfirm={(proposedName, editExtension) =>
          void organization.submitFileCommand('rename', [
            {
              entityId: dialog.file.entityId,
              action: { kind: 'rename', proposedName, editExtension },
            },
          ])
        }
      />
    )
  }

  if (dialog?.kind === 'batch_rename') {
    return (
      <BatchRenameDialog
        entityIds={dialog.files.map((file) => file.entityId)}
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
    )
  }

  if (dialog?.kind === 'destination') {
    return (
      <DestinationDialog
        mode={dialog.mode}
        entityIds={dialog.files.map((file) => file.entityId)}
        folders={folders}
        busy={organization.operationBusy}
        initialDestinationId={dialog.initialDestinationId}
        initialPreflight={dialog.initialPreflight}
        requestPreflight={(items) => organization.preflightFileCommand(dialog.mode, items)}
        onCancel={organization.closeOperationDialog}
        onConfirm={(items, conflicts) =>
          void organization.submitFileCommand(dialog.mode, items, conflicts)
        }
      />
    )
  }

  if (dialog?.kind === 'trash') {
    return (
      <TrashConfirmation
        count={dialog.files.length}
        busy={organization.operationBusy}
        onCancel={organization.closeOperationDialog}
        onConfirm={() =>
          void organization.submitFileCommand(
            'trash',
            dialog.files.map((file) => ({
              entityId: file.entityId,
              action: { kind: 'trash' },
            })),
          )
        }
      />
    )
  }

  return null
}
