import { type RefObject, useEffect, useLayoutEffect, useReducer, useRef, useState } from 'react'
import type {
  VideoError,
  VideoEvent,
  VideoFailureKind,
  VideoFile,
  VideoMedia,
  VideoMetadata,
} from '../../api/types'
import type { ViewerBridge } from '../../api/viewer'
import { fitVideoRect, hasVideoGeometry } from './videoGeometry'
import { createPreparingVideoState, reduceVideoState, type VideoPreviewState } from './videoState'

export type VideoPreviewBridge = Pick<
  ViewerBridge,
  'listenVideo' | 'videoClose' | 'videoOpen' | 'videoSetSurfaceRect'
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
  const stageRect = useMeasuredVideoStage(stage)
  const activeGeneration = useRef<number | null>(null)
  const activeLifecycle = useRef<ActiveVideoLifecycle | null>(null)
  const lastSurfaceGeometry = useRef<string | null>(null)
  const metadataMedia: VideoMedia = file.videoMetadata
  const indexedFailure = indexedVideoError(file.videoMetadata)
  const geometryReady = stageRect !== null && hasVideoGeometry(stageRect, metadataMedia)
  const lifecycleKey =
    file.videoMetadata.probeStatus === 'ready' && geometryReady
      ? `${file.entityId}:${retryKey}`
      : null

  useLayoutEffect(() => {
    activeGeneration.current = null
    lastSurfaceGeometry.current = null
    setMedia(null)
    dispatch({ type: 'reset' })
    if (indexedFailure !== null) dispatch({ type: 'openFailed', error: indexedFailure })
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
    const initialRect = fitVideoRect(stageRect, metadataMedia)
    let disposed = false
    let generation: number | null = null
    let detached = false
    let unlisten: (() => void) | null = null
    const bufferedEvents: VideoEvent[] = []
    const closedGenerations = new Set<number>()

    const detach = () => {
      if (detached || unlisten === null) return
      detached = true
      unlisten()
    }
    const closeOnce = (target: number) => {
      if (closedGenerations.has(target)) return
      closedGenerations.add(target)
      void bridge.videoClose({ generation: target }).catch(() => undefined)
    }
    const consume = (event: VideoEvent) => {
      if (disposed) return
      if (generation === null) {
        bufferedEvents.push(event)
        return
      }
      if (event.generation !== generation) return
      if (event.type === 'prepared') setMedia(event.media)
      dispatch(event)
    }

    void (async () => {
      try {
        const subscribed = await bridge.listenVideo(consume)
        if (disposed) {
          subscribed()
          return
        }
        unlisten = subscribed
        const session = await bridge.videoOpen({
          entityId: file.entityId,
          surfaceRect: initialRect,
        })
        generation = session.generation
        if (disposed) {
          closeOnce(session.generation)
          return
        }
        activeGeneration.current = session.generation
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
        for (const event of bufferedEvents) consume(event)
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
      if (generation !== null) closeOnce(generation)
      if (activeGeneration.current === generation) activeGeneration.current = null
      if (activeLifecycle.current?.generation === generation) activeLifecycle.current = null
    }
  }, [bridge, file.entityId, lifecycleKey])

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
    const geometryKey = `${state.generation}:${rect.x}:${rect.y}:${rect.width}:${rect.height}`
    if (lastSurfaceGeometry.current === geometryKey) return
    lastSurfaceGeometry.current = geometryKey
    const generation = state.generation
    void bridge.videoSetSurfaceRect({ generation, ...rect }).catch((error) => {
      const lifecycle = activeLifecycle.current
      if (lifecycle?.generation === generation) {
        lifecycle.fail(videoError(error, 'video_surface_failed'))
      }
    })
  }, [bridge, media, stageRect, state.generation])

  return { state, media }
}

function useMeasuredVideoStage(stage: RefObject<HTMLElement | null>): DOMRectReadOnly | null {
  const [rect, setRect] = useState<DOMRectReadOnly | null>(null)

  useLayoutEffect(() => {
    const node = stage.current
    if (node === null) return
    const publish = () => {
      const next = node.getBoundingClientRect()
      if (!finiteRect(next)) return
      setRect((current) => (sameRect(current, next) ? current : copyRect(next)))
    }
    publish()
    const observer =
      typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(() => publish())
    observer?.observe(node)
    window.addEventListener('resize', publish)
    return () => {
      observer?.disconnect()
      window.removeEventListener('resize', publish)
    }
  }, [stage])

  return rect
}

function finiteRect(rect: DOMRectReadOnly): boolean {
  return [rect.left, rect.top, rect.width, rect.height].every(Number.isFinite)
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
