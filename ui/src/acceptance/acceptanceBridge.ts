import type { BrowserFile, Marker, SearchHit } from '../api/types'
import type { ViewerBridge } from '../api/viewer'
import {
  ACCEPTANCE_FILES,
  ACCEPTANCE_FOLDER_TREE,
  ACCEPTANCE_PROJECT_SNAPSHOT,
  ACCEPTANCE_TEXT_FILES,
  ACCEPTANCE_UNSUPPORTED_FILE,
  acceptanceFile,
  acceptanceWorkspace,
  imageRepresentation,
} from './acceptanceFixtures'

export type AcceptanceBridgeOverrides = Partial<ViewerBridge>

export function createAcceptanceBridge(overrides: AcceptanceBridgeOverrides = {}): ViewerBridge {
  const bridge = {
    async chooseProject() {
      return '/ViewerAcceptance/测试图'
    },
    async openProject() {
      return ACCEPTANCE_PROJECT_SNAPSHOT
    },
    async closeProject() {
      return 'closed' as const
    },
    async projectSnapshot() {
      return ACCEPTANCE_PROJECT_SNAPSHOT
    },
    async getViewerSettings() {
      return {
        schemaVersion: 4 as const,
        thumbnailDensity: 'standard' as const,
        magnifier: {
          shape: 'circle' as const,
          magnification: 2 as const,
          area: 'small' as const,
        },
      }
    },
    async updateViewerSettings(settings) {
      return { schemaVersion: 4 as const, ...settings }
    },
    async folderTree() {
      return ACCEPTANCE_FOLDER_TREE
    },
    async queryFolder(entityId, showingAggregate) {
      return acceptanceWorkspace(entityId, showingAggregate)
    },
    async requestImage({ entityId, representation }, signal) {
      if (signal?.aborted) throw new DOMException('Image request aborted', 'AbortError')
      return imageRepresentation(acceptanceFile(entityId), representation.kind)
    },
    async previewText({ entityId }) {
      const file = acceptanceFile(entityId)
      if (file.kind === 'markdown') {
        return {
          entityId,
          format: 'markdown' as const,
          plainText: null,
          markdownHtml: '<h1>Viewer 视觉验收</h1><p>确定性的 Markdown 预览内容。</p>',
          encoding: 'utf8' as const,
          truncated: false,
        }
      }
      if (file.kind === 'text') {
        return {
          entityId,
          format: 'plain_text' as const,
          plainText: 'Viewer 确定性的纯文本预览内容。',
          markdownHtml: null,
          encoding: 'utf8' as const,
          truncated: false,
        }
      }
      return unexpected('previewText')
    },
    async openExternalLink() {
      return undefined
    },
    async revealProjectInFileManager() {
      return undefined
    },
    async cancelTask() {
      return false
    },
    async searchProject(request) {
      const hits = allFiles()
        .filter((file) =>
          request.text.length === 0
            ? true
            : `${file.name} ${file.relativePath}`.includes(request.text),
        )
        .map(searchHit)
      return {
        revision: request.revision,
        total: hits.length,
        hits: hits.slice(request.offset, request.offset + request.limit),
        progress: {
          imagesTotal: ACCEPTANCE_FILES.length,
          imagesReady: ACCEPTANCE_FILES.length,
          imagesFailed: 0,
          textTotal: ACCEPTANCE_TEXT_FILES.length,
          textReady: ACCEPTANCE_TEXT_FILES.length,
          textSkipped: 0,
          textFailed: 0,
          complete: true,
        },
      }
    },
    async searchTextSnippet(request) {
      return {
        revision: request.revision,
        entityId: request.entityId,
        snippet: `…${request.query}…`,
      }
    },
    async setReviewState(request) {
      return {
        changes: request.entityIds.map((entityId) =>
          markerChange(entityId, {
            reviewState: request.reviewState,
            favorite: acceptanceFile(entityId).marker.favorite,
          }),
        ),
      }
    },
    async toggleFavorite(request) {
      return {
        changes: request.entityIds.map((entityId) => {
          const file = acceptanceFile(entityId)
          return markerChange(entityId, {
            reviewState: file.marker.reviewState,
            favorite: !file.marker.favorite,
          })
        }),
      }
    },
    async selectionInfo(request) {
      const files = request.entityIds.map(acceptanceFile)
      return {
        relativePaths: files.map(({ relativePath }) => relativePath),
        totalSize: files.reduce((total, { size }) => total + size, 0),
        types: {
          folders: 0,
          images: files.filter(({ kind }) => kind === 'jpeg' || kind === 'png').length,
          videos: files.filter(({ kind }) => kind === 'video').length,
          otherFiles: files.filter(
            ({ kind }) => kind !== 'jpeg' && kind !== 'png' && kind !== 'video',
          ).length,
        },
        commonReview:
          files.length === 0 ? { state: 'none_selected' as const } : { state: 'mixed' as const },
        commonFavorite:
          files.length === 0 ? { state: 'none_selected' as const } : { state: 'mixed' as const },
      }
    },
    async previewRename() {
      return { rows: [], executable: false }
    },
    async preflightFileCommand() {
      return { rows: [], executable: false }
    },
    async executeFileCommand() {
      return unexpected('executeFileCommand')
    },
    async operationStatus() {
      return unexpected('operationStatus')
    },
    async operationResults() {
      return unexpected('operationResults')
    },
    async cancelOperation() {
      return unexpected('cancelOperation')
    },
    async undoLastOperation() {
      return unexpected('undoLastOperation')
    },
    async beginFinderDrag() {
      return unexpected('beginFinderDrag')
    },
    async videoOpen() {
      return unexpected('videoOpen')
    },
    async videoClose() {
      return unexpected('videoClose')
    },
    async videoPlay() {
      return unexpected('videoPlay')
    },
    async videoPause() {
      return unexpected('videoPause')
    },
    async videoSeek() {
      return unexpected('videoSeek')
    },
    async videoStep() {
      return unexpected('videoStep')
    },
    async videoSetVolume() {
      return unexpected('videoSetVolume')
    },
    async videoSetMuted() {
      return unexpected('videoSetMuted')
    },
    async videoSetRate() {
      return unexpected('videoSetRate')
    },
    async videoSetSurfaceRect() {
      return unexpected('videoSetSurfaceRect')
    },
    async videoSetFullscreen() {
      return unexpected('videoSetFullscreen')
    },
    async videoRequestThumbnail() {
      return unexpected('videoRequestThumbnail')
    },
    async videoCacheStats() {
      return unexpected('videoCacheStats')
    },
    async videoCacheClear() {
      return unexpected('videoCacheClear')
    },
    async openPermissionSettings() {
      return undefined
    },
    async listenScan() {
      return noOpUnlisten
    },
    async listenIndexProgress() {
      return noOpUnlisten
    },
    async listenOperationProgress() {
      return noOpUnlisten
    },
    async listenProjectChanged() {
      return noOpUnlisten
    },
    async listenCloseBlocked() {
      return noOpUnlisten
    },
    async listenVideo() {
      return noOpUnlisten
    },
    async listenProjectClosed() {
      return noOpUnlisten
    },
    async listenProjectDrops() {
      return noOpUnlisten
    },
    async listenProjectDropEvents() {
      return noOpUnlisten
    },
  } satisfies ViewerBridge

  return { ...bridge, ...overrides }
}

function allFiles(): BrowserFile[] {
  return [...ACCEPTANCE_FILES, ...ACCEPTANCE_TEXT_FILES, ACCEPTANCE_UNSUPPORTED_FILE]
}

function searchHit(file: BrowserFile): SearchHit {
  return {
    entityId: file.entityId,
    relativePath: file.relativePath,
    name: file.name,
    kind: file.kind,
    size: file.size,
    modifiedNs: file.modifiedNs,
    marker: file.marker,
    imageMetadata: file.imageMetadata,
    matchedField: 'filename',
    score: 1,
    groupRelativePath: file.relativePath.split('/').slice(0, -1).join('/') || null,
    matchRanges: [],
  }
}

function markerChange(entityId: string, marker: Marker) {
  const file = acceptanceFile(entityId)
  return { entityId, relativePath: file.relativePath, kind: file.kind, marker }
}

function noOpUnlisten() {}

function unexpected<T>(method: string): Promise<T> {
  return Promise.reject(new Error(`Unexpected acceptance bridge call: ${method}`))
}
