import type { FolderWorkspace, Marker, MarkerChange, ScanEvent } from '../../api/types'
import { MAX_COMPARE_IMAGES } from '../comparePolicy'
import type { ScanState, ViewerAction, ViewerState } from '../viewerState'
import { isCurrentEvent, isCurrentProjection, sameStrings, unique } from './shared'

export function reduceWorkspaceAction(
  state: ViewerState,
  action: ViewerAction,
): ViewerState | undefined {
  switch (action.type) {
    case 'scan_received':
      if (!isCurrentEvent(state, action.event)) return state
      return { ...state, scan: reduceScan(state.scan, action.event) }
    case 'index_progress_received':
      if (!isCurrentProjection(state, action.progress.sessionId, action.progress.generation)) {
        return state
      }
      return { ...state, indexProgress: action.progress }
    case 'scan_cancelled':
      if (state.scan?.taskId !== action.taskId) return state
      return { ...state, scan: { ...state.scan, phase: 'cancelled' } }
    case 'projection_loaded':
      if (!isCurrentProjection(state, action.sessionId, action.generation)) return state
      return repairContextAfterProjection(state, {
        ...state,
        folders: action.folders,
        workspace: action.workspace,
        selectedFolderId: action.selectedFolderId,
        selectedFolderPath: action.selectedFolderPath,
        showingAggregate: action.showingAggregate,
        errorMessage: null,
      })
    case 'projection_failed':
      if (!isCurrentProjection(state, action.sessionId, action.generation)) return state
      return { ...state, errorMessage: action.message }
    case 'selection_changed':
      return {
        ...state,
        selectedEntityIds: unique(action.entityIds),
        selectionInfo: null,
      }
    case 'context_repair_consumed':
      return state.contextRepair?.suggestedEntityId
        ? {
            ...state,
            contextRepair: { ...state.contextRepair, suggestedEntityId: null },
          }
        : state
    case 'preview_context_changed':
      return { ...state, previewEntityId: action.entityId }
    case 'compare_context_changed': {
      const entityIds = unique(action.entityIds)
      if (entityIds.length > MAX_COMPARE_IMAGES) return state
      return { ...state, compareEntityIds: entityIds }
    }
    case 'project_changed_received':
      if (!isCurrentProjection(state, action.change.sessionId, action.change.generation)) {
        return state
      }
      return {
        ...state,
        pendingProjectChange: action.change,
        search: state.search.showResults
          ? {
              ...state.search,
              queryVersion: state.search.queryVersion + 1,
              schedule: 'immediate',
            }
          : state.search,
      }
    case 'selection_info_loaded':
      if (
        !isCurrentProjection(state, action.sessionId, action.generation) ||
        !sameStrings(state.selectedEntityIds, action.entityIds)
      ) {
        return state
      }
      return { ...state, selectionInfo: action.info }
    case 'marker_changes_applied':
      if (!isCurrentProjection(state, action.sessionId, action.generation)) return state
      return applyMarkerChanges(state, action.changes)
    default:
      return undefined
  }
}

function repairContextAfterProjection(previous: ViewerState, next: ViewerState): ViewerState {
  if (previous.pendingProjectChange === null) return next
  const previousOrder = orderedWorkspaceEntityIds(previous.workspace)
  const nextOrder = orderedWorkspaceEntityIds(next.workspace)
  const previouslyKnown = new Set([
    ...previous.folders.map((folder) => folder.entityId),
    ...previousOrder,
  ])
  const currentlyLive = new Set([...next.folders.map((folder) => folder.entityId), ...nextOrder])
  const active = unique([
    ...previous.selectedEntityIds,
    ...(previous.previewEntityId ? [previous.previewEntityId] : []),
    ...previous.compareEntityIds,
  ])
  const removedEntityIds = active.filter(
    (entityId) => previouslyKnown.has(entityId) && !currentlyLive.has(entityId),
  )
  if (removedEntityIds.length === 0) {
    return {
      ...next,
      pendingProjectChange: null,
      contextRepair: null,
    }
  }
  const removed = new Set(removedEntityIds)
  const firstRemovedIndex = previousOrder.findIndex((entityId) => removed.has(entityId))
  const suggestedEntityId =
    firstRemovedIndex < 0 || nextOrder.length === 0
      ? null
      : (nextOrder[Math.min(firstRemovedIndex, nextOrder.length - 1)] ?? null)
  return {
    ...next,
    pendingProjectChange: null,
    selectedEntityIds: previous.selectedEntityIds.filter((id) => !removed.has(id)),
    selectionInfo: null,
    previewEntityId:
      previous.previewEntityId && removed.has(previous.previewEntityId)
        ? null
        : previous.previewEntityId,
    compareEntityIds: previous.compareEntityIds.filter((id) => !removed.has(id)),
    contextRepair: {
      removedEntityIds,
      suggestedEntityId,
      message: '部分正在查看的文件已在项目外发生变化。',
    },
  }
}

function orderedWorkspaceEntityIds(workspace: FolderWorkspace | null): string[] {
  if (workspace === null || workspace.workspace === 'empty') return []
  if (workspace.workspace === 'category') {
    return workspace.folders.map((folder) => folder.entityId)
  }
  return [...workspace.images, ...workspace.otherFiles].map((file) => file.entityId)
}

function applyMarkerChanges(state: ViewerState, changes: MarkerChange[]): ViewerState {
  const markers = new Map(changes.map((change) => [change.entityId, change.marker]))
  const markerFor = (entityId: string, current: Marker) => markers.get(entityId) ?? current
  const folders = state.folders.map((folder) => ({
    ...folder,
    marker: markerFor(folder.entityId, folder.marker),
  }))
  const workspace = state.workspace && updateWorkspaceMarkers(state.workspace, markers)
  const page = state.search.page && {
    ...state.search.page,
    hits: state.search.page.hits.map((hit) => ({
      ...hit,
      marker: markerFor(hit.entityId, hit.marker),
    })),
  }
  return {
    ...state,
    folders,
    workspace,
    search: { ...state.search, page },
  }
}

function updateWorkspaceMarkers(
  workspace: FolderWorkspace,
  markers: Map<string, Marker>,
): FolderWorkspace {
  if (workspace.workspace === 'empty') return workspace
  if (workspace.workspace === 'category') {
    return {
      ...workspace,
      folders: workspace.folders.map((folder) => ({
        ...folder,
        marker: markers.get(folder.entityId) ?? folder.marker,
        representativeImages: folder.representativeImages.map((file) => ({
          ...file,
          marker: markers.get(file.entityId) ?? file.marker,
        })),
      })),
    }
  }
  return {
    ...workspace,
    images: workspace.images.map((file) => ({
      ...file,
      marker: markers.get(file.entityId) ?? file.marker,
    })),
    otherFiles: workspace.otherFiles.map((file) => ({
      ...file,
      marker: markers.get(file.entityId) ?? file.marker,
    })),
  }
}

function reduceScan(previous: ScanState | null, event: ScanEvent): ScanState {
  const current =
    previous?.taskId === event.taskId
      ? previous
      : {
          taskId: event.taskId,
          phase: 'running' as const,
          publishedFolders: 0,
          publishedFiles: 0,
          failedItems: [],
        }
  switch (event.type) {
    case 'folders':
      return {
        ...current,
        phase: 'running',
        publishedFolders: current.publishedFolders + event.nodes.length,
      }
    case 'files':
      return {
        ...current,
        phase: 'running',
        publishedFiles: current.publishedFiles + event.nodes.length,
      }
    case 'failed_item':
      return {
        ...current,
        phase: 'running',
        failedItems: [
          ...current.failedItems,
          { relativePath: event.relativePath, code: event.code },
        ],
      }
    case 'finished':
      return { ...current, phase: 'finished', totals: event.totals }
  }
}
