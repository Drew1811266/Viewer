import { describe, expect, it } from 'vitest'
import type { ProjectSnapshot, ScanEvent, SearchPage } from '../api/types'
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
})

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
