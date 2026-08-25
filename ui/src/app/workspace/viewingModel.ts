import type { BrowserFile, FolderWorkspace, SearchHit, VideoFile } from '../../api/types'
import type { TextPreviewFiles } from '../../components/TextPreview'
import { defined } from '../../defined'
import { isPreviewableText, isVideoFile } from '../../fileKinds'
import { videoPreviewNeighbors } from '../../state/previewPolicy'
import type { ContextRepair } from '../../state/viewerState'
import type { PreviewSession } from '../usePreviewSession'

export interface PreviewRepairMemory {
  unavailableEntityIds: ReadonlySet<string>
  message: string | null
}

export const EMPTY_PREVIEW_REPAIR: PreviewRepairMemory = {
  unavailableEntityIds: new Set(),
  message: null,
}

export function viewingVideoNeighbors(
  workspace: FolderWorkspace | null,
  searchResultsOpen: boolean,
  searchHits: readonly Pick<SearchHit, 'entityId' | 'kind'>[],
): VideoFile[] {
  if (workspace?.workspace !== 'content') return []
  return videoPreviewNeighbors(workspace.videos, searchResultsOpen ? searchHits : null)
}

export function activeTextPreviewFiles(
  activePreview: PreviewSession | null,
): TextPreviewFiles | null {
  const sessionFiles = activePreview === null ? [] : (activePreview.files ?? [activePreview.file])
  return sessionFiles.length === 1 &&
    isPreviewableText(defined(sessionFiles[0], 'Missing single preview file'))
    ? [defined(sessionFiles[0], 'Missing single preview file')]
    : sessionFiles.length === 2 && sessionFiles.every(isPreviewableText)
      ? [
          defined(sessionFiles[0], 'Missing left text preview file'),
          defined(sessionFiles[1], 'Missing right text preview file'),
        ]
      : null
}

export function resolveActivePreviewFiles(
  activePreview: PreviewSession | null,
  workspace: FolderWorkspace | null,
  currentVideoPreviewFiles: readonly VideoFile[],
): BrowserFile[] {
  return activePreview?.file.kind === 'video'
    ? (activePreview.files ?? currentVideoPreviewFiles)
        .filter(isVideoFile)
        .map(
          (file) =>
            (workspace?.workspace === 'content'
              ? workspace.videos.find((workspaceVideo) => workspaceVideo.entityId === file.entityId)
              : undefined) ?? file,
        )
    : (activePreview?.files ?? (workspace?.workspace === 'content' ? workspace.images : []))
}

export function resolveActivePreviewFile(
  activePreview: PreviewSession | null,
  activePreviewFiles: readonly BrowserFile[],
): BrowserFile | null {
  return activePreview === null
    ? null
    : (activePreviewFiles.find((candidate) => candidate.entityId === activePreview.file.entityId) ??
        activePreview.file)
}

export function accumulatePreviewRepair(
  current: PreviewRepairMemory,
  activePreview: PreviewSession | null,
  contextRepair: ContextRepair | null,
): PreviewRepairMemory {
  if (activePreview === null || contextRepair === null) return current
  const sessionEntityIds = new Set(
    (activePreview.files ?? [activePreview.file]).map((file) => file.entityId),
  )
  const removedSessionEntityIds = contextRepair.removedEntityIds.filter((entityId) =>
    sessionEntityIds.has(entityId),
  )
  if (removedSessionEntityIds.length === 0) return current
  const unavailableEntityIds = new Set(current.unavailableEntityIds)
  let changed = current.message !== contextRepair.message
  for (const entityId of removedSessionEntityIds) {
    if (!unavailableEntityIds.has(entityId)) {
      unavailableEntityIds.add(entityId)
      changed = true
    }
  }
  return changed ? { unavailableEntityIds, message: contextRepair.message } : current
}

export function unavailablePreviewEntityIds(
  previewRepair: PreviewRepairMemory,
  contextRepair: ContextRepair | null,
): ReadonlySet<string> {
  return new Set([
    ...previewRepair.unavailableEntityIds,
    ...(contextRepair?.removedEntityIds ?? []),
  ])
}
