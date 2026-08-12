import { describe, expect, it } from 'vitest'
import type { VideoMedia, VideoSession } from '../../api/types'
import { createPreparingVideoState, openedVideoState, reduceVideoState } from './videoState'

const media: VideoMedia = {
  durationUs: 8_000_000,
  displayWidth: 1_920,
  displayHeight: 1_080,
  rotationDegrees: 0,
}

function session(generation: number): VideoSession {
  return {
    generation,
    sessionId: `video-session-${generation}`,
    media,
  }
}

describe('video preview generation state', () => {
  it('opens a generation behind the first-frame gate', () => {
    expect(openedVideoState(session(2))).toEqual({
      generation: 2,
      phase: 'preparing',
      prepared: true,
      firstFrameReady: false,
      surfaceVisible: false,
      timeUs: 0,
      durationUs: 8_000_000,
      error: null,
    })
  })

  it('reveals only the active generation and keeps the reveal one-way', () => {
    const opened = openedVideoState(session(2))

    const stale = reduceVideoState(opened, { type: 'firstFrameReady', generation: 1 })
    const revealed = reduceVideoState(stale, { type: 'firstFrameReady', generation: 2 })
    const repeated = reduceVideoState(revealed, { type: 'firstFrameReady', generation: 2 })

    expect(stale.surfaceVisible).toBe(false)
    expect(revealed).toEqual(
      expect.objectContaining({ firstFrameReady: true, surfaceVisible: true }),
    )
    expect(repeated).toBe(revealed)
  })

  it('filters every stale event and reduces active progress, state, and failure', () => {
    const opened = openedVideoState(session(4))
    expect(
      reduceVideoState(opened, {
        type: 'progress',
        generation: 3,
        timeUs: 7_000_000,
        durationUs: 9_000_000,
      }),
    ).toBe(opened)

    const progressed = reduceVideoState(opened, {
      type: 'progress',
      generation: 4,
      timeUs: 1_250_000,
      durationUs: 8_500_000,
    })
    const paused = reduceVideoState(progressed, {
      type: 'stateChanged',
      generation: 4,
      state: 'paused',
    })
    const failed = reduceVideoState(paused, {
      type: 'failed',
      generation: 4,
      error: { code: 'decode_failed', retryable: true },
    })

    expect(progressed).toEqual(
      expect.objectContaining({ timeUs: 1_250_000, durationUs: 8_500_000 }),
    )
    expect(paused.phase).toBe('paused')
    expect(failed).toEqual(
      expect.objectContaining({
        phase: 'failed',
        surfaceVisible: false,
        error: { code: 'decode_failed', retryable: true },
      }),
    )
  })

  it('keeps a bridge-open failure in the shell before a generation exists', () => {
    const preparing = createPreparingVideoState()
    const failed = reduceVideoState(preparing, {
      type: 'openFailed',
      error: { code: 'video_open_failed', retryable: true },
    })

    expect(failed).toEqual(
      expect.objectContaining({
        generation: 0,
        phase: 'failed',
        surfaceVisible: false,
        error: { code: 'video_open_failed', retryable: true },
      }),
    )
  })

  it('resets a previous generation before opening another file or retrying', () => {
    const revealed = reduceVideoState(openedVideoState(session(9)), {
      type: 'firstFrameReady',
      generation: 9,
    })

    expect(reduceVideoState(revealed, { type: 'reset' })).toEqual(createPreparingVideoState())
  })

  it('does not let a late same-generation first frame reopen a failed surface', () => {
    const revealed = reduceVideoState(openedVideoState(session(6)), {
      type: 'firstFrameReady',
      generation: 6,
    })
    const failedByState = reduceVideoState(revealed, {
      type: 'stateChanged',
      generation: 6,
      state: 'failed',
    })
    const lateFrame = reduceVideoState(failedByState, {
      type: 'firstFrameReady',
      generation: 6,
    })
    const latePlaying = reduceVideoState(failedByState, {
      type: 'stateChanged',
      generation: 6,
      state: 'playing',
    })

    expect(failedByState.surfaceVisible).toBe(false)
    expect(lateFrame).toBe(failedByState)
    expect(latePlaying).toBe(failedByState)
  })
})
