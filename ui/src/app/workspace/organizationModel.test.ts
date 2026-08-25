import { describe, expect, it } from 'vitest'
import type { ViewerState } from '../../state/viewerState'
import {
  canMutateOrganizationSelection,
  finderDragFailureMessage,
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
})
