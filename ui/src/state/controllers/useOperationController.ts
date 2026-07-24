import type { MutableRefObject } from 'react'
import { useCallback, useEffect, useRef } from 'react'
import type {
  ConflictResolution,
  FileCommandItem,
  FileCommandKind,
  FileCommandPreflight,
  OperationProgressEvent,
  OperationResultPage,
  OperationStarted,
  RenamePreview,
  RenameRules,
  UndoReceipt,
} from '../../api/types'
import { safeUserMessage } from '../../api/viewer'
import type { ControllerCore, RefreshProjection } from './types'
import { refreshDesiredProjection } from './useProjectSessionController'

export interface OperationController {
  previewRename(entityIds: string[], rules: RenameRules): Promise<RenamePreview | null>
  preflightFileCommand(
    kind: FileCommandKind,
    items: FileCommandItem[],
  ): Promise<FileCommandPreflight | null>
  executeFileCommand(
    kind: FileCommandKind,
    items: FileCommandItem[],
    conflicts?: ConflictResolution[],
  ): Promise<OperationStarted | null>
  cancelOperation(): Promise<boolean>
  loadOperationResults(offset: number): Promise<OperationResultPage | null>
  undoLastOperation(): Promise<UndoReceipt | null>
  receiveOperationProgress(progress: OperationProgressEvent): void
}

interface OperationControllerInternals {
  operationRequestPendingRef: MutableRefObject<boolean>
  undoRequestPendingRef: MutableRefObject<boolean>
  activeBatchRef: MutableRefObject<string | null>
  resetSessionRequests(sessionEpoch: number): void
}

const internalsByController = new WeakMap<OperationController, OperationControllerInternals>()

/** @internal Connects operation-owned admission state to the selection/marker controller. */
export function getOperationControllerInternals(
  controller: OperationController,
): OperationControllerInternals {
  const internals = internalsByController.get(controller)
  if (internals === undefined) throw new Error('Operation controller internals are unavailable')
  return internals
}

export function useOperationController(
  core: ControllerCore,
  sessionEpoch: number,
  refreshProjection: RefreshProjection,
): OperationController {
  const { bridge, dispatch, sessionEpochRef, stateRef } = core
  const activeBatchRef = useRef<string | null>(null)
  const operationRequestPendingRef = useRef(false)
  const undoRequestPendingRef = useRef(false)
  const operationResultsRequestRef = useRef(0)
  const completedBatchesRef = useRef(new Set<string>())
  const clearedSessionEpochRef = useRef(sessionEpoch)

  const resetSessionRequests = useCallback((nextSessionEpoch: number) => {
    if (clearedSessionEpochRef.current === nextSessionEpoch) return
    clearedSessionEpochRef.current = nextSessionEpoch
    activeBatchRef.current = null
    operationRequestPendingRef.current = false
    undoRequestPendingRef.current = false
    operationResultsRequestRef.current += 1
    completedBatchesRef.current.clear()
  }, [])

  useEffect(() => {
    resetSessionRequests(sessionEpoch)
  }, [resetSessionRequests, sessionEpoch])

  const previewRename = useCallback(
    async (entityIds: string[], rules: RenameRules) => {
      const current = stateRef.current
      if (
        current.status !== 'active' ||
        current.project === null ||
        current.project.access === 'read_only'
      )
        return null
      const project = current.project
      const requestSessionEpoch = sessionEpochRef.current
      try {
        const preview = await bridge.previewRename({
          sessionId: project.sessionId,
          generation: project.generation,
          entityIds,
          rules,
        })
        const latest = stateRef.current
        if (
          requestSessionEpoch !== sessionEpochRef.current ||
          latest.status !== 'active' ||
          latest.project?.sessionId !== project.sessionId ||
          latest.project.generation !== project.generation
        ) {
          return null
        }
        return preview
      } catch (error) {
        if (requestSessionEpoch !== sessionEpochRef.current) return null
        dispatch({ type: 'input_rejected', message: safeUserMessage(error) })
        return null
      }
    },
    [bridge, dispatch, sessionEpochRef, stateRef],
  )

  const preflightFileCommand = useCallback(
    async (kind: FileCommandKind, items: FileCommandItem[]) => {
      const current = stateRef.current
      if (
        current.status !== 'active' ||
        current.project === null ||
        current.project.access === 'read_only'
      )
        return null
      const project = current.project
      const requestSessionEpoch = sessionEpochRef.current
      try {
        const preflight = await bridge.preflightFileCommand({
          sessionId: project.sessionId,
          generation: project.generation,
          kind,
          items,
        })
        const latest = stateRef.current
        if (
          requestSessionEpoch !== sessionEpochRef.current ||
          latest.status !== 'active' ||
          latest.project?.sessionId !== project.sessionId ||
          latest.project.generation !== project.generation
        ) {
          return null
        }
        return preflight
      } catch (error) {
        if (requestSessionEpoch !== sessionEpochRef.current) return null
        dispatch({ type: 'input_rejected', message: safeUserMessage(error) })
        return null
      }
    },
    [bridge, dispatch, sessionEpochRef, stateRef],
  )

  const finishOperation = useCallback(
    async (progress: OperationProgressEvent) => {
      const project = stateRef.current.project
      if (
        project === null ||
        project.sessionId !== progress.sessionId ||
        project.generation !== progress.generation ||
        activeBatchRef.current !== progress.batchId ||
        completedBatchesRef.current.has(progress.batchId)
      ) {
        return
      }
      const requestSessionEpoch = sessionEpochRef.current
      const projectIsCurrent = () => {
        const latestProject = stateRef.current.project
        return (
          requestSessionEpoch === sessionEpochRef.current &&
          latestProject?.sessionId === project.sessionId &&
          latestProject.generation === project.generation
        )
      }
      completedBatchesRef.current.add(progress.batchId)
      const loadResults = bridge
        .operationResults({
          sessionId: project.sessionId,
          generation: project.generation,
          batchId: progress.batchId,
          offset: 0,
          limit: 200,
        })
        .then((page) => {
          if (!projectIsCurrent()) return
          dispatch({
            type: 'operation_results_loaded',
            sessionId: project.sessionId,
            generation: project.generation,
            batchId: progress.batchId,
            page,
          })
        })
        .catch((error: unknown) => {
          if (!projectIsCurrent()) return
          dispatch({
            type: 'operation_failed',
            sessionId: project.sessionId,
            generation: project.generation,
            message: safeUserMessage(error),
          })
        })
      const refreshAfterTerminalBatch = (async () => {
        await refreshDesiredProjection(refreshProjection, project, true)
        if (!projectIsCurrent()) return
        dispatch({ type: 'search_refresh_requested' })
      })()
      try {
        await Promise.all([loadResults, refreshAfterTerminalBatch])
      } finally {
        const latestProject = stateRef.current.project
        if (
          requestSessionEpoch === sessionEpochRef.current &&
          latestProject?.sessionId === project.sessionId &&
          activeBatchRef.current === progress.batchId
        ) {
          activeBatchRef.current = null
          dispatch({
            type: 'operation_finish_settled',
            sessionId: latestProject.sessionId,
            generation: latestProject.generation,
            batchId: progress.batchId,
          })
        }
      }
    },
    [bridge, dispatch, refreshProjection, sessionEpochRef, stateRef],
  )

  const receiveOperationProgress = useCallback(
    (progress: OperationProgressEvent) => {
      const project = stateRef.current.project
      if (
        project === null ||
        project.sessionId !== progress.sessionId ||
        project.generation !== progress.generation ||
        activeBatchRef.current !== progress.batchId
      ) {
        return
      }
      dispatch({ type: 'operation_progress_received', progress })
      if (progress.lifecycle === 'completed') void finishOperation(progress)
    },
    [dispatch, finishOperation, stateRef],
  )

  const executeFileCommand = useCallback(
    async (
      kind: FileCommandKind,
      items: FileCommandItem[],
      conflicts: ConflictResolution[] = [],
    ) => {
      const current = stateRef.current
      if (
        current.status !== 'active' ||
        current.project === null ||
        current.project.access === 'read_only' ||
        operationRequestPendingRef.current ||
        undoRequestPendingRef.current ||
        activeBatchRef.current !== null
      ) {
        return null
      }
      const project = current.project
      const requestSessionEpoch = sessionEpochRef.current
      operationRequestPendingRef.current = true
      try {
        const started = await bridge.executeFileCommand({
          sessionId: project.sessionId,
          generation: project.generation,
          kind,
          items,
          conflicts,
        })
        if (
          requestSessionEpoch !== sessionEpochRef.current ||
          stateRef.current.project?.sessionId !== project.sessionId ||
          stateRef.current.project.generation !== project.generation
        ) {
          return null
        }
        activeBatchRef.current = started.batchId
        operationResultsRequestRef.current += 1
        dispatch({
          type: 'operation_started',
          sessionId: project.sessionId,
          generation: project.generation,
          batchId: started.batchId,
          kind,
        })
        void bridge
          .operationStatus({
            sessionId: project.sessionId,
            generation: project.generation,
            batchId: started.batchId,
          })
          .then((progress) => {
            if (requestSessionEpoch !== sessionEpochRef.current) return
            if (progress) receiveOperationProgress(progress)
          })
          .catch(() => {
            if (requestSessionEpoch !== sessionEpochRef.current) return
          })
        return started
      } catch (error) {
        if (requestSessionEpoch !== sessionEpochRef.current) return null
        dispatch({ type: 'input_rejected', message: safeUserMessage(error) })
        return null
      } finally {
        if (requestSessionEpoch === sessionEpochRef.current) {
          operationRequestPendingRef.current = false
        }
      }
    },
    [bridge, dispatch, receiveOperationProgress, sessionEpochRef, stateRef],
  )

  const cancelOperation = useCallback(async () => {
    const project = stateRef.current.project
    const batchId = activeBatchRef.current
    if (project === null || batchId === null) return false
    const requestSessionEpoch = sessionEpochRef.current
    try {
      const cancelled = await bridge.cancelOperation({
        sessionId: project.sessionId,
        generation: project.generation,
        batchId,
      })
      if (requestSessionEpoch !== sessionEpochRef.current) return false
      return cancelled
    } catch (error) {
      if (requestSessionEpoch !== sessionEpochRef.current) return false
      dispatch({ type: 'input_rejected', message: safeUserMessage(error) })
      return false
    }
  }, [bridge, dispatch, sessionEpochRef, stateRef])

  const loadOperationResults = useCallback(
    async (offset: number) => {
      const current = stateRef.current
      const active = current.operation.active
      if (current.project === null || active === null) return null
      const requestSessionEpoch = sessionEpochRef.current
      const requestId = ++operationResultsRequestRef.current
      const project = current.project
      const batchId = active.batchId
      try {
        const page = await bridge.operationResults({
          sessionId: project.sessionId,
          generation: project.generation,
          batchId,
          offset: Math.max(0, offset),
          limit: 200,
        })
        if (
          requestSessionEpoch !== sessionEpochRef.current ||
          requestId !== operationResultsRequestRef.current ||
          stateRef.current.project?.sessionId !== project.sessionId ||
          stateRef.current.project.generation !== project.generation ||
          stateRef.current.operation.active?.batchId !== batchId
        )
          return null
        dispatch({
          type: 'operation_results_loaded',
          sessionId: project.sessionId,
          generation: project.generation,
          batchId,
          page,
        })
        return page
      } catch (error) {
        if (
          requestSessionEpoch !== sessionEpochRef.current ||
          requestId !== operationResultsRequestRef.current
        )
          return null
        dispatch({
          type: 'operation_failed',
          sessionId: project.sessionId,
          generation: project.generation,
          message: safeUserMessage(error),
        })
        return null
      }
    },
    [bridge, dispatch, sessionEpochRef, stateRef],
  )

  const undoLastOperation = useCallback(async () => {
    const current = stateRef.current
    if (
      current.status !== 'active' ||
      current.project === null ||
      current.project.access === 'read_only' ||
      operationRequestPendingRef.current ||
      undoRequestPendingRef.current ||
      activeBatchRef.current !== null
    )
      return null
    const requestSessionEpoch = sessionEpochRef.current
    undoRequestPendingRef.current = true
    dispatch({ type: 'operation_request_pending', pending: true })
    try {
      const receipt = await bridge.undoLastOperation({
        sessionId: current.project.sessionId,
        generation: current.project.generation,
      })
      if (requestSessionEpoch !== sessionEpochRef.current) return null
      if (receipt) {
        await refreshDesiredProjection(refreshProjection, current.project, true)
        if (requestSessionEpoch !== sessionEpochRef.current) return null
        dispatch({ type: 'search_refresh_requested' })
      }
      return receipt
    } catch (error) {
      if (requestSessionEpoch !== sessionEpochRef.current) return null
      dispatch({ type: 'input_rejected', message: safeUserMessage(error) })
      return null
    } finally {
      if (requestSessionEpoch === sessionEpochRef.current) {
        undoRequestPendingRef.current = false
        dispatch({ type: 'operation_request_pending', pending: false })
      }
    }
  }, [bridge, dispatch, refreshProjection, sessionEpochRef, stateRef])

  const controller: OperationController = {
    previewRename,
    preflightFileCommand,
    executeFileCommand,
    cancelOperation,
    loadOperationResults,
    undoLastOperation,
    receiveOperationProgress,
  }
  internalsByController.set(controller, {
    operationRequestPendingRef,
    undoRequestPendingRef,
    activeBatchRef,
    resetSessionRequests,
  })
  return controller
}
