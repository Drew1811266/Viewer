import { useCallback, useEffect, useMemo, useState } from 'react'
import type { BrowserFile } from '../../api/types'
import type { TextPreviewFiles } from '../../components/TextPreview'
import { isVideoFile } from '../../fileKinds'
import type { ViewerController } from '../../state/useViewerController'
import type { ViewerState } from '../../state/viewerState'
import {
  getPreviewSessionInternals,
  type PreviewSession,
  usePreviewSession,
} from '../usePreviewSession'
import type { OpenPreviewIntent } from './intents'
import {
  accumulatePreviewRepair,
  activeTextPreviewFiles,
  EMPTY_PREVIEW_REPAIR,
  resolveActivePreviewFile,
  resolveActivePreviewFiles,
  unavailablePreviewEntityIds,
  viewingVideoNeighbors,
} from './viewingModel'

type PreviewCommands = Pick<ViewerController, 'setPreviewEntityId'>

export interface PreviewCoordinator {
  activePreview: PreviewSession | null
  activePreviewFile: BrowserFile | null
  activePreviewFiles: BrowserFile[]
  activeTextPreviewFiles: TextPreviewFiles | null
  folderOverviewIdentity: string
  unavailablePreviewEntityIds: ReadonlySet<string>
  contextRepairMessage: string | null
  dimensions: Record<string, { width: number; height: number } | undefined>
  openPreview(file: BrowserFile): void
  openVideoPreview(entityId: string): void
  openFilmstripPreview(file: BrowserFile, files: BrowserFile[]): void
  openIntentPreview(intent: OpenPreviewIntent): void
  navigatePreview(file: BrowserFile): void
  closePreview(): void
  recordDimensions(entityId: string, width: number, height: number): void
}

export function usePreviewCoordinator(
  state: ViewerState,
  projectSessionId: string,
  commands: PreviewCommands,
): PreviewCoordinator {
  const previewSession = usePreviewSession(projectSessionId)
  const {
    activePreview,
    dimensions,
    openPreview: openPreviewSession,
    openVideoPreview: openVideoPreviewSession,
    closePreview: closePreviewSession,
    recordDimensions,
  } = previewSession
  const { navigatePreview: navigatePreviewSession } = getPreviewSessionInternals(previewSession)
  const [previewRepair, setPreviewRepair] = useState(EMPTY_PREVIEW_REPAIR)
  const folderOverviewIdentity = useFolderOverviewIdentity(state)
  const currentVideoPreviewFiles = useMemo(
    () =>
      viewingVideoNeighbors(
        state.workspace,
        state.search.showResults,
        state.search.page?.hits ?? [],
      ),
    [state.search.page?.hits, state.search.showResults, state.workspace],
  )

  useEffect(() => setPreviewRepair(EMPTY_PREVIEW_REPAIR), [projectSessionId])

  const openVideoPreview = useCallback(
    (entityId: string) => {
      const file = currentVideoPreviewFiles.find((candidate) => candidate.entityId === entityId)
      if (file === undefined) return
      setPreviewRepair(EMPTY_PREVIEW_REPAIR)
      openVideoPreviewSession(file, currentVideoPreviewFiles)
      commands.setPreviewEntityId(file.entityId)
    },
    [commands.setPreviewEntityId, currentVideoPreviewFiles, openVideoPreviewSession],
  )
  const openPreview = useCallback(
    (file: BrowserFile) => {
      if (isVideoFile(file)) {
        openVideoPreview(file.entityId)
        return
      }
      setPreviewRepair(EMPTY_PREVIEW_REPAIR)
      openPreviewSession({ file, files: null, folderOverviewIdentity: null })
      commands.setPreviewEntityId(file.entityId)
    },
    [commands.setPreviewEntityId, openPreviewSession, openVideoPreview],
  )
  const openFilmstripPreview = useCallback(
    (file: BrowserFile, files: BrowserFile[]) => {
      setPreviewRepair(EMPTY_PREVIEW_REPAIR)
      openPreviewSession({ file, files, folderOverviewIdentity })
      commands.setPreviewEntityId(file.entityId)
    },
    [commands.setPreviewEntityId, folderOverviewIdentity, openPreviewSession],
  )
  const openIntentPreview = useCallback(
    (intent: OpenPreviewIntent) => {
      if (isVideoFile(intent.file)) {
        openVideoPreview(intent.file.entityId)
        return
      }
      setPreviewRepair(EMPTY_PREVIEW_REPAIR)
      openPreviewSession({
        file: intent.file,
        files: intent.files === null ? null : [...intent.files],
        folderOverviewIdentity: intent.folderOverviewIdentity,
      })
      commands.setPreviewEntityId(intent.file.entityId)
    },
    [commands.setPreviewEntityId, openPreviewSession, openVideoPreview],
  )
  const navigatePreview = useCallback(
    (file: BrowserFile) => {
      navigatePreviewSession(file)
      commands.setPreviewEntityId(file.entityId)
    },
    [commands.setPreviewEntityId, navigatePreviewSession],
  )
  const closePreview = useCallback(() => {
    setPreviewRepair(EMPTY_PREVIEW_REPAIR)
    closePreviewSession()
    commands.setPreviewEntityId(null)
  }, [closePreviewSession, commands.setPreviewEntityId])

  useEffect(() => {
    setPreviewRepair((current) =>
      accumulatePreviewRepair(current, activePreview, state.contextRepair),
    )
  }, [activePreview, state.contextRepair])

  const resolvedUnavailablePreviewEntityIds = useMemo(
    () => unavailablePreviewEntityIds(previewRepair, state.contextRepair),
    [previewRepair, state.contextRepair],
  )

  useEffect(() => {
    if (
      activePreview !== null &&
      activePreview.folderOverviewIdentity !== null &&
      activePreview.folderOverviewIdentity !== folderOverviewIdentity
    ) {
      setPreviewRepair(EMPTY_PREVIEW_REPAIR)
      closePreviewSession()
      commands.setPreviewEntityId(null)
    }
  }, [activePreview, closePreviewSession, commands.setPreviewEntityId, folderOverviewIdentity])

  const activePreviewFiles = resolveActivePreviewFiles(
    activePreview,
    state.workspace,
    currentVideoPreviewFiles,
  )

  return {
    activePreview,
    activePreviewFile: resolveActivePreviewFile(activePreview, activePreviewFiles),
    activePreviewFiles,
    activeTextPreviewFiles: activeTextPreviewFiles(activePreview),
    folderOverviewIdentity,
    unavailablePreviewEntityIds: resolvedUnavailablePreviewEntityIds,
    contextRepairMessage:
      state.contextRepair?.message ?? (activePreview === null ? null : previewRepair.message),
    dimensions,
    openPreview,
    openVideoPreview,
    openFilmstripPreview,
    openIntentPreview,
    navigatePreview,
    closePreview,
    recordDimensions,
  }
}

function useFolderOverviewIdentity(state: ViewerState): string {
  const [projectionState, setProjectionState] = useState(() => ({
    projection: state.workspace,
    sequence: 0,
  }))
  let sequence = projectionState.sequence
  if (projectionState.projection !== state.workspace) {
    sequence += 1
    setProjectionState({ projection: state.workspace, sequence })
  }
  return [
    state.project?.sessionId ?? 'no-session',
    state.project?.generation ?? 0,
    state.selectedFolderId ?? 'root',
    sequence,
  ].join(':')
}
