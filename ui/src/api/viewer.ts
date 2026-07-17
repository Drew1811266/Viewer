import { invoke } from '@tauri-apps/api/core'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { open } from '@tauri-apps/plugin-dialog'
import type {
  FolderTreeItem,
  FolderWorkspace,
  ImageRepresentation,
  ImageRequest,
  ProjectSnapshot,
  ScanEvent,
  TextPreview,
  TextPreviewRequest,
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
  listenScan(handler: (event: ScanEvent) => void): Promise<UnlistenFn>
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
  listenScan(handler) {
    return listen<ScanEvent>('viewer://scan-progress', ({ payload }) => handler(payload))
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
