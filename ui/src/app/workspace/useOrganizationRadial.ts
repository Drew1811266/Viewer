import { useCallback, useMemo } from 'react'
import type { RadialLeafAction } from '../../components/radialMenuModel'
import { defined } from '../../defined'
import { compareEntryAvailability } from '../../state/comparePolicy'
import { validatePreviewSelection } from '../../state/previewPolicy'
import type { ViewerState } from '../../state/viewerState'
import { useRadialMenuContextToken, useRadialMenuSession } from '../useRadialMenuSession'
import type { WorkspaceIntentSink } from './intents'
import { buildOrganizationRadialModel } from './organizationModel'
import type { OrganizationCommands } from './organizationTypes'
import type { OrganizationOperations } from './useOrganizationOperations'

const ACTIVE_PREVIEW_CONTEXT = {}

export interface OrganizationRadial {
  radialMenu: ReturnType<typeof useRadialMenuSession>['radialMenu']
  activeRadialMenu: ReturnType<typeof useRadialMenuSession>['activeRadialMenu']
  radialModel: ReturnType<typeof buildOrganizationRadialModel>
  beginRadialSession: ReturnType<typeof useRadialMenuSession>['beginRadialSession']
  finishRadialSession: ReturnType<typeof useRadialMenuSession>['finishRadialSession']
  runRadialAction(action: RadialLeafAction): void
}

export function useOrganizationRadial({
  state,
  projectIdentity,
  workspaceIdentity,
  compareOpen,
  activePreviewOpen,
  infoOpen,
  emitIntent,
  operations,
  commands,
}: {
  state: ViewerState
  projectIdentity: string
  workspaceIdentity: string
  compareOpen: boolean
  activePreviewOpen: boolean
  infoOpen: boolean
  emitIntent: WorkspaceIntentSink
  operations: OrganizationOperations
  commands: Pick<OrganizationCommands, 'setReviewState' | 'toggleFavorite'>
}): OrganizationRadial {
  const radialContextKey = useRadialMenuContextToken({
    activePreview: activePreviewOpen ? ACTIVE_PREVIEW_CONTEXT : null,
    compareOpen,
    infoOpen,
    operationDialog: operations.operationDialog,
    organizationWorkspaceIdentity: workspaceIdentity,
    resultsBatchId: operations.resultsBatchId,
    closeBlocked: state.closeBlocked,
    contextRepair: state.contextRepair,
  })
  const { radialMenu, activeRadialMenu, beginRadialSession, finishRadialSession } =
    useRadialMenuSession({
      projectIdentity,
      projectStatus: state.status,
      contextKey: radialContextKey,
    })
  const compareContextAvailable =
    compareEntryAvailability({
      workspace: state.workspace,
      searchResultsOpen: state.search.showResults,
      operationBusy: operations.operationBusy,
    }) === 'available'
  const radialModel = useMemo(
    () =>
      buildOrganizationRadialModel({
        files: activeRadialMenu?.files ?? [],
        projectAccess: state.project?.access ?? null,
        operationBusy: operations.operationBusy,
        compareContextAvailable,
      }),
    [activeRadialMenu, compareContextAvailable, operations.operationBusy, state.project?.access],
  )
  const runRadialAction = useRadialActionRunner({
    state,
    projectIdentity,
    radialMenu,
    finishRadialSession,
    emitIntent,
    operations,
    commands,
  })

  return {
    radialMenu,
    activeRadialMenu,
    radialModel,
    beginRadialSession,
    finishRadialSession,
    runRadialAction,
  }
}

function useRadialActionRunner({
  state,
  projectIdentity,
  radialMenu,
  finishRadialSession,
  emitIntent,
  operations,
  commands,
}: {
  state: ViewerState
  projectIdentity: string
  radialMenu: OrganizationRadial['radialMenu']
  finishRadialSession: OrganizationRadial['finishRadialSession']
  emitIntent: WorkspaceIntentSink
  operations: Pick<OrganizationOperations, 'openDestinationDialog' | 'openTrashDialog'>
  commands: Pick<OrganizationCommands, 'setReviewState' | 'toggleFavorite'>
}): OrganizationRadial['runRadialAction'] {
  return useCallback(
    (action: RadialLeafAction) => {
      const files =
        state.status === 'active' && radialMenu?.projectIdentity === projectIdentity
          ? radialMenu.files
          : []
      if (files.length === 0) {
        finishRadialSession()
        return
      }
      finishRadialSession()
      if (action === 'preview') {
        emitPreviewIntent(files, emitIntent)
        return
      }
      const ids = files.map((file) => file.entityId)
      if (runReviewAction(action, ids, commands)) return
      runWorkspaceAction(action, files, emitIntent, operations)
    },
    [
      commands.setReviewState,
      commands.toggleFavorite,
      emitIntent,
      finishRadialSession,
      operations.openDestinationDialog,
      operations.openTrashDialog,
      projectIdentity,
      radialMenu,
      state.status,
    ],
  )
}

function emitPreviewIntent(
  files: NonNullable<OrganizationRadial['radialMenu']>['files'],
  emitIntent: WorkspaceIntentSink,
) {
  const validation = validatePreviewSelection(files)
  if (!validation.ok) return
  if (validation.mode === 'single') {
    emitIntent({
      kind: 'open-preview',
      file: defined(files[0], 'Single preview requires one file'),
      files: null,
      folderOverviewIdentity: null,
    })
    return
  }
  const first = defined(files[0], 'Split text preview requires a left file')
  const second = defined(files[1], 'Split text preview requires a right file')
  emitIntent({
    kind: 'open-preview',
    file: first,
    files: [first, second],
    folderOverviewIdentity: null,
  })
}

function runReviewAction(
  action: RadialLeafAction,
  ids: string[],
  commands: Pick<OrganizationCommands, 'setReviewState' | 'toggleFavorite'>,
): boolean {
  if (action === 'mark.keep') void commands.setReviewState('keep', ids)
  else if (action === 'mark.pending') void commands.setReviewState('pending', ids)
  else if (action === 'mark.reject') void commands.setReviewState('reject', ids)
  else if (action === 'mark.clear') void commands.setReviewState(null, ids)
  else if (action === 'mark.favorite') void commands.toggleFavorite(ids)
  else return false
  return true
}

function runWorkspaceAction(
  action: RadialLeafAction,
  files: NonNullable<OrganizationRadial['radialMenu']>['files'],
  emitIntent: WorkspaceIntentSink,
  operations: Pick<OrganizationOperations, 'openDestinationDialog' | 'openTrashDialog'>,
) {
  if (action === 'organize.rename') emitIntent({ kind: 'start-rename', files })
  else if (action === 'organize.copy') operations.openDestinationDialog('copy', files)
  else if (action === 'organize.move') operations.openDestinationDialog('move', files)
  else if (action === 'trash') operations.openTrashDialog(files)
  else if (action === 'compare') emitIntent({ kind: 'enter-compare', files })
  else if (action === 'info') emitIntent({ kind: 'open-info' })
}
