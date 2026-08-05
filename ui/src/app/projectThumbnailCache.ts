import type { BrowserFile } from '../api/types'

export const PROJECT_THUMBNAIL_CACHE_CAPACITY = 512

export type ThumbnailLoader = (
  file: BrowserFile,
  maxPixels: number,
  scaleMilli: number,
) => Promise<string>

export interface ProjectThumbnailCache {
  request: ThumbnailLoader
  clear(): void
}

export function createProjectThumbnailCache(
  projectSessionId: string,
  loader: ThumbnailLoader,
  resolvedCapacity = PROJECT_THUMBNAIL_CACHE_CAPACITY,
): ProjectThumbnailCache {
  const inFlight = new Map<string, Promise<string>>()
  const resolved = new Map<string, string>()
  let generation = 0

  const request: ThumbnailLoader = (file, maxPixels, scaleMilli) => {
    const key = [projectSessionId, file.entityId, file.modifiedNs, maxPixels, scaleMilli].join(':')
    const cached = resolved.get(key)
    if (cached !== undefined) {
      resolved.delete(key)
      resolved.set(key, cached)
      return Promise.resolve(cached)
    }
    const existingRequest = inFlight.get(key)
    if (existingRequest !== undefined) return existingRequest

    const requestGeneration = generation
    const pending = loader(file, maxPixels, scaleMilli).then(
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
    return pending
  }

  return {
    request,
    clear() {
      generation += 1
      inFlight.clear()
      resolved.clear()
    },
  }
}
