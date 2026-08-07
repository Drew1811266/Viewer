import { describe, expect, it } from 'vitest'
import { createCompareOriginalRequestScheduler } from './compareOriginalRequestScheduler'

describe('compareOriginalRequestScheduler', () => {
  it('starts at most two jobs and drains every queued job in FIFO order', async () => {
    const scheduler = createCompareOriginalRequestScheduler<string>(2)
    const first = deferred<string>()
    const second = deferred<string>()
    const third = deferred<string>()
    const fourth = deferred<string>()
    const started: string[] = []

    const jobs = [
      scheduler.enqueue('a:1', () => {
        started.push('a')
        return first.promise
      }),
      scheduler.enqueue('b:1', () => {
        started.push('b')
        return second.promise
      }),
      scheduler.enqueue('c:1', () => {
        started.push('c')
        return third.promise
      }),
      scheduler.enqueue('d:1', () => {
        started.push('d')
        return fourth.promise
      }),
    ]

    expect(started).toEqual(['a', 'b'])
    first.resolve('first')
    await expect(jobs[0]).resolves.toBe('first')
    await Promise.resolve()
    expect(started).toEqual(['a', 'b', 'c'])
    second.resolve('second')
    await expect(jobs[1]).resolves.toBe('second')
    await Promise.resolve()
    expect(started).toEqual(['a', 'b', 'c', 'd'])

    third.resolve('third')
    fourth.resolve('fourth')
    await expect(Promise.all(jobs)).resolves.toEqual(['first', 'second', 'third', 'fourth'])
    scheduler.dispose()
  })

  it('coalesces a duplicate key until the shared job settles', async () => {
    const scheduler = createCompareOriginalRequestScheduler<string>(2)
    const original = deferred<string>()
    let starts = 0
    const first = scheduler.enqueue('same:1', () => {
      starts += 1
      return original.promise
    })
    const duplicate = scheduler.enqueue('same:1', () => {
      starts += 1
      return Promise.resolve('duplicate')
    })

    expect(duplicate).toBe(first)
    expect(starts).toBe(1)
    original.resolve('original')
    await expect(duplicate).resolves.toBe('original')
    scheduler.dispose()
  })

  it('removes an aborted queued job without starving the following job', async () => {
    const scheduler = createCompareOriginalRequestScheduler<string>(1)
    const first = deferred<string>()
    const third = deferred<string>()
    const queuedController = new AbortController()
    const started: string[] = []

    const running = scheduler.enqueue('a:1', () => {
      started.push('a')
      return first.promise
    })
    const cancelled = scheduler.enqueue(
      'b:1',
      () => {
        started.push('b')
        return Promise.resolve('second')
      },
      queuedController.signal,
    )
    const following = scheduler.enqueue('c:1', () => {
      started.push('c')
      return third.promise
    })

    const cancelledResult = expect(cancelled).rejects.toEqual({ code: 'image_request_cancelled' })
    queuedController.abort()
    await cancelledResult
    expect(started).toEqual(['a'])

    first.resolve('first')
    await expect(running).resolves.toBe('first')
    await Promise.resolve()
    expect(started).toEqual(['a', 'c'])
    third.resolve('third')
    await expect(following).resolves.toBe('third')
    scheduler.dispose()
  })

  it('rejects running, queued, and future jobs when disposed', async () => {
    const scheduler = createCompareOriginalRequestScheduler<string>(1)
    const runningSource = deferred<string>()
    const running = scheduler.enqueue('a:1', () => runningSource.promise)
    const queued = scheduler.enqueue('b:1', () => Promise.resolve('second'))
    const runningResult = expect(running).rejects.toEqual({ code: 'image_request_cancelled' })
    const queuedResult = expect(queued).rejects.toEqual({ code: 'image_request_cancelled' })

    scheduler.dispose()

    await runningResult
    await queuedResult
    await expect(scheduler.enqueue('c:1', () => Promise.resolve('third'))).rejects.toEqual({
      code: 'image_request_cancelled',
    })
    runningSource.resolve('late')
  })
})

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((accept, decline) => {
    resolve = accept
    reject = decline
  })
  return { promise, resolve, reject }
}
