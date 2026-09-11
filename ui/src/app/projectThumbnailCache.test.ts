import { describe, expect, it, vi } from 'vitest'
import type { BrowserFile } from '../api/types'
import {
  createProjectThumbnailCache,
  PROJECT_THUMBNAIL_CACHE_CAPACITY,
} from './projectThumbnailCache'

function image(entityId: string, modifiedNs = '1'): BrowserFile {
  return {
    entityId,
    relativePath: `${entityId}.jpg`,
    name: `${entityId}.jpg`,
    kind: 'jpeg',
    size: 100,
    modifiedNs,
    marker: { reviewState: null, favorite: false },
    imageMetadata: { width: 1, height: 1 },
    imageUrl: null,
    videoMetadata: null,
  }
}

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((next, fail) => {
    resolve = next
    reject = fail
  })
  return { promise, resolve, reject }
}

describe('projectThumbnailCache', () => {
  it('coalesces the exact key and invalidates file version, size, and scale independently', async () => {
    const pending = deferred<string>()
    const loader = vi.fn().mockImplementation(() => pending.promise)
    const cache = createProjectThumbnailCache('session-1', loader)
    const file = image('image-1')

    const first = cache.request(file, 198, 1_000)
    const duplicate = cache.request({ ...file }, 198, 1_000)
    expect(loader).toHaveBeenCalledOnce()
    expect(duplicate).toBe(first)

    pending.resolve('viewer-image://resolved')
    await expect(first).resolves.toBe('viewer-image://resolved')
    await expect(cache.request({ ...file }, 198, 1_000)).resolves.toBe('viewer-image://resolved')
    expect(loader).toHaveBeenCalledOnce()

    loader.mockResolvedValue('viewer-image://variant')
    await cache.request(image('image-1', '2'), 198, 1_000)
    await cache.request(file, 199, 1_000)
    await cache.request(file, 198, 2_000)
    expect(loader).toHaveBeenCalledTimes(4)
  })

  it('removes a failed request so the same thumbnail can retry', async () => {
    const loader = vi
      .fn()
      .mockRejectedValueOnce(new Error('decode failed'))
      .mockResolvedValueOnce('viewer-image://retry')
    const cache = createProjectThumbnailCache('session-1', loader)

    await expect(cache.request(image('image-1'), 198, 1_000)).rejects.toThrow('decode failed')
    await expect(cache.request(image('image-1'), 198, 1_000)).resolves.toBe('viewer-image://retry')
    expect(loader).toHaveBeenCalledTimes(2)
  })

  it('touches resolved entries and bounds the default resolved cache to 512 items', async () => {
    const loader = vi.fn(async (file: BrowserFile) => `viewer-image://${file.entityId}`)
    const small = createProjectThumbnailCache('session-1', loader, 2)

    await small.request(image('a'), 198, 1_000)
    await small.request(image('b'), 198, 1_000)
    await small.request(image('a'), 198, 1_000)
    await small.request(image('c'), 198, 1_000)
    await small.request(image('b'), 198, 1_000)
    expect(loader.mock.calls.map(([file]) => file.entityId)).toEqual(['a', 'b', 'c', 'b'])

    loader.mockClear()
    const bounded = createProjectThumbnailCache('session-2', loader)
    for (let index = 0; index <= PROJECT_THUMBNAIL_CACHE_CAPACITY; index += 1) {
      await bounded.request(image(`image-${index}`), 198, 1_000)
    }
    await bounded.request(image('image-0'), 198, 1_000)
    await bounded.request(image(`image-${PROJECT_THUMBNAIL_CACHE_CAPACITY}`), 198, 1_000)
    expect(loader).toHaveBeenCalledTimes(PROJECT_THUMBNAIL_CACHE_CAPACITY + 2)
  })

  it('prevents an old generation from overwriting or deleting a new same-key request', async () => {
    const older = deferred<string>()
    const newer = deferred<string>()
    const loader = vi
      .fn()
      .mockImplementationOnce(() => older.promise)
      .mockImplementationOnce(() => newer.promise)
    const cache = createProjectThumbnailCache('session-1', loader)
    const file = image('image-1')

    const oldRequest = cache.request(file, 198, 1_000)
    cache.clear()
    const newRequest = cache.request(file, 198, 1_000)
    older.resolve('viewer-image://old')
    await expect(oldRequest).resolves.toBe('viewer-image://old')
    expect(cache.request(file, 198, 1_000)).toBe(newRequest)

    newer.resolve('viewer-image://new')
    await expect(newRequest).resolves.toBe('viewer-image://new')
    await expect(cache.request(file, 198, 1_000)).resolves.toBe('viewer-image://new')
    expect(loader).toHaveBeenCalledTimes(2)
  })

  it('rejects an aborted caller immediately and lets a fresh request re-issue the loader', async () => {
    const pending = deferred<string>()
    const loader = vi
      .fn()
      .mockImplementationOnce(() => pending.promise)
      .mockResolvedValueOnce('viewer-image://reissued')
    const cache = createProjectThumbnailCache('session-1', loader)
    const file = image('image-1')
    const controller = new AbortController()

    const first = cache.request(file, 198, 1_000, controller.signal)
    controller.abort()
    await expect(first).rejects.toThrow('thumbnail request aborted')

    await expect(cache.request(file, 198, 1_000)).resolves.toBe('viewer-image://reissued')
    expect(loader).toHaveBeenCalledTimes(2)
    pending.resolve('viewer-image://late')
  })

  it('removes the in-flight entry when a caller aborts so remounts never await a doomed promise', async () => {
    const pending = deferred<string>()
    const loader = vi.fn().mockImplementation(() => pending.promise)
    const cache = createProjectThumbnailCache('session-1', loader)
    const file = image('image-1')
    const shared = new AbortController()

    void cache.request(file, 198, 1_000)
    const attached = cache.request(file, 198, 1_000, shared.signal)
    shared.abort()
    await expect(attached).rejects.toThrow('thumbnail request aborted')

    const reissued = cache.request(file, 198, 1_000)
    expect(loader).toHaveBeenCalledTimes(2)
    pending.resolve('viewer-image://late')
    await expect(reissued).resolves.toBe('viewer-image://late')
    await expect(cache.request(file, 198, 1_000)).resolves.toBe('viewer-image://late')
  })

  it('keeps unrelated callers resolved when one caller aborts a shared request', async () => {
    const pending = deferred<string>()
    const loader = vi.fn().mockImplementation(() => pending.promise)
    const cache = createProjectThumbnailCache('session-1', loader)
    const file = image('image-1')
    const aborting = new AbortController()

    const patient = cache.request(file, 198, 1_000)
    const cancellable = cache.request(file, 198, 1_000, aborting.signal)
    aborting.abort()
    await expect(cancellable).rejects.toThrow('thumbnail request aborted')

    pending.resolve('viewer-image://shared')
    await expect(patient).resolves.toBe('viewer-image://shared')
    await expect(cache.request(file, 198, 1_000)).resolves.toBe('viewer-image://shared')
    expect(loader).toHaveBeenCalledOnce()
  })

  it('prefetches through the loader with low priority and coalesces with interactive keys', async () => {
    const loader = vi.fn(
      async (
        _file: BrowserFile,
        _max: number,
        _scale: number,
        _signal?: AbortSignal,
        priority?: string,
      ) => `viewer-image://${priority ?? 'interactive'}`,
    )
    const cache = createProjectThumbnailCache('session-1', loader)
    const file = image('image-1')

    await expect(cache.prefetch(file, 198, 1_000)).resolves.toBe('viewer-image://prefetch')
    expect(loader).toHaveBeenCalledTimes(1)
    expect(loader.mock.calls[0]?.[4]).toBe('prefetch')

    // 同键命中 resolved 缓存：交互请求直接复用预取结果，不重复发起。
    await expect(cache.request(file, 198, 1_000)).resolves.toBe('viewer-image://prefetch')
    expect(loader).toHaveBeenCalledTimes(1)

    // 预取命中在途交互请求时共享同一个 pending，不重复发起。
    const inFlight = deferred<string>()
    const dedupeLoader = vi.fn().mockImplementation(() => inFlight.promise)
    const dedupeCache = createProjectThumbnailCache('session-2', dedupeLoader)
    void dedupeCache.request(file, 198, 1_000)
    void dedupeCache.prefetch(file, 198, 1_000)
    expect(dedupeLoader).toHaveBeenCalledOnce()
    inFlight.resolve('viewer-image://either')
  })
})
