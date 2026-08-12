import { act, renderHook, waitFor } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { BrowserFile, ImageRepresentation } from '../../api/types'
import { useCurrentOriginal } from './useCurrentOriginal'

function image(entityId: string): BrowserFile {
  return {
    entityId,
    relativePath: `${entityId}.jpg`,
    name: `${entityId}.jpg`,
    kind: 'jpeg',
    size: 100,
    modifiedNs: '1',
    marker: { reviewState: null, favorite: false },
    imageMetadata: null,
    imageUrl: null,
    videoMetadata: null,
  }
}

function representation(entityId: string): ImageRepresentation {
  return {
    cacheKey: entityId,
    url: `viewer-image://localhost/session/${entityId}`,
    width: 6000,
    height: 4000,
    backend: 'image_io',
  }
}

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason: unknown) => void
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise
    reject = rejectPromise
  })
  return { promise, resolve, reject }
}

describe('useCurrentOriginal', () => {
  it('requests the current original automatically once it is available', async () => {
    const requestImage = vi.fn(async () => representation('one'))
    const hook = renderHook(
      ({ available }) => useCurrentOriginal({ file: image('one'), available, requestImage }),
      { initialProps: { available: false } },
    )

    expect(hook.result.current).toEqual({
      status: 'idle',
      entityId: 'one',
      representation: null,
    })
    expect(requestImage).not.toHaveBeenCalled()

    hook.rerender({ available: true })
    await waitFor(() => expect(hook.result.current.status).toBe('ready'))
    expect(requestImage).toHaveBeenCalledTimes(1)
    expect(requestImage).toHaveBeenCalledWith(
      expect.objectContaining({ entityId: 'one' }),
      { kind: 'original100_percent' },
      expect.any(AbortSignal),
    )
  })

  it('deduplicates an identity and prevents a stale completion from replacing the next image', async () => {
    const first = deferred<ImageRepresentation>()
    const second = deferred<ImageRepresentation>()
    const signals = new Map<string, AbortSignal>()
    const requestImage = vi.fn((file: BrowserFile, _request: unknown, signal?: AbortSignal) => {
      if (signal) signals.set(file.entityId, signal)
      return file.entityId === 'one' ? first.promise : second.promise
    })
    const hook = renderHook(
      ({ file }) => useCurrentOriginal({ file, available: true, requestImage }),
      { initialProps: { file: image('one') } },
    )
    await waitFor(() => expect(hook.result.current.status).toBe('loading'))

    hook.rerender({ file: { ...image('one'), name: 'renamed.jpg' } })
    expect(requestImage).toHaveBeenCalledTimes(1)

    hook.rerender({ file: image('two') })
    await waitFor(() => expect(requestImage).toHaveBeenCalledTimes(2))
    expect(signals.get('one')?.aborted).toBe(true)
    expect(hook.result.current).toEqual({
      status: 'loading',
      entityId: 'two',
      representation: null,
    })

    await act(async () => first.resolve(representation('one')))
    expect(hook.result.current.entityId).toBe('two')
    expect(hook.result.current.representation).toBeNull()

    await act(async () => second.resolve(representation('two')))
    expect(hook.result.current).toEqual({
      status: 'ready',
      entityId: 'two',
      representation: representation('two'),
    })

    hook.rerender({ file: { ...image('two'), size: 200 } })
    expect(requestImage).toHaveBeenCalledTimes(2)
  })

  it('aborts on unavailability and unmount', async () => {
    const requests: AbortSignal[] = []
    const requestImage = vi.fn((_file: BrowserFile, _request: unknown, signal?: AbortSignal) => {
      if (signal) requests.push(signal)
      return new Promise<ImageRepresentation>(() => undefined)
    })
    const hook = renderHook(
      ({ available }) => useCurrentOriginal({ file: image('one'), available, requestImage }),
      { initialProps: { available: true } },
    )
    await waitFor(() => expect(requests).toHaveLength(1))

    hook.rerender({ available: false })
    expect(requests[0]?.aborted).toBe(true)
    expect(hook.result.current.status).toBe('idle')

    hook.rerender({ available: true })
    await waitFor(() => expect(requests).toHaveLength(2))
    hook.unmount()
    expect(requests[1]?.aborted).toBe(true)
  })

  it.each([
    [{ code: 'image_budget_exceeded' }, 'budget_error'],
    [new Error('decode failed'), 'error'],
    [new DOMException('aborted', 'AbortError'), 'idle'],
  ] as const)(
    'maps a rejected original to %s without leaking the prior failure',
    async (error, status) => {
      const next = deferred<ImageRepresentation>()
      const requestImage = vi
        .fn()
        .mockRejectedValueOnce(error)
        .mockImplementationOnce(() => next.promise)
      const hook = renderHook(
        ({ file }) => useCurrentOriginal({ file, available: true, requestImage }),
        { initialProps: { file: image('one') } },
      )

      await waitFor(() => expect(hook.result.current.status).toBe(status))
      hook.rerender({ file: image('two') })
      await waitFor(() => expect(hook.result.current.status).toBe('loading'))
      expect(hook.result.current.entityId).toBe('two')

      await act(async () => next.resolve(representation('two')))
      expect(hook.result.current.status).toBe('ready')
    },
  )
})
