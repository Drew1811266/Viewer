export type ProjectAccess = 'read_write' | 'read_only'

export type ThumbnailDensity = 'compact' | 'standard' | 'large' | 'extra_large' | 'maximum'

export type MagnifierShape = 'circle' | 'rounded_rectangle'
export type MagnifierMagnification = 3 | 4 | 5 | 6
export type MagnifierArea = 'small' | 'medium' | 'large'

export interface MagnifierPreferences {
  shape: MagnifierShape
  magnification: MagnifierMagnification
  area: MagnifierArea
}

export interface ViewerSettingsUpdate {
  thumbnailDensity: ThumbnailDensity
  magnifier: MagnifierPreferences
}

export interface ViewerSettings extends ViewerSettingsUpdate {
  schemaVersion: 2
}

export interface ProjectSnapshot {
  projectId: string
  sessionId: string
  generation: number
  displayName: string
  access: ProjectAccess
  recoveryReport?: RecoveryReport
}

export interface RecoveryReport {
  recovered: number
  needsUserReview: number
}

export type FileKind =
  | 'directory'
  | 'jpeg'
  | 'png'
  | 'markdown'
  | 'text'
  | 'unsupported_image'
  | 'other'
export type ReviewState = 'keep' | 'pending' | 'reject'

export interface Marker {
  reviewState: ReviewState | null
  favorite: boolean
}

export interface ImageMetadata {
  width: number
  height: number
}

export interface ScannedNode {
  entityId: string
  relativePath: string
  kind: FileKind
  size: number
  modifiedNs: string
}

interface ScanEventBase {
  sessionId: string
  generation: number
  taskId: string
}

export type ScanEvent =
  | (ScanEventBase & { type: 'folders'; nodes: ScannedNode[] })
  | (ScanEventBase & { type: 'files'; nodes: ScannedNode[] })
  | (ScanEventBase & {
      type: 'failed_item'
      relativePath: string
      code: string
    })
  | (ScanEventBase & {
      type: 'finished'
      totals: { folders: number; files: number; failed: number }
    })

export interface FolderTreeItem {
  entityId: string
  parentEntityId: string | null
  relativePath: string
  name: string
  marker: Marker
}

export interface BrowserFile {
  entityId: string
  relativePath: string
  name: string
  kind: Exclude<FileKind, 'directory'>
  size: number
  modifiedNs: string
  marker: Marker
  imageMetadata: ImageMetadata | null
  imageUrl: string | null
}

export interface FolderReviewProgress {
  total: number
  keep: number
  pending: number
  reject: number
  unmarked: number
  favorite: number
}

export interface ContentFolderCard {
  entityId: string
  relativePath: string
  name: string
  marker: Marker
  imageCount: number
  otherFileCount: number
  reviewProgress: FolderReviewProgress
  representativeImages: BrowserFile[]
}

export type FolderWorkspace =
  | { workspace: 'category'; folders: ContentFolderCard[] }
  | { workspace: 'content'; images: BrowserFile[]; otherFiles: BrowserFile[] }
  | { workspace: 'empty' }

export type ImageRepresentationRequest =
  | { kind: 'thumbnail'; maxPixels: number; scaleMilli: number }
  | {
      kind: 'fit_preview'
      maxWidth: number
      maxHeight: number
      scaleMilli: number
    }
  | { kind: 'original100_percent' }

export interface ImageRequest {
  entityId: string
  representation: ImageRepresentationRequest
}

export interface ImageRepresentation {
  cacheKey: string
  url: string
  width: number
  height: number
  backend: 'quick_look' | 'image_io'
}

export type TextEncoding = 'utf8' | 'utf16_le' | 'utf16_be' | 'gb18030'

export interface TextPreviewRequest {
  entityId: string
  encoding?: TextEncoding
}

export interface TextPreview {
  entityId: string
  format: 'plain_text' | 'markdown'
  plainText: string | null
  markdownHtml: string | null
  encoding: TextEncoding
  truncated: boolean
}

export interface ViewerCommandError {
  code: string
  category: 'validation' | 'conflict' | 'environment' | 'content' | 'consistency' | 'internal'
  userMessage: string
  retryable: boolean
  taskId: string | null
  itemId: string | null
}

export type ImageOrientation = 'landscape' | 'portrait' | 'square'
export type SearchSortKey =
  | 'relevance'
  | 'natural_name'
  | 'modified_time'
  | 'size'
  | 'pixel_dimensions'
  | 'review_state'
export type SortDirection = 'ascending' | 'descending'
export type SearchLayout = 'grouped' | 'flat'
export type MatchedField = 'exact_filename' | 'filename' | 'path' | 'body'

export interface SearchFilters {
  kinds: FileKind[]
  reviewStates: ReviewState[]
  favoriteOnly: boolean
  unmarkedOnly: boolean
  orientations: ImageOrientation[]
  widthMin: number | null
  widthMax: number | null
  heightMin: number | null
  heightMax: number | null
  sizeMin: number | null
  sizeMax: number | null
  modifiedNsMin: string | null
  modifiedNsMax: string | null
}

export interface SearchSort {
  key: SearchSortKey
  direction: SortDirection
}

export interface SearchQueryModel {
  text: string
  scopeFolderId: string | null
  filters: SearchFilters
  sort: SearchSort
  layout: SearchLayout
}

export interface SearchProjectRequest extends SearchQueryModel {
  sessionId: string
  generation: number
  revision: number
  offset: number
  limit: number
}

export interface MatchRange {
  start: number
  end: number
}

export interface SearchHit {
  entityId: string
  relativePath: string
  name: string
  kind: FileKind
  size: number
  modifiedNs: string
  marker: Marker
  imageMetadata: ImageMetadata | null
  matchedField: MatchedField
  score: number
  groupRelativePath: string | null
  matchRanges: MatchRange[]
}

export interface SearchProgress {
  imagesTotal: number
  imagesReady: number
  imagesFailed: number
  textTotal: number
  textReady: number
  textSkipped: number
  textFailed: number
  complete: boolean
}

export interface SearchPage {
  revision: number
  total: number
  hits: SearchHit[]
  progress: SearchProgress
}

export interface SearchTextSnippetRequest {
  sessionId: string
  generation: number
  revision: number
  entityId: string
  query: string
}

export interface TextSnippet {
  revision: number
  entityId: string
  snippet: string | null
}

export interface MarkerChange {
  entityId: string
  relativePath: string
  kind: FileKind
  marker: Marker
}

export interface MarkerBatchResult {
  changes: MarkerChange[]
}

interface MarkerRequestBase {
  sessionId: string
  generation: number
  entityIds: string[]
}

export interface SetReviewStateRequest extends MarkerRequestBase {
  reviewState: ReviewState | null
}

export type ToggleFavoriteRequest = MarkerRequestBase
export type SelectionInfoRequest = MarkerRequestBase

export type SelectionAgreement<T> =
  | { state: 'none_selected' }
  | { state: 'common'; value: T }
  | { state: 'mixed' }

export interface SelectionInfo {
  relativePaths: string[]
  totalSize: number
  types: { folders: number; images: number; otherFiles: number }
  commonReview: SelectionAgreement<ReviewState | null>
  commonFavorite: SelectionAgreement<boolean>
}

export interface IndexProgressEvent extends SearchProgress {
  sessionId: string
  generation: number
}

export type FileCommandKind = 'rename' | 'copy' | 'move' | 'trash'
export type ConflictPolicy = 'skip' | 'keep_both' | 'replace'

export interface RenameRules {
  find: string
  replacement: string
  prefix: string
  suffix: string
  sequence: { start: number; digits: number } | null
}

export interface PreviewRenameRequest {
  sessionId: string
  generation: number
  entityIds: string[]
  rules: RenameRules
}

export interface RenamePreviewRow {
  entityId: string
  sourceRelativePath: string
  destinationRelativePath: string | null
  proposedName: string
  errors: string[]
}

export interface RenamePreview {
  rows: RenamePreviewRow[]
  executable: boolean
}

export type FileCommandAction =
  | { kind: 'rename'; proposedName: string; editExtension: boolean }
  | { kind: 'copy'; destinationFolderId: string }
  | { kind: 'move'; destinationFolderId: string }
  | { kind: 'trash' }

export interface FileCommandItem {
  entityId: string
  action: FileCommandAction
}

export interface ConflictResolution {
  entityId: string
  policy: ConflictPolicy
  applyToRemaining: boolean
}

export interface ExecuteFileCommandRequest {
  sessionId: string
  generation: number
  kind: FileCommandKind
  items: FileCommandItem[]
  conflicts: ConflictResolution[]
}

export type PreflightFileCommandRequest = Omit<ExecuteFileCommandRequest, 'conflicts'>

export type FileCommandPreflightState = 'ready' | 'conflict' | 'blocked'

export interface FileCommandPreflightRow {
  entityId: string
  relativePath: string
  state: FileCommandPreflightState
  code?: BatchResultCode
}

export interface FileCommandPreflight {
  rows: FileCommandPreflightRow[]
  executable: boolean
}

export interface OperationStarted {
  batchId: string
}

export type BatchLifecycle = 'queued' | 'running' | 'cancelling' | 'completed'
export type BatchItemStatus = 'completed' | 'failed' | 'skipped' | 'cancelled'
export type BatchResultCode =
  | 'renamed'
  | 'copied'
  | 'moved'
  | 'moved_to_trash'
  | 'conflict_skipped'
  | 'cancelled'
  | 'session_stale'
  | 'source_missing'
  | 'destination_occupied'
  | 'permission_denied'
  | 'verification_failed'
  | 'projection_stale'
  | 'backend_unavailable'
  | 'invalid_target'

export interface OperationProgressEvent {
  sessionId: string
  generation: number
  batchId: string
  lifecycle: BatchLifecycle
  requested: number
  completed: number
  failed: number
  skipped: number
  cancelled: number
  activeEntityId: string | null
}

export interface OperationStatusRequest {
  sessionId: string
  generation: number
  batchId: string
}

export interface OperationResultItem {
  entityId: string
  relativePath: string
  status: BatchItemStatus
  code: BatchResultCode
}

export interface OperationResultPage {
  total: number
  offset: number
  items: OperationResultItem[]
}

export interface OperationResultsRequest extends OperationStatusRequest {
  offset: number
  limit: number
}

export type CancelOperationRequest = OperationStatusRequest

export interface UndoLastOperationRequest {
  sessionId: string
  generation: number
}

export interface UndoReceipt {
  batchId: string
  kind: Exclude<FileCommandKind, 'trash' | 'copy'> | 'set_review_state' | 'set_favorite'
  actionCount: number
}

export interface BeginFinderDragRequest {
  sessionId: string
  generation: number
  entityIds: string[]
}

export interface FinderDragReceipt {
  fileCount: number
}

export interface ProjectChangedEvent {
  sessionId: string
  generation: number
  reason: 'external_change' | 'expected_viewer_change' | 'overflow'
  added: number
  removed: number
  modified: number
  moved: number
  markerPathsMoved: number
  failed: number
}

export interface CloseBlockedEvent {
  sessionId: string
  generation: number
  batchId: string
  target: CloseTarget
}

export type CloseChoice = 'wait' | 'cancel_pending' | 'stay'

export type CloseRequestOutcome = 'closed' | 'stayed'

export type CloseTarget = 'project' | 'window' | 'application'
