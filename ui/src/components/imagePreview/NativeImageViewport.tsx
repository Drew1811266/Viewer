import type { KeyboardEvent, ReactNode } from 'react'
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import { isPreviewableImage } from '../../fileKinds'
import type {
  ImageRendererCamera,
  ImageRendererCommand,
  ImageRendererEvent,
  ImageRendererPort,
  ImageRendererRect,
  ImageRendererSession,
} from '../../rendering/imageRendererTypes'
import UnsupportedFileState from '../UnsupportedFileState'
import ViewerButton, { ViewerIconButton } from '../ui/ViewerButton'
import ViewerLocalFeedback from '../ui/ViewerLocalFeedback'
import ViewerSegmentedControl from '../ui/ViewerSegmentedControl'
import ImagePreviewChrome from './ImagePreviewChrome'
import ImagePreviewLoading from './ImagePreviewLoading'
import type { ImagePreviewSurfaceProps } from './ImagePreviewSurface'
import {
  clampOffset,
  displayScale,
  type ImageViewportGeometry,
  type ImageViewportState,
  type PreviewRotation,
  panBounds,
  type Size,
  zoomAtAnchor,
} from './imageGeometry'
import { createImagePreviewProjection } from './imagePreviewProjection'
import { lensDimensions } from './magnifierGeometry'
import WebImageViewport from './WebImageViewport'

type NativeImageViewportProps = Omit<ImagePreviewSurfaceProps, 'renderer'> & {
  renderer: ImageRendererPort
}

const EMPTY_ENTITY_IDS: ReadonlySet<string> = new Set()
const EMPTY_SIZE: Size = { width: 0, height: 0 }
const FIT_INSET = 0.9
const DEFAULT_CAMERA: ImageViewportState = {
  mode: 'fit',
  zoom: 1,
  rotation: 0,
  offset: { x: 0, y: 0 },
}
let rendererSessionSequence = 0

export default function NativeImageViewport({
  renderer,
  file,
  files,
  magnifier,
  pointerClientPoint,
  unavailableEntityIds = EMPTY_ENTITY_IDS,
  requestImage,
  onNavigate,
  onDimensions,
  ariaLabel,
  toolbarLabel = '图片预览工具',
  onEscape,
  slots,
  ...webOnlyProps
}: NativeImageViewportProps) {
  const stage = useRef<HTMLDivElement>(null)
  const navigationElement = useRef<HTMLElement>(null)
  const sessionId = useRef(nextRendererSessionId())
  const generation = useRef(0)
  const session = useRef<ImageRendererSession | null>(null)
  const resourceReadyGeneration = useRef<number | null>(null)
  const lastAppliedSurface = useRef<(ImageRendererRect & { scaleFactor: number }) | null>(null)
  const pendingSurface = useRef<(ImageRendererRect & { scaleFactor: number }) | null>(null)
  const lastAppliedExclusions = useRef<ImageRendererRect[] | null>(null)
  const pendingExclusions = useRef<ImageRendererRect[] | null>(null)
  const [activeGeneration, setActiveGeneration] = useState(0)
  const [sessionEpoch, setSessionEpoch] = useState(0)
  const [camera, setCamera] = useState<ImageViewportState>(DEFAULT_CAMERA)
  const [sourceSize, setSourceSize] = useState<Size>(file.imageMetadata ?? EMPTY_SIZE)
  const [stageSize, setStageSize] = useState<Size>(EMPTY_SIZE)
  const [ready, setReady] = useState(false)
  const [fallbackToWeb, setFallbackToWeb] = useState(false)
  const [magnifierEnabled, setMagnifierEnabled] = useState(false)
  const [magnifierAnnounced, setMagnifierAnnounced] = useState(false)
  const [announcedScalePercent, setAnnouncedScalePercent] = useState(100)
  const [error, setError] = useState<string | null>(null)
  const metadataWidth = file.imageMetadata?.width
  const metadataHeight = file.imageMetadata?.height
  const currentIndex = files.findIndex((candidate) => candidate.entityId === file.entityId)
  const unavailable = unavailableEntityIds.has(file.entityId)
  const transformsDisabled = unavailable || !isPreviewableImage(file)
  const geometry = useMemo<ImageViewportGeometry>(
    () => ({ stage: stageSize, source: sourceSize, fitInset: FIT_INSET }),
    [sourceSize, stageSize],
  )
  const scalePercent = Math.round((camera.mode === 'free' ? camera.zoom : 1) * 100)

  useEffect(() => {
    const timer = window.setTimeout(() => setAnnouncedScalePercent(scalePercent), 300)
    return () => window.clearTimeout(timer)
  }, [scalePercent])

  useEffect(() => {
    if (transformsDisabled) return
    const assetGeneration = generation.current + 1
    generation.current = assetGeneration
    setActiveGeneration(assetGeneration)
    setCamera(DEFAULT_CAMERA)
    setSourceSize(
      metadataWidth !== undefined && metadataHeight !== undefined
        ? { width: metadataWidth, height: metadataHeight }
        : EMPTY_SIZE,
    )
    setReady(false)
    setFallbackToWeb(false)
    setMagnifierEnabled(false)
    setError(null)
    resourceReadyGeneration.current = null
    lastAppliedSurface.current = null
    pendingSurface.current = null
    lastAppliedExclusions.current = null
    pendingExclusions.current = null
    let disposed = false
    let stopListening: (() => void) | undefined
    let opened: ImageRendererSession | undefined

    const receive = (event: ImageRendererEvent) => {
      if (
        disposed ||
        event.sessionId !== sessionId.current ||
        event.assetGeneration !== assetGeneration
      ) {
        return
      }
      if (event.type === 'ready') {
        resourceReadyGeneration.current = event.assetGeneration
        const dimensions = { width: event.width, height: event.height }
        setSourceSize(dimensions)
        setError(null)
        onDimensions?.(file.entityId, event.width, event.height)
      } else if (event.type === 'frame_presented') {
        if (resourceReadyGeneration.current !== event.assetGeneration) return
        setError(null)
        setReady(true)
      } else if (event.type === 'camera_changed') {
        setCamera(fromRendererCamera(event.camera))
      } else if (event.type === 'backend_activated' && event.backend === 'web') {
        setFallbackToWeb(true)
      } else if (event.type === 'failed') {
        setError(nativeFailureCopy(event.code))
      }
    }

    void (async () => {
      try {
        stopListening = await renderer.listen(receive)
        if (disposed) {
          stopListening()
          return
        }
        opened = await renderer.open({
          sessionId: sessionId.current,
          entityId: file.entityId,
          assetGeneration,
        })
        if (disposed) {
          await opened.close()
          return
        }
        session.current = opened
        if (opened.backend === 'web') setFallbackToWeb(true)
        setSessionEpoch((current) => current + 1)
      } catch (cause) {
        if (!disposed) setError(nativeFailureCopy(errorCode(cause)))
      }
    })()

    return () => {
      disposed = true
      stopListening?.()
      const current = opened ?? session.current
      if (session.current === current) session.current = null
      if (current !== null && current !== undefined) void current.close()
    }
  }, [file.entityId, metadataHeight, metadataWidth, onDimensions, renderer, transformsDisabled])

  useLayoutEffect(() => {
    if (fallbackToWeb || transformsDisabled || activeGeneration === 0) return
    const node = stage.current
    if (node === null) return
    let disposed = false

    const sync = () => {
      if (disposed) return
      const current = session.current
      if (current === null) return
      const bounds = node.getBoundingClientRect()
      const surface = {
        left: bounds.left,
        top: bounds.top,
        width: bounds.width,
        height: bounds.height,
        scaleFactor: window.devicePixelRatio || 1,
      }

      const navigationBounds = navigationElement.current?.getBoundingClientRect()
      const exclusions = navigationBounds === undefined ? [] : [rectFromBounds(navigationBounds)]
      if (
        exclusions.every(validRect) &&
        !sameRects(lastAppliedExclusions.current, exclusions) &&
        !sameRects(pendingExclusions.current, exclusions)
      ) {
        pendingExclusions.current = exclusions
        void dispatch(current, { type: 'set_input_exclusions', exclusions }).then(
          (ack) => {
            if (disposed) return
            if (sameRects(pendingExclusions.current, exclusions)) pendingExclusions.current = null
            if (ack.disposition === 'applied' || ack.disposition === 'ignored_duplicate') {
              lastAppliedExclusions.current = exclusions
            }
          },
          (cause) => {
            if (disposed) return
            if (sameRects(pendingExclusions.current, exclusions)) pendingExclusions.current = null
            setError(nativeFailureCopy(errorCode(cause)))
          },
        )
      }

      if (validSurface(surface)) {
        setStageSize({ width: surface.width, height: surface.height })
        if (
          !sameSurface(lastAppliedSurface.current, surface) &&
          !sameSurface(pendingSurface.current, surface)
        ) {
          pendingSurface.current = surface
          void dispatch(current, { type: 'set_surface', surface }).then(
            (ack) => {
              if (disposed) return
              if (sameSurface(pendingSurface.current, surface)) pendingSurface.current = null
              if (ack.disposition === 'applied' || ack.disposition === 'ignored_duplicate') {
                lastAppliedSurface.current = surface
              }
              if (ack.backend === 'web') setFallbackToWeb(true)
            },
            (cause) => {
              if (disposed) return
              if (sameSurface(pendingSurface.current, surface)) pendingSurface.current = null
              setError(nativeFailureCopy(errorCode(cause)))
            },
          )
        }
      }
    }

    sync()
    const observer = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(sync)
    observer?.observe(node)
    if (navigationElement.current !== null) observer?.observe(navigationElement.current)
    window.addEventListener('resize', sync)
    window.visualViewport?.addEventListener('resize', sync)
    return () => {
      disposed = true
      observer?.disconnect()
      window.removeEventListener('resize', sync)
      window.visualViewport?.removeEventListener('resize', sync)
    }
  }, [activeGeneration, fallbackToWeb, sessionEpoch, transformsDisabled])

  const sendCamera = useCallback((next: ImageViewportState) => {
    setCamera(next)
    const current = session.current
    if (current !== null) void dispatch(current, { type: 'camera', camera: toRendererCamera(next) })
  }, [])

  const navigate = useCallback(
    (delta: number) => {
      const next = files[currentIndex + delta]
      if (next) onNavigate(next)
    },
    [currentIndex, files, onNavigate],
  )

  const toggleMagnifier = useCallback(() => {
    setMagnifierAnnounced(true)
    setMagnifierEnabled((enabled) => {
      const next = !enabled
      const current = session.current
      if (current !== null) {
        void dispatch(current, {
          type: 'set_magnifier',
          magnifier: next ? nativeMagnifierPreferences(magnifier) : null,
        })
      }
      return next
    })
  }, [magnifier])

  const keyboard = useCallback(
    (event: KeyboardEvent<HTMLElement>) => {
      if (event.defaultPrevented || event.nativeEvent.isComposing) return
      const target = event.target
      const textEntry =
        target instanceof HTMLElement &&
        (target.matches('input, textarea, select') ||
          target.isContentEditable ||
          target.closest('[contenteditable="true"], [contenteditable=""]') !== null)
      if (textEntry && event.key !== 'Escape') return
      if (ownsMagnifierShortcut(event) && !transformsDisabled) {
        event.preventDefault()
        toggleMagnifier()
        return
      }
      if (event.key === 'Escape') {
        event.preventDefault()
        onEscape?.()
      }
      if (event.key === 'ArrowLeft') {
        event.preventDefault()
        navigate(-1)
      }
      if (event.key === 'ArrowRight') {
        event.preventDefault()
        navigate(1)
      }
    },
    [navigate, onEscape, toggleMagnifier, transformsDisabled],
  )

  if (fallbackToWeb) {
    return (
      <WebImageViewport
        file={file}
        files={files}
        magnifier={magnifier}
        pointerClientPoint={pointerClientPoint}
        unavailableEntityIds={unavailableEntityIds}
        requestImage={requestImage}
        onNavigate={onNavigate}
        onDimensions={onDimensions}
        ariaLabel={ariaLabel}
        toolbarLabel={toolbarLabel}
        onEscape={onEscape}
        slots={slots}
        {...webOnlyProps}
      />
    )
  }

  const displayControls: ReactNode = (
    <ViewerSegmentedControl label="图片显示控制">
      <ViewerButton
        active={camera.mode === 'fit'}
        disabled={transformsDisabled}
        onClick={() => sendCamera({ ...camera, mode: 'fit', zoom: 1, offset: { x: 0, y: 0 } })}
      >
        适应窗口
      </ViewerButton>
      <ViewerIconButton
        icon="minus"
        label="缩小"
        disabled={transformsDisabled}
        onClick={() =>
          sendCamera(
            zoomAtAnchor(camera, geometry, 0.8, {
              x: geometry.stage.width / 2,
              y: geometry.stage.height / 2,
            }),
          )
        }
      />
      <span className="preview-scale-label">{scalePercent}%</span>
      <span className="visually-hidden" data-testid="preview-scale-announcement" aria-live="polite">
        缩放比例 {announcedScalePercent}%
      </span>
      <ViewerIconButton
        icon="plus"
        label="放大"
        disabled={transformsDisabled}
        onClick={() =>
          sendCamera(
            zoomAtAnchor(camera, geometry, 1.25, {
              x: geometry.stage.width / 2,
              y: geometry.stage.height / 2,
            }),
          )
        }
      />
    </ViewerSegmentedControl>
  )
  const previewActions: ReactNode = (
    <>
      <ViewerIconButton
        icon="zoom-in"
        label="放大镜"
        title="放大镜（Q）"
        tone="quiet"
        active={magnifierEnabled}
        aria-keyshortcuts="Q"
        disabled={transformsDisabled}
        onClick={toggleMagnifier}
      />
      <ViewerIconButton
        icon="rotate-cw"
        label="顺时针旋转"
        tone="quiet"
        disabled={transformsDisabled}
        onClick={() => {
          const rotation = ((camera.rotation + 90) % 360) as PreviewRotation
          const next = { ...camera, rotation }
          sendCamera({ ...next, offset: clampOffset(next.offset, panBounds(next, geometry)) })
        }}
      />
      {slots?.toolbarActions}
    </>
  )
  const previewStage: ReactNode = (
    <div
      ref={stage}
      className="image-preview-stage native-image-viewport"
      data-testid="native-image-viewport"
      data-ready={ready}
    >
      {unavailable ? (
        <UnsupportedFileState file={file} unavailable />
      ) : !isPreviewableImage(file) ? (
        <UnsupportedFileState file={file} />
      ) : null}
      {!unavailable && isPreviewableImage(file) && error === null && (
        <ImagePreviewLoading visible={!ready} />
      )}
      {error !== null && (
        <ViewerLocalFeedback tone="danger" title="无法显示这张图片">
          {error}
        </ViewerLocalFeedback>
      )}
      {magnifierAnnounced && (
        <span className="visually-hidden" aria-live="polite">
          {magnifierEnabled ? '放大镜已开启' : '放大镜已关闭'}
        </span>
      )}
      {slots?.stageOverlay?.(
        createImagePreviewProjection(
          stage.current?.getBoundingClientRect() ?? null,
          camera,
          geometry,
          sourceSize,
        ),
      )}
    </div>
  )
  const navigation: ReactNode = (
    <>
      <ViewerIconButton
        icon="chevron-left"
        label="上一张"
        tone="quiet"
        disabled={currentIndex <= 0}
        onClick={() => navigate(-1)}
      />
      <span>
        {currentIndex + 1} / {files.length}
      </span>
      <ViewerIconButton
        icon="chevron-right"
        label="下一张"
        tone="quiet"
        disabled={currentIndex < 0 || currentIndex >= files.length - 1}
        onClick={() => navigate(1)}
      />
    </>
  )

  return (
    <ImagePreviewChrome
      ariaLabel={ariaLabel ?? `图片预览 ${file.name}`}
      toolbarLabel={toolbarLabel}
      toolbarLeading={slots?.toolbarLeading}
      toolbarCenter={displayControls}
      toolbarActions={previewActions}
      stage={previewStage}
      sidePanel={slots?.sidePanel}
      navigation={navigation}
      navigationRef={navigationElement}
      onKeyDown={keyboard}
    />
  )
}

export function nativeFittedImageRect(source: Size, viewport: Size) {
  const geometry = { source, stage: viewport, fitInset: FIT_INSET }
  const scale = displayScale(DEFAULT_CAMERA, geometry)
  const width = source.width * scale
  const height = source.height * scale
  return {
    left: (viewport.width - width) / 2,
    top: (viewport.height - height) / 2,
    width,
    height,
  }
}

function dispatch(session: ImageRendererSession, command: ImageRendererCommand['command']) {
  return session.dispatch({ sceneRevision: 0, command })
}

function nextRendererSessionId(): string {
  rendererSessionSequence += 1
  if (!Number.isSafeInteger(rendererSessionSequence)) rendererSessionSequence = 1
  return String(rendererSessionSequence)
}

function toRendererCamera(camera: ImageViewportState): ImageRendererCamera {
  return {
    mode: camera.mode,
    zoom: camera.zoom,
    rotation: `deg${camera.rotation}` as ImageRendererCamera['rotation'],
    offset: camera.offset,
  }
}

function fromRendererCamera(camera: ImageRendererCamera): ImageViewportState {
  return {
    mode: camera.mode,
    zoom: camera.zoom,
    rotation: Number(camera.rotation.slice(3)) as PreviewRotation,
    offset: camera.offset,
  }
}

function nativeMagnifierPreferences(preferences: NativeImageViewportProps['magnifier']) {
  const lens = lensDimensions(preferences.shape, preferences.area)
  return {
    widthPx: lens.width,
    heightPx: lens.height,
    magnification: preferences.magnification,
    shape: preferences.shape,
  }
}

function rectFromBounds(bounds: DOMRect): ImageRendererRect {
  return { left: bounds.left, top: bounds.top, width: bounds.width, height: bounds.height }
}

function validRect(rect: ImageRendererRect): boolean {
  return (
    [rect.left, rect.top, rect.width, rect.height].every(Number.isFinite) &&
    rect.left >= 0 &&
    rect.top >= 0 &&
    rect.width > 0 &&
    rect.height > 0
  )
}

function sameRects(left: ImageRendererRect[] | null, right: ImageRendererRect[]): boolean {
  return (
    left !== null &&
    left.length === right.length &&
    left.every((rect, index) => sameRect(rect, right[index]))
  )
}

function sameRect(left: ImageRendererRect, right: ImageRendererRect | undefined): boolean {
  return (
    right !== undefined &&
    left.left === right.left &&
    left.top === right.top &&
    left.width === right.width &&
    left.height === right.height
  )
}

function validSurface(surface: ImageRendererRect & { scaleFactor: number }): boolean {
  return (
    [surface.left, surface.top, surface.width, surface.height, surface.scaleFactor].every(
      Number.isFinite,
    ) &&
    surface.left >= 0 &&
    surface.top >= 0 &&
    surface.width > 0 &&
    surface.height > 0 &&
    surface.scaleFactor > 0
  )
}

function sameSurface(
  left: (ImageRendererRect & { scaleFactor: number }) | null,
  right: ImageRendererRect & { scaleFactor: number },
): boolean {
  return (
    left?.left === right.left &&
    left.top === right.top &&
    left.width === right.width &&
    left.height === right.height &&
    left.scaleFactor === right.scaleFactor
  )
}

function ownsMagnifierShortcut(event: KeyboardEvent<HTMLElement>): boolean {
  return (
    event.key.toLowerCase() === 'q' &&
    !event.altKey &&
    !event.ctrlKey &&
    !event.metaKey &&
    !event.shiftKey &&
    !event.repeat &&
    !event.nativeEvent.isComposing &&
    !event.defaultPrevented
  )
}

function errorCode(error: unknown): string {
  if (typeof error === 'object' && error !== null && 'code' in error) return String(error.code)
  return error instanceof Error ? error.message : String(error)
}

function nativeFailureCopy(code: string): string {
  if (code.includes('decode')) return '无法解码该图片。'
  if (code.includes('unauthorized')) return '这张图片不属于当前项目。'
  return '原生图片渲染器暂时无法完成预览。'
}
