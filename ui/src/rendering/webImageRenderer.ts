import { assertProtocolInteger, OrderedCommandLane } from './imageRendererSession'
import type {
  ImageRendererAck,
  ImageRendererCommand,
  ImageRendererEvent,
  ImageRendererPort,
  ImageRendererSession,
  LegacyWebImageRendererAdapter,
  LegacyWebImageRendererSession,
  OpenImageRendererRequest,
} from './imageRendererTypes'

export class WebImageRenderer implements ImageRendererPort {
  readonly backend = 'web' as const
  private readonly activeGenerations = new Map<string, number>()
  private readonly legacy: LegacyWebImageRendererAdapter

  constructor(legacy: LegacyWebImageRendererAdapter) {
    this.legacy = legacy
  }

  async open(request: OpenImageRendererRequest): Promise<ImageRendererSession> {
    const previousGeneration = this.activeGenerations.get(request.sessionId)
    this.activeGenerations.set(request.sessionId, request.assetGeneration)
    let legacy: LegacyWebImageRendererSession
    try {
      legacy = await this.legacy.open(request)
    } catch (error) {
      if (this.activeGenerations.get(request.sessionId) === request.assetGeneration) {
        if (previousGeneration === undefined) this.activeGenerations.delete(request.sessionId)
        else this.activeGenerations.set(request.sessionId, previousGeneration)
      }
      throw error
    }
    const session = new WebImageRendererSession(request, legacy, () => {
      if (this.activeGenerations.get(request.sessionId) === request.assetGeneration) {
        this.activeGenerations.delete(request.sessionId)
      }
    })
    return session
  }

  async listen(handler: (event: ImageRendererEvent) => void): Promise<() => void> {
    return this.legacy.listen((event) => {
      if (this.activeGenerations.get(event.sessionId) === event.assetGeneration) handler(event)
    })
  }
}

class WebImageRendererSession implements ImageRendererSession {
  readonly sessionId: string
  readonly assetGeneration: number
  private readonly lane = new OrderedCommandLane()
  private acceptedRevision = 0
  private closing = false
  private closePromise?: Promise<void>
  private readonly legacy: LegacyWebImageRendererSession
  private readonly onClose: () => void

  constructor(
    request: OpenImageRendererRequest,
    legacy: LegacyWebImageRendererSession,
    onClose: () => void,
  ) {
    this.legacy = legacy
    this.onClose = onClose
    this.sessionId = request.sessionId
    this.assetGeneration = request.assetGeneration
  }

  dispatch(command: ImageRendererCommand): Promise<ImageRendererAck> {
    if (this.closing) return Promise.reject(new Error('image renderer session is closed'))
    return this.lane.enqueue(async () => {
      assertProtocolInteger(command.sceneRevision, 'sceneRevision')
      if (command.sceneRevision < this.acceptedRevision) {
        throw new Error('image renderer command revision regressed')
      }
      await this.legacy.dispatch(command)
      this.acceptedRevision = command.sceneRevision
      return {
        disposition: 'applied',
        acceptedRevision: this.acceptedRevision,
        backend: 'web',
      }
    })
  }

  close(): Promise<void> {
    if (this.closePromise !== undefined) return this.closePromise
    this.closing = true
    this.closePromise = this.lane.enqueue(async () => {
      try {
        await this.legacy.close()
      } finally {
        this.onClose()
      }
    })
    return this.closePromise
  }
}
