import { useCallback, useEffect, useMemo, useState } from 'react'
import type { BrowserFile, ImageRepresentationRequest, TextEncoding } from '../../api/types'
import type { TextPreviewFiles } from '../../components/TextPreview'
import { isVideoFile } from '../../fileKinds'
import {
  compareEntryAvailability,
  compareValidationMessage,
  validateCompareCandidates,
} from '../../state/comparePolicy'
import type { ViewerController } from '../../state/useViewerController'
import type { ViewerState } from '../../state/viewerState'
import { createProjectThumbnailCache, type ThumbnailLoader } from '../projectThumbnailCache'
import {
  getPreviewSessionInternals,
  type PreviewSession,
  usePreviewSession,
} from '../usePreviewSession'
import type { OpenPreviewIntent } from './intents'
import type { PreviewDataPort, VideoPlaybackPort } from './ports'
import {
  accumulatePreviewRepair,
  activeTextPreviewFiles,
  EMPTY_PREVIEW_REPAIR,
  resolveActivePreviewFile,
  resolveActivePreviewFiles,
  resolveCompareFiles,
  unavailablePreviewEntityIds,
  viewingVideoNeighbors,
} from './viewingModel'

export type ViewingCommands = Pick<ViewerController, 'setPreviewEntityId' | 'setCompareEntityIds'>

export interface ViewingOptions {
  state: ViewerState
  projectSessionId: string
  port: PreviewDataPort
  playbackPort: VideoPlaybackPort
  commands: ViewingCommands
}

export interface ViewingCoordinator {
  activePreview: PreviewSession | null
  activePreviewFile: BrowserFile | null
  activePreviewFiles: BrowserFile[]
  activeTextPreviewFiles: TextPreviewFiles | null
  folderOverviewIdentity: string
  unavailablePreviewEntityIds: ReadonlySet<string>
  contextRepairMessage: string | null
  dimensions: Record<string, { width: number; height: number } | undefined>
  requestThumbnail: ThumbnailLoader
  requestFolderImages(entityId: string): Promise<BrowserFile[]>
  requestPreviewImage(
    file: BrowserFile,
    representation: ImageRepresentationRequest,
    signal?: AbortSignal,
  ): ReturnType<PreviewDataPort['requestImage']>
  requestTextPreview(
    file: BrowserFile,
    encoding?: TextEncoding,
  ): ReturnType<PreviewDataPort['previewText']>
  requestVideoCover(entityId: string): Promise<string>
  openExternalLink: PreviewDataPort['openExternalLink']
  openPreview(file: BrowserFile): void
  openVideoPreview(entityId: string): void
  openFilmstripPreview(file: BrowserFile, files: BrowserFile[]): void
  openIntentPreview(intent: OpenPreviewIntent): void
  navigatePreview(file: BrowserFile): void
  closePreview(): void
  recordDimensions(entityId: string, width: number, height: number): void
  playbackPort: VideoPlaybackPort
  compareFiles: BrowserFile[]
  compareOpen: boolean
  compareStatus: string | null
  enterCompare(files: readonly BrowserFile[], operationBusy: boolean): void
  changeComparedEntities(entityIds: string[]): void
  setCompareStatus(message: string | null): void
}

export function useViewingCoordinator({
  state,
  projectSessionId,
  port,
  playbackPort,
  commands,
}: ViewingOptions): ViewingCoordinator {
  const { setPreviewEntityId, setCompareEntityIds } = commands
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
  const [compareStatus, setCompareStatus] = useState<string | null>(null)
  const [folderOverviewProjectionState, setFolderOverviewProjectionState] = useState(() => ({
    projection: state.workspace,
    sequence: 0,
  }))
  let folderOverviewSequence = folderOverviewProjectionState.sequence
  if (folderOverviewProjectionState.projection !== state.workspace) {
    folderOverviewSequence += 1
    setFolderOverviewProjectionState({
      projection: state.workspace,
      sequence: folderOverviewSequence,
    })
  }
  const folderOverviewIdentity = [
    state.project?.sessionId ?? 'no-session',
    state.project?.generation ?? 0,
    state.selectedFolderId ?? 'root',
    folderOverviewSequence,
  ].join(':')

  useEffect(() => {
    setPreviewRepair(EMPTY_PREVIEW_REPAIR)
    setCompareStatus(null)
  }, [projectSessionId])

  const loadThumbnail = useCallback<ThumbnailLoader>(
    (file, maxPixels, scaleMilli) =>
      port
        .requestImage({
          entityId: file.entityId,
          representation: { kind: 'thumbnail', maxPixels, scaleMilli },
        })
        .then((image) => image.url),
    [port],
  )
  const thumbnailCache = useMemo(
    () => createProjectThumbnailCache(projectSessionId, loadThumbnail),
    [loadThumbnail, projectSessionId],
  )
  useEffect(() => () => thumbnailCache.clear(), [thumbnailCache])

  const requestFolderImages = useCallback(
    async (entityId: string) => {
      const workspace = await port.queryFolder(entityId, false)
      return workspace.workspace === 'content' ? workspace.images : []
    },
    [port],
  )
  const requestPreviewImage = useCallback(
    (file: BrowserFile, representation: ImageRepresentationRequest, signal?: AbortSignal) =>
      port.requestImage({ entityId: file.entityId, representation }, signal),
    [port],
  )
  const requestTextPreview = useCallback(
    (file: BrowserFile, encoding?: TextEncoding) =>
      port.previewText({ entityId: file.entityId, encoding }),
    [port],
  )
  const requestVideoCover = useCallback(
    (entityId: string) => port.videoRequestCover(entityId),
    [port],
  )
  const currentVideoPreviewFiles = useMemo(
    () =>
      viewingVideoNeighbors(
        state.workspace,
        state.search.showResults,
        state.search.page?.hits ?? [],
      ),
    [state.search.page?.hits, state.search.showResults, state.workspace],
  )

  const openVideoPreview = useCallback(
    (entityId: string) => {
      const file = currentVideoPreviewFiles.find((candidate) => candidate.entityId === entityId)
      if (file === undefined) return
      setPreviewRepair(EMPTY_PREVIEW_REPAIR)
      openVideoPreviewSession(file, currentVideoPreviewFiles)
      setPreviewEntityId(file.entityId)
    },
    [currentVideoPreviewFiles, openVideoPreviewSession, setPreviewEntityId],
  )
  const openPreview = useCallback(
    (file: BrowserFile) => {
      if (isVideoFile(file)) {
        openVideoPreview(file.entityId)
        return
      }
      setPreviewRepair(EMPTY_PREVIEW_REPAIR)
      openPreviewSession({ file, files: null, folderOverviewIdentity: null })
      setPreviewEntityId(file.entityId)
    },
    [openPreviewSession, openVideoPreview, setPreviewEntityId],
  )
  const openFilmstripPreview = useCallback(
    (file: BrowserFile, files: BrowserFile[]) => {
      setPreviewRepair(EMPTY_PREVIEW_REPAIR)
      openPreviewSession({ file, files, folderOverviewIdentity })
      setPreviewEntityId(file.entityId)
    },
    [folderOverviewIdentity, openPreviewSession, setPreviewEntityId],
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
      setPreviewEntityId(intent.file.entityId)
    },
    [openPreviewSession, openVideoPreview, setPreviewEntityId],
  )
  const navigatePreview = useCallback(
    (file: BrowserFile) => {
      navigatePreviewSession(file)
      setPreviewEntityId(file.entityId)
    },
    [navigatePreviewSession, setPreviewEntityId],
  )
  const closePreview = useCallback(() => {
    setPreviewRepair(EMPTY_PREVIEW_REPAIR)
    closePreviewSession()
    setPreviewEntityId(null)
  }, [closePreviewSession, setPreviewEntityId])

  const compareFiles = useMemo(
    () => resolveCompareFiles(state.workspace, state.compareEntityIds),
    [state.compareEntityIds, state.workspace],
  )
  const compareOpen = state.compareEntityIds.length >= 2 && compareFiles.length >= 2
  const enterCompare = useCallback(
    (files: readonly BrowserFile[], operationBusy: boolean) => {
      const availability = compareEntryAvailability({
        workspace: state.workspace,
        searchResultsOpen: state.search.showResults,
        operationBusy,
      })
      if (availability !== 'available') {
        setCompareStatus(
          availability === 'busy'
            ? '请等待当前文件操作完成后再开始对比。'
            : '请先返回文件夹内容，再选择图片进行对比。',
        )
        return
      }
      const validation = validateCompareCandidates(files)
      if (!validation.ok) {
        setCompareStatus(compareValidationMessage(validation.reason))
        return
      }
      setCompareStatus(null)
      closePreview()
      setCompareEntityIds(files.map((file) => file.entityId))
    },
    [closePreview, setCompareEntityIds, state.search.showResults, state.workspace],
  )
  const changeComparedEntities = useCallback(
    (entityIds: string[]) => {
      setCompareStatus(null)
      if (entityIds.length >= 2) {
        setCompareEntityIds(entityIds)
        return
      }
      setCompareEntityIds([])
      if (entityIds.length !== 1 || state.workspace?.workspace !== 'content') return
      const survivor = state.workspace.images.find((file) => file.entityId === entityIds[0])
      if (survivor !== undefined) openPreview(survivor)
    },
    [openPreview, setCompareEntityIds, state.workspace],
  )

  useEffect(() => {
    if (state.compareEntityIds.length === 0) return
    const liveIds = compareFiles.map((file) => file.entityId)
    if (liveIds.length !== state.compareEntityIds.length || liveIds.length < 2) {
      changeComparedEntities(liveIds)
    }
  }, [changeComparedEntities, compareFiles, state.compareEntityIds])

  useEffect(() => {
    if (compareOpen && state.search.showResults) changeComparedEntities([])
  }, [changeComparedEntities, compareOpen, state.search.showResults])

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
      setPreviewEntityId(null)
    }
  }, [activePreview, closePreviewSession, folderOverviewIdentity, setPreviewEntityId])

  const activePreviewFiles = resolveActivePreviewFiles(
    activePreview,
    state.workspace,
    currentVideoPreviewFiles,
  )
  const activePreviewFile = resolveActivePreviewFile(activePreview, activePreviewFiles)

  return {
    activePreview,
    activePreviewFile,
    activePreviewFiles,
    activeTextPreviewFiles: activeTextPreviewFiles(activePreview),
    folderOverviewIdentity,
    unavailablePreviewEntityIds: resolvedUnavailablePreviewEntityIds,
    contextRepairMessage:
      state.contextRepair?.message ?? (activePreview === null ? null : previewRepair.message),
    dimensions,
    requestThumbnail: thumbnailCache.request,
    requestFolderImages,
    requestPreviewImage,
    requestTextPreview,
    requestVideoCover,
    openExternalLink: port.openExternalLink,
    openPreview,
    openVideoPreview,
    openFilmstripPreview,
    openIntentPreview,
    navigatePreview,
    closePreview,
    recordDimensions,
    playbackPort,
    compareFiles,
    compareOpen,
    compareStatus,
    enterCompare,
    changeComparedEntities,
    setCompareStatus,
  }
}
