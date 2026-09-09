import type { ImageRendererSession } from '../../rendering/imageRendererTypes'

export type NativeCloseBarrier = () => Promise<void>

/** A failed teardown remains owned and retryable until it is acknowledged. */
export function nativeCloseBarrier(
  previous: NativeCloseBarrier,
  initialization: Promise<ImageRendererSession | undefined>,
): NativeCloseBarrier {
  let prior: NativeCloseBarrier | undefined = previous
  let initializing: Promise<ImageRendererSession | undefined> | undefined = initialization
  let target: ImageRendererSession | undefined
  let attempt: Promise<void> | undefined
  return () => {
    if (attempt !== undefined) return attempt
    attempt = (async () => {
      if (prior !== undefined) {
        await prior()
        prior = undefined
      }
      if (initializing !== undefined) {
        target = await initializing
        initializing = undefined
      }
      await target?.close()
      target = undefined
    })()
    void attempt.catch(() => {
      // Keep the failed session (or prior barrier), not a permanently rejected
      // promise. A subsequent explicit retry must actually finish its Close.
      attempt = undefined
    })
    return attempt
  }
}
