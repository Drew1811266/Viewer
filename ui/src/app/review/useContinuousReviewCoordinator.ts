import { useEffect, useMemo, useRef, useSyncExternalStore } from 'react'
import type { ReviewWorkspaceError, ReviewWorkspacePort } from '../../api/reviewWorkspaceTypes'
import { ContinuousReviewSession } from './continuousReviewSession'

export interface ContinuousReviewCoordinatorOptions {
  sessionId: string
  generation: number
  port: ReviewWorkspacePort
  onError?: (cause: ReviewWorkspaceError) => void
}

export function useContinuousReviewCoordinator({
  sessionId,
  generation,
  port,
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
  const snapshot = useSyncExternalStore(session.subscribe, session.getSnapshot)
  useEffect(() => {
    const stop = session.start(handoverRef.current)
    return () => {
      handoverRef.current = stop()
    }
  }, [session])
  return {
    ...snapshot,
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
