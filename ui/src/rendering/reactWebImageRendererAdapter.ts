import type {
  LegacyWebImageRendererAdapter,
  LegacyWebImageRendererSession,
} from './imageRendererTypes'

/**
 * The migration adapter reserves the ordered renderer command lane while the
 * existing React Web viewport remains the visual implementation. It carries
 * no image bytes and therefore cannot become a second geometry authority.
 */
export function createReactWebImageRendererAdapter(): LegacyWebImageRendererAdapter {
  return {
    async open(): Promise<LegacyWebImageRendererSession> {
      let closed = false
      return {
        async dispatch() {
          if (closed) throw new Error('legacy Web image renderer session is closed')
        },
        async close() {
          closed = true
        },
      }
    },
    async listen() {
      return () => undefined
    },
  }
}
