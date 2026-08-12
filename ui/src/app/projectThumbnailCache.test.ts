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
})
