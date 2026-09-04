import {
  assertProtocolInteger,
  ImageRendererReplayState,
  numericSessionId,
  OrderedCommandLane,
} from './imageRendererSession'
import type {
  ImageRendererAck,
  ImageRendererBackend,
  ImageRendererBridge,
  ImageRendererCommand,
  ImageRendererEvent,
  ImageRendererMigrationPolicy,
  ImageRendererPort,
  ImageRendererSession,
  OpenImageRendererRequest,
} from './imageRendererTypes'
import { NativeImageRenderer } from './nativeImageRenderer'
import { WebImageRenderer } from './webImageRenderer'

export type CreateImageRendererPortOptions = {
  bridge: ImageRendererBridge
  migrationPolicy: ImageRendererMigrationPolicy
}

export function createImageRendererPort({
  bridge,
  migrationPolicy,
}: CreateImageRendererPortOptions): ImageRendererPort {
  return new MigratingImageRendererPort(
    new NativeImageRenderer(bridge),
    new WebImageRenderer(migrationPolicy.legacyWeb),
    migrationPolicy.initialBackend,
  )
}

class MigratingImageRendererPort implements ImageRendererPort {
  private readonly openLane = new OrderedCommandLane()
  private readonly pendingOpens = new Map<string, Promise<ImageRendererSession>>()
  private readonly handlers = new Set<(event: ImageRendererEvent) => void>()
  private nativeMonitorPromise?: Promise<boolean>
  private webMonitorPromise?: Promise<void>
  private active?: MigratingImageRendererSession
  private opening?: OpeningRendererSession
  private readonly native: ImageRendererPort
  private readonly web: ImageRendererPort
  private readonly initialBackend: ImageRendererBackend

  constructor(
    native: ImageRendererPort,
    web: ImageRendererPort,
    initialBackend: ImageRendererBackend,
  ) {
    this.native = native
    this.web = web
    this.initialBackend = initialBackend
  }

  get backend(): ImageRendererBackend {
    return this.initialBackend
  }

  open(request: OpenImageRendererRequest): Promise<ImageRendererSession> {
    assertProtocolInteger(request.assetGeneration, 'assetGeneration', false)
    if (this.initialBackend === 'native') numericSessionId(request.sessionId)
    if (this.active?.matches(request) && !this.active.closed) return Promise.resolve(this.active)
    const key = openRequestKey(request)
    const pending = this.pendingOpens.get(key)
    if (pending !== undefined) return pending
    const opening = this.openLane.enqueue(() => this.openOrdered(request))
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

  private async openOrdered(request: OpenImageRendererRequest): Promise<ImageRendererSession> {
    await this.ensureWebMonitoring()
    let backend = this.initialBackend
    let opened: OpenedRendererSession
    if (backend === 'native') {
      if (await this.ensureNativeMonitoring()) {
        try {
          opened = await this.openBackend('native', this.native, request)
        } catch (error) {
          if (!isNativeInitializationError(error)) throw error
          backend = 'web'
          opened = await this.openBackend('web', this.web, request)
        }
      } else {
        backend = 'web'
        opened = await this.openBackend('web', this.web, request)
      }
    } else {
      opened = await this.openBackend('web', this.web, request)
    }
    const migrating = new MigratingImageRendererSession(
      request,
      opened.session,
      backend,
      this.web,
      (event) => this.receive('web', event),
      (closed) => {
        if (this.active === closed) this.active = undefined
      },
    )
    this.active = migrating
    this.finishOpening(opened.opening)
    return migrating
  }

  async listen(handler: (event: ImageRendererEvent) => void): Promise<() => void> {
    this.handlers.add(handler)
    try {
      await this.ensureWebMonitoring()
      if (this.initialBackend === 'native') await this.ensureNativeMonitoring()
      return () => this.handlers.delete(handler)
    } catch (error) {
      this.handlers.delete(handler)
      throw error
    }
  }

  private ensureNativeMonitoring(): Promise<boolean> {
    this.nativeMonitorPromise ??= this.native
      .listen((event) => this.receive('native', event))
      .then(
        () => true,
        () => false,
      )
    return this.nativeMonitorPromise
  }

  private ensureWebMonitoring(): Promise<void> {
    this.webMonitorPromise ??= this.web
      .listen((event) => this.receive('web', event))
      .then(() => undefined)
    return this.webMonitorPromise
  }

  private async openBackend(
    backend: ImageRendererBackend,
    port: ImageRendererPort,
    request: OpenImageRendererRequest,
  ): Promise<OpenedRendererSession> {
    const opening = new OpeningRendererSession(backend, request)
    this.opening = opening
    try {
      return { session: await port.open(request), opening }
    } catch (error) {
      if (this.opening === opening) this.opening = undefined
      throw error
    }
  }

  private finishOpening(opening: OpeningRendererSession): void {
    if (this.opening === opening) this.opening = undefined
    for (const event of opening.takeEvents()) this.receive(opening.backend, event)
  }

  private receive(source: ImageRendererBackend, event: ImageRendererEvent): void {
    if (this.opening?.accept(source, event)) return
    const active = this.active
    if (
      active === undefined ||
      active.sessionId !== event.sessionId ||
      active.assetGeneration !== event.assetGeneration
    ) {
      return
    }
    if (active.bufferTransitionEvent(source, event)) return
    if (active.backend !== source) return
    active.observe(event)
    for (const handler of this.handlers) handler(event)
  }
}

type OpenedRendererSession = {
  session: ImageRendererSession
  opening: OpeningRendererSession
}

class OpeningRendererSession {
  readonly backend: ImageRendererBackend
  private readonly events: ImageRendererEvent[] = []
  private readonly sessionId: string
  private readonly assetGeneration: number

  constructor(backend: ImageRendererBackend, request: OpenImageRendererRequest) {
    this.backend = backend
    this.sessionId = request.sessionId
    this.assetGeneration = request.assetGeneration
  }

  accept(source: ImageRendererBackend, event: ImageRendererEvent): boolean {
    if (
      source !== this.backend ||
      event.sessionId !== this.sessionId ||
      event.assetGeneration !== this.assetGeneration
    ) {
      return false
    }
    bufferOpeningEvent(this.events, event)
    return true
  }

  takeEvents(): ImageRendererEvent[] {
    return this.events.splice(0)
  }
}

class MigratingImageRendererSession implements ImageRendererSession {
  readonly sessionId: string
  readonly assetGeneration: number
  private readonly lane = new OrderedCommandLane()
  private readonly replay = new ImageRendererReplayState()
  private current: ImageRendererSession
  private currentBackend: ImageRendererBackend
  private recoveryFailures = 0
  private switchPromise?: Promise<void>
  private closing = false
  private closePromise?: Promise<void>
  private readonly request: OpenImageRendererRequest
  private readonly web: ImageRendererPort
  private readonly publishWebEvent: (event: ImageRendererEvent) => void
  private readonly onClosed: (session: MigratingImageRendererSession) => void
  private nativeInitialized: boolean
  private switchingToWeb = false
  private readonly transitionEvents: ImageRendererEvent[] = []

  constructor(
    request: OpenImageRendererRequest,
    session: ImageRendererSession,
    backend: ImageRendererBackend,
    web: ImageRendererPort,
    publishWebEvent: (event: ImageRendererEvent) => void,
    onClosed: (session: MigratingImageRendererSession) => void,
  ) {
    this.request = request
    this.web = web
    this.sessionId = request.sessionId
    this.assetGeneration = request.assetGeneration
    this.current = session
    this.currentBackend = backend
    this.nativeInitialized = backend !== 'native'
    this.publishWebEvent = publishWebEvent
    this.onClosed = onClosed
  }

  get backend(): ImageRendererBackend {
    return this.currentBackend
  }

  get closed(): boolean {
    return this.closing
  }

  matches(request: OpenImageRendererRequest): boolean {
    return (
      this.sessionId === request.sessionId &&
      this.assetGeneration === request.assetGeneration &&
      this.request.entityId === request.entityId
    )
  }

  dispatch(command: ImageRendererCommand): Promise<ImageRendererAck> {
    if (this.closing) return Promise.reject(new Error('image renderer session is closed'))
    return this.lane.enqueue(async () => {
      let ack: ImageRendererAck
      try {
        ack = await this.current.dispatch(command)
      } catch (error) {
        if (
          this.currentBackend !== 'native' ||
          this.nativeInitialized ||
          command.command.type !== 'set_surface' ||
          !isNativeInitializationError(error)
        ) {
          throw error
        }
        await this.switchToWebNow()
        ack = await this.current.dispatch(command)
      }
      if (
        this.currentBackend === 'native' &&
        command.command.type === 'set_surface' &&
        (ack.disposition === 'applied' || ack.disposition === 'ignored_duplicate')
      ) {
        this.nativeInitialized = true
      }
      if (ack.disposition === 'applied' || ack.disposition === 'ignored_duplicate') {
        this.replay.record(command)
      }
      return ack
    })
  }

  close(): Promise<void> {
    if (this.closePromise !== undefined) return this.closePromise
    this.closing = true
    this.closePromise = this.lane.enqueue(async () => {
      try {
        await this.current.close()
      } finally {
        this.onClosed(this)
      }
    })
    return this.closePromise
  }

  observe(event: ImageRendererEvent): void {
    if (event.type === 'frame_presented') {
      this.recoveryFailures = 0
      return
    }
    if (event.type !== 'failed' || this.currentBackend !== 'native') return
    if (!event.retryable) {
      this.recoveryFailures = 0
      return
    }
    this.recoveryFailures += 1
    if (this.recoveryFailures >= 2) void this.switchToWeb().catch(() => undefined)
  }

  bufferTransitionEvent(source: ImageRendererBackend, event: ImageRendererEvent): boolean {
    if (source !== 'web' || !this.switchingToWeb) return false
    bufferOpeningEvent(this.transitionEvents, event)
    return true
  }

  private switchToWeb(): Promise<void> {
    if (this.currentBackend === 'web') return Promise.resolve()
    this.switchPromise ??= this.lane.enqueue(() => this.switchToWebNow())
    this.switchPromise.catch(() => {
      this.switchPromise = undefined
    })
    return this.switchPromise
  }

  private async switchToWebNow(): Promise<void> {
    if (this.currentBackend === 'web' || this.closing) return
    this.switchingToWeb = true
    let webSession: ImageRendererSession | undefined
    try {
      webSession = await this.web.open(this.request)
      for (const command of this.replay.commands()) await webSession.dispatch(command)
      const nativeSession = this.current
      this.current = webSession
      this.currentBackend = 'web'
      this.nativeInitialized = true
      this.recoveryFailures = 0
      await nativeSession.close().catch(() => undefined)
    } catch (error) {
      this.transitionEvents.splice(0)
      await webSession?.close().catch(() => undefined)
      throw error
    } finally {
      this.switchingToWeb = false
    }
    this.publishWebEvent({
      type: 'backend_activated',
      sessionId: this.sessionId,
      assetGeneration: this.assetGeneration,
      backend: 'web',
    })
    for (const event of this.transitionEvents.splice(0)) this.publishWebEvent(event)
  }
}

const MAX_OPENING_EVENTS = 128

function bufferOpeningEvent(events: ImageRendererEvent[], event: ImageRendererEvent): void {
  if (events.length >= MAX_OPENING_EVENTS) {
    const disposable = events.findIndex(({ type }) =>
      ['frame_presented', 'camera_changed', 'draft_changed', 'editor_placement_changed'].includes(
        type,
      ),
    )
    if (disposable === -1) return
    events.splice(disposable, 1)
  }
  events.push(event)
}

function isNativeInitializationError(error: unknown): boolean {
  if (typeof error !== 'object' || error === null || !('code' in error)) return false
  return (
    error.code === 'image_render_driver_unavailable' || error.code === 'image_render_driver_failed'
  )
}

function openRequestKey(request: OpenImageRendererRequest): string {
  return JSON.stringify([request.sessionId, request.assetGeneration, request.entityId])
}
