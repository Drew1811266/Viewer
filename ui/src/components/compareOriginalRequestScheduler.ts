export interface CompareOriginalRequestScheduler<T> {
  enqueue(key: string, run: () => Promise<T>, signal?: AbortSignal): Promise<T>
  dispose(): void
}

interface OriginalRequestJob<T> {
  key: string
  run: () => Promise<T>
  signal?: AbortSignal
  promise: Promise<T>
  resolve: (value: T) => void
  reject: (reason: unknown) => void
  running: boolean
  settled: boolean
  removeAbortListener: () => void
}

type JobResult<T> = { value: T } | { reason: unknown }

export function createCompareOriginalRequestScheduler<T>(
  maxConcurrent = 2,
): CompareOriginalRequestScheduler<T> {
  if (!Number.isInteger(maxConcurrent) || maxConcurrent < 1) {
    throw new Error('Original request concurrency must be a positive integer')
  }

  let disposed = false
  let running = 0
  const queued: OriginalRequestJob<T>[] = []
  const jobs = new Map<string, OriginalRequestJob<T>>()

  function finish(job: OriginalRequestJob<T>, result: JobResult<T>) {
    if (job.settled) return
    job.settled = true
    job.removeAbortListener()
    if (jobs.get(job.key) === job) jobs.delete(job.key)
    if ('value' in result) job.resolve(result.value)
    else job.reject(result.reason)
  }

  function pump() {
    while (!disposed && running < maxConcurrent && queued.length > 0) {
      const job = queued.shift()
      if (job === undefined) return
      if (job.settled) continue
      if (job.signal?.aborted) {
        finish(job, { reason: cancelledImageRequest() })
        continue
      }

      job.running = true
      running += 1
      let operation: Promise<T>
      try {
        operation = job.run()
      } catch (reason: unknown) {
        running -= 1
        finish(job, { reason })
        continue
      }
      void operation
        .then(
          (value) =>
            finish(job, job.signal?.aborted ? { reason: cancelledImageRequest() } : { value }),
          (reason: unknown) =>
            finish(job, {
              reason: job.signal?.aborted ? cancelledImageRequest() : reason,
            }),
        )
        .finally(() => {
          running -= 1
          pump()
        })
    }
  }

  return {
    enqueue(key, run, signal) {
      const existing = jobs.get(key)
      if (existing !== undefined) return existing.promise
      if (disposed || signal?.aborted) return Promise.reject(cancelledImageRequest())

      let resolve!: (value: T) => void
      let reject!: (reason: unknown) => void
      const promise = new Promise<T>((accept, decline) => {
        resolve = accept
        reject = decline
      })
      const job: OriginalRequestJob<T> = {
        key,
        run,
        signal,
        promise,
        resolve,
        reject,
        running: false,
        settled: false,
        removeAbortListener: () => undefined,
      }
      const abort = () => {
        const queuedIndex = queued.indexOf(job)
        if (!job.running && queuedIndex >= 0) queued.splice(queuedIndex, 1)
        finish(job, { reason: cancelledImageRequest() })
        pump()
      }
      job.removeAbortListener = () => signal?.removeEventListener('abort', abort)
      signal?.addEventListener('abort', abort, { once: true })
      jobs.set(key, job)
      queued.push(job)
      pump()
      return promise
    },
    dispose() {
      if (disposed) return
      disposed = true
      for (const job of [...jobs.values()]) {
        finish(job, { reason: cancelledImageRequest() })
      }
      queued.length = 0
    },
  }
}

function cancelledImageRequest() {
  return { code: 'image_request_cancelled' as const }
}
