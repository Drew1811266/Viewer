import { describe, expect, it } from 'vitest'
import type { ProjectSnapshot, ScanEvent } from '../api/types'
import { initialViewerState, viewerReducer } from './viewerReducer'

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
})
