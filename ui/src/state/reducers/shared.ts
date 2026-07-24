import type { ScanEvent } from '../../api/types'
import type { ViewerState } from '../viewerState'

export function unique(values: string[]): string[] {
  return [...new Set(values)]
}

export function sameStrings(left: string[], right: string[]): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index])
}

export function isCurrentEvent(state: ViewerState, event: ScanEvent): boolean {
  return (
    state.project !== null &&
    state.project.sessionId === event.sessionId &&
    state.project.generation === event.generation
  )
}

export function isCurrentProjection(
  state: ViewerState,
  sessionId: string,
  generation: number,
): boolean {
  return state.project?.sessionId === sessionId && state.project.generation === generation
}
