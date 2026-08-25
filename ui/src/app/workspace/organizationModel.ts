import type { ProjectAccess } from '../../api/types'
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
