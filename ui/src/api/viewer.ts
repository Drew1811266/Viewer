import { invoke } from '@tauri-apps/api/core'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { open } from '@tauri-apps/plugin-dialog'
import type {
  BeginFinderDragRequest,
  CancelOperationRequest,
  CloseBlockedEvent,
  CloseChoice,
  CloseRequestOutcome,
  CloseTarget,
  ExecuteFileCommandRequest,
  FileCommandPreflight,
  FinderDragReceipt,
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
  PreflightFileCommandRequest,
  PreviewRenameRequest,
  ProjectChangedEvent,
  ProjectSnapshot,
  RenamePreview,
  ReviewAddFeedbackRequest,
  ReviewCompleteRequest,
  ReviewCompletionProposal,
  ReviewDeleteFeedbackRequest,
  ReviewMutationGuardRequest,
  ReviewPreviewStartRequest,
  ReviewProgress,
  ReviewScopeProposal,
  ReviewSessionRequest,
  ReviewSessionSnapshot,
  ReviewStartRequest,
  ReviewUpdateFeedbackRequest,
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
  VideoCacheStats,
  VideoEvent,
  VideoFullscreenRequest,
  VideoGenerationRequest,
  VideoMutedRequest,
  VideoOpenAttemptRequest,
  VideoOpenRequest,
  VideoRateRequest,
  VideoSeekRequest,
  VideoSession,
  VideoStepRequest,
  VideoThumbnailRequest,
  VideoVolumeRequest,
  ViewerSettings,
  ViewerSettingsUpdate,
} from './types'

export type ProjectDropEvent =
  | { type: 'enter'; paths: string[] }
  | { type: 'over' }
  | { type: 'drop'; paths: string[] }
  | { type: 'leave' }

export interface ViewerBridge {
  chooseProject(): Promise<string | null>
  openProject(path: string): Promise<ProjectSnapshot>
  closeProject(choice?: CloseChoice, target?: CloseTarget): Promise<CloseRequestOutcome>
  projectSnapshot(): Promise<ProjectSnapshot | null>
  getViewerSettings(): Promise<ViewerSettings>
  updateViewerSettings(settings: ViewerSettingsUpdate): Promise<ViewerSettings>
  folderTree(): Promise<FolderTreeItem[]>
  queryFolder(entityId: string | null, aggregate?: boolean): Promise<FolderWorkspace>
  requestImage(request: ImageRequest, signal?: AbortSignal): Promise<ImageRepresentation>
  previewText(request: TextPreviewRequest): Promise<TextPreview>
  openExternalLink(url: string): Promise<void>
  revealProjectInFileManager(): Promise<void>
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
  beginFinderDrag(request: BeginFinderDragRequest): Promise<FinderDragReceipt>
  reviewStatus(request: ReviewSessionRequest): Promise<ReviewSessionSnapshot>
  reviewPreviewStart(request: ReviewPreviewStartRequest): Promise<ReviewScopeProposal>
  reviewStart(request: ReviewStartRequest): Promise<ReviewSessionSnapshot>
  reviewResume(request: ReviewSessionRequest): Promise<ReviewSessionSnapshot>
  reviewAddFeedback(request: ReviewAddFeedbackRequest): Promise<ReviewSessionSnapshot>
  reviewUpdateFeedback(request: ReviewUpdateFeedbackRequest): Promise<ReviewSessionSnapshot>
  reviewDeleteFeedback(request: ReviewDeleteFeedbackRequest): Promise<ReviewSessionSnapshot>
  reviewCompletionSummary(request: ReviewMutationGuardRequest): Promise<ReviewCompletionProposal>
  reviewComplete(request: ReviewCompleteRequest): Promise<ReviewSessionSnapshot>
  reviewAbandon(request: ReviewMutationGuardRequest): Promise<ReviewSessionSnapshot>
  reviewCancelTask(request: ReviewSessionRequest): Promise<boolean>
  videoOpen(request: VideoOpenRequest): Promise<VideoSession>
  videoCancelOpen(request: VideoOpenAttemptRequest): Promise<boolean>
  videoClose(request: VideoGenerationRequest): Promise<void>
  videoPlay(request: VideoGenerationRequest): Promise<void>
  videoPause(request: VideoGenerationRequest): Promise<void>
  videoSeek(request: VideoSeekRequest): Promise<void>
  videoStep(request: VideoStepRequest): Promise<void>
  videoSetVolume(request: VideoVolumeRequest): Promise<void>
  videoSetMuted(request: VideoMutedRequest): Promise<void>
  videoSetRate(request: VideoRateRequest): Promise<void>
  videoSetFullscreen(request: VideoFullscreenRequest): Promise<void>
  videoRequestCover(entityId: string): Promise<string>
  videoRequestThumbnail(request: VideoThumbnailRequest): Promise<void>
  videoCacheStats(): Promise<VideoCacheStats>
  videoCacheClear(): Promise<VideoCacheStats>
  openPermissionSettings(): Promise<void>
  listenScan(handler: (event: ScanEvent) => void): Promise<UnlistenFn>
  listenIndexProgress(handler: (event: IndexProgressEvent) => void): Promise<UnlistenFn>
  listenOperationProgress(handler: (event: OperationProgressEvent) => void): Promise<UnlistenFn>
  listenProjectChanged(handler: (event: ProjectChangedEvent) => void): Promise<UnlistenFn>
  listenCloseBlocked(handler: (event: CloseBlockedEvent) => void): Promise<UnlistenFn>
  listenVideo(handler: (event: VideoEvent) => void): Promise<UnlistenFn>
  listenReviewProgress(handler: (event: ReviewProgress) => void): Promise<UnlistenFn>
  listenProjectClosed(handler: () => void): Promise<UnlistenFn>
  listenProjectDrops(handler: (paths: string[]) => void): Promise<UnlistenFn>
  listenProjectDropEvents(handler: (event: ProjectDropEvent) => void): Promise<UnlistenFn>
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
  closeProject(choice, target) {
    return invoke<CloseRequestOutcome>('close_project', {
      choice: choice ?? null,
      target: target ?? 'project',
    })
  },
  projectSnapshot() {
    return invoke<ProjectSnapshot | null>('project_snapshot')
  },
  getViewerSettings() {
    return invoke<ViewerSettings>('get_viewer_settings')
  },
  updateViewerSettings(settings) {
    return invoke<ViewerSettings>('update_viewer_settings', { settings })
  },
  folderTree() {
    return invoke<FolderTreeItem[]>('folder_tree')
  },
  queryFolder(entityId, aggregate = false) {
    return invoke<FolderWorkspace>('query_folder', { folderId: entityId, aggregate })
  },
  requestImage({ entityId, representation }, signal) {
    if (signal?.aborted) return Promise.reject(abortedImageRequest())
    const requestId = crypto.randomUUID()
    const nativeRequest = invoke<ImageRepresentation>('request_image_representation', {
      entityId,
      representation,
      requestId,
    })
    if (signal === undefined) return nativeRequest

    return new Promise<ImageRepresentation>((resolve, reject) => {
      let settled = false
      const finish = (complete: () => void) => {
        if (settled) return
        settled = true
        signal.removeEventListener('abort', abort)
        complete()
      }
      const abort = () => {
        finish(() => {
          void invoke<boolean>('cancel_image_request', { requestId }).catch(() => undefined)
          reject(abortedImageRequest())
        })
      }
      signal.addEventListener('abort', abort, { once: true })
      if (signal.aborted) {
        abort()
        return
      }
      void nativeRequest.then(
        (image) => finish(() => resolve(image)),
        (error: unknown) => finish(() => reject(error)),
      )
    })
  },
  previewText({ entityId, encoding }) {
    return invoke<TextPreview>('preview_text', { entityId, encoding: encoding ?? null })
  },
  openExternalLink(url) {
    return invoke<void>('open_external_link', { url })
  },
  revealProjectInFileManager() {
    return invoke<void>('reveal_project_in_file_manager')
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
  beginFinderDrag(request) {
    return invoke<FinderDragReceipt>('begin_finder_drag', { request })
  },
  reviewStatus(request) {
    return invoke<ReviewSessionSnapshot>('review_status', { request })
  },
  reviewPreviewStart(request) {
    return invoke<ReviewScopeProposal>('review_preview_start', { request })
  },
  reviewStart(request) {
    return invoke<ReviewSessionSnapshot>('review_start', { request })
  },
  reviewResume(request) {
    return invoke<ReviewSessionSnapshot>('review_resume', { request })
  },
  reviewAddFeedback(request) {
    return invoke<ReviewSessionSnapshot>('review_add_feedback', { request })
  },
  reviewUpdateFeedback(request) {
    return invoke<ReviewSessionSnapshot>('review_update_feedback', { request })
  },
  reviewDeleteFeedback(request) {
    return invoke<ReviewSessionSnapshot>('review_delete_feedback', { request })
  },
  reviewCompletionSummary(request) {
    return invoke<ReviewCompletionProposal>('review_completion_summary', { request })
  },
  reviewComplete(request) {
    return invoke<ReviewSessionSnapshot>('review_complete', { request })
  },
  reviewAbandon(request) {
    return invoke<ReviewSessionSnapshot>('review_abandon', { request })
  },
  reviewCancelTask(request) {
    return invoke<boolean>('review_cancel_task', { request })
  },
  videoOpen(request) {
    return invoke<VideoSession>('video_open', { request })
  },
  videoCancelOpen(request) {
    return invoke<boolean>('video_cancel_open', { request })
  },
  videoClose(request) {
    return invoke<void>('video_close', { request })
  },
  videoPlay(request) {
    return invoke<void>('video_play', { request })
  },
  videoPause(request) {
    return invoke<void>('video_pause', { request })
  },
  videoSeek(request) {
    return invoke<void>('video_seek', { request })
  },
  videoStep(request) {
    return invoke<void>('video_step', { request })
  },
  videoSetVolume(request) {
    return invoke<void>('video_set_volume', { request })
  },
  videoSetMuted(request) {
    return invoke<void>('video_set_muted', { request })
  },
  videoSetRate(request) {
    return invoke<void>('video_set_rate', { request })
  },
  videoSetFullscreen(request) {
    return invoke<void>('video_set_fullscreen', { request })
  },
  videoRequestCover(entityId) {
    return invoke<string>('video_request_cover', { entityId })
  },
  videoRequestThumbnail(request) {
    return invoke<void>('video_request_thumbnail', { request })
  },
  videoCacheStats() {
    return invoke<VideoCacheStats>('video_cache_stats')
  },
  videoCacheClear() {
    return invoke<VideoCacheStats>('video_cache_clear')
  },
  openPermissionSettings() {
    return invoke<void>('open_permission_settings')
  },
  listenScan(handler) {
    return listen<ScanEvent>('viewer://scan-progress', ({ payload }) => handler(payload))
  },
  listenIndexProgress(handler) {
    return listen<IndexProgressEvent>('viewer://index-progress', ({ payload }) => handler(payload))
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
    return listen<CloseBlockedEvent>('viewer://close-blocked', ({ payload }) => handler(payload))
  },
  listenVideo(handler) {
    return listen<VideoEvent>('viewer://video-event', ({ payload }) => handler(payload))
  },
  listenReviewProgress(handler) {
    return listen<ReviewProgress>('viewer://review-progress', ({ payload }) => handler(payload))
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
  listenProjectDropEvents(handler) {
    return getCurrentWebview().onDragDropEvent(({ payload }) => {
      if (payload.type === 'enter' || payload.type === 'drop') {
        handler({ type: payload.type, paths: payload.paths })
      } else {
        handler({ type: payload.type })
      }
    })
  },
}

function abortedImageRequest() {
  return new DOMException('Image request aborted', 'AbortError')
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
