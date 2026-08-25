import { useEffect } from 'react'
import type { BrowserFile } from '../../api/types'
import { defined } from '../../defined'
import { organizationShortcutIsOwned } from '../../state/organizationShortcutOwnership'
import useReviewShortcuts from '../../state/useReviewShortcuts'
import type { ViewerState } from '../../state/viewerState'
import type { OperationDialog } from '../useOperationDialogs'
import type { WorkspaceIntentSink } from './intents'
import { isToggleInfoShortcut, organizationShortcutAction } from './organizationShortcutModel'
import type { OrganizationCommands } from './organizationTypes'

export function useOrganizationShortcuts({
  state,
  selectedFiles,
  canMutateSelection,
  operationBusy,
  operationDialog,
  resultsBatchId,
  activePreviewOpen,
  compareOpen,
  infoOpen,
  emitIntent,
  openTrashDialog,
  commands,
}: {
  state: ViewerState
  selectedFiles: BrowserFile[]
  canMutateSelection: boolean
  operationBusy: boolean
  operationDialog: OperationDialog | null
  resultsBatchId: string | null
  activePreviewOpen: boolean
  compareOpen: boolean
  infoOpen: boolean
  emitIntent: WorkspaceIntentSink
  openTrashDialog(): void
  commands: Pick<OrganizationCommands, 'setReviewState' | 'toggleFavorite' | 'undoLastOperation'>
}) {
  useEffect(() => {
    function toggleInfo(event: KeyboardEvent) {
      const blocked =
        operationDialog !== null ||
        activePreviewOpen ||
        compareOpen ||
        resultsBatchId !== null ||
        operationBusy ||
        state.closeBlocked !== null
      if (!isToggleInfoShortcut(event) || organizationShortcutIsOwned(event, blocked)) return
      event.preventDefault()
      emitIntent({ kind: 'open-info' })
    }
    window.addEventListener('keydown', toggleInfo)
    return () => window.removeEventListener('keydown', toggleInfo)
  }, [
    activePreviewOpen,
    compareOpen,
    emitIntent,
    operationBusy,
    operationDialog,
    resultsBatchId,
    state.closeBlocked,
  ])

  useEffect(() => {
    function handleOrganizationShortcut(event: KeyboardEvent) {
      const blocked =
        operationDialog !== null ||
        activePreviewOpen ||
        compareOpen ||
        infoOpen ||
        resultsBatchId !== null ||
        operationBusy ||
        state.status !== 'active' ||
        state.closeBlocked !== null
      if (organizationShortcutIsOwned(event, blocked)) return
      const action = organizationShortcutAction(
        event,
        selectedFiles.length,
        canMutateSelection,
        operationBusy,
      )
      if (action === null) return
      event.preventDefault()
      if (action === 'undo') void commands.undoLastOperation()
      else if (action === 'preview') {
        emitIntent({
          kind: 'open-preview',
          file: defined(selectedFiles[0], 'Missing selected preview file'),
          files: null,
          folderOverviewIdentity: null,
        })
      } else if (action === 'compare') emitIntent({ kind: 'enter-compare', files: selectedFiles })
      else if (action === 'rename') emitIntent({ kind: 'start-rename', files: selectedFiles })
      else openTrashDialog()
    }
    window.addEventListener('keydown', handleOrganizationShortcut)
    return () => window.removeEventListener('keydown', handleOrganizationShortcut)
  }, [
    activePreviewOpen,
    canMutateSelection,
    commands.undoLastOperation,
    compareOpen,
    emitIntent,
    infoOpen,
    openTrashDialog,
    operationBusy,
    operationDialog,
    resultsBatchId,
    selectedFiles,
    state.closeBlocked,
    state.status,
  ])

  useReviewShortcuts({
    disabled:
      state.project?.access === 'read_only' ||
      state.selectedEntityIds.length === 0 ||
      operationBusy ||
      state.status !== 'active' ||
      operationDialog !== null ||
      activePreviewOpen ||
      compareOpen ||
      infoOpen ||
      resultsBatchId !== null ||
      state.closeBlocked !== null,
    onSetReview: (reviewState) => void commands.setReviewState(reviewState),
    onToggleFavorite: () => void commands.toggleFavorite(),
  })
}
