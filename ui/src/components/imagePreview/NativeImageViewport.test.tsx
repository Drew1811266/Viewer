import { readFileSync } from 'node:fs'
import { act, render, screen, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { BrowserFile } from '../../api/types'
import type {
  ImageRendererCommand,
  ImageRendererEvent,
  ImageRendererPort,
  ImageRendererSession,
} from '../../rendering/imageRendererTypes'
import NativeImageViewport, { nativeFittedImageRect } from './NativeImageViewport'

const callbacks = new Map<Element, ResizeObserverCallback>()
let stageRect = rect(120, 84, 960, 600)

beforeEach(() => {
  callbacks.clear()
  stageRect = rect(120, 84, 960, 600)
  vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (
    this: HTMLElement,
  ) {
    if (this.classList.contains('image-preview-stage')) return stageRect
    if (this.classList.contains('preview-navigation-float')) return rect(520, 620, 160, 48)
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

  it('retries the same surface after a failed dispatch instead of caching it as applied', async () => {
    const harness = rendererHarness()
    harness.failNextSurface = true
    renderPreview(harness.port)

    await waitFor(() => expect(surfaceCommands(harness.commands)).toHaveLength(1))
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

  it('switches to the complete Web viewport only after backend activation is published', async () => {
    const harness = rendererHarness()
    renderPreview(harness.port)
    await waitFor(() => expect(harness.request).not.toBeNull())

    act(() =>
      harness.publish({
        type: 'backend_activated',
        sessionId: harness.request?.sessionId ?? '',
        assetGeneration: harness.request?.assetGeneration ?? 0,
        backend: 'web',
      }),
    )

    await waitFor(() =>
      expect(screen.queryByTestId('native-image-viewport')).not.toBeInTheDocument(),
    )
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
})

function renderPreview(renderer: ImageRendererPort) {
  const file: BrowserFile = {
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
  return render(
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
      slots={{
        toolbarActions: <button type="button">返回网格</button>,
      }}
    />,
  )
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
      if (harness.failNextSurface && command.command.type === 'set_surface') {
        harness.failNextSurface = false
        throw { code: 'image_render_driver_failed' }
      }
      return { disposition: 'applied', acceptedRevision: command.sceneRevision, backend: 'native' }
    },
    async close() {},
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
    publish(event: ImageRendererEvent): void
  } = {
    port,
    commands,
    request: null,
    failNextSurface: false,
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
