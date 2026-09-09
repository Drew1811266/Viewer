import { assertProtocolInteger, numericSessionId, OrderedCommandLane } from './imageRendererSession'
import type {
  ImageRenderCommandEnvelope,
  ImageRendererAck,
  ImageRendererBridge,
  ImageRendererCommand,
  ImageRendererEvent,
  ImageRendererPort,
  ImageRendererSession,
  OpenImageRendererRequest,
} from './imageRendererTypes'

export class NativeImageRenderer implements ImageRendererPort {
  readonly backend = 'native' as const
  private readonly activeGenerations = new Map<string, number>()
  private readonly activeSessions = new Map<string, ImageRendererSession>()
  private readonly pendingOpens = new Map<string, Promise<ImageRendererSession>>()
  private readonly bridge: ImageRendererBridge

  constructor(bridge: ImageRendererBridge) {
    this.bridge = bridge
  }

  async open(request: OpenImageRendererRequest): Promise<ImageRendererSession> {
    numericSessionId(request.sessionId)
    const key = openRequestKey(request)
    const active = this.activeSessions.get(key)
    if (active !== undefined) return active
    const pending = this.pendingOpens.get(key)
    if (pending !== undefined) return pending
    const previousGeneration = this.activeGenerations.get(request.sessionId)
    this.activeGenerations.set(request.sessionId, request.assetGeneration)
    const opening = (async () => {
      const session = new NativeImageRendererSession(this.bridge, request, () => {
        if (this.activeGenerations.get(request.sessionId) === request.assetGeneration) {
          this.activeGenerations.delete(request.sessionId)
        }
        this.activeSessions.delete(key)
      })
      try {
        await session.open()
        this.activeSessions.set(key, session)
        return session
      } catch (error) {
        if (this.activeGenerations.get(request.sessionId) === request.assetGeneration) {
          if (previousGeneration === undefined) this.activeGenerations.delete(request.sessionId)
          else this.activeGenerations.set(request.sessionId, previousGeneration)
        }
        throw error
      }
    })()
    this.pendingOpens.set(key, opening)
    opening.then(
      () => {
        if (this.pendingOpens.get(key) === opening) this.pendingOpens.delete(key)
      },
      () => {
        if (this.pendingOpens.get(key) === opening) this.pendingOpens.delete(key)
      },
    )
    return opening
  }

  async listen(handler: (event: ImageRendererEvent) => void): Promise<() => void> {
    return this.bridge.listenImageRender((event) => {
      if (this.activeGenerations.get(event.sessionId) === event.assetGeneration) handler(event)
    })
  }
}

function openRequestKey(request: OpenImageRendererRequest): string {
  return JSON.stringify([request.sessionId, request.assetGeneration, request.entityId])
}

class NativeImageRendererSession implements ImageRendererSession {
  readonly backend = 'native' as const
  readonly sessionId: string
  readonly assetGeneration: number
  private readonly numericId: number
  private readonly lane = new OrderedCommandLane()
  private commandId = 0
  private acceptedRevision = 0
  private closing = false
  private closePromise?: Promise<void>
  private readonly bridge: ImageRendererBridge
  private readonly request: OpenImageRendererRequest
  private readonly onClose: () => void

  constructor(bridge: ImageRendererBridge, request: OpenImageRendererRequest, onClose: () => void) {
    this.bridge = bridge
    this.request = request
    this.onClose = onClose
    this.sessionId = request.sessionId
    this.assetGeneration = request.assetGeneration
    this.numericId = numericSessionId(request.sessionId)
  }

  async open(): Promise<void> {
    const ack = await this.send(0, { type: 'open', entityId: this.request.entityId })
    assertProtocolInteger(ack.acceptedRevision, 'acceptedRevision')
    this.acceptedRevision = ack.acceptedRevision
  }

  dispatch(command: ImageRendererCommand): Promise<ImageRendererAck> {
    if (this.closing) return Promise.reject(new Error('image renderer session is closed'))
    return this.lane.enqueue(async () => {
      assertProtocolInteger(command.sceneRevision, 'sceneRevision')
      if (command.sceneRevision < this.acceptedRevision) {
        throw new Error('image renderer command revision regressed')
      }
      const ack = await this.send(command.sceneRevision, command.command)
      assertProtocolInteger(ack.acceptedRevision, 'acceptedRevision')
      this.acceptedRevision = Math.max(this.acceptedRevision, ack.acceptedRevision)
      return ack
    })
  }

  close(): Promise<void> {
    if (this.closePromise !== undefined) return this.closePromise
    this.closing = true
    this.closePromise = this.lane.enqueue(async () => {
      await this.send(this.acceptedRevision, { type: 'close' })
      this.onClose()
    })
    void this.closePromise.catch(() => {
      this.closePromise = undefined
    })
    return this.closePromise
  }

  private send(
    sceneRevision: number,
    command: ImageRenderCommandEnvelope['command'],
  ): Promise<ImageRendererAck> {
    this.commandId += 1
    return this.bridge.imageRenderCommand({
      sessionId: this.numericId,
      assetGeneration: this.assetGeneration,
      sceneRevision,
      commandId: this.commandId,
      command,
    })
  }
}
