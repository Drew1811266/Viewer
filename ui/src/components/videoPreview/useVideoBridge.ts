import {
  type RefObject,
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useReducer,
  useRef,
  useState,
} from 'react'
import type {
  VideoError,
  VideoEvent,
  VideoFailureKind,
  VideoFile,
  VideoMedia,
  VideoMetadata,
  VideoRate as VideoRateRequestValue,
} from '../../api/types'
import type { ViewerBridge } from '../../api/viewer'
import type { VideoControlCommands, VideoPlaybackRate } from './VideoControls'
import {
  fitVideoMatteInsets,
  fitVideoRect,
  hasVideoGeometry,
  type VideoMatteInsets,
} from './videoGeometry'
import { createPreparingVideoState, reduceVideoState, type VideoPreviewState } from './videoState'

export type VideoPreviewBridge = Pick<
  ViewerBridge,
  | 'listenVideo'
  | 'videoClose'
  | 'videoCancelOpen'
  | 'videoOpen'
  | 'videoPause'
  | 'videoPlay'
  | 'videoRequestThumbnail'
  | 'videoSeek'
  | 'videoSetFullscreen'
  | 'videoSetMuted'
  | 'videoSetRate'
  | 'videoSetSurfaceRect'
  | 'videoSetVolume'
  | 'videoStep'
>

export interface UseVideoBridgeOptions {
  bridge: VideoPreviewBridge
  file: VideoFile
  retryKey: number
  stage: RefObject<HTMLElement | null>
}

export interface UseVideoBridgeState {
  state: VideoPreviewState
  media: VideoMedia | null
  matteInsets: VideoMatteInsets | null
  controlState: VideoBridgeControlState
  commands: VideoControlCommands
}

export interface VideoBridgeControlState {
  volumePercent: number
  muted: boolean
  rate: VideoPlaybackRate
  fullscreen: boolean
  timelineThumbnail: Extract<VideoEvent, { type: 'timelineThumbnailReady' }> | null
}

interface ActiveVideoLifecycle {
  generation: number
  fail(error: VideoError): void
}

export function useVideoBridge({
  bridge,
  file,
  retryKey,
  stage,
}: UseVideoBridgeOptions): UseVideoBridgeState {
  const [state, dispatch] = useReducer(reduceVideoState, undefined, createPreparingVideoState)
  const [media, setMedia] = useState<VideoMedia | null>(null)
  const [controlState, setControlState] = useState<VideoBridgeControlState>(
    createVideoBridgeControlState,
  )
  const stageRect = useMeasuredVideoStage(stage)
  const activeGeneration = useRef<number | null>(null)
  const activeLifecycle = useRef<ActiveVideoLifecycle | null>(null)
  const lastSurfaceGeometry = useRef<string | null>(null)
  const surfaceUpdateSequence = useRef(0)
  const seekRequestId = useRef(0)
  const commandQueue = useRef<Promise<void>>(Promise.resolve())
  const closeTail = useRef<Promise<void>>(Promise.resolve())
  const playbackIntent = useRef(createBooleanControlIntent(false))
  const mutedIntent = useRef(createBooleanControlIntent(false))
  const fullscreenIntent = useRef(createBooleanControlIntent(false))
  const metadataMedia: VideoMedia = file.videoMetadata
  const indexedFailure = indexedVideoError(file.videoMetadata)
  const geometryReady = stageRect !== null && hasVideoGeometry(stageRect, metadataMedia)
  const retryingIndexedFailure =
    retryKey > 0 && indexedFailure !== null && stageRect !== null && hasStageGeometry(stageRect)
  const lifecycleKey =
    (file.videoMetadata.probeStatus === 'ready' && geometryReady) || retryingIndexedFailure
      ? `${file.entityId}:${retryKey}`
      : null
  const matteMedia = media ?? (geometryReady ? metadataMedia : null)
  const matteInsets =
    stageRect !== null && matteMedia !== null && hasVideoGeometry(stageRect, matteMedia)
      ? fitVideoMatteInsets(stageRect, matteMedia)
      : null

  useLayoutEffect(() => {
    activeGeneration.current = null
    lastSurfaceGeometry.current = null
    surfaceUpdateSequence.current = 0
    seekRequestId.current = 0
    commandQueue.current = Promise.resolve()
    resetBooleanControlIntent(playbackIntent.current, null, false)
    resetBooleanControlIntent(mutedIntent.current, null, false)
    resetBooleanControlIntent(fullscreenIntent.current, null, false)
    setMedia(null)
    setControlState(createVideoBridgeControlState())
    dispatch({ type: 'reset' })
    if (indexedFailure !== null && !retryingIndexedFailure) {
      dispatch({ type: 'openFailed', error: indexedFailure })
    }
  }, [
    file.entityId,
    file.videoMetadata.displayHeight,
    file.videoMetadata.displayWidth,
    file.videoMetadata.failureKind,
    file.videoMetadata.probeStatus,
    retryKey,
  ])

  useEffect(() => {
    if (lifecycleKey === null || stageRect === null) return
    const attemptId = crypto.randomUUID()
    const initialRect = geometryReady
      ? fitVideoRect(stageRect, metadataMedia)
      : stageVideoRect(stageRect)
    let disposed = false
    let generation: number | null = null
    let detached = false
    let unlisten: (() => void) | null = null
    const bufferedEvents: VideoEvent[] = []
    const closedGenerations = new Set<number>()
    let eventsReady = false
    let eventTail = Promise.resolve()
    let retryGeometryReady = !retryingIndexedFailure
    let pendingFirstFrame: Extract<VideoEvent, { type: 'firstFrameReady' }> | null = null

    const detach = () => {
      if (detached || unlisten === null) return
      detached = true
      unlisten()
    }
    const closeOnce = (target: number): Promise<void> => {
      if (closedGenerations.has(target)) return closeTail.current
      closedGenerations.add(target)
      closeTail.current = closeTail.current
        .then(() => bridge.videoClose({ generation: target }))
        .catch(() => undefined)
      return closeTail.current
    }
    const consume = async (event: VideoEvent) => {
      if (disposed) return
      if (generation === null) return
      if (event.generation !== generation) return
      if (retryingIndexedFailure && event.type === 'firstFrameReady' && !retryGeometryReady) {
        pendingFirstFrame = event
        return
      }
      if (event.type === 'prepared') {
        if (retryingIndexedFailure && hasVideoGeometry(stageRect, event.media)) {
          const fittedRect = fitVideoRect(stageRect, event.media)
          await bridge.videoSetSurfaceRect({
            generation: event.generation,
            sequence: ++surfaceUpdateSequence.current,
            ...fittedRect,
          })
          if (disposed) return
          lastSurfaceGeometry.current = surfaceGeometryKey(event.generation, fittedRect)
          retryGeometryReady = true
        }
        setMedia(event.media)
      }
      if (event.type === 'stateChanged') {
        if (event.state === 'playing') {
          acknowledgeBooleanControlIntent(playbackIntent.current, event.generation, true)
        } else if (event.state === 'paused' || event.state === 'ended') {
          acknowledgeBooleanControlIntent(playbackIntent.current, event.generation, false)
        }
      } else if (event.type === 'ended') {
        acknowledgeBooleanControlIntent(playbackIntent.current, event.generation, false)
      }
      if (event.type === 'settingsChanged') {
        acknowledgeBooleanControlIntent(mutedIntent.current, event.generation, event.muted)
        setControlState((current) => ({
          ...current,
          volumePercent: clampVolume(event.volumePercent),
          muted: event.muted,
          rate: eventRate(event.rate),
        }))
      } else if (event.type === 'fullscreenChanged') {
        acknowledgeBooleanControlIntent(
          fullscreenIntent.current,
          event.generation,
          event.fullscreen,
        )
        setControlState((current) => ({ ...current, fullscreen: event.fullscreen }))
      } else if (event.type === 'timelineThumbnailReady') {
        setControlState((current) => ({ ...current, timelineThumbnail: event }))
      }
      dispatch(event)
      if (retryGeometryReady && pendingFirstFrame !== null) {
        const ready = pendingFirstFrame
        pendingFirstFrame = null
        dispatch(ready)
      }
    }
    const enqueue = (event: VideoEvent) => {
      if (!eventsReady) {
        bufferedEvents.push(event)
        return
      }
      if (!retryingIndexedFailure) {
        void consume(event)
        return
      }
      eventTail = eventTail
        .then(() => consume(event))
        .catch((error) => {
          const lifecycle = activeLifecycle.current
          if (generation !== null && lifecycle?.generation === generation) {
            lifecycle.fail(videoError(error, 'video_surface_failed'))
          }
        })
    }

    void (async () => {
      try {
        await closeTail.current
        if (disposed) return
        const subscribed = await bridge.listenVideo(enqueue)
        if (disposed) {
          subscribed()
          return
        }
        unlisten = subscribed
        const session = await bridge.videoOpen({
          attemptId,
          entityId: file.entityId,
          surfaceRect: initialRect,
        })
        generation = session.generation
        if (disposed) {
          closeOnce(session.generation)
          return
        }
        if (retryingIndexedFailure && hasVideoGeometry(stageRect, session.media)) {
          const fittedRect = fitVideoRect(stageRect, session.media)
          try {
            await bridge.videoSetSurfaceRect({
              generation: session.generation,
              sequence: ++surfaceUpdateSequence.current,
              ...fittedRect,
            })
          } catch (error) {
            await closeOnce(session.generation)
            throw error
          }
          lastSurfaceGeometry.current = surfaceGeometryKey(session.generation, fittedRect)
          retryGeometryReady = true
          if (disposed) {
            closeOnce(session.generation)
            return
          }
        }
        activeGeneration.current = session.generation
        seekRequestId.current = 0
        commandQueue.current = Promise.resolve()
        resetBooleanControlIntent(playbackIntent.current, session.generation, false)
        resetBooleanControlIntent(mutedIntent.current, session.generation, false)
        resetBooleanControlIntent(fullscreenIntent.current, session.generation, false)
        activeLifecycle.current = {
          generation: session.generation,
          fail(error) {
            if (disposed) return
            disposed = true
            detach()
            bufferedEvents.length = 0
            closeOnce(session.generation)
            if (activeGeneration.current === session.generation) {
              activeGeneration.current = null
            }
            dispatch({ type: 'openFailed', error })
          },
        }
        setMedia(session.media)
        dispatch({ type: 'opened', session })
        eventsReady = true
        for (const event of bufferedEvents) enqueue(event)
        bufferedEvents.length = 0
      } catch (error) {
        if (!disposed) {
          detach()
          bufferedEvents.length = 0
          dispatch({ type: 'openFailed', error: videoError(error) })
        }
      }
    })()

    return () => {
      disposed = true
      detach()
      if (generation !== null) {
        closeOnce(generation)
      } else {
        void bridge.videoCancelOpen({ attemptId }).catch(() => undefined)
      }
      if (activeGeneration.current === generation) activeGeneration.current = null
      if (activeLifecycle.current?.generation === generation) activeLifecycle.current = null
    }
  }, [bridge, file.entityId, geometryReady, lifecycleKey, retryingIndexedFailure])

  useEffect(() => {
    if (
      state.generation === 0 ||
      media === null ||
      stageRect === null ||
      !hasVideoGeometry(stageRect, media)
    ) {
      return
    }
    const rect = fitVideoRect(stageRect, media)
    const geometryKey = surfaceGeometryKey(state.generation, rect)
    if (lastSurfaceGeometry.current === geometryKey) return
    lastSurfaceGeometry.current = geometryKey
    const generation = state.generation
    const updateSequence = ++surfaceUpdateSequence.current
    void bridge.videoSetSurfaceRect({ generation, sequence: updateSequence, ...rect }).catch(() => {
      // Window and WebView geometry are published by separate native/DOM
      // layout passes. Keep the last valid surface alive when one transient
      // resize sample is rejected; a newer measured rect supersedes it.
      if (
        activeGeneration.current === generation &&
        surfaceUpdateSequence.current === updateSequence &&
        lastSurfaceGeometry.current === geometryKey
      ) {
        lastSurfaceGeometry.current = null
      }
    })
  }, [bridge, media, stageRect, state.generation])

  const withGeneration = useCallback(async (command: (generation: number) => Promise<void>) => {
    const generation = activeGeneration.current
    if (generation === null) return
    await command(generation)
  }, [])

  const enqueueCommand = useCallback(
    (
      generation: number,
      command: () => Promise<void>,
      onSkipped: () => void = () => undefined,
    ): Promise<void> => {
      const queued = commandQueue.current.then(async () => {
        if (activeGeneration.current !== generation) {
          onSkipped()
          return
        }
        await command()
      })
      commandQueue.current = queued.catch(() => undefined)
      return queued
    },
    [],
  )

  const commands = useMemo<VideoControlCommands>(() => {
    const issueBooleanCommand = (
      intent: BooleanControlIntent,
      value: boolean,
      send: (generation: number, value: boolean) => Promise<void>,
    ): Promise<void> => {
      const generation = activeGeneration.current
      if (generation === null) return Promise.resolve()
      const reservation = reserveBooleanControlIntent(intent, generation, value)
      return enqueueCommand(
        generation,
        async () => {
          try {
            await send(generation, value)
          } catch (error) {
            releaseBooleanControlIntent(intent, reservation)
            throw error
          }
        },
        () => releaseBooleanControlIntent(intent, reservation),
      )
    }
    const toggleBooleanCommand = (
      intent: BooleanControlIntent,
      send: (generation: number, value: boolean) => Promise<void>,
    ): Promise<void> => {
      const generation = activeGeneration.current
      if (generation === null || intent.generation !== generation) return Promise.resolve()
      return issueBooleanCommand(intent, !intent.desired, send)
    }

    return {
      play: () =>
        issueBooleanCommand(playbackIntent.current, true, (generation) =>
          bridge.videoPlay({ generation }),
        ),
      pause: () =>
        issueBooleanCommand(playbackIntent.current, false, (generation) =>
          bridge.videoPause({ generation }),
        ),
      togglePlayback: () =>
        toggleBooleanCommand(playbackIntent.current, (generation, playing) =>
          playing ? bridge.videoPlay({ generation }) : bridge.videoPause({ generation }),
        ),
      step(direction) {
        const generation = activeGeneration.current
        if (generation === null) return Promise.resolve()
        const shouldPause =
          playbackIntent.current.generation === generation && playbackIntent.current.desired
        const pauseReservation = shouldPause
          ? reserveBooleanControlIntent(playbackIntent.current, generation, false)
          : null
        return enqueueCommand(
          generation,
          async () => {
            if (pauseReservation !== null) {
              try {
                await bridge.videoPause({ generation })
              } catch (error) {
                releaseBooleanControlIntent(playbackIntent.current, pauseReservation)
                throw error
              }
              if (activeGeneration.current !== generation) return
            }
            await bridge.videoStep({ generation, direction })
          },
          () => {
            if (pauseReservation !== null) {
              releaseBooleanControlIntent(playbackIntent.current, pauseReservation)
            }
          },
        )
      },
      previewSeek: (timeUs) =>
        withGeneration((generation) =>
          bridge.videoSeek({
            generation,
            requestId: ++seekRequestId.current,
            timeUs: boundedTime(timeUs, state.durationUs),
            intent: 'preview',
          }),
        ),
      seek: (timeUs) =>
        withGeneration((generation) =>
          bridge.videoSeek({
            generation,
            requestId: ++seekRequestId.current,
            timeUs: boundedTime(timeUs, state.durationUs),
            intent: 'commit',
          }),
        ),
      setVolume: (volumePercent) =>
        withGeneration((generation) =>
          bridge.videoSetVolume({ generation, volumePercent: clampVolume(volumePercent) }),
        ),
      setMuted: (muted) =>
        issueBooleanCommand(mutedIntent.current, muted, (generation, value) =>
          bridge.videoSetMuted({ generation, muted: value }),
        ),
      toggleMuted: () =>
        toggleBooleanCommand(mutedIntent.current, (generation, muted) =>
          bridge.videoSetMuted({ generation, muted }),
        ),
      setRate: (rate) =>
        withGeneration((generation) =>
          bridge.videoSetRate({ generation, rate: rateRequestValue(rate) }),
        ),
      setFullscreen: (fullscreen) =>
        issueBooleanCommand(fullscreenIntent.current, fullscreen, (generation, value) =>
          bridge.videoSetFullscreen({ generation, fullscreen: value }),
        ),
      toggleFullscreen: () =>
        toggleBooleanCommand(fullscreenIntent.current, (generation, fullscreen) =>
          bridge.videoSetFullscreen({ generation, fullscreen }),
        ),
      requestThumbnail: ({ requestId, timeUs }) =>
        withGeneration((generation) =>
          bridge.videoRequestThumbnail({
            generation,
            requestId,
            timeUs: boundedTime(timeUs, state.durationUs),
          }),
        ),
    }
  }, [bridge, enqueueCommand, state.durationUs, withGeneration])

  return { state, media, matteInsets, controlState, commands }
}

interface BooleanControlIntent {
  generation: number | null
  confirmed: boolean
  desired: boolean
  nextId: number
  pending: BooleanControlReservation[]
}

interface BooleanControlReservation {
  generation: number
  id: number
  value: boolean
}

function createBooleanControlIntent(confirmed: boolean): BooleanControlIntent {
  return {
    generation: null,
    confirmed,
    desired: confirmed,
    nextId: 1,
    pending: [],
  }
}

function resetBooleanControlIntent(
  intent: BooleanControlIntent,
  generation: number | null,
  confirmed: boolean,
): void {
  intent.generation = generation
  intent.confirmed = confirmed
  intent.desired = confirmed
  intent.nextId = 1
  intent.pending = []
}

function reserveBooleanControlIntent(
  intent: BooleanControlIntent,
  generation: number,
  value: boolean,
): BooleanControlReservation {
  if (intent.generation !== generation)
    resetBooleanControlIntent(intent, generation, intent.confirmed)
  const reservation = { generation, id: intent.nextId++, value }
  intent.pending.push(reservation)
  intent.desired = value
  return reservation
}

function releaseBooleanControlIntent(
  intent: BooleanControlIntent,
  reservation: BooleanControlReservation,
): void {
  if (intent.generation !== reservation.generation) return
  const index = intent.pending.findIndex((candidate) => candidate.id === reservation.id)
  if (index === -1) return
  intent.pending.splice(index, 1)
  intent.desired = intent.pending.at(-1)?.value ?? intent.confirmed
}

function acknowledgeBooleanControlIntent(
  intent: BooleanControlIntent,
  generation: number,
  confirmed: boolean,
): void {
  if (intent.generation !== generation) return
  intent.confirmed = confirmed
  const index = intent.pending.findIndex((candidate) => candidate.value === confirmed)
  if (index >= 0) intent.pending.splice(0, index + 1)
  intent.desired = intent.pending.at(-1)?.value ?? confirmed
}

function createVideoBridgeControlState(): VideoBridgeControlState {
  return {
    volumePercent: 100,
    muted: false,
    rate: 1,
    fullscreen: false,
    timelineThumbnail: null,
  }
}

function clampVolume(volumePercent: number): number {
  if (!Number.isFinite(volumePercent)) return 100
  return Math.round(Math.min(100, Math.max(0, volumePercent)))
}

function boundedTime(timeUs: number, durationUs: number | null): number {
  const duration =
    durationUs !== null && Number.isFinite(durationUs) && durationUs > 0
      ? Math.floor(durationUs)
      : 0
  const bounded = Number.isFinite(timeUs) ? Math.floor(timeUs) : 0
  return Math.min(duration, Math.max(0, bounded))
}

function eventRate(rate: number): VideoPlaybackRate {
  if (rate === 0.5 || rate === 0.75 || rate === 1.25 || rate === 1.5 || rate === 2) return rate
  return 1
}

function rateRequestValue(rate: VideoPlaybackRate): VideoRateRequestValue {
  if (rate === 0.5) return 'half'
  if (rate === 0.75) return 'three_quarters'
  if (rate === 1.25) return 'one_and_quarter'
  if (rate === 1.5) return 'one_and_half'
  if (rate === 2) return 'double'
  return 'normal'
}

function useMeasuredVideoStage(stage: RefObject<HTMLElement | null>): DOMRectReadOnly | null {
  const [rect, setRect] = useState<DOMRectReadOnly | null>(null)

  useLayoutEffect(() => {
    const node = stage.current
    if (node === null) return
    let frame: number | null = null
    const publish = () => {
      const next = node.getBoundingClientRect()
      if (!finiteRect(next)) return
      setRect((current) => (sameRect(current, next) ? current : copyRect(next)))
    }
    const schedule = () => {
      if (frame !== null) return
      if (typeof requestAnimationFrame === 'undefined') {
        publish()
        return
      }
      frame = requestAnimationFrame(() => {
        frame = null
        publish()
      })
    }
    publish()
    const observer =
      typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(() => schedule())
    observer?.observe(node)
    window.addEventListener('resize', schedule)
    return () => {
      observer?.disconnect()
      window.removeEventListener('resize', schedule)
      if (frame !== null && typeof cancelAnimationFrame !== 'undefined') {
        cancelAnimationFrame(frame)
      }
    }
  }, [stage])

  return rect
}

function finiteRect(rect: DOMRectReadOnly): boolean {
  return (
    [rect.left, rect.top, rect.width, rect.height].every(Number.isFinite) &&
    rect.width > 0 &&
    rect.height > 0
  )
}

function sameRect(left: DOMRectReadOnly | null, right: DOMRectReadOnly): boolean {
  return (
    left !== null &&
    left.left === right.left &&
    left.top === right.top &&
    left.width === right.width &&
    left.height === right.height
  )
}

function hasStageGeometry(stage: DOMRectReadOnly): boolean {
  return (
    Number.isFinite(stage.left) &&
    Number.isFinite(stage.top) &&
    Number.isFinite(stage.width) &&
    stage.width > 0 &&
    Number.isFinite(stage.height) &&
    stage.height > 0
  )
}

function stageVideoRect(stage: DOMRectReadOnly) {
  if (!hasStageGeometry(stage)) throw new RangeError('Video stage geometry is not ready')
  return {
    x: Math.round(stage.left),
    y: Math.round(stage.top),
    width: Math.round(stage.width),
    height: Math.round(stage.height),
  }
}

function surfaceGeometryKey(
  generation: number,
  rect: { x: number; y: number; width: number; height: number },
): string {
  return `${generation}:${rect.x}:${rect.y}:${rect.width}:${rect.height}`
}

function copyRect(rect: DOMRectReadOnly): DOMRectReadOnly {
  return {
    x: rect.left,
    y: rect.top,
    left: rect.left,
    top: rect.top,
    right: rect.left + rect.width,
    bottom: rect.top + rect.height,
    width: rect.width,
    height: rect.height,
    toJSON: () => undefined,
  }
}

function videoError(error: unknown, fallbackCode = 'video_open_failed'): VideoError {
  if (typeof error !== 'object' || error === null) return { code: fallbackCode, retryable: true }
  const candidate = error as { code?: unknown; retryable?: unknown }
  return {
    code: typeof candidate.code === 'string' ? candidate.code : fallbackCode,
    retryable: typeof candidate.retryable === 'boolean' ? candidate.retryable : true,
  }
}

function indexedVideoError(metadata: VideoMetadata): VideoError | null {
  if (
    metadata.probeStatus === 'ready' &&
    (!finitePositive(metadata.displayWidth) || !finitePositive(metadata.displayHeight))
  ) {
    return { code: 'video_geometry_unavailable', retryable: false }
  }
  if (metadata.probeStatus !== 'failed') return null
  const code = metadata.failureKind ?? 'video_probe_failed'
  return { code, retryable: metadata.failureKind === null || retryableProbeFailure(code) }
}

function finitePositive(value: number | null): value is number {
  return value !== null && Number.isFinite(value) && value > 0
}

function retryableProbeFailure(code: VideoFailureKind | 'video_probe_failed'): boolean {
  return (
    code === 'unreadable' ||
    code === 'missing' ||
    code === 'engine_initialization' ||
    code === 'render_surface' ||
    code === 'thumbnail_unavailable' ||
    code === 'video_probe_failed'
  )
}
