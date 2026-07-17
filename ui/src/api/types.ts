export type ProjectAccess = 'read_write' | 'read_only'

export interface ProjectSnapshot {
  projectId: string
  sessionId: string
  generation: number
  displayName: string
  access: ProjectAccess
}

export type FileKind = 'directory' | 'jpeg' | 'png' | 'markdown' | 'text'
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
  textCount: number
  reviewProgress: FolderReviewProgress
  representativeImages: BrowserFile[]
}

export type FolderWorkspace =
  | { workspace: 'category'; folders: ContentFolderCard[] }
  | { workspace: 'content'; images: BrowserFile[]; textFiles: BrowserFile[] }
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
  category:
    | 'validation'
    | 'conflict'
    | 'environment'
    | 'content'
    | 'consistency'
    | 'internal'
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
  types: { folders: number; images: number; textFiles: number }
  commonReview: SelectionAgreement<ReviewState | null>
  commonFavorite: SelectionAgreement<boolean>
}

export interface IndexProgressEvent extends SearchProgress {
  sessionId: string
  generation: number
}
