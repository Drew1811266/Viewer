import type { ViewerState } from '../../state/viewerState'
import type { WorkspaceIntentSink } from './intents'
import { organizationProjectIdentity, organizationWorkspaceIdentity } from './organizationModel'
import type { OrganizationCommands } from './organizationTypes'
import { type OrganizationDrag, useOrganizationDrag } from './useOrganizationDrag'
import { type OrganizationOperations, useOrganizationOperations } from './useOrganizationOperations'
import { type OrganizationRadial, useOrganizationRadial } from './useOrganizationRadial'
import { type OrganizationSelection, useOrganizationSelection } from './useOrganizationSelection'
import { useOrganizationShortcuts } from './useOrganizationShortcuts'

export type { OrganizationCommands, OrganizationFileCommandKind } from './organizationTypes'

export interface OrganizationOptions {
  state: ViewerState
  commands: OrganizationCommands
  emitIntent: WorkspaceIntentSink
  compareOpen: boolean
  activePreviewOpen: boolean
  infoOpen: boolean
}

export interface OrganizationCoordinator
  extends OrganizationSelection,
    Omit<OrganizationOperations, 'dropFiles'>,
    OrganizationDrag,
    OrganizationRadial {}

export function useOrganizationCoordinator({
  state,
  commands,
  emitIntent,
  compareOpen,
  activePreviewOpen,
  infoOpen,
}: OrganizationOptions): OrganizationCoordinator {
  const projectSessionId = state.project?.sessionId ?? 'no-session'
  const projectIdentity = organizationProjectIdentity(state)
  const workspaceIdentity = organizationWorkspaceIdentity(state)
  const selection = useOrganizationSelection(projectSessionId, commands.setSelectedEntityIds)
  const operations = useOrganizationOperations({
    state,
    selectedFiles: selection.selectedFiles,
    commands,
  })
  const drag = useOrganizationDrag({
    state,
    projectSessionId,
    workspaceIdentity,
    compareOpen,
    operationBusy: operations.operationBusy,
    beginFinderDrag: commands.beginFinderDrag,
    dropFiles: operations.dropFiles,
  })
  const radial = useOrganizationRadial({
    state,
    projectIdentity,
    workspaceIdentity,
    compareOpen,
    activePreviewOpen,
    infoOpen,
    emitIntent,
    operations,
    commands,
  })

  useOrganizationShortcuts({
    state,
    selectedFiles: selection.selectedFiles,
    canMutateSelection: operations.canMutateSelection,
    operationBusy: operations.operationBusy,
    operationDialog: operations.operationDialog,
    resultsBatchId: operations.resultsBatchId,
    activePreviewOpen,
    compareOpen,
    infoOpen,
    emitIntent,
    openTrashDialog: operations.openTrashDialog,
    commands,
  })

  return { ...selection, ...operations, ...drag, ...radial }
}
