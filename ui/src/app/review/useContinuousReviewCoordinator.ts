import { useEffect, useMemo, useRef, useSyncExternalStore } from 'react'
import type { ReviewWorkspaceError, ReviewWorkspacePort } from '../../api/reviewWorkspaceTypes'
import { ContinuousReviewSession } from './continuousReviewSession'

export interface ContinuousReviewCoordinatorOptions {
  sessionId: string
  generation: number
  port: ReviewWorkspacePort
  enabled?: boolean
  onError?: (cause: ReviewWorkspaceError) => void
}

export function useContinuousReviewCoordinator({
  sessionId,
  generation,
  port,
  enabled = true,
  onError,
}: ContinuousReviewCoordinatorOptions) {
  const onErrorRef = useRef(onError)
  const handoverRef = useRef<Promise<unknown> | null>(null)
  onErrorRef.current = onError
  const session = useMemo(
    () =>
      new ContinuousReviewSession(port, { sessionId, generation }, (error) =>
        onErrorRef.current?.(error),
      ),
    [port, sessionId, generation],
  )
  const workbenchSessionKey = useMemo(
    () => `${sessionId}:${generation}:${crypto.randomUUID()}`,
    [session, sessionId, generation],
  )
  const snapshot = useSyncExternalStore(session.subscribe, session.getSnapshot)
  useEffect(() => {
    if (!enabled) return undefined
    const stop = session.start(handoverRef.current)
    return () => {
      handoverRef.current = stop()
    }
  }, [enabled, session])
  return {
    ...snapshot,
    workbenchSessionKey,
    currentSnapshotId: snapshot.view?.current?.reference.snapshotId ?? null,
    getSnapshot: session.getSnapshot,
    beginEditor: session.beginEditor,
    saveFeedback: session.saveFeedback,
    retry: session.retry,
    refresh: session.refresh,
    cancel: session.cancel,
    setEditorText: session.setEditorText,
    setEditorTargets: session.setEditorTargets,
    discardEditor: session.discardEditor,
    rebaseEditor: session.rebaseEditor,
    hasUncommittedInput: session.hasUncommittedInput(),
    ...session.actions,
  }
}

export type ContinuousReviewCoordinator = ReturnType<typeof useContinuousReviewCoordinator>
