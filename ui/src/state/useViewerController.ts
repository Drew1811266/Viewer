import { useCallback, useEffect, useReducer, useRef } from 'react'
import type { ViewerBridge } from '../api/viewer'
import { safeUserMessage } from '../api/viewer'
import type { ProjectSnapshot, ScanEvent } from '../api/types'
import { initialViewerState, viewerReducer } from './viewerReducer'

export function useViewerController(bridge: ViewerBridge) {
  const [state, dispatch] = useReducer(viewerReducer, initialViewerState)
  const stateRef = useRef(state)
  const projectionRequestRef = useRef(0)
  const reconcilingGenerationRef = useRef<number | null>(null)
  const desiredProjectionRef = useRef({
    selectedFolderId: null as string | null,
    selectedFolderPath: '',
    showingAggregate: false,
  })
  stateRef.current = state

  const refreshProjection = useCallback(
    async (
      project: ProjectSnapshot,
      selectedFolderId: string | null,
      selectedFolderPath: string,
      showingAggregate: boolean,
    ) => {
      desiredProjectionRef.current = {
        selectedFolderId,
        selectedFolderPath,
        showingAggregate,
      }
      const requestId = ++projectionRequestRef.current
      try {
        const [folders, workspace] = await Promise.all([
          bridge.folderTree(),
          bridge.queryFolder(selectedFolderId, showingAggregate),
        ])
        if (requestId !== projectionRequestRef.current) return
        dispatch({
          type: 'projection_loaded',
          sessionId: project.sessionId,
          generation: project.generation,
          folders,
          workspace,
          selectedFolderId,
          selectedFolderPath,
          showingAggregate,
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
    [bridge],
  )

  const openProject = useCallback(
    async (path: string) => {
      if (!path || !['empty', 'error'].includes(stateRef.current.status)) return
      dispatch({ type: 'project_open_requested' })
      try {
        const project = await bridge.openProject(path)
        dispatch({ type: 'project_opened', project })
        await refreshProjection(project, null, '', false)
      } catch (error) {
        dispatch({ type: 'project_open_failed', message: safeUserMessage(error) })
      }
    },
    [bridge, refreshProjection],
  )

  const closeProject = useCallback(async () => {
    if (stateRef.current.project === null || stateRef.current.status === 'closing') return
    dispatch({ type: 'project_close_requested' })
    projectionRequestRef.current += 1
    desiredProjectionRef.current = {
      selectedFolderId: null,
      selectedFolderPath: '',
      showingAggregate: false,
    }
    try {
      await bridge.closeProject()
      dispatch({ type: 'project_closed' })
    } catch (error) {
      dispatch({ type: 'project_close_failed', message: safeUserMessage(error) })
    }
  }, [bridge])

  const receiveScan = useCallback(
    (event: ScanEvent) => {
      const project = stateRef.current.project
      if (project === null || event.sessionId !== project.sessionId) return
      if (event.generation < project.generation) return
      if (event.generation === project.generation) {
        const desired = desiredProjectionRef.current
        dispatch({ type: 'scan_received', event })
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
    [bridge, refreshProjection],
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
        bridge.listenProjectDrops((paths) => {
          if (!['empty', 'error'].includes(stateRef.current.status)) return
          if (paths.length !== 1) {
            dispatch({ type: 'input_rejected', message: '一次只能导入一个项目文件夹。' })
            return
          }
          void openProject(paths[0])
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
  }, [bridge, openProject])

  const selectFolder = useCallback(
    async (entityId: string | null) => {
      const project = stateRef.current.project
      if (project === null || stateRef.current.status !== 'active') return
      const selected = entityId
        ? stateRef.current.folders.find((folder) => folder.entityId === entityId)
        : undefined
      await refreshProjection(project, entityId, selected?.relativePath ?? '', false)
    },
    [refreshProjection],
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
  }, [refreshProjection])

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
    [bridge],
  )

  return {
    state,
    openProject,
    closeProject,
    selectFolder,
    showAllDescendants,
    cancelTask,
  }
}
