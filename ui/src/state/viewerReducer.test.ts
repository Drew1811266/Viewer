import { describe, expect, it } from 'vitest'
import type {
  BrowserFile,
  OperationProgressEvent,
  ProjectChangedEvent,
  ProjectSnapshot,
  ScanEvent,
  SearchPage,
} from '../api/types'
import {
  emptySearchFilters,
  initialViewerState,
  viewerReducer,
} from './viewerReducer'

const project: ProjectSnapshot = {
  projectId: 'project-1',
  sessionId: 'session-1',
  generation: 7,
  displayName: 'Catalog',
  access: 'read_write',
}

function scan(generation: number): ScanEvent {
  return {
    type: 'folders',
    sessionId: project.sessionId,
    generation,
    taskId: 'task-1',
    nodes: [],
  }
}

describe('viewerReducer', () => {
  it('always starts empty and ignores stale or wrong-session scan events', () => {
    const active = viewerReducer(initialViewerState, { type: 'project_opened', project })

    expect(viewerReducer(active, { type: 'scan_received', event: scan(6) })).toEqual(active)
    expect(
      viewerReducer(active, {
        type: 'scan_received',
        event: { ...scan(7), sessionId: 'another-session' },
      }),
    ).toEqual(active)
  })

  it('accepts current scan progress and returns to a clean empty state on close', () => {
    const active = viewerReducer(initialViewerState, { type: 'project_opened', project })
    const scanning = viewerReducer(active, { type: 'scan_received', event: scan(7) })

    expect(scanning.scan?.taskId).toBe('task-1')
    expect(viewerReducer(scanning, { type: 'project_closed' })).toEqual(initialViewerState)
  })

  it('tracks search intent and query controls while folder switches preserve them', () => {
    let state = viewerReducer(initialViewerState, { type: 'project_opened', project })
    state = viewerReducer(state, { type: 'search_focus_requested' })
    state = viewerReducer(state, { type: 'search_text_changed', text: 'shoe' })
    state = viewerReducer(state, { type: 'search_scope_changed', folderId: 'folder-2' })
    state = viewerReducer(state, {
      type: 'search_filters_changed',
      filters: {
        ...emptySearchFilters,
        kinds: ['jpeg', 'png'],
        reviewStates: ['keep'],
        favoriteOnly: true,
      },
    })
    state = viewerReducer(state, {
      type: 'search_sort_changed',
      sort: { key: 'size', direction: 'descending' },
    })
    state = viewerReducer(state, { type: 'search_layout_changed', layout: 'flat' })

    expect(state.search.focusRequest).toBe(1)
    expect(state.search.query.text).toBe('shoe')
    expect(state.search.query.scopeFolderId).toBe('folder-2')
    expect(state.search.query.filters.favoriteOnly).toBe(true)
    expect(state.search.query.sort.key).toBe('size')
    expect(state.search.query.layout).toBe('flat')
    expect(state.search.schedule).toBe('immediate')

    const switched = viewerReducer(state, {
      type: 'projection_loaded',
      sessionId: project.sessionId,
      generation: project.generation,
      folders: [],
      workspace: { workspace: 'empty' },
      selectedFolderId: 'folder-9',
      selectedFolderPath: 'folder-9',
      showingAggregate: false,
    })
    expect(switched.search).toEqual(state.search)

    const withoutPng = viewerReducer(switched, {
      type: 'search_filter_chip_removed',
      chip: { kind: 'file_kind', value: 'png' },
    })
    expect(withoutPng.search.query.filters.kinds).toEqual(['jpeg'])
    const cleared = viewerReducer(withoutPng, { type: 'search_filters_cleared' })
    expect(cleared.search.query.filters).toEqual(emptySearchFilters)
    expect(viewerReducer(cleared, { type: 'project_closed' })).toEqual(initialViewerState)
  })

  it('ignores out-of-order search and progress results', () => {
    let state = viewerReducer(initialViewerState, { type: 'project_opened', project })
    state = viewerReducer(state, { type: 'search_requested', revision: 2 })
    const stale = viewerReducer(state, {
      type: 'search_loaded',
      sessionId: project.sessionId,
      generation: project.generation,
      page: page(1, 'old'),
    })
    expect(stale).toEqual(state)
    state = viewerReducer(state, {
      type: 'search_loaded',
      sessionId: project.sessionId,
      generation: project.generation,
      page: page(2, 'new'),
    })
    expect(state.search.page?.hits[0]?.name).toBe('new')

    const wrongProgress = viewerReducer(state, {
      type: 'index_progress_received',
      progress: {
        sessionId: 'another-session',
        generation: 7,
        imagesTotal: 2,
        imagesReady: 1,
        imagesFailed: 0,
        textTotal: 0,
        textReady: 0,
        textSkipped: 0,
        textFailed: 0,
        complete: false,
      },
    })
    expect(wrongProgress).toEqual(state)
  })

  it('applies marker responses everywhere without clearing multi-selection', () => {
    const marker = { reviewState: null, favorite: false }
    let state = viewerReducer(initialViewerState, { type: 'project_opened', project })
    state = viewerReducer(state, {
      type: 'projection_loaded',
      sessionId: project.sessionId,
      generation: project.generation,
      folders: [
        {
          entityId: 'folder-1',
          parentEntityId: null,
          relativePath: 'id-1',
          name: 'id-1',
          marker,
        },
      ],
      workspace: { workspace: 'empty' },
      selectedFolderId: null,
      selectedFolderPath: '',
      showingAggregate: false,
    })
    state = viewerReducer(state, {
      type: 'selection_changed',
      entityIds: ['folder-1', 'image-1'],
    })
    state = viewerReducer(state, {
      type: 'marker_changes_applied',
      sessionId: project.sessionId,
      generation: project.generation,
      changes: [
        {
          entityId: 'folder-1',
          relativePath: 'id-1',
          kind: 'directory',
          marker: { reviewState: 'keep', favorite: true },
        },
      ],
    })
    expect(state.folders[0]?.marker).toEqual({ reviewState: 'keep', favorite: true })
    expect(state.selectedEntityIds).toEqual(['folder-1', 'image-1'])
  })

  it('keeps operation recovery and close state scoped to the current session', () => {
    let state = viewerReducer(initialViewerState, {
      type: 'project_opened',
      project: {
        ...project,
        recoveryReport: { recovered: 2, needsUserReview: 1 },
      },
    })
    expect(state.recoveryReport).toEqual({ recovered: 2, needsUserReview: 1 })
    state = viewerReducer(state, {
      type: 'operation_started',
      sessionId: project.sessionId,
      generation: project.generation,
      batchId: 'batch-1',
      kind: 'rename',
    })
    const current = progress('session-1', 7, 'batch-1', 'running')
    expect(
      viewerReducer(state, {
        type: 'operation_progress_received',
        progress: { ...current, sessionId: 'old-session' },
      }),
    ).toEqual(state)
    state = viewerReducer(state, { type: 'operation_progress_received', progress: current })
    expect(state.operation.active?.lifecycle).toBe('running')
    state = viewerReducer(state, {
      type: 'operation_results_loaded',
      sessionId: project.sessionId,
      generation: project.generation,
      batchId: 'batch-1',
      page: {
        total: 1,
        offset: 0,
        items: [
          {
            entityId: 'image-1',
            relativePath: 'id/image.jpg',
            status: 'completed',
            code: 'renamed',
          },
        ],
      },
    })
    expect(state.operation.results?.items[0]?.code).toBe('renamed')
    state = viewerReducer(state, {
      type: 'close_blocked_received',
      event: {
        sessionId: project.sessionId,
        generation: 7,
        batchId: 'batch-1',
        target: 'project',
      },
    })
    expect(state.closeBlocked?.batchId).toBe('batch-1')
    state = viewerReducer(state, { type: 'project_close_requested' })
    expect(state.status).toBe('closing')
    state = viewerReducer(state, { type: 'project_close_stayed' })
    expect(state.status).toBe('active')
    expect(state.closeBlocked?.batchId).toBe('batch-1')
    expect(viewerReducer(state, { type: 'project_closed' })).toEqual(initialViewerState)
  })

  it('repairs only active entities confirmed missing from the prior projection', () => {
    let state = viewerReducer(initialViewerState, { type: 'project_opened', project })
    state = viewerReducer(state, {
      type: 'projection_loaded',
      sessionId: project.sessionId,
      generation: project.generation,
      folders: [],
      workspace: contentWorkspace(['a', 'b', 'c']),
      selectedFolderId: null,
      selectedFolderPath: '',
      showingAggregate: false,
    })
    state = viewerReducer(state, { type: 'selection_changed', entityIds: ['b', 'c'] })
    state = viewerReducer(state, { type: 'preview_context_changed', entityId: 'b' })
    state = viewerReducer(state, {
      type: 'compare_context_changed',
      entityIds: ['a', 'b', 'c'],
    })
    state = viewerReducer(state, {
      type: 'project_changed_received',
      change: projectChange('session-1', 7),
    })
    state = viewerReducer(state, {
      type: 'projection_loaded',
      sessionId: project.sessionId,
      generation: project.generation,
      folders: [],
      workspace: contentWorkspace(['a', 'c']),
      selectedFolderId: null,
      selectedFolderPath: '',
      showingAggregate: false,
    })
    expect(state.selectedEntityIds).toEqual(['c'])
    expect(state.previewEntityId).toBeNull()
    expect(state.compareEntityIds).toEqual(['a', 'c'])
    expect(state.contextRepair).toEqual({
      removedEntityIds: ['b'],
      suggestedEntityId: 'c',
      message: '部分正在查看的文件已在项目外发生变化。',
    })

    const stable = viewerReducer(state, {
      type: 'project_changed_received',
      change: projectChange('session-1', 7),
    })
    const refreshed = viewerReducer(stable, {
      type: 'projection_loaded',
      sessionId: project.sessionId,
      generation: project.generation,
      folders: [],
      workspace: contentWorkspace(['a', 'c', 'd']),
      selectedFolderId: null,
      selectedFolderPath: '',
      showingAggregate: false,
    })
    expect(refreshed.contextRepair).toBeNull()
  })

  it('ignores stale project change and close-blocked events', () => {
    const active = viewerReducer(initialViewerState, { type: 'project_opened', project })
    expect(
      viewerReducer(active, {
        type: 'project_changed_received',
        change: projectChange('old-session', 7),
      }),
    ).toEqual(active)
    expect(
      viewerReducer(active, {
        type: 'close_blocked_received',
        event: {
          sessionId: 'session-1',
          generation: 6,
          batchId: 'old',
          target: 'project',
        },
      }),
    ).toEqual(active)
  })
})

function progress(
  sessionId: string,
  generation: number,
  batchId: string,
  lifecycle: OperationProgressEvent['lifecycle'],
): OperationProgressEvent {
  return {
    sessionId,
    generation,
    batchId,
    lifecycle,
    requested: 1,
    completed: 0,
    failed: 0,
    skipped: 0,
    cancelled: 0,
    activeEntityId: 'image-1',
  }
}

function projectChange(sessionId: string, generation: number): ProjectChangedEvent {
  return {
    sessionId,
    generation,
    reason: 'external_change',
    added: 0,
    removed: 1,
    modified: 0,
    moved: 0,
    markerPathsMoved: 0,
    failed: 0,
  }
}

function contentWorkspace(ids: string[]) {
  return {
    workspace: 'content' as const,
    images: ids.map(file),
    textFiles: [],
  }
}

function file(entityId: string): BrowserFile {
  return {
    entityId,
    relativePath: `${entityId}.jpg`,
    name: `${entityId}.jpg`,
    kind: 'jpeg',
    size: 1,
    modifiedNs: '1',
    marker: { reviewState: null, favorite: false },
    imageMetadata: { width: 1, height: 1 },
    imageUrl: null,
  }
}

function page(revision: number, name: string): SearchPage {
  return {
    revision,
    total: 1,
    progress: {
      imagesTotal: 1,
      imagesReady: 1,
      imagesFailed: 0,
      textTotal: 0,
      textReady: 0,
      textSkipped: 0,
      textFailed: 0,
      complete: true,
    },
    hits: [
      {
        entityId: `entity-${revision}`,
        relativePath: `id/${name}.jpg`,
        name,
        kind: 'jpeg',
        size: 10,
        modifiedNs: '1',
        marker: { reviewState: null, favorite: false },
        imageMetadata: { width: 10, height: 10 },
        matchedField: 'filename',
        score: 1,
        groupRelativePath: 'id',
        matchRanges: [],
      },
    ],
  }
}
