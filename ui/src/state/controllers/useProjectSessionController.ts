import type { MutableRefObject } from 'react'
import { useCallback, useEffect, useRef, useState } from 'react'
import type {
  CloseChoice,
  CloseRequestOutcome,
  CloseTarget,
  ProjectSnapshot,
  ScanEvent,
} from '../../api/types'
import { safeUserMessage } from '../../api/viewer'
import type { ControllerCore, RefreshProjection } from './types'

export interface ProjectSessionController {
  sessionEpoch: number
  refreshProjection: RefreshProjection
  openProject(path: string): Promise<'opened' | 'invalid-root' | 'failed'>
  closeProject(choice?: CloseChoice, target?: CloseTarget): Promise<CloseRequestOutcome | undefined>
  reselectProject(): Promise<CloseRequestOutcome | undefined>
  selectFolder(entityId: string | null): Promise<void>
  showAllDescendants(): Promise<void>
  cancelTask(taskId: string): Promise<void>
}

interface DesiredProjection {
  selectedFolderId: string | null
  selectedFolderPath: string
  showingAggregate: boolean
}

const NATIVE_INITIAL_PROJECTION_HOLD_MS = 600

async function holdNativeInitialProjectionForVisualStability() {
  if (import.meta.env.MODE === 'test' && !('__VIEWER_TEST_NATIVE_LOADING_HOLD__' in globalThis)) {
    return
  }
  await new Promise<void>((resolve) => {
    setTimeout(resolve, NATIVE_INITIAL_PROJECTION_HOLD_MS)
  })
}

const desiredProjectionByRefresh = new WeakMap<
  RefreshProjection,
  MutableRefObject<DesiredProjection>
>()

export function refreshDesiredProjection(
  refreshProjection: RefreshProjection,
  project: ProjectSnapshot,
  repairMissingFolder = false,
): Promise<void> {
  const desiredRef = desiredProjectionByRefresh.get(refreshProjection)
  if (desiredRef === undefined) {
    throw new Error('Refresh projection is not owned by a project session controller')
  }
  const desired = desiredRef.current
  return refreshProjection(
    project,
    desired.selectedFolderId,
    desired.selectedFolderPath,
    desired.showingAggregate,
    repairMissingFolder,
  )
}

export function useProjectSessionController(core: ControllerCore): ProjectSessionController {
  const { bridge, dispatch, sessionEpochRef, stateRef } = core
  const [sessionEpoch, setSessionEpoch] = useState(sessionEpochRef.current)
  const projectionRequestRef = useRef(0)
  const reconcilingGenerationRef = useRef<number | null>(null)
  const closeRequestPendingRef = useRef(false)
  const initialProjectionHoldRef = useRef(false)
  const desiredProjectionRef = useRef<DesiredProjection>({
    selectedFolderId: null,
    selectedFolderPath: '',
    showingAggregate: false,
  })

  const advanceSessionEpoch = useCallback(() => {
    const nextEpoch = sessionEpochRef.current + 1
    sessionEpochRef.current = nextEpoch
    setSessionEpoch(nextEpoch)
    return nextEpoch
  }, [sessionEpochRef])

  const refreshProjection = useCallback<RefreshProjection>(
    async (
      project,
      selectedFolderId,
      selectedFolderPath,
      showingAggregate,
      repairMissingFolder = false,
    ) => {
      desiredProjectionRef.current = {
        selectedFolderId,
        selectedFolderPath,
        showingAggregate,
      }
      const requestId = ++projectionRequestRef.current
      try {
        const foldersPromise = bridge.folderTree()
        const workspacePromise = repairMissingFolder
          ? null
          : bridge.queryFolder(selectedFolderId, showingAggregate)
        const folders = await foldersPromise
        const repairedFolderId =
          repairMissingFolder &&
          selectedFolderId !== null &&
          !folders.some((folder) => folder.entityId === selectedFolderId)
            ? null
            : selectedFolderId
        const repairedFolderPath = repairedFolderId === null ? '' : selectedFolderPath
        const repairedAggregate = repairedFolderId === null ? false : showingAggregate
        const workspace =
          workspacePromise === null
            ? await bridge.queryFolder(repairedFolderId, repairedAggregate)
            : await workspacePromise
        if (requestId !== projectionRequestRef.current) return
        desiredProjectionRef.current = {
          selectedFolderId: repairedFolderId,
          selectedFolderPath: repairedFolderPath,
          showingAggregate: repairedAggregate,
        }
        dispatch({
          type: 'projection_loaded',
          sessionId: project.sessionId,
          generation: project.generation,
          folders,
          workspace,
          selectedFolderId: repairedFolderId,
          selectedFolderPath: repairedFolderPath,
          showingAggregate: repairedAggregate,
        })
      } catch (error) {
        if (requestId !== projectionRequestRef.current) return
        dispatch({
          type: 'projection_failed',
          sessionId: project.sessionId,
          generation: project.generation,
          message: safeUserMessage(error),
        })
      }
    },
    [bridge, dispatch],
  )
  desiredProjectionByRefresh.set(refreshProjection, desiredProjectionRef)

  const openProject = useCallback(
    async (path: string) => {
      if (!path || !['empty', 'error'].includes(stateRef.current.status)) return 'failed' as const
      const requestEpoch = advanceSessionEpoch()
      dispatch({ type: 'project_open_requested' })
      try {
        const project = await bridge.openProject(path)
        if (requestEpoch !== sessionEpochRef.current) return 'failed' as const
        initialProjectionHoldRef.current = true
        dispatch({ type: 'project_opened', project })
        await holdNativeInitialProjectionForVisualStability()
        initialProjectionHoldRef.current = false
        if (requestEpoch !== sessionEpochRef.current) return 'failed' as const
        await refreshProjection(project, null, '', false)
        return 'opened' as const
      } catch (error) {
        if (requestEpoch !== sessionEpochRef.current) return 'failed' as const
        if (
          typeof error === 'object' &&
          error !== null &&
          'code' in error &&
          error.code === 'invalid_project_root'
        ) {
          dispatch({ type: 'project_closed' })
          return 'invalid-root' as const
        }
        dispatch({ type: 'project_open_failed', message: safeUserMessage(error) })
        return 'failed' as const
      }
    },
    [advanceSessionEpoch, bridge, dispatch, refreshProjection, sessionEpochRef, stateRef],
  )

  const resetProjectSessionRequests = useCallback(() => {
    initialProjectionHoldRef.current = false
    advanceSessionEpoch()
    projectionRequestRef.current += 1
    desiredProjectionRef.current = {
      selectedFolderId: null,
      selectedFolderPath: '',
      showingAggregate: false,
    }
  }, [advanceSessionEpoch])

  const requestProjectClose = useCallback(
    async (
      choice?: CloseChoice,
      closedMessage?: string,
      target: CloseTarget = 'project',
    ): Promise<CloseRequestOutcome | undefined> => {
      if (
        stateRef.current.project === null ||
        stateRef.current.status === 'closing' ||
        closeRequestPendingRef.current
      )
        return
      closeRequestPendingRef.current = true
      dispatch({ type: 'project_close_requested' })
      try {
        const outcome = await bridge.closeProject(choice, target)
        if (outcome === 'stayed') {
          dispatch({ type: 'project_close_stayed' })
          return outcome
        }
        resetProjectSessionRequests()
        dispatch({ type: 'project_closed', message: closedMessage })
        return outcome
      } catch (error) {
        if (isTerminalCloseCleanupFailure(error)) {
          resetProjectSessionRequests()
          dispatch({ type: 'project_closed', message: safeUserMessage(error) })
          return 'closed'
        }
        dispatch({ type: 'project_close_failed', message: safeUserMessage(error) })
        return undefined
      } finally {
        closeRequestPendingRef.current = false
      }
    },
    [bridge, dispatch, resetProjectSessionRequests, stateRef],
  )

  const closeProject = useCallback(
    (choice?: CloseChoice, target: CloseTarget = 'project') =>
      requestProjectClose(choice, undefined, target),
    [requestProjectClose],
  )

  const reselectProject = useCallback(
    () => requestProjectClose(undefined, '已关闭只读项目，请选择已授权的目录。', 'project'),
    [requestProjectClose],
  )

  const receiveScan = useCallback(
    (event: ScanEvent) => {
      const project = stateRef.current.project
      if (project === null || event.sessionId !== project.sessionId) return
      if (event.generation < project.generation) return
      if (event.generation === project.generation) {
        const desired = desiredProjectionRef.current
        dispatch({ type: 'scan_received', event })
        if (initialProjectionHoldRef.current) return
        void refreshProjection(
          project,
          desired.selectedFolderId,
          desired.selectedFolderPath,
          desired.showingAggregate,
        )
        return
      }
      if (reconcilingGenerationRef.current === event.generation) return
      reconcilingGenerationRef.current = event.generation
      void bridge
        .projectSnapshot()
        .then((snapshot) => {
          if (
            snapshot === null ||
            snapshot.sessionId !== event.sessionId ||
            snapshot.generation !== event.generation
          ) {
            return
          }
          dispatch({ type: 'project_reconciled', project: snapshot })
          dispatch({ type: 'scan_received', event })
          const desired = desiredProjectionRef.current
          void refreshProjection(
            snapshot,
            desired.selectedFolderId,
            desired.selectedFolderPath,
            desired.showingAggregate,
          )
        })
        .catch(() => undefined)
        .finally(() => {
          if (reconcilingGenerationRef.current === event.generation) {
            reconcilingGenerationRef.current = null
          }
        })
    },
    [bridge, dispatch, refreshProjection, stateRef],
  )

  useEffect(() => {
    let disposed = false
    let unlisten: (() => void) | undefined
    void Promise.resolve()
      .then(() => bridge.listenScan(receiveScan))
      .then((cleanup) => {
        if (disposed) cleanup()
        else unlisten = cleanup
      })
      .catch(() => undefined)
    return () => {
      disposed = true
      unlisten?.()
    }
  }, [bridge, receiveScan])

  useEffect(() => {
    let disposed = false
    let unlisten: (() => void) | undefined
    void Promise.resolve()
      .then(() =>
        bridge.listenIndexProgress((progress) => {
          dispatch({ type: 'index_progress_received', progress })
        }),
      )
      .then((cleanup) => {
        if (disposed) cleanup()
        else unlisten = cleanup
      })
      .catch(() => undefined)
    return () => {
      disposed = true
      unlisten?.()
    }
  }, [bridge, dispatch])

  useEffect(() => {
    let disposed = false
    let unlisten: (() => void) | undefined
    void Promise.resolve()
      .then(() =>
        bridge.listenProjectClosed(() => {
          resetProjectSessionRequests()
          reconcilingGenerationRef.current = null
          dispatch({ type: 'project_closed' })
        }),
      )
      .then((cleanup) => {
        if (disposed) cleanup()
        else unlisten = cleanup
      })
      .catch(() => undefined)
    return () => {
      disposed = true
      unlisten?.()
    }
  }, [bridge, dispatch, resetProjectSessionRequests])

  const selectFolder = useCallback(
    async (entityId: string | null) => {
      const project = stateRef.current.project
      if (project === null || stateRef.current.status !== 'active') return
      const selected = entityId
        ? stateRef.current.folders.find((folder) => folder.entityId === entityId)
        : undefined
      await refreshProjection(project, entityId, selected?.relativePath ?? '', false)
    },
    [refreshProjection, stateRef],
  )

  const showAllDescendants = useCallback(async () => {
    const current = stateRef.current
    if (current.project === null || current.status !== 'active') return
    const desired = desiredProjectionRef.current
    await refreshProjection(
      current.project,
      desired.selectedFolderId,
      desired.selectedFolderPath,
      true,
    )
  }, [refreshProjection, stateRef])

  const cancelTask = useCallback(
    async (taskId: string) => {
      try {
        if (await bridge.cancelTask(taskId)) {
          dispatch({ type: 'scan_cancelled', taskId })
        }
      } catch (error) {
        dispatch({ type: 'input_rejected', message: safeUserMessage(error) })
      }
    },
    [bridge, dispatch],
  )

  return {
    sessionEpoch,
    refreshProjection,
    openProject,
    closeProject,
    reselectProject,
    selectFolder,
    showAllDescendants,
    cancelTask,
  }
}

function isTerminalCloseCleanupFailure(error: unknown): boolean {
  return (
    typeof error === 'object' &&
    error !== null &&
    'code' in error &&
    error.code === 'project_closed_cache_cleanup_failed'
  )
}
