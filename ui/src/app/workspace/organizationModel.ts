import type { BrowserFile, FolderTreeItem, ProjectAccess } from '../../api/types'
import { buildRadialMenuModel } from '../../components/radialMenuModel'
import { isImageFile } from '../../fileKinds'
import { validatePreviewSelection } from '../../state/previewPolicy'
import type { OrganizationDragMode } from '../../state/useOrganizationPointerDrag'
import type { ViewerState, ViewerStatus } from '../../state/viewerState'

export interface OrganizationBusyInput {
  operationSubmitting: boolean
  projectStatus: ViewerStatus
  operationFinishing: boolean
  operationPending: boolean
  activeOperation: ViewerState['operation']['active']
}

export function organizationOperationBusy({
  operationSubmitting,
  projectStatus,
  operationFinishing,
  operationPending,
  activeOperation,
}: OrganizationBusyInput): boolean {
  return (
    operationSubmitting ||
    projectStatus !== 'active' ||
    operationFinishing ||
    operationPending ||
    (activeOperation !== null && activeOperation.lifecycle !== 'completed')
  )
}

export function canMutateOrganizationSelection(input: {
  selectionCount: number
  projectAccess: ProjectAccess | null
  operationBusy: boolean
}): boolean {
  return input.selectionCount > 0 && input.projectAccess === 'read_write' && !input.operationBusy
}

export function finderDragFailureMessage(error: unknown): string {
  const code =
    typeof error === 'object' && error !== null && 'code' in error ? String(error.code) : null
  return code === 'finder_drag_selection_stale' || code === 'stale_project_session'
    ? '部分文件已发生变化，请刷新后重试。'
    : '无法拖到 Finder，请重新拖动。'
}

export function isOrganizationDropTargetValid(input: {
  workspace: ViewerState['workspace']
  folders: readonly FolderTreeItem[]
  entityIds: readonly string[]
  destinationId: string
  mode: OrganizationDragMode
}): boolean {
  if (input.workspace?.workspace !== 'content') return false
  const destination = input.folders.find((folder) => folder.entityId === input.destinationId)
  if (!destination) return false
  const currentFiles = [
    ...input.workspace.images,
    ...input.workspace.videos,
    ...input.workspace.otherFiles,
  ]
  const byId = new Map(currentFiles.map((file) => [file.entityId, file]))
  const files = input.entityIds.map((entityId) => byId.get(entityId))
  if (files.some((file) => file === undefined)) return false
  return (
    input.mode === 'copy' ||
    files.every(
      (file) =>
        file !== undefined && parentRelativePath(file.relativePath) !== destination.relativePath,
    )
  )
}

export function buildOrganizationRadialModel(input: {
  files: readonly BrowserFile[]
  projectAccess: ProjectAccess | null
  operationBusy: boolean
  compareContextAvailable: boolean
}) {
  const reviews = new Set(input.files.map((file) => file.marker.reviewState))
  const favorites = new Set(input.files.map((file) => file.marker.favorite))
  const previewValidation = validatePreviewSelection(input.files)
  return buildRadialMenuModel({
    selectedCount: input.files.length,
    selectedImageCount: input.files.filter(isImageFile).length,
    previewEnabled: previewValidation.ok,
    previewDisabledReason: previewValidation.ok ? undefined : previewValidation.reason,
    readOnly: input.projectAccess === 'read_only',
    busy: input.operationBusy,
    compareContextAvailable: input.compareContextAvailable,
    commonReview: reviews.size === 1 ? (input.files[0]?.marker.reviewState ?? null) : 'mixed',
    commonFavorite: favorites.size === 1 ? (input.files[0]?.marker.favorite ?? false) : 'mixed',
  })
}

export function organizationProjectIdentity(state: ViewerState): string {
  return state.project ? `${state.project.sessionId}:${state.project.generation}` : 'no-project'
}

export function organizationWorkspaceIdentity(state: ViewerState): string {
  return [
    state.workspace?.workspace ?? 'none',
    state.selectedFolderId ?? 'root',
    state.showingAggregate ? 'aggregate' : 'folder',
    state.search.showResults ? 'search' : 'browser',
  ].join(':')
}

function parentRelativePath(relativePath: string): string {
  const separator = relativePath.lastIndexOf('/')
  return separator === -1 ? '' : relativePath.slice(0, separator)
}
