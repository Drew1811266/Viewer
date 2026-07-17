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
  stateRef.current = state

  const refreshProjection = useCallback(
    async (project: ProjectSnapshot) => {
      const requestId = ++projectionRequestRef.current
      try {
        const [folders, workspace] = await Promise.all([
          bridge.folderTree(),
          bridge.queryFolder(null),
        ])
        if (requestId !== projectionRequestRef.current) return
        dispatch({
          type: 'projection_loaded',
          sessionId: project.sessionId,
          generation: project.generation,
          folders,
          workspace,
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
        await refreshProjection(project)
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
        dispatch({ type: 'scan_received', event })
        void refreshProjection(project)
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
          void refreshProjection(snapshot)
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

  return { state, openProject, closeProject }
}
