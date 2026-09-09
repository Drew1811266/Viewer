import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createImageRendererPort } from './imageRendererPort'
import type {
  ImageRenderCommandEnvelope,
  ImageRendererBridge,
  ImageRendererCommand,
  ImageRendererEvent,
} from './imageRendererTypes'

const request = {
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

function ack(revision: number) {
  return { disposition: 'applied' as const, acceptedRevision: revision, backend: 'native' as const }
}

function fixture() {
  let handler: ((event: ImageRendererEvent) => void) | undefined
  const imageRenderCommand = vi.fn(async (command: ImageRenderCommandEnvelope) =>
    ack(command.sceneRevision),
  )
  const bridge: ImageRendererBridge = {
    imageRenderCommand,
    listenImageRender: vi.fn(async (next) => {
      handler = next
      return vi.fn()
    }),
  }
  return {
    bridge,
    imageRenderCommand,
    emit: (event: ImageRendererEvent) => handler?.(event),
    port: createImageRendererPort({ bridge }),
  }
}

beforeEach(() => vi.restoreAllMocks())

describe('ImageRendererPort', () => {
  it('is native-only and forwards detail events', async () => {
    const { port, emit } = fixture()
    const received = vi.fn()
    await port.listen(received)
    const session = await port.open(request)
    const event = {
      type: 'detail_availability_changed' as const,
      ...request,
      resourceRevision: 3,
      available: false,
    }
    emit(event)
    expect(port.backend).toBe('native')
    expect(session.backend).toBe('native')
    expect(received).toHaveBeenCalledWith(event)
  })

  it('deduplicates active and concurrent opens for the same asset', async () => {
    let resolve!: (value: ReturnType<typeof ack>) => void
    const { port, imageRenderCommand } = fixture()
    imageRenderCommand.mockImplementationOnce(
      () =>
        new Promise((next) => {
          resolve = next
        }),
    )
    const first = port.open(request)
    const second = port.open({ ...request })
    resolve(ack(0))
    const [session, concurrent] = await Promise.all([first, second])
    expect(concurrent).toBe(session)
    expect(await port.open({ ...request })).toBe(session)
    expect(imageRenderCommand).toHaveBeenCalledOnce()
  })

  it('serializes commands, revisions, and idempotent close', async () => {
    const { port, imageRenderCommand } = fixture()
    const session = await port.open(request)
    await session.dispatch(surface)
    await session.dispatch({
      sceneRevision: 1,
      command: { type: 'set_scene', scene: { annotations: [], draft: null } },
    })
    await session.close()
    await session.close()
    expect(imageRenderCommand.mock.calls.map(([envelope]) => envelope)).toEqual([
      {
        sessionId: 41,
        assetGeneration: 1,
        sceneRevision: 0,
        commandId: 1,
        command: { type: 'open', entityId: request.entityId },
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

  it('rejects invalid or regressing revisions before bridge calls', async () => {
    const { port, imageRenderCommand } = fixture()
    const session = await port.open(request)
    await session.dispatch({
      sceneRevision: 2,
      command: { type: 'set_scene', scene: { annotations: [], draft: null } },
    })
    await expect(session.dispatch(surface)).rejects.toThrow('revision')
    await expect(session.dispatch({ ...surface, sceneRevision: Number.NaN })).rejects.toThrow(
      'sceneRevision',
    )
    expect(imageRenderCommand).toHaveBeenCalledTimes(2)
  })

  it('filters events by active session generation', async () => {
    const { port, emit } = fixture()
    const received = vi.fn()
    await port.listen(received)
    await port.open(request)
    emit({ type: 'ready', sessionId: '99', assetGeneration: 1, width: 10, height: 10 })
    emit({ type: 'ready', sessionId: request.sessionId, assetGeneration: 9, width: 10, height: 10 })
    emit({
      type: 'ready',
      sessionId: request.sessionId,
      assetGeneration: 1,
      width: 900,
      height: 600,
    })
    expect(received).toHaveBeenCalledOnce()
  })

  it('keeps native after bridge failures and permits retrying close', async () => {
    const { port, imageRenderCommand } = fixture()
    imageRenderCommand.mockRejectedValueOnce({
      code: 'image_render_driver_unavailable',
      retryable: true,
    })
    await expect(port.open(request)).rejects.toMatchObject({
      code: 'image_render_driver_unavailable',
    })
    expect(port.backend).toBe('native')

    const session = await port.open(request)
    imageRenderCommand.mockRejectedValueOnce({
      code: 'image_render_driver_failed',
      retryable: true,
    })
    await expect(session.close()).rejects.toMatchObject({ code: 'image_render_driver_failed' })
    await session.close()
    expect(imageRenderCommand.mock.calls.map(([envelope]) => envelope.command.type)).toEqual([
      'open',
      'open',
      'close',
      'close',
    ])
  })

  it('buffers matching events while opening', async () => {
    let resolve!: (value: ReturnType<typeof ack>) => void
    const { port, imageRenderCommand, emit } = fixture()
    imageRenderCommand.mockImplementationOnce(
      () =>
        new Promise((next) => {
          resolve = next
        }),
    )
    const received = vi.fn()
    await port.listen(received)
    const opening = port.open(request)
    emit({
      type: 'ready',
      sessionId: request.sessionId,
      assetGeneration: 1,
      width: 900,
      height: 600,
    })
    resolve(ack(0))
    await opening
    expect(received).toHaveBeenCalledOnce()
  })
})
