import type { VideoError, VideoEvent, VideoSession } from '../../api/types'

export interface VideoPreviewState {
  generation: number
  phase: 'preparing' | 'playing' | 'paused' | 'seeking' | 'ended' | 'failed' | 'closing'
  prepared: boolean
  firstFrameReady: boolean
  surfaceVisible: boolean
  timeUs: number
  durationUs: number | null
  error: VideoError | null
}

export type VideoStateAction =
  | VideoEvent
  | { type: 'reset' }
  | { type: 'opened'; session: VideoSession }
  | { type: 'openFailed'; error: VideoError }

export function createPreparingVideoState(): VideoPreviewState {
  return {
    generation: 0,
    phase: 'preparing',
    prepared: false,
    firstFrameReady: false,
    surfaceVisible: false,
    timeUs: 0,
    durationUs: null,
    error: null,
  }
}

export function openedVideoState(session: VideoSession): VideoPreviewState {
  return reduceVideoState(createPreparingVideoState(), { type: 'opened', session })
}

export function reduceVideoState(
  state: VideoPreviewState,
  action: VideoStateAction,
): VideoPreviewState {
  if (action.type === 'reset') return createPreparingVideoState()
  if (action.type === 'openFailed') {
    return {
      ...state,
      phase: 'failed',
      surfaceVisible: false,
      error: action.error,
    }
  }
  if (action.type === 'opened') {
    return {
      ...createPreparingVideoState(),
      generation: action.session.generation,
      prepared: true,
      durationUs: action.session.media.durationUs,
    }
  }
  if (action.generation !== state.generation) return state
  if (state.phase === 'failed' || state.phase === 'closing') return state

  if (action.type === 'prepared') {
    return {
      ...state,
      prepared: true,
      durationUs: action.media.durationUs,
    }
  }
  if (action.type === 'firstFrameReady') {
    if (state.firstFrameReady) return state
    return { ...state, firstFrameReady: true, surfaceVisible: true }
  }
  if (action.type === 'stateChanged') {
    const phase = phaseForNativeState(action.state)
    if (phase === 'failed' || phase === 'closing') {
      return { ...state, phase, surfaceVisible: false }
    }
    return phase === state.phase ? state : { ...state, phase }
  }
  if (action.type === 'progress') {
    return { ...state, timeUs: action.timeUs, durationUs: action.durationUs }
  }
  if (action.type === 'ended') return { ...state, phase: 'ended' }
  if (action.type === 'failed') {
    return { ...state, phase: 'failed', surfaceVisible: false, error: action.error }
  }
  if (action.type === 'closed') return { ...state, phase: 'closing', surfaceVisible: false }
  return state
}

function phaseForNativeState(state: Extract<VideoEvent, { type: 'stateChanged' }>['state']) {
  if (state === 'playing') return 'playing' as const
  if (state === 'paused') return 'paused' as const
  if (state === 'seeking' || state === 'frame_stepping') return 'seeking' as const
  if (state === 'ended') return 'ended' as const
  if (state === 'failed') return 'failed' as const
  if (state === 'closing' || state === 'idle') return 'closing' as const
  return 'preparing' as const
}
