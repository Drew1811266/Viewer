import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createImageRendererPort } from './imageRendererPort'
import type {
  ImageRenderCommandEnvelope,
  ImageRendererBridge,
  ImageRendererCommand,
  ImageRendererEvent,
  LegacyWebImageRendererAdapter,
  LegacyWebImageRendererSession,
} from './imageRendererTypes'

function applied(revision: number, backend: 'native' | 'web' = 'native') {
  return { disposition: 'applied' as const, acceptedRevision: revision, backend }
}

function commandError(code: string) {
  return {
    code,
    category: code.includes('driver') ? 'environment' : 'validation',
    userMessage: 'test error',
    retryable: code.includes('driver'),
    taskId: null,
    itemId: null,
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

function fixture() {
  let nativeHandler: ((event: ImageRendererEvent) => void) | undefined
  let webHandler: ((event: ImageRendererEvent) => void) | undefined
  const imageRenderCommand = vi.fn(async (command: ImageRenderCommandEnvelope) =>
    applied(command.sceneRevision),
  )
  const bridge: ImageRendererBridge = {
    imageRenderCommand,
    listenImageRender: vi.fn(async (handler) => {
      nativeHandler = handler
      return vi.fn()
    }),
  }
  const webSession: LegacyWebImageRendererSession = {
    dispatch: vi.fn(async () => undefined),
    close: vi.fn(async () => undefined),
  }
  const legacyWeb: LegacyWebImageRendererAdapter = {
    open: vi.fn(async () => webSession),
    listen: vi.fn(async (handler) => {
      webHandler = handler
      return vi.fn()
    }),
  }
  const port = createImageRendererPort({
    bridge,
    migrationPolicy: { initialBackend: 'native', legacyWeb },
  })
  return {
    port,
    bridge,
    imageRenderCommand,
    listenImageRender: bridge.listenImageRender,
    legacyWeb,
    webSession,
    emitNative: (event: ImageRendererEvent) => nativeHandler?.(event),
    emitWeb: (event: ImageRendererEvent) => webHandler?.(event),
  }
}

const openRequest = {
  sessionId: '41',
  entityId: '00000000-0000-4000-8000-000000000007',
  assetGeneration: 1,
}

const surface: ImageRendererCommand = {
  sceneRevision: 0,
  command: {
    type: 'set_surface',
    surface: { left: 10, top: 20, width: 900, height: 600, scaleFactor: 2 },
  },
}

beforeEach(() => {
  vi.restoreAllMocks()
})

describe('ImageRendererPort', () => {
  it('returns the active session when the same asset open is repeated', async () => {
    const { port, imageRenderCommand } = fixture()

    const first = await port.open(openRequest)
    const second = await port.open({ ...openRequest })

    expect(second).toBe(first)
    expect(imageRenderCommand).toHaveBeenCalledOnce()
  })

  it('serializes native open, commands, revisions and idempotent close', async () => {
    const { port, imageRenderCommand } = fixture()
    const session = await port.open(openRequest)

    await session.dispatch(surface)
    await session.dispatch({
      sceneRevision: 1,
      command: { type: 'set_scene', scene: { annotations: [], draft: null } },
    })
    await session.close()
    await session.close()

    expect(port.backend).toBe('native')
    expect(imageRenderCommand.mock.calls.map(([envelope]) => envelope)).toEqual([
      {
        sessionId: 41,
        assetGeneration: 1,
        sceneRevision: 0,
        commandId: 1,
        command: { type: 'open', entityId: openRequest.entityId },
      },
      { ...surface, sessionId: 41, assetGeneration: 1, commandId: 2 },
      {
        sessionId: 41,
        assetGeneration: 1,
        sceneRevision: 1,
        commandId: 3,
        command: { type: 'set_scene', scene: { annotations: [], draft: null } },
      },
      {
        sessionId: 41,
        assetGeneration: 1,
        sceneRevision: 1,
        commandId: 4,
        command: { type: 'close' },
      },
    ])
  })

  it('rejects a client-side revision regression before crossing the bridge', async () => {
    const { port, imageRenderCommand } = fixture()
    const session = await port.open(openRequest)
    await session.dispatch({
      sceneRevision: 2,
      command: { type: 'set_scene', scene: { annotations: [], draft: null } },
    })

    await expect(session.dispatch(surface)).rejects.toThrow('revision')
    expect(imageRenderCommand).toHaveBeenCalledTimes(2)
  })

  it('rejects malformed revisions before crossing the bridge', async () => {
    const { port, imageRenderCommand } = fixture()
    const session = await port.open(openRequest)

    await expect(session.dispatch({ ...surface, sceneRevision: Number.NaN })).rejects.toThrow(
      'sceneRevision',
    )
    expect(imageRenderCommand).toHaveBeenCalledTimes(1)
  })

  it('forwards only events for the active session generation', async () => {
    const { port, emitNative } = fixture()
    const handler = vi.fn()
    await port.listen(handler)
    await port.open(openRequest)

    emitNative({ type: 'ready', sessionId: '99', assetGeneration: 1, width: 10, height: 10 })
    emitNative({ type: 'ready', sessionId: '41', assetGeneration: 9, width: 10, height: 10 })
    emitNative({ type: 'ready', sessionId: '41', assetGeneration: 1, width: 900, height: 600 })

    expect(handler).toHaveBeenCalledOnce()
    expect(handler).toHaveBeenCalledWith({
      type: 'ready',
      sessionId: '41',
      assetGeneration: 1,
      width: 900,
      height: 600,
    })
  })

  it('buffers a matching native event until its opening session becomes active', async () => {
    const pendingOpen = deferred<ReturnType<typeof applied>>()
    const { port, imageRenderCommand, emitNative } = fixture()
    imageRenderCommand.mockReturnValueOnce(pendingOpen.promise)
    const handler = vi.fn()
    await port.listen(handler)

    const opening = port.open(openRequest)
    await vi.waitFor(() => expect(imageRenderCommand).toHaveBeenCalledOnce())
    emitNative({ type: 'ready', sessionId: '41', assetGeneration: 1, width: 900, height: 600 })
    pendingOpen.resolve(applied(0))
    await opening

    expect(handler).toHaveBeenCalledWith({
      type: 'ready',
      sessionId: '41',
      assetGeneration: 1,
      width: 900,
      height: 600,
    })
  })

  it('buffers a matching Web event until its fallback session becomes active', async () => {
    const pendingWebOpen = deferred<LegacyWebImageRendererSession>()
    const { port, imageRenderCommand, legacyWeb, webSession, emitWeb } = fixture()
    imageRenderCommand.mockRejectedValueOnce(commandError('image_render_driver_unavailable'))
    vi.mocked(legacyWeb.open).mockReturnValueOnce(pendingWebOpen.promise)
    const handler = vi.fn()
    await port.listen(handler)

    const opening = port.open(openRequest)
    await vi.waitFor(() => expect(legacyWeb.open).toHaveBeenCalledOnce())
    emitWeb({ type: 'ready', sessionId: '41', assetGeneration: 1, width: 900, height: 600 })
    pendingWebOpen.resolve(webSession)
    await opening

    expect(handler).toHaveBeenCalledWith({
      type: 'ready',
      sessionId: '41',
      assetGeneration: 1,
      width: 900,
      height: 600,
    })
  })

  it('switches to the complete legacy Web renderer when native initialization fails', async () => {
    const { port, imageRenderCommand, legacyWeb, webSession } = fixture()
    imageRenderCommand.mockRejectedValueOnce(commandError('image_render_driver_unavailable'))

    const session = await port.open(openRequest)
    await session.dispatch(surface)
    await session.close()
    await session.close()

    expect(port.backend).toBe('native')
    expect(session.backend).toBe('web')
    expect(legacyWeb.open).toHaveBeenCalledWith(openRequest)
    expect(webSession.close).toHaveBeenCalledOnce()
  })

  it('does not bypass native authorization or protocol failures through Web fallback', async () => {
    const { port, imageRenderCommand, legacyWeb } = fixture()
    const unauthorized = commandError('image_render_unauthorized_entity')
    imageRenderCommand.mockRejectedValueOnce(unauthorized)

    await expect(port.open(openRequest)).rejects.toEqual(unauthorized)
    expect(legacyWeb.open).not.toHaveBeenCalled()
  })

  it('falls back when the first surface command reveals native driver initialization failure', async () => {
    const { port, imageRenderCommand, legacyWeb, webSession } = fixture()
    imageRenderCommand.mockImplementation(async (command) => {
      if (command.command.type === 'set_surface') {
        throw commandError('image_render_driver_failed')
      }
      return applied(command.sceneRevision)
    })
    const session = await port.open(openRequest)

    const ack = await session.dispatch(surface)

    expect(ack).toEqual(applied(0, 'web'))
    expect(port.backend).toBe('native')
    expect(session.backend).toBe('web')
    expect(legacyWeb.open).toHaveBeenCalledWith(openRequest)
    expect(webSession.dispatch).toHaveBeenCalledWith(surface)
  })

  it('uses the complete Web renderer when native event transport initialization fails', async () => {
    const { port, listenImageRender, legacyWeb } = fixture()
    vi.mocked(listenImageRender).mockRejectedValueOnce(new Error('native events unavailable'))

    const session = await port.open(openRequest)
    await session.dispatch(surface)

    expect(port.backend).toBe('native')
    expect(session.backend).toBe('web')
    expect(legacyWeb.open).toHaveBeenCalledWith(openRequest)
  })

  it('does not count a non-retryable failure toward consecutive recovery failures', async () => {
    const { port, emitNative, legacyWeb } = fixture()
    await port.open(openRequest)

    emitNative({
      type: 'failed',
      sessionId: '41',
      assetGeneration: 1,
      code: 'resource_recovery_failed',
      retryable: true,
    })
    emitNative({
      type: 'failed',
      sessionId: '41',
      assetGeneration: 1,
      code: 'invalid_scene',
      retryable: false,
    })
    emitNative({
      type: 'failed',
      sessionId: '41',
      assetGeneration: 1,
      code: 'resource_recovery_failed',
      retryable: true,
    })
    await Promise.resolve()

    expect(port.backend).toBe('native')
    expect(legacyWeb.open).not.toHaveBeenCalled()
  })

  it('keeps native after one recovery failure and switches after two consecutive failures', async () => {
    const { port, emitNative, legacyWeb, webSession } = fixture()
    await port.open(openRequest)
    const session = await port.open({ ...openRequest, assetGeneration: 2 })
    await session.dispatch(surface)

    emitNative({
      type: 'failed',
      sessionId: '41',
      assetGeneration: 2,
      code: 'resource_recovery_failed',
      retryable: true,
    })
    await Promise.resolve()
    expect(port.backend).toBe('native')
    expect(legacyWeb.open).not.toHaveBeenCalled()

    emitNative({
      type: 'failed',
      sessionId: '41',
      assetGeneration: 2,
      code: 'resource_recovery_failed',
      retryable: true,
    })
    await vi.waitFor(() => expect(session.backend).toBe('web'))
    expect(port.backend).toBe('native')
    expect(legacyWeb.open).toHaveBeenCalledWith({ ...openRequest, assetGeneration: 2 })
    expect(webSession.dispatch).toHaveBeenCalledWith(surface)
  })

  it('does not treat decode readiness as a successfully presented recovery frame', async () => {
    const { port, emitNative, legacyWeb } = fixture()
    const session = await port.open(openRequest)
    await session.dispatch(surface)

    emitNative({
      type: 'failed',
      sessionId: '41',
      assetGeneration: 1,
      code: 'resource_recovery_failed',
      retryable: true,
    })
    emitNative({
      type: 'ready',
      sessionId: '41',
      assetGeneration: 1,
      width: 900,
      height: 600,
    })
    emitNative({
      type: 'failed',
      sessionId: '41',
      assetGeneration: 1,
      code: 'resource_recovery_failed',
      retryable: true,
    })

    await vi.waitFor(() => expect(session.backend).toBe('web'))
    expect(legacyWeb.open).toHaveBeenCalledWith(openRequest)
  })

  it('buffers Web events emitted while a recovery fallback is opening', async () => {
    const pendingWebOpen = deferred<LegacyWebImageRendererSession>()
    const { port, emitNative, emitWeb, legacyWeb, webSession } = fixture()
    const handler = vi.fn()
    await port.listen(handler)
    const session = await port.open(openRequest)
    await session.dispatch(surface)
    vi.mocked(legacyWeb.open).mockReturnValueOnce(pendingWebOpen.promise)

    emitNative({
      type: 'failed',
      sessionId: '41',
      assetGeneration: 1,
      code: 'resource_recovery_failed',
      retryable: true,
    })
    emitNative({
      type: 'failed',
      sessionId: '41',
      assetGeneration: 1,
      code: 'resource_recovery_failed',
      retryable: true,
    })
    await vi.waitFor(() => expect(legacyWeb.open).toHaveBeenCalledOnce())
    emitWeb({ type: 'ready', sessionId: '41', assetGeneration: 1, width: 900, height: 600 })
    pendingWebOpen.resolve(webSession)
    await vi.waitFor(() => expect(session.backend).toBe('web'))

    expect(handler).toHaveBeenCalledWith({
      type: 'ready',
      sessionId: '41',
      assetGeneration: 1,
      width: 900,
      height: 600,
    })
  })

  it('keeps Web transition events ordered while the native session is still closing', async () => {
    const pendingWebOpen = deferred<LegacyWebImageRendererSession>()
    const pendingNativeClose = deferred<ReturnType<typeof applied>>()
    const { port, emitNative, emitWeb, imageRenderCommand, legacyWeb, webSession } = fixture()
    imageRenderCommand.mockImplementation(async (command) => {
      if (command.command.type === 'close') return pendingNativeClose.promise
      return applied(command.sceneRevision)
    })
    vi.mocked(legacyWeb.open).mockReturnValueOnce(pendingWebOpen.promise)
    const handler = vi.fn()
    await port.listen(handler)
    const session = await port.open(openRequest)
    await session.dispatch(surface)

    emitNative({
      type: 'failed',
      sessionId: '41',
      assetGeneration: 1,
      code: 'resource_recovery_failed',
      retryable: true,
    })
    emitNative({
      type: 'failed',
      sessionId: '41',
      assetGeneration: 1,
      code: 'resource_recovery_failed',
      retryable: true,
    })
    await vi.waitFor(() => expect(legacyWeb.open).toHaveBeenCalledOnce())
    emitWeb({ type: 'ready', sessionId: '41', assetGeneration: 1, width: 900, height: 600 })
    pendingWebOpen.resolve(webSession)
    await vi.waitFor(() =>
      expect(
        imageRenderCommand.mock.calls.some(([command]) => command.command.type === 'close'),
      ).toBe(true),
    )
    emitWeb({
      type: 'frame_presented',
      sessionId: '41',
      assetGeneration: 1,
      sceneRevision: 0,
      frameIndex: 1,
    })
    expect(handler).not.toHaveBeenCalledWith(expect.objectContaining({ type: 'frame_presented' }))

    pendingNativeClose.resolve(applied(0))
    await vi.waitFor(() => expect(session.backend).toBe('web'))
    await vi.waitFor(() => expect(handler).toHaveBeenCalledTimes(5))

    expect(handler.mock.calls.slice(-3).map(([event]) => event.type)).toEqual([
      'backend_activated',
      'ready',
      'frame_presented',
    ])
  })

  it('publishes backend activation after transition and retries native for the next session', async () => {
    const { port, emitNative, imageRenderCommand, legacyWeb } = fixture()
    const handler = vi.fn()
    await port.listen(handler)
    const first = await port.open(openRequest)
    await first.dispatch(surface)

    emitNative({
      type: 'failed',
      sessionId: '41',
      assetGeneration: 1,
      code: 'resource_recovery_failed',
      retryable: true,
    })
    emitNative({
      type: 'failed',
      sessionId: '41',
      assetGeneration: 1,
      code: 'resource_recovery_failed',
      retryable: true,
    })
    await vi.waitFor(() => expect(first.backend).toBe('web'))
    expect(handler).toHaveBeenCalledWith({
      type: 'backend_activated',
      sessionId: '41',
      assetGeneration: 1,
      backend: 'web',
    })

    await first.close()
    const second = await port.open({ ...openRequest, assetGeneration: 2 })

    expect(second.backend).toBe('native')
    expect(legacyWeb.open).toHaveBeenCalledTimes(1)
    expect(
      imageRenderCommand.mock.calls.filter(([command]) => command.command.type === 'open'),
    ).toHaveLength(2)
  })
})
