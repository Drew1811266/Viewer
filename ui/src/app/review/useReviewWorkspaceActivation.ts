import type { ReviewWorkspacePort } from '../../api/reviewWorkspaceTypes'
import { tauriReviewWorkspaceBridge } from '../../api/viewer'
import { useContinuousReviewCoordinator } from './useContinuousReviewCoordinator'

interface ReviewWorkspaceActivationOptions {
  port: ReviewWorkspacePort | null
  sessionId: string | undefined
  generation: number | undefined
}

export function useReviewWorkspaceActivation({
  port,
  sessionId,
  generation,
}: ReviewWorkspaceActivationOptions) {
  const enabled = port !== null && sessionId !== undefined && generation !== undefined
  const coordinator = useContinuousReviewCoordinator({
    port: port ?? tauriReviewWorkspaceBridge,
    sessionId: sessionId ?? 'no-session',
    generation: generation ?? 0,
    enabled,
  })
  const presentation =
    enabled && coordinator.state.kind === 'ready' && coordinator.view?.migration === null
      ? coordinator
      : undefined
  return {
    enabled,
    protocol: enabled ? ('continuous' as const) : ('legacy' as const),
    coordinator: enabled ? coordinator : undefined,
    presentation,
  }
}
