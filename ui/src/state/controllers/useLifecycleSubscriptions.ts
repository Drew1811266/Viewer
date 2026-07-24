import { useEffect } from 'react'
import type { OperationProgressEvent } from '../../api/types'
import type { ControllerCore, RefreshProjection } from './types'
import { refreshDesiredProjection } from './useProjectSessionController'

export function useLifecycleSubscriptions(
  core: ControllerCore,
  handlers: {
    receiveOperationProgress(progress: OperationProgressEvent): void
    refreshProjection: RefreshProjection
  },
): void {
  const { bridge, dispatch, stateRef } = core
  const { receiveOperationProgress, refreshProjection } = handlers

  useEffect(() => {
    let disposed = false
    let unlisten: (() => void) | undefined
    void Promise.resolve()
      .then(() => bridge.listenOperationProgress(receiveOperationProgress))
      .then((cleanup) => {
        if (disposed) cleanup()
        else unlisten = cleanup
      })
      .catch(() => undefined)
    return () => {
      disposed = true
      unlisten?.()
    }
  }, [bridge, receiveOperationProgress])

  useEffect(() => {
    let disposed = false
    let unlisten: (() => void) | undefined
    void Promise.resolve()
      .then(() =>
        bridge.listenProjectChanged((change) => {
          const project = stateRef.current.project
          if (
            project === null ||
            project.sessionId !== change.sessionId ||
            project.generation !== change.generation
          ) {
            return
          }
          if (change.reason === 'expected_viewer_change') return
          dispatch({ type: 'project_changed_received', change })
          void refreshDesiredProjection(refreshProjection, project, true)
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
  }, [bridge, dispatch, refreshProjection, stateRef])

  useEffect(() => {
    let disposed = false
    let unlisten: (() => void) | undefined
    void Promise.resolve()
      .then(() =>
        bridge.listenCloseBlocked((event) => {
          dispatch({ type: 'close_blocked_received', event })
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
}
