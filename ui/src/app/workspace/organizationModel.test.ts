import { describe, expect, it } from 'vitest'
import type { BrowserFile, FolderTreeItem, VideoFile } from '../../api/types'
import type { ViewerState } from '../../state/viewerState'
import {
  buildOrganizationRadialModel,
  canMutateOrganizationSelection,
  finderDragFailureMessage,
  isOrganizationDropTargetValid,
  organizationOperationBusy,
} from './organizationModel'

const activeOperation: NonNullable<ViewerState['operation']['active']> = {
  sessionId: 'session-1',
  generation: 1,
  batchId: 'batch-1',
  lifecycle: 'completed',
  requested: 1,
  completed: 1,
  failed: 0,
  skipped: 0,
  cancelled: 0,
  activeEntityId: null,
}

function file(entityId: string, relativePath: string): BrowserFile {
  return {
    entityId,
    relativePath,
    name: relativePath.split('/').at(-1) ?? relativePath,
    kind: 'jpeg',
    size: 1,
    modifiedNs: '1',
    marker: { reviewState: null, favorite: false },
    imageMetadata: { width: 1, height: 1 },
    imageUrl: null,
    videoMetadata: null,
  }
}

function video(entityId: string, relativePath: string): VideoFile {
  return {
    ...file(entityId, relativePath),
    kind: 'video',
    videoMetadata: {
      durationUs: 1_000_000,
      displayWidth: 1_920,
      displayHeight: 1_080,
      rotationDegrees: 0,
      frameRateMillihertz: 30_000,
      videoCodec: 'h264',
      audioCodec: 'aac',
      probeStatus: 'ready',
      failureKind: null,
      coverUrl: null,
    },
  }
}

const folders: FolderTreeItem[] = [
  {
    entityId: 'folder-a',
    parentEntityId: null,
    relativePath: 'a',
    name: 'a',
    marker: { reviewState: null, favorite: false },
  },
  {
    entityId: 'folder-b',
    parentEntityId: null,
    relativePath: 'b',
    name: 'b',
    marker: { reviewState: null, favorite: false },
  },
]

describe('organizationModel', () => {
  it.each([
    ['submitting', { operationSubmitting: true }],
    ['non-active project', { projectStatus: 'opening' as const }],
    ['finishing', { operationFinishing: true }],
    ['pending', { operationPending: true }],
    ['queued operation', { activeOperation: { ...activeOperation, lifecycle: 'queued' as const } }],
    [
      'running operation',
      { activeOperation: { ...activeOperation, lifecycle: 'running' as const } },
    ],
    [
      'cancelling operation',
      { activeOperation: { ...activeOperation, lifecycle: 'cancelling' as const } },
    ],
  ])('reports busy while %s', (_label, override) => {
    expect(
      organizationOperationBusy({
        operationSubmitting: false,
        projectStatus: 'active',
        operationFinishing: false,
        operationPending: false,
        activeOperation,
        ...override,
      }),
    ).toBe(true)
  })

  it('reports idle only for an active project with no pending operation work', () => {
    expect(
      organizationOperationBusy({
        operationSubmitting: false,
        projectStatus: 'active',
        operationFinishing: false,
        operationPending: false,
        activeOperation,
      }),
    ).toBe(false)
  })

  it('allows mutation only for a non-empty writable idle selection', () => {
    expect(
      canMutateOrganizationSelection({
        selectionCount: 1,
        projectAccess: 'read_write',
        operationBusy: false,
      }),
    ).toBe(true)
    expect(
      canMutateOrganizationSelection({
        selectionCount: 0,
        projectAccess: 'read_write',
        operationBusy: false,
      }),
    ).toBe(false)
    expect(
      canMutateOrganizationSelection({
        selectionCount: 1,
        projectAccess: 'read_only',
        operationBusy: false,
      }),
    ).toBe(false)
    expect(
      canMutateOrganizationSelection({
        selectionCount: 1,
        projectAccess: 'read_write',
        operationBusy: true,
      }),
    ).toBe(false)
  })

  it.each(['finder_drag_selection_stale', 'stale_project_session'])(
    'maps stale Finder code %s to the existing refresh message',
    (code) => {
      expect(finderDragFailureMessage({ code })).toBe('部分文件已发生变化，请刷新后重试。')
    },
  )

  it('maps all other Finder failures to the existing retry message', () => {
    expect(finderDragFailureMessage(new Error('nope'))).toBe('无法拖到 Finder，请重新拖动。')
    expect(finderDragFailureMessage({ code: 'backend_unavailable' })).toBe(
      '无法拖到 Finder，请重新拖动。',
    )
  })

  it('preserves copy and same-folder move validation across every content file collection', () => {
    const workspace: ViewerState['workspace'] = {
      workspace: 'content',
      images: [file('image-1', 'a/front.jpg')],
      videos: [video('video-1', 'a/clip.mp4')],
      otherFiles: [file('text-1', 'b/notes.txt')],
    }
    const input = { workspace, folders, entityIds: ['image-1'], destinationId: 'folder-a' }

    expect(isOrganizationDropTargetValid({ ...input, mode: 'move' })).toBe(false)
    expect(isOrganizationDropTargetValid({ ...input, mode: 'copy' })).toBe(true)
    expect(
      isOrganizationDropTargetValid({
        ...input,
        entityIds: ['video-1'],
        mode: 'move',
      }),
    ).toBe(false)
    expect(
      isOrganizationDropTargetValid({
        ...input,
        entityIds: ['text-1'],
        destinationId: 'folder-b',
        mode: 'move',
      }),
    ).toBe(false)
    expect(
      isOrganizationDropTargetValid({ ...input, destinationId: 'folder-b', mode: 'move' }),
    ).toBe(true)
    expect(isOrganizationDropTargetValid({ ...input, entityIds: ['missing'], mode: 'copy' })).toBe(
      false,
    )
  })

  it('builds the existing radial availability from the selected files', () => {
    const model = buildOrganizationRadialModel({
      files: [file('image-1', 'a/front.jpg'), file('image-2', 'a/back.jpg')],
      projectAccess: 'read_write',
      operationBusy: false,
      compareContextAvailable: true,
    })

    expect(model.find((item) => item.id === 'compare')).toMatchObject({ disabled: false })
    expect(model.find((item) => item.id === 'organize')).toMatchObject({ disabled: false })
  })
})
