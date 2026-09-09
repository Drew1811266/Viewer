import { readFileSync } from 'node:fs'
import { act, render, screen, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { BrowserFile } from '../../api/types'
import type {
  ImageRendererCommand,
  ImageRendererEvent,
  ImageRendererPort,
  ImageRendererSession,
  ImageRendererViewportBinding,
} from '../../rendering/imageRendererTypes'
import NativeImageViewport, { nativeFittedImageRect } from './NativeImageViewport'

const callbacks = new Map<Element, ResizeObserverCallback>()
let stageRect = rect(120, 84, 960, 600)
let editorRect = rect(360, 240, 288, 176)

beforeEach(() => {
  callbacks.clear()
  stageRect = rect(120, 84, 960, 600)
  editorRect = rect(360, 240, 288, 176)
  vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (
    this: HTMLElement,
  ) {
    if (this.classList.contains('image-preview-stage')) return stageRect
    if (this.classList.contains('preview-navigation-float')) return rect(520, 620, 160, 48)
    if (this.dataset.nativeInputExclusion === 'true') return editorRect
    return rect(0, 0, this.clientWidth, this.clientHeight)
  })
  vi.stubGlobal('devicePixelRatio', 2)
  vi.stubGlobal(
    'ResizeObserver',
    class {
      private readonly callback: ResizeObserverCallback
      constructor(callback: ResizeObserverCallback) {
        this.callback = callback
      }
      observe(node: Element) {
        callbacks.set(node, this.callback)
      }
      disconnect() {}
    },
  )
})

describe('NativeImageViewport', () => {
  it('keeps detail unavailability across preview frames until the latest demand recovers', async () => {
    const harness = rendererHarness()
    renderPreview(harness.port)
    await waitFor(() => expect(harness.request).not.toBeNull())
    if (harness.request === null) throw new Error('native session did not open')
    const request = { ...harness.request }
    const unavailable = {
      type: 'detail_availability_changed' as const,
      ...request,
      resourceRevision: 4,
      available: false,
    }
    act(() => harness.publish(unavailable))
    expect(screen.getByText('高清细节暂不可用')).toBeInTheDocument()
    act(() => {
      harness.publish({ type: 'ready', ...request, width: 600, height: 400 })
      harness.publish({ type: 'frame_presented', ...request, sceneRevision: 0, frameIndex: 1 })
      harness.publish({ ...unavailable, resourceRevision: 3, available: true })
      harness.publish({
        ...unavailable,
        assetGeneration: request.assetGeneration - 1,
        available: true,
      })
    })
    expect(screen.getByText('高清细节暂不可用')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '重试预览' })).not.toBeInTheDocument()
    act(() => harness.publish({ ...unavailable, available: true }))
    expect(screen.queryByText('高清细节暂不可用')).not.toBeInTheDocument()
  })

  it('does not let detail recovery clear a terminal error or leak status into a new asset', async () => {
    const harness = rendererHarness()
    const preview = renderPreview(harness.port)
    await waitFor(() => expect(harness.request).not.toBeNull())
    if (harness.request === null) throw new Error('native session did not open')
    const request = { ...harness.request }
    const detail = {
      type: 'detail_availability_changed' as const,
      ...request,
      resourceRevision: 2,
      available: false,
    }
    act(() => harness.publish(detail))
    expect(screen.getByText('高清细节暂不可用')).toBeInTheDocument()
    act(() => {
      harness.publish({ type: 'failed', ...request, code: 'device_lost', retryable: false })
      harness.publish({ ...detail, available: true })
    })
    expect(screen.getByRole('alert')).toBeInTheDocument()
    preview.updateFile({ modifiedNs: '2' })
    await waitFor(() => expect(harness.request?.assetGeneration).toBe(request.assetGeneration + 1))
    act(() => harness.publish(detail))
    expect(screen.queryByText('高清细节暂不可用')).not.toBeInTheDocument()
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
  })

  it.each(['close', 'camera'] as const)(
    'retains zoom, rotation and magnifier intent while retry %s is pending',
    async (pending) => {
      const harness = rendererHarness()
      renderPreview(harness.port, reviewBinding())
      await waitFor(() => expect(sceneCommands(harness.commands)).toHaveLength(1))
      if (harness.request === null) throw new Error('native session did not open')
      const previous = { ...harness.request }
      act(() => screen.getByRole('button', { name: '放大镜' }).click())
      harness.holdClose = pending === 'close'
      harness.holdCamera = pending === 'camera'
      act(() =>
        harness.publish({ type: 'failed', ...previous, code: 'device_lost', retryable: true }),
      )
      act(() => screen.getByRole('button', { name: '重试预览' }).click())
      await waitFor(() =>
        expect(pending === 'close' ? harness.pendingCloses : harness.pendingCameras).toHaveLength(
          1,
        ),
      )
      act(() => screen.getByRole('button', { name: '放大' }).click())
      act(() => screen.getByRole('button', { name: '顺时针旋转' }).click())
      act(() => screen.getByRole('button', { name: '放大镜' }).click())
      harness.holdCamera = false
      await act(async () => {
        for (const settle of [
          ...harness.pendingCloses.splice(0),
          ...harness.pendingCameras.splice(0),
        ])
          settle()
      })
      await waitFor(() => expect(sceneCommands(harness.commands)).toHaveLength(2))
      const camera = harness.commands
        .flatMap(({ command }) => (command.type === 'camera' ? [command.camera] : []))
        .at(-1)
      expect(camera).toMatchObject({ mode: 'free', zoom: 1.25, rotation: 'deg90' })
      expect(magnifierCommands(harness.commands).at(-1)?.magnifier).toBeNull()
    },
  )

  it('retries failed teardown without allowing later retry generations to bypass it', async () => {
    const harness = rendererHarness()
    renderPreview(harness.port)
    await waitFor(() => expect(surfaceCommands(harness.commands)).toHaveLength(1))
    if (harness.request === null) throw new Error('native session did not open')
    const previous = { ...harness.request }
    harness.failClose = true
    act(() =>
      harness.publish({ type: 'failed', ...previous, code: 'device_lost', retryable: true }),
    )
    for (let attempt = 0; attempt < 3; attempt += 1) {
      act(() => screen.getByRole('button', { name: '重试预览' }).click())
      await waitFor(() => expect(screen.getByRole('alert')).toBeInTheDocument())
      expect(harness.request?.assetGeneration).toBe(previous.assetGeneration)
    }
    harness.failClose = false
    act(() => screen.getByRole('button', { name: '重试预览' }).click())
    await waitFor(() =>
      expect(harness.request?.assetGeneration).toBeGreaterThan(previous.assetGeneration),
    )
  })

  it('does not reopen the same image when decoded metadata is enriched', async () => {
    const harness = rendererHarness()
    const preview = renderPreview(harness.port)
    await waitFor(() => expect(harness.request).not.toBeNull())
    const generation = harness.request?.assetGeneration
    preview.updateFile({ imageMetadata: { width: 600, height: 400 } })
    await act(async () => {})
    expect(harness.request?.assetGeneration).toBe(generation)
  })

  it('opens a new generation when the same entity contains a changed source file', async () => {
    const harness = rendererHarness()
    const preview = renderPreview(harness.port)
    await waitFor(() => expect(harness.request).not.toBeNull())
    const generation = harness.request?.assetGeneration ?? 0
    preview.updateFile({ modifiedNs: '2' })
    await waitFor(() => expect(harness.request?.assetGeneration).toBe(generation + 1))
  })

  it('does not expose the retry session until camera and magnifier hydration are complete', async () => {
    const harness = rendererHarness()
    const preview = renderPreview(harness.port, reviewBinding())
    await waitFor(() => expect(sceneCommands(harness.commands)).toHaveLength(1))
    if (harness.request === null) throw new Error('native session did not open')
    const previous = { ...harness.request }
    act(() => screen.getByRole('button', { name: '放大镜' }).click())
    harness.holdCamera = true
    act(() =>
      harness.publish({ type: 'failed', ...previous, code: 'device_lost', retryable: true }),
    )
    act(() => screen.getByRole('button', { name: '重试预览' }).click())
    await waitFor(() => expect(harness.pendingCameras).toHaveLength(1))
    preview.updateBinding({ ...reviewBinding(), sceneRevision: 4 })
    expect(sceneCommands(harness.commands)).toHaveLength(1)
    await act(async () => {
      harness.pendingCameras.shift()?.()
    })
    await waitFor(() => expect(sceneCommands(harness.commands)).toHaveLength(2))
    const types = harness.commands.map(({ command }) => command.type)
    expect(types.lastIndexOf('camera')).toBeLessThan(types.lastIndexOf('set_magnifier'))
    expect(types.lastIndexOf('set_magnifier')).toBeLessThan(types.lastIndexOf('set_scene'))
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
  })

  it('waits for the old native session to close before opening a retry generation', async () => {
    const harness = rendererHarness()
    renderPreview(harness.port)
    await waitFor(() => expect(harness.request).not.toBeNull())
    if (harness.request === null) throw new Error('native session did not open')
    const previous = { ...harness.request }
    harness.holdClose = true
    act(() =>
      harness.publish({ type: 'failed', ...previous, code: 'device_lost', retryable: true }),
    )
    act(() => screen.getByRole('button', { name: '重试预览' }).click())
    await waitFor(() => expect(harness.pendingCloses).toHaveLength(1))
    expect(harness.request.assetGeneration).toBe(previous.assetGeneration)
    await act(async () => {
      harness.pendingCloses.shift()?.()
    })
    await waitFor(() => expect(harness.request?.assetGeneration).toBe(previous.assetGeneration + 1))
  })

  it.each(['surface', 'exclusion'] as const)(
    'enqueues the latest %s bounds when they return to A while B is awaiting acknowledgment',
    async (kind) => {
      const harness = rendererHarness()
      renderPreview(harness.port, reviewBinding())
      await waitFor(() => expect(inputExclusionCommands(harness.commands)).toHaveLength(1))
      await waitFor(() => expect(surfaceCommands(harness.commands)).toHaveLength(1))
      harness.holdLayoutCommands = true
      const stage = screen.getByTestId('native-image-viewport')
      const notifyLayout = () => callbacks.get(stage)?.([], {} as ResizeObserver)
      const count = () =>
        (kind === 'surface'
          ? surfaceCommands(harness.commands)
          : inputExclusionCommands(harness.commands)
        ).length
      if (kind === 'surface') stageRect = rect(120, 84, 800, 540)
      else editorRect = rect(440, 310, 288, 176)
      act(notifyLayout)
      await waitFor(() => expect(count()).toBe(2))
      stageRect = rect(120, 84, 960, 600)
      editorRect = rect(360, 240, 288, 176)
      act(notifyLayout)
      await waitFor(() => expect(count()).toBe(3))
      await act(async () => {
        for (const settle of harness.pendingLayouts.splice(0)) settle()
      })
      act(notifyLayout)
      expect(count()).toBe(3)
    },
  )

  it('refreshes editor exclusions after camera movement without a resize or scene edit', async () => {
    const harness = rendererHarness()
    renderPreview(harness.port, reviewBinding())
    await waitFor(() => expect(inputExclusionCommands(harness.commands)).toHaveLength(1))
    editorRect = rect(440, 310, 288, 176)
    act(() =>
      harness.publish({
        type: 'camera_changed',
        sessionId: harness.request?.sessionId ?? '',
        assetGeneration: harness.request?.assetGeneration ?? 0,
        camera: { mode: 'free', zoom: 2, rotation: 'deg0', offset: { x: 80, y: 70 } },
      }),
    )
    await waitFor(() =>
      expect(inputExclusionCommands(harness.commands).at(-1)?.exclusions).toContainEqual({
        left: 440,
        top: 310,
        width: 288,
        height: 176,
      }),
    )
    expect(surfaceCommands(harness.commands)).toHaveLength(1)
    expect(sceneCommands(harness.commands)).toHaveLength(1)
  })

  it('offers an explicit native retry after failure and rejects events from the old generation', async () => {
    const harness = rendererHarness()
    renderPreview(harness.port, reviewBinding())
    await waitFor(() => expect(harness.request).not.toBeNull())
    if (harness.request === null) throw new Error('native session did not open')
    const previous = { ...harness.request }
    act(() =>
      harness.publish({
        type: 'camera_changed',
        ...previous,
        camera: { mode: 'free', zoom: 2, rotation: 'deg90', offset: { x: 80, y: 70 } },
      }),
    )
    act(() =>
      harness.publish({
        type: 'failed',
        ...previous,
        code: 'resource_decode_failed',
        retryable: true,
      }),
    )
    const retry = await screen.findByRole('button', { name: '重试预览' })
    expect(retry.closest('[data-native-input-exclusion="true"]')).not.toBeNull()
    act(() => retry.click())
    await waitFor(() => expect(harness.request?.assetGeneration).toBe(previous.assetGeneration + 1))
    await waitFor(() =>
      expect(harness.commands).toContainEqual({
        sceneRevision: 0,
        command: {
          type: 'camera',
          camera: { mode: 'free', zoom: 2, rotation: 'deg90', offset: { x: 80, y: 70 } },
        },
      }),
    )
    expect(screen.getByText('200%')).toBeInTheDocument()
    act(() =>
      harness.publish({
        type: 'failed',
        ...previous,
        code: 'resource_decode_failed',
        retryable: true,
      }),
    )
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
    expect(screen.getByText('原生编辑器')).toBeInTheDocument()
    expect(screen.queryByText('Web 评审层')).not.toBeInTheDocument()
  })

  it('sends the native surface once and ignores unchanged resize observations', async () => {
    const harness = rendererHarness()
    renderPreview(harness.port)

    await waitFor(() => expect(surfaceCommands(harness.commands)).toHaveLength(1))
    expect(surfaceCommands(harness.commands)[0]).toEqual({
      type: 'set_surface',
      surface: { left: 120, top: 84, width: 960, height: 600, scaleFactor: 2 },
    })

    const stage = screen.getByTestId('native-image-viewport')
    act(() => {
      callbacks.get(stage)?.(
        [{ target: stage, contentRect: stageRect } as unknown as ResizeObserverEntry],
        {} as ResizeObserver,
      )
    })
    await Promise.resolve()
    expect(surfaceCommands(harness.commands)).toHaveLength(1)

    stageRect = rect(120, 84, 900, 540)
    act(() => {
      callbacks.get(stage)?.(
        [{ target: stage, contentRect: stageRect } as unknown as ResizeObserverEntry],
        {} as ResizeObserver,
      )
    })
    await waitFor(() => expect(surfaceCommands(harness.commands)).toHaveLength(2))
  })

  it('routes the floating navigation back to the WebView', async () => {
    const harness = rendererHarness()
    renderPreview(harness.port)

    await waitFor(() => expect(inputExclusionCommands(harness.commands)).toHaveLength(1))
    expect(inputExclusionCommands(harness.commands)[0]).toEqual({
      type: 'set_input_exclusions',
      exclusions: [{ left: 520, top: 620, width: 160, height: 48 }],
    })
    expect(harness.commands[0]?.command.type).toBe('set_input_exclusions')
    expect(harness.commands[1]?.command.type).toBe('set_surface')
  })

  it('refreshes exclusions when a fixed toolbar popover mounts over the native stage', async () => {
    const harness = rendererHarness()
    renderPreview(harness.port)

    await waitFor(() => expect(inputExclusionCommands(harness.commands)).toHaveLength(1))
    const popover = document.createElement('div')
    popover.dataset.nativeInputExclusion = 'true'
    document.querySelector('.image-preview')?.append(popover)

    await waitFor(() =>
      expect(inputExclusionCommands(harness.commands).at(-1)?.exclusions).toContainEqual({
        left: 360,
        top: 240,
        width: 288,
        height: 176,
      }),
    )
  })

  it('retries the same surface after a failed dispatch instead of caching it as applied', async () => {
    const harness = rendererHarness()
    harness.failNextSurface = true
    renderPreview(harness.port)

    await screen.findByRole('alert')
    const stage = screen.getByTestId('native-image-viewport')
    act(() => {
      callbacks.get(stage)?.(
        [{ target: stage, contentRect: stageRect } as unknown as ResizeObserverEntry],
        {} as ResizeObserver,
      )
    })

    await waitFor(() => expect(surfaceCommands(harness.commands)).toHaveLength(2))
  })

  it('does not attach high-frequency DOM input handlers to the native stage', () => {
    const source = readFileSync('src/components/imagePreview/NativeImageViewport.tsx', 'utf8')
    expect(source).not.toMatch(/onPointerMove|onWheel|usePreviewGestures/)
  })

  it('fits equal aspect ratios identically regardless of pixels, bytes, or representation backend', () => {
    const viewport = { width: 960, height: 600 }
    expect(nativeFittedImageRect({ width: 300, height: 200 }, viewport)).toEqual(
      nativeFittedImageRect({ width: 6_570, height: 4_380 }, viewport),
    )
    expect(nativeFittedImageRect({ width: 300, height: 200 }, viewport)).toEqual({
      left: 75,
      top: 30,
      width: 810,
      height: 540,
    })
  })

  it('keeps shared chrome mounted and waits for the first presented frame', async () => {
    const harness = rendererHarness()
    renderPreview(harness.port)
    expect(screen.getByRole('toolbar', { name: '图片预览工具' })).toBeVisible()
    expect(screen.getByRole('button', { name: '适应窗口' })).toBeVisible()
    expect(screen.getByRole('button', { name: '返回网格' })).toBeVisible()

    await waitFor(() => expect(harness.request).not.toBeNull())
    act(() =>
      harness.publish({
        type: 'ready',
        sessionId: harness.request?.sessionId ?? '',
        assetGeneration: harness.request?.assetGeneration ?? 0,
        width: 6_570,
        height: 4_380,
      }),
    )
    expect(screen.getByTestId('native-image-viewport')).toHaveAttribute('data-ready', 'false')
    act(() =>
      harness.publish({
        type: 'frame_presented',
        sessionId: harness.request?.sessionId ?? '',
        assetGeneration: harness.request?.assetGeneration ?? 0,
        sceneRevision: 0,
        frameIndex: 1,
      }),
    )
    await waitFor(() =>
      expect(screen.getByTestId('native-image-viewport')).toHaveAttribute('data-ready', 'true'),
    )
  })

  it('does not accept a clearing frame before the current asset is ready', async () => {
    const harness = rendererHarness()
    renderPreview(harness.port)
    await waitFor(() => expect(harness.request).not.toBeNull())

    act(() =>
      harness.publish({
        type: 'frame_presented',
        sessionId: harness.request?.sessionId ?? '',
        assetGeneration: harness.request?.assetGeneration ?? 0,
        sceneRevision: 0,
        frameIndex: 1,
      }),
    )

    expect(screen.getByTestId('native-image-viewport')).toHaveAttribute('data-ready', 'false')
  })

  it('keeps the native viewport when a stale Web activation event is received', async () => {
    const harness = rendererHarness()
    renderPreview(harness.port, reviewBinding())
    expect(screen.getByText('原生编辑器')).toBeInTheDocument()
    expect(screen.queryByText('Web 评审层')).not.toBeInTheDocument()
    await waitFor(() => expect(harness.request).not.toBeNull())

    act(() =>
      harness.publish({
        type: 'backend_activated',
        sessionId: harness.request?.sessionId ?? '',
        assetGeneration: harness.request?.assetGeneration ?? 0,
        backend: 'web',
      }),
    )

    expect(screen.getByTestId('native-image-viewport')).toBeInTheDocument()
    expect(screen.getByText('原生编辑器')).toBeInTheDocument()
    expect(screen.queryByText('Web 评审层')).not.toBeInTheDocument()
  })

  it('sends only magnifier preferences and leaves pointer tracking to the native actor', async () => {
    const harness = rendererHarness()
    renderPreview(harness.port)
    await waitFor(() => expect(harness.request).not.toBeNull())

    screen.getByRole('button', { name: '放大镜' }).click()

    await waitFor(() => expect(magnifierCommands(harness.commands)).toHaveLength(1))
    expect(magnifierCommands(harness.commands)[0]).toEqual({
      type: 'set_magnifier',
      magnifier: {
        widthPx: 200,
        heightPx: 200,
        magnification: 2,
        shape: 'circle',
      },
    })
  })

  it('installs one retained review scene, semantic tool and editor input exclusion', async () => {
    const harness = rendererHarness()
    const onEvent = vi.fn()
    const binding: ImageRendererViewportBinding = {
      sceneRevision: 4,
      scene: {
        annotations: [
          {
            id: 'target-1',
            ordinal: 1,
            geometry: { type: 'point', position: { x: 0.2, y: 0.3 } },
            style: { color: [0.7, 0.13, 0.09, 1], lineWidthPx: 2, dashed: false },
            selected: false,
            draft: false,
            visible: true,
          },
        ],
        draft: null,
      },
      tool: 'point',
      inputExclusionRevision: 4,
      onEvent,
    }
    renderPreview(harness.port, binding)

    await waitFor(() => expect(sceneCommands(harness.commands)).toHaveLength(1))
    expect(sceneCommands(harness.commands)[0]).toMatchObject({
      sceneRevision: 4,
      command: { type: 'set_scene', scene: binding.scene },
    })
    await waitFor(() => expect(toolCommands(harness.commands)).toHaveLength(1))
    expect(toolCommands(harness.commands)[0]?.command).toEqual({ type: 'set_tool', tool: 'point' })
    await waitFor(() =>
      expect(inputExclusionCommands(harness.commands).at(-1)?.exclusions).toContainEqual({
        left: 360,
        top: 240,
        width: 288,
        height: 176,
      }),
    )

    act(() =>
      harness.publish({
        type: 'selection_changed',
        sessionId: harness.request?.sessionId ?? '',
        assetGeneration: harness.request?.assetGeneration ?? 0,
        annotationId: 'target-1',
      }),
    )
    expect(onEvent).toHaveBeenCalledWith(expect.objectContaining({ type: 'selection_changed' }))
  })
})

function renderPreview(renderer: ImageRendererPort, nativeBinding?: ImageRendererViewportBinding) {
  let file: BrowserFile = {
    entityId: 'image-1',
    relativePath: 'images/one.jpg',
    name: 'one.jpg',
    kind: 'jpeg',
    size: 8_000_000,
    modifiedNs: '1',
    marker: { reviewState: null, favorite: false },
    imageMetadata: { width: 300, height: 200 },
    imageUrl: null,
    videoMetadata: null,
  }
  const element = (binding?: ImageRendererViewportBinding) => (
    <NativeImageViewport
      renderer={renderer}
      file={file}
      files={[file]}
      magnifier={{ shape: 'circle', magnification: 2, area: 'small' }}
      pointerClientPoint={{ current: null }}
      requestImage={vi.fn(async () => ({
        cacheKey: 'image-1:fit',
        url: 'blob:image-1',
        width: 300,
        height: 200,
        backend: 'image_io' as const,
      }))}
      onNavigate={vi.fn()}
      onEscape={vi.fn()}
      nativeBinding={binding}
      slots={{
        toolbarActions: <button type="button">返回网格</button>,
        stageOverlay: nativeBinding === undefined ? undefined : () => <section>Web 评审层</section>,
        nativeStageOverlay:
          nativeBinding === undefined
            ? undefined
            : () => <section data-native-input-exclusion="true">原生编辑器</section>,
      }}
    />
  )
  const preview = render(element(nativeBinding))
  return {
    ...preview,
    updateBinding: (binding: ImageRendererViewportBinding) => preview.rerender(element(binding)),
    updateFile: (patch: Partial<BrowserFile>) => {
      file = { ...file, ...patch }
      preview.rerender(element(nativeBinding))
    },
  }
}

function reviewBinding(): ImageRendererViewportBinding {
  return {
    sceneRevision: 1,
    scene: { annotations: [], draft: null },
    tool: 'browse',
    inputExclusionRevision: 1,
    onEvent: vi.fn(),
  }
}

function rendererHarness() {
  const commands: ImageRendererCommand[] = []
  let handler: ((event: ImageRendererEvent) => void) | null = null
  const session: ImageRendererSession = {
    backend: 'native',
    sessionId: '1',
    assetGeneration: 1,
    async dispatch(command) {
      commands.push(command)
      if (harness.holdCamera && command.command.type === 'camera') {
        await new Promise<void>((resolve) => harness.pendingCameras.push(resolve))
      }
      if (
        harness.holdLayoutCommands &&
        ['set_surface', 'set_input_exclusions'].includes(command.command.type)
      ) {
        await new Promise<void>((resolve) => harness.pendingLayouts.push(resolve))
      }
      if (harness.failNextSurface && command.command.type === 'set_surface') {
        harness.failNextSurface = false
        throw { code: 'image_render_driver_failed' }
      }
      return { disposition: 'applied', acceptedRevision: command.sceneRevision, backend: 'native' }
    },
    async close() {
      if (harness.failClose) throw { code: 'image_render_driver_failed' }
      if (harness.holdClose)
        await new Promise<void>((resolve) => harness.pendingCloses.push(resolve))
    },
  }
  const port: ImageRendererPort = {
    backend: 'native',
    async open(request) {
      harness.request = request
      return session
    },
    async listen(next) {
      handler = next
      return () => {
        handler = null
      }
    },
  }
  const harness: {
    port: ImageRendererPort
    commands: ImageRendererCommand[]
    request: { sessionId: string; assetGeneration: number } | null
    failNextSurface: boolean
    holdLayoutCommands: boolean
    pendingLayouts: Array<() => void>
    holdClose: boolean
    failClose: boolean
    pendingCloses: Array<() => void>
    holdCamera: boolean
    pendingCameras: Array<() => void>
    publish(event: ImageRendererEvent): void
  } = {
    port,
    commands,
    request: null,
    failNextSurface: false,
    holdLayoutCommands: false,
    pendingLayouts: [],
    holdClose: false,
    failClose: false,
    pendingCloses: [],
    holdCamera: false,
    pendingCameras: [],
    publish(event: ImageRendererEvent) {
      handler?.(event)
    },
  }
  return harness
}

function surfaceCommands(commands: ImageRendererCommand[]) {
  return commands.flatMap(({ command }) => (command.type === 'set_surface' ? [command] : []))
}

function inputExclusionCommands(commands: ImageRendererCommand[]) {
  return commands.flatMap(({ command }) =>
    command.type === 'set_input_exclusions' ? [command] : [],
  )
}

function magnifierCommands(commands: ImageRendererCommand[]) {
  return commands.flatMap(({ command }) => (command.type === 'set_magnifier' ? [command] : []))
}

function sceneCommands(commands: ImageRendererCommand[]) {
  return commands.filter(({ command }) => command.type === 'set_scene')
}

function toolCommands(commands: ImageRendererCommand[]) {
  return commands.filter(({ command }) => command.type === 'set_tool')
}

function rect(left: number, top: number, width: number, height: number): DOMRect {
  return {
    x: left,
    y: top,
    left,
    top,
    width,
    height,
    right: left + width,
    bottom: top + height,
    toJSON: () => undefined,
  }
}
