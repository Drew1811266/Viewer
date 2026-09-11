import type { BrowserFile, ImageRequestPriority } from '../api/types'

export const PROJECT_THUMBNAIL_CACHE_CAPACITY = 512

export type ThumbnailLoader = (
  file: BrowserFile,
  maxPixels: number,
  scaleMilli: number,
  signal?: AbortSignal,
  priority?: ImageRequestPriority,
) => Promise<string>

export interface ProjectThumbnailCache {
  request: ThumbnailLoader
  /** 预取入口：与 request 共享缓存键去重，但以低 IPC 优先级进入解码调度器。 */
  prefetch: ThumbnailLoader
  clear(): void
}

export function abortedThumbnailRequest(): Error {
  return new Error('thumbnail request aborted')
}

function isAbortError(error: unknown): boolean {
  return error instanceof Error && error.message === 'thumbnail request aborted'
}

export function createProjectThumbnailCache(
  projectSessionId: string,
  loader: ThumbnailLoader,
  resolvedCapacity = PROJECT_THUMBNAIL_CACHE_CAPACITY,
): ProjectThumbnailCache {
  const inFlight = new Map<string, Promise<string>>()
  const resolved = new Map<string, string>()
  let generation = 0

  /**
   * 每个调用方独立结算：取消的调用方立即拒绝并让出 in-flight 记录，
   * 让后续（如重挂载）的同键请求重新发起，而不是拿到一个注定被丢弃的旧 Promise。
   */
  const attachCancellation = (
    key: string,
    pending: Promise<string>,
    signal: AbortSignal,
  ): Promise<string> =>
    new Promise<string>((resolve, reject) => {
      const onAbort = () => {
        signal.removeEventListener('abort', onAbort)
        if (inFlight.get(key) === pending) inFlight.delete(key)
        reject(abortedThumbnailRequest())
      }
      if (signal.aborted) {
        onAbort()
        return
      }
      signal.addEventListener('abort', onAbort, { once: true })
      void pending.then(resolve, reject)
    })

  const enqueue = (
    file: BrowserFile,
    maxPixels: number,
    scaleMilli: number,
    priority: ImageRequestPriority,
    signal?: AbortSignal,
  ): Promise<string> => {
    const key = [projectSessionId, file.entityId, file.modifiedNs, maxPixels, scaleMilli].join(':')
    const cached = resolved.get(key)
    if (cached !== undefined) {
      resolved.delete(key)
      resolved.set(key, cached)
      return Promise.resolve(cached)
    }
    if (signal?.aborted) return Promise.reject(abortedThumbnailRequest())

    const existingRequest = inFlight.get(key)
    if (existingRequest !== undefined) {
      return signal === undefined
        ? existingRequest
        : attachCancellation(key, existingRequest, signal)
    }

    const requestGeneration = generation
    const pending = loader(file, maxPixels, scaleMilli, signal, priority).then(
      (url) => {
        if (inFlight.get(key) === pending) inFlight.delete(key)
        if (generation === requestGeneration) {
          resolved.delete(key)
          resolved.set(key, url)
          while (resolved.size > resolvedCapacity) {
            const oldestKey = resolved.keys().next().value
            if (oldestKey === undefined) break
            resolved.delete(oldestKey)
          }
        }
        return url
      },
      (error: unknown) => {
        if (inFlight.get(key) === pending) inFlight.delete(key)
        throw error
      },
    )
    inFlight.set(key, pending)
    return signal === undefined ? pending : attachCancellation(key, pending, signal)
  }

  const request: ThumbnailLoader = (file, maxPixels, scaleMilli, signal) =>
    enqueue(file, maxPixels, scaleMilli, 'interactive', signal)
  const prefetch: ThumbnailLoader = (file, maxPixels, scaleMilli, signal) =>
    enqueue(file, maxPixels, scaleMilli, 'prefetch', signal)

  return {
    request,
    prefetch,
    clear() {
      generation += 1
      inFlight.clear()
      resolved.clear()
    },
  }
}

export { isAbortError }
