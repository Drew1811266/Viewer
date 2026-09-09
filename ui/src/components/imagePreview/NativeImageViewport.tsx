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
import { type NativeCloseBarrier, nativeCloseBarrier } from './nativeCloseBarrier'
import { NativeLayoutCommands } from './nativeLayoutCommands'

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
  unavailableEntityIds = EMPTY_ENTITY_IDS,
  onNavigate,
  onDimensions,
  ariaLabel,
  toolbarLabel = '图片预览工具',
  onEscape,
  slots,
  nativeBinding,
}: NativeImageViewportProps) {
  const stage = useRef<HTMLDivElement>(null)
  const navigationElement = useRef<HTMLElement>(null)
  const sessionId = useRef(nextRendererSessionId())
  const generation = useRef(0)
  const session = useRef<ImageRendererSession | null>(null)
  const sessionClosure = useRef<NativeCloseBarrier>(() => Promise.resolve())
  const nativeBindingRef = useRef(nativeBinding)
  nativeBindingRef.current = nativeBinding
  const sourceMetadata = useRef(file.imageMetadata)
  sourceMetadata.current = file.imageMetadata
  const onDimensionsRef = useRef(onDimensions)
  onDimensionsRef.current = onDimensions
  const resourceReadyGeneration = useRef<number | null>(null)
  const surfaceCommands = useRef(new NativeLayoutCommands(sameSurface))
  const exclusionCommands = useRef(new NativeLayoutCommands(sameRects))
  const sceneRevision = useRef(0)
  const [activeGeneration, setActiveGeneration] = useState(0)
  const [sessionEpoch, setSessionEpoch] = useState(0)
  const [camera, setCamera] = useState<ImageViewportState>(DEFAULT_CAMERA)
  const [sourceSize, setSourceSize] = useState<Size>(file.imageMetadata ?? EMPTY_SIZE)
  const [stageSize, setStageSize] = useState<Size>(EMPTY_SIZE)
  const [ready, setReady] = useState(false)
  const [magnifierEnabled, setMagnifierEnabled] = useState(false)
  const [magnifierAnnounced, setMagnifierAnnounced] = useState(false)
  const [announcedScalePercent, setAnnouncedScalePercent] = useState(100)
  const [error, setError] = useState<string | null>(null)
  const [detailUnavailable, setDetailUnavailable] = useState(false)
  const [retryEpoch, setRetryEpoch] = useState(0)
  const retryView = useRef<{
    entityId: string
    camera: ImageViewportState
    magnifier: ReturnType<typeof nativeMagnifierPreferences> | null
  } | null>(null)
  const desiredView = useRef<{
    camera: ImageViewportState
    magnifier: ReturnType<typeof nativeMagnifierPreferences> | null
  }>({ camera: DEFAULT_CAMERA, magnifier: null })
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
    const restoredView = retryView.current?.entityId === file.entityId ? retryView.current : null
    retryView.current = null
    const initialView = {
      camera: restoredView?.camera ?? DEFAULT_CAMERA,
      magnifier: restoredView?.magnifier ?? null,
    }
    desiredView.current = initialView
    generation.current = assetGeneration
    setActiveGeneration(assetGeneration)
    setCamera(restoredView?.camera ?? DEFAULT_CAMERA)
    setSourceSize(sourceMetadata.current ?? EMPTY_SIZE)
    setReady(false)
    setMagnifierEnabled(restoredView?.magnifier != null)
    setError(null)
    setDetailUnavailable(false)
    resourceReadyGeneration.current = null
    surfaceCommands.current = new NativeLayoutCommands(sameSurface)
    exclusionCommands.current = new NativeLayoutCommands(sameRects)
    sceneRevision.current = 0
    let disposed = false
    let stopListening: (() => void) | undefined
    let opened: ImageRendererSession | undefined
    let latestDetailRevision = -1

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
        onDimensionsRef.current?.(file.entityId, event.width, event.height)
      } else if (event.type === 'frame_presented') {
        if (resourceReadyGeneration.current !== event.assetGeneration) return
        setError(null)
        setReady(true)
      } else if (event.type === 'camera_changed') {
        const next = fromRendererCamera(event.camera)
        desiredView.current = { ...desiredView.current, camera: next }
        setCamera(next)
      } else if (event.type === 'detail_availability_changed') {
        if (event.resourceRevision < latestDetailRevision) return
        latestDetailRevision = event.resourceRevision
        setDetailUnavailable(!event.available)
      } else if (event.type === 'failed') {
        setError(nativeFailureCopy(event.code))
      }
      nativeBindingRef.current?.onEvent(event)
    }

    const previousClosure = sessionClosure.current
    const initialize = (async () => {
      try {
        await previousClosure()
        if (disposed) return
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
        if (disposed) return
        // Controls stay usable while Close/Open/restore is pending. Reconcile
        // the latest intent, including an explicit magnifier-off, before scene
        // revisions or input are allowed to enter the new session.
        let hydrated = restoredView === null ? initialView : null
        while (!disposed && desiredView.current !== hydrated) {
          const next = desiredView.current
          await dispatch(opened, { type: 'camera', camera: toRendererCamera(next.camera) }, 0)
          if (disposed) return
          await dispatch(opened, { type: 'set_magnifier', magnifier: next.magnifier }, 0)
          hydrated = next
        }
        if (disposed) return
        session.current = opened
        setSessionEpoch((current) => current + 1)
      } catch (cause) {
        if (!disposed) setError(nativeFailureCopy(errorCode(cause)))
      }
    })()

    return () => {
      disposed = true
      stopListening?.()
      if (session.current === opened) session.current = null
      // Even an in-flight Open/hydration must finish and close before a new
      // generation opens; otherwise its late Close is stale and cannot reset
      // a faulted native actor. Rejections are surfaced by the next Open.
      sessionClosure.current = nativeCloseBarrier(
        previousClosure,
        initialize.then(() => opened),
      )
      void sessionClosure.current().catch(() => undefined)
    }
  }, [file.entityId, file.modifiedNs, file.size, renderer, retryEpoch, transformsDisabled])

  useEffect(() => {
    if (transformsDisabled || activeGeneration === 0 || nativeBinding === undefined) return
    const current = session.current
    if (current === null) return
    const revision = Math.max(sceneRevision.current + 1, nativeBinding.sceneRevision)
    sceneRevision.current = revision
    let disposed = false
    void dispatch(current, { type: 'set_scene', scene: nativeBinding.scene }, revision).then(
      (ack) => {
        if (disposed) return
        sceneRevision.current = Math.max(sceneRevision.current, ack.acceptedRevision)
      },
      (cause) => {
        if (!disposed) setError(nativeFailureCopy(errorCode(cause)))
      },
    )
    return () => {
      disposed = true
    }
  }, [
    activeGeneration,
    nativeBinding?.scene,
    nativeBinding?.sceneRevision,
    sessionEpoch,
    transformsDisabled,
  ])

  useEffect(() => {
    if (transformsDisabled || activeGeneration === 0 || nativeBinding === undefined) return
    const current = session.current
    if (current === null) return
    void dispatch(
      current,
      { type: 'set_tool', tool: nativeBinding.tool },
      sceneRevision.current,
    ).catch((cause) => setError(nativeFailureCopy(errorCode(cause))))
  }, [activeGeneration, nativeBinding?.tool, sessionEpoch, transformsDisabled])

  useLayoutEffect(() => {
    if (transformsDisabled || activeGeneration === 0) return
    const node = stage.current
    if (node === null) return
    let disposed = false

    const sync = () => {
      if (disposed) return
      const current = session.current
      if (current === null) return
      const assetGeneration = generation.current
      const surfaces = surfaceCommands.current
      const exclusionsState = exclusionCommands.current
      const isCurrentSession = () =>
        session.current === current && generation.current === assetGeneration
      const bounds = node.getBoundingClientRect()
      const surface = {
        left: bounds.left,
        top: bounds.top,
        width: bounds.width,
        height: bounds.height,
        scaleFactor: window.devicePixelRatio || 1,
      }

      const navigationBounds = navigationElement.current?.getBoundingClientRect()
      const root = node.closest('.image-preview')
      const editorBounds =
        root === null
          ? []
          : Array.from(root.querySelectorAll<HTMLElement>('[data-native-input-exclusion="true"]'))
              .map((element) => rectFromBounds(element.getBoundingClientRect()))
              .filter(validRect)
      const exclusions = [
        ...(navigationBounds === undefined ? [] : [rectFromBounds(navigationBounds)]),
        ...editorBounds,
      ]
      const exclusionSequence = exclusions.every(validRect)
        ? exclusionsState.enqueue(exclusions)
        : null
      if (exclusionSequence !== null) {
        void dispatch(
          current,
          { type: 'set_input_exclusions', exclusions },
          sceneRevision.current,
        ).then(
          (ack) => {
            if (!isCurrentSession()) return
            exclusionsState.settle(
              exclusionSequence,
              exclusions,
              ack.disposition === 'applied' || ack.disposition === 'ignored_duplicate',
            )
          },
          (cause) => {
            if (!isCurrentSession()) return
            if (exclusionsState.settle(exclusionSequence, exclusions, false)) {
              setError(nativeFailureCopy(errorCode(cause)))
            }
          },
        )
      }

      if (validSurface(surface)) {
        setStageSize({ width: surface.width, height: surface.height })
        const surfaceSequence = surfaces.enqueue(surface)
        if (surfaceSequence !== null) {
          void dispatch(current, { type: 'set_surface', surface }, sceneRevision.current).then(
            (ack) => {
              if (!isCurrentSession()) return
              surfaces.settle(
                surfaceSequence,
                surface,
                ack.disposition === 'applied' || ack.disposition === 'ignored_duplicate',
              )
            },
            (cause) => {
              if (!isCurrentSession()) return
              if (surfaces.settle(surfaceSequence, surface, false)) {
                setError(nativeFailureCopy(errorCode(cause)))
              }
            },
          )
        }
      }
    }

    sync()
    const observer = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(sync)
    // Fixed-position controls (for example the annotation tool menu) can be
    // mounted after the native surface has been initialized and may overlap
    // its input region. Observe structural changes so their exclusion bounds
    // reach the native router before the next pointer event.
    const mutationObserver =
      typeof MutationObserver === 'undefined' ? null : new MutationObserver(sync)
    observer?.observe(node)
    if (navigationElement.current !== null) observer?.observe(navigationElement.current)
    const root = node.closest('.image-preview')
    for (const element of root?.querySelectorAll<HTMLElement>(
      '[data-native-input-exclusion="true"]',
    ) ?? []) {
      observer?.observe(element)
    }
    mutationObserver?.observe(root ?? node, { childList: true, subtree: true })
    window.addEventListener('resize', sync)
    window.visualViewport?.addEventListener('resize', sync)
    return () => {
      disposed = true
      observer?.disconnect()
      mutationObserver?.disconnect()
      window.removeEventListener('resize', sync)
      window.visualViewport?.removeEventListener('resize', sync)
    }
  }, [
    activeGeneration,
    camera,
    error,
    nativeBinding?.inputExclusionRevision,
    sessionEpoch,
    transformsDisabled,
  ])

  const sendCamera = useCallback((next: ImageViewportState) => {
    desiredView.current = { ...desiredView.current, camera: next }
    setCamera(next)
    const current = session.current
    if (current !== null)
      void dispatch(
        current,
        { type: 'camera', camera: toRendererCamera(next) },
        sceneRevision.current,
      )
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
    const next =
      desiredView.current.magnifier === null ? nativeMagnifierPreferences(magnifier) : null
    desiredView.current = { ...desiredView.current, magnifier: next }
    setMagnifierEnabled(next !== null)
    const current = session.current
    if (current !== null) {
      void dispatch(current, { type: 'set_magnifier', magnifier: next }, sceneRevision.current)
    }
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
        <div data-native-input-exclusion="true">
          <ViewerLocalFeedback
            tone="danger"
            title="无法显示这张图片"
            action={
              <ViewerButton
                onClick={() => {
                  retryView.current = {
                    entityId: file.entityId,
                    camera,
                    magnifier: magnifierEnabled ? nativeMagnifierPreferences(magnifier) : null,
                  }
                  setRetryEpoch((epoch) => epoch + 1)
                }}
              >
                重试预览
              </ViewerButton>
            }
          >
            {error}
          </ViewerLocalFeedback>
        </div>
      )}
      {error === null && detailUnavailable && (
        <div style={{ pointerEvents: 'none' }}>
          <ViewerLocalFeedback tone="warning" title="高清细节暂不可用">
            当前预览尚未恢复所需的清晰度，资源恢复后会自动更新。
          </ViewerLocalFeedback>
        </div>
      )}
      {magnifierAnnounced && (
        <span className="visually-hidden" aria-live="polite">
          {magnifierEnabled ? '放大镜已开启' : '放大镜已关闭'}
        </span>
      )}
      {(slots?.nativeStageOverlay ?? slots?.stageOverlay)?.(
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

function dispatch(
  session: ImageRendererSession,
  command: ImageRendererCommand['command'],
  sceneRevision: number,
) {
  return session.dispatch({ sceneRevision, command })
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
