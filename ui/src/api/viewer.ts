import { invoke } from '@tauri-apps/api/core'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { open } from '@tauri-apps/plugin-dialog'
import type {
  CancelOperationRequest,
  CloseBlockedEvent,
  ExecuteFileCommandRequest,
  FileCommandPreflight,
  FolderTreeItem,
  FolderWorkspace,
  ImageRepresentation,
  ImageRequest,
  IndexProgressEvent,
  MarkerBatchResult,
  OperationProgressEvent,
  OperationResultPage,
  OperationResultsRequest,
  OperationStarted,
  OperationStatusRequest,
  PreviewRenameRequest,
  PreflightFileCommandRequest,
  ProjectSnapshot,
  ProjectChangedEvent,
  RenamePreview,
  ScanEvent,
  SearchPage,
  SearchProjectRequest,
  SearchTextSnippetRequest,
  SelectionInfo,
  SelectionInfoRequest,
  SetReviewStateRequest,
  TextPreview,
  TextPreviewRequest,
  TextSnippet,
  ToggleFavoriteRequest,
  UndoLastOperationRequest,
  UndoReceipt,
} from './types'

export interface ViewerBridge {
  chooseProject(): Promise<string | null>
  openProject(path: string): Promise<ProjectSnapshot>
  closeProject(): Promise<void>
  projectSnapshot(): Promise<ProjectSnapshot | null>
  folderTree(): Promise<FolderTreeItem[]>
  queryFolder(entityId: string | null, aggregate?: boolean): Promise<FolderWorkspace>
  requestImage(request: ImageRequest): Promise<ImageRepresentation>
  previewText(request: TextPreviewRequest): Promise<TextPreview>
  openExternalLink(url: string): Promise<void>
  cancelTask(taskId: string): Promise<boolean>
  searchProject(request: SearchProjectRequest): Promise<SearchPage>
  searchTextSnippet(request: SearchTextSnippetRequest): Promise<TextSnippet>
  setReviewState(request: SetReviewStateRequest): Promise<MarkerBatchResult>
  toggleFavorite(request: ToggleFavoriteRequest): Promise<MarkerBatchResult>
  selectionInfo(request: SelectionInfoRequest): Promise<SelectionInfo>
  previewRename(request: PreviewRenameRequest): Promise<RenamePreview>
  preflightFileCommand(request: PreflightFileCommandRequest): Promise<FileCommandPreflight>
  executeFileCommand(request: ExecuteFileCommandRequest): Promise<OperationStarted>
  operationStatus(request: OperationStatusRequest): Promise<OperationProgressEvent>
  operationResults(request: OperationResultsRequest): Promise<OperationResultPage>
  cancelOperation(request: CancelOperationRequest): Promise<boolean>
  undoLastOperation(request: UndoLastOperationRequest): Promise<UndoReceipt | null>
  openPermissionSettings(): Promise<void>
  listenScan(handler: (event: ScanEvent) => void): Promise<UnlistenFn>
  listenIndexProgress(handler: (event: IndexProgressEvent) => void): Promise<UnlistenFn>
  listenOperationProgress(handler: (event: OperationProgressEvent) => void): Promise<UnlistenFn>
  listenProjectChanged(handler: (event: ProjectChangedEvent) => void): Promise<UnlistenFn>
  listenCloseBlocked(handler: (event: CloseBlockedEvent) => void): Promise<UnlistenFn>
  listenProjectClosed(handler: () => void): Promise<UnlistenFn>
  listenProjectDrops(handler: (paths: string[]) => void): Promise<UnlistenFn>
}

export const tauriViewerBridge: ViewerBridge = {
  async chooseProject() {
    const selected = await open({
      title: '选择 Viewer 项目文件夹',
      directory: true,
      multiple: false,
      recursive: false,
      canCreateDirectories: false,
    })
    return typeof selected === 'string' ? selected : null
  },
  openProject(path) {
    return invoke<ProjectSnapshot>('open_project', { root: path })
  },
  closeProject() {
    return invoke<void>('close_project')
  },
  projectSnapshot() {
    return invoke<ProjectSnapshot | null>('project_snapshot')
  },
  folderTree() {
    return invoke<FolderTreeItem[]>('folder_tree')
  },
  queryFolder(entityId, aggregate = false) {
    return invoke<FolderWorkspace>('query_folder', { folderId: entityId, aggregate })
  },
  requestImage({ entityId, representation }) {
    return invoke<ImageRepresentation>('request_image_representation', {
      entityId,
      representation,
    })
  },
  previewText({ entityId, encoding }) {
    return invoke<TextPreview>('preview_text', { entityId, encoding: encoding ?? null })
  },
  openExternalLink(url) {
    return invoke<void>('open_external_link', { url })
  },
  cancelTask(taskId) {
    return invoke<boolean>('cancel_task', { taskId })
  },
  searchProject(request) {
    return invoke<SearchPage>('search_project', { request })
  },
  searchTextSnippet(request) {
    return invoke<TextSnippet>('search_text_snippet', { request })
  },
  setReviewState(request) {
    return invoke<MarkerBatchResult>('set_review_state', { request })
  },
  toggleFavorite(request) {
    return invoke<MarkerBatchResult>('toggle_favorite', { request })
  },
  selectionInfo(request) {
    return invoke<SelectionInfo>('selection_info', { request })
  },
  previewRename(request) {
    return invoke<RenamePreview>('preview_rename', { request })
  },
  preflightFileCommand(request) {
    return invoke<FileCommandPreflight>('preflight_file_command', { request })
  },
  executeFileCommand(request) {
    return invoke<OperationStarted>('execute_file_command', { request })
  },
  operationStatus(request) {
    return invoke<OperationProgressEvent>('operation_status', { request })
  },
  operationResults(request) {
    return invoke<OperationResultPage>('operation_results', { request })
  },
  cancelOperation(request) {
    return invoke<boolean>('cancel_operation', { request })
  },
  undoLastOperation(request) {
    return invoke<UndoReceipt | null>('undo_last_operation', { request })
  },
  openPermissionSettings() {
    return invoke<void>('open_permission_settings')
  },
  listenScan(handler) {
    return listen<ScanEvent>('viewer://scan-progress', ({ payload }) => handler(payload))
  },
  listenIndexProgress(handler) {
    return listen<IndexProgressEvent>('viewer://index-progress', ({ payload }) =>
      handler(payload),
    )
  },
  listenOperationProgress(handler) {
    return listen<OperationProgressEvent>('viewer://operation-progress', ({ payload }) =>
      handler(payload),
    )
  },
  listenProjectChanged(handler) {
    return listen<ProjectChangedEvent>('viewer://project-changed', ({ payload }) =>
      handler(payload),
    )
  },
  listenCloseBlocked(handler) {
    return listen<CloseBlockedEvent>('viewer://close-blocked', ({ payload }) =>
      handler(payload),
    )
  },
  listenProjectClosed(handler) {
    return listen<void>('viewer://project-closed', handler)
  },
  listenProjectDrops(handler) {
    return getCurrentWebview().onDragDropEvent(({ payload }) => {
      if (payload.type === 'drop') {
        handler(payload.paths)
      }
    })
  },
}

export function safeUserMessage(error: unknown): string {
  if (
    typeof error === 'object' &&
    error !== null &&
    'userMessage' in error &&
    typeof error.userMessage === 'string' &&
    error.userMessage.length > 0
  ) {
    return error.userMessage
  }
  return '操作未完成，请重试。'
}
