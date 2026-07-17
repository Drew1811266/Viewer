export type ProjectAccess = 'read_write' | 'read_only'

export interface ProjectSnapshot {
  projectId: string
  sessionId: string
  generation: number
  displayName: string
  access: ProjectAccess
}

export type FileKind = 'directory' | 'jpeg' | 'png' | 'markdown' | 'text'

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
}

export interface BrowserFile {
  entityId: string
  relativePath: string
  name: string
  kind: Exclude<FileKind, 'directory'>
  size: number
  modifiedNs: string
  imageUrl: string | null
}

export interface ContentFolderCard {
  entityId: string
  relativePath: string
  name: string
  imageCount: number
  textCount: number
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
