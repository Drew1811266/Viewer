import type { CSSProperties, PointerEvent as ReactPointerEvent } from 'react'
import { useEffect, useRef, useState } from 'react'
import type {
  BrowserFile,
  ImageRepresentation,
  ImageRepresentationRequest,
  ReviewState,
} from '../api/types'
import { isPreviewableImage } from '../fileKinds'
import type { PaneMetrics, PaneTransform } from '../state/compareModel'
import { MarkerButtons } from './MarkerControls'
import UnsupportedFileState from './UnsupportedFileState'
import { ViewerIconButton } from './ui/ViewerButton'
import ViewerLocalFeedback from './ui/ViewerLocalFeedback'

interface ComparePaneProps {
  file: BrowserFile
  transform: PaneTransform
  active: boolean
  readOnly: boolean
  requestImage: (
    file: BrowserFile,
    representation: ImageRepresentationRequest,
    signal?: AbortSignal,
  ) => Promise<ImageRepresentation>
  onActivate: () => void
  onMetrics: (entityId: string, metrics: PaneMetrics) => void
  onPan: (entityId: string, deltaX: number, deltaY: number) => void
  onZoomAt: (entityId: string, factor: number, anchorX: number, anchorY: number) => void
  panWheelEnabled: boolean
  onRemove: (entityId: string) => void
  onSetReview: (entityId: string, review: ReviewState | null) => void
  onToggleFavorite: (entityId: string) => void
  onOriginalUnavailable: (entityId: string, reason: 'budget' | 'load') => void
}

interface Viewport {
  width: number
  height: number
}

interface DisplayGeometry {
  baseWidth: number
  baseHeight: number
  displayedWidth: number
  displayedHeight: number
}

interface LoadedRepresentation {
  entityId: string
  sourceRevision: string
  image: ImageRepresentation
}

interface LoadedProxy extends LoadedRepresentation {
  quarterTurn: boolean
}

export default function ComparePane({
  file,
  transform,
  active,
  readOnly,
  requestImage,
  onActivate,
  onMetrics,
  onPan,
  onZoomAt,
  panWheelEnabled,
  onRemove,
  onSetReview,
  onToggleFavorite,
  onOriginalUnavailable,
}: ComparePaneProps) {
  const stageRef = useRef<HTMLDivElement>(null)
  const proxyRevision = useRef(0)
  const originalRevision = useRef(0)
  const dragStart = useRef<{ x: number; y: number } | null>(null)
  const geometryRef = useRef<DisplayGeometry | null>(null)
  const viewportRef = useRef<Viewport | null>(null)
  const wheelFrame = useRef<number | null>(null)
  const pendingWheel = useRef<
    | { kind: 'pinch'; deltaY: number; anchorX: number; anchorY: number }
    | { kind: 'pan'; deltaX: number; deltaY: number }
    | null
  >(null)
  const panRef = useRef(onPan)
  const zoomAtRef = useRef(onZoomAt)
  const fileRef = useRef(file)
  const metricsCallbackRef = useRef(onMetrics)
  const originalUnavailableRef = useRef(onOriginalUnavailable)
  fileRef.current = file
  panRef.current = onPan
  zoomAtRef.current = onZoomAt
  metricsCallbackRef.current = onMetrics
  originalUnavailableRef.current = onOriginalUnavailable
  const [viewport, setViewport] = useState<Viewport | null>(null)
  const [proxy, setProxy] = useState<LoadedProxy | null>(null)
  const [original, setOriginal] = useState<LoadedRepresentation | null>(null)
  const [proxyError, setProxyError] = useState<string | null>(null)
  const [originalError, setOriginalError] = useState<string | null>(null)
  const [showOriginal, setShowOriginal] = useState(false)
  viewportRef.current = viewport
  const previewable = isPreviewableImage(file)

  useEffect(() => {
    setProxy(null)
    setOriginal(null)
    setProxyError(null)
    setOriginalError(null)
  }, [file.entityId, file.kind, file.modifiedNs, file.size])

  const proxyQuarterTurn = transform.rotation === 90 || transform.rotation === 270

  useEffect(() => {
    const stage = stageRef.current
    if (stage === null) return
    if (typeof ResizeObserver === 'undefined') {
      const bounds = stage.getBoundingClientRect()
      setViewport({
        width: positiveDimension(bounds.width, 640),
        height: positiveDimension(bounds.height, 480),
      })
      return
    }
    const observer = new ResizeObserver((entries) => {
      const bounds = entries[0]?.contentRect
      if (bounds === undefined || bounds.width <= 0 || bounds.height <= 0) return
      setViewport({ width: Math.round(bounds.width), height: Math.round(bounds.height) })
    })
    observer.observe(stage)
    return () => observer.disconnect()
  }, [])

  useEffect(() => {
    if (!previewable || viewport === null) return
    const revision = ++proxyRevision.current
    const controller = new AbortController()
    setProxyError(null)
    const requested: ImageRepresentationRequest = {
      kind: 'fit_preview',
      maxWidth: proxyQuarterTurn ? viewport.height : viewport.width,
      maxHeight: proxyQuarterTurn ? viewport.width : viewport.height,
      scaleMilli: deviceScaleMilli(),
    }
    const entityId = file.entityId
    const sourceRevision = fileSourceRevision(file)
    void requestImage(fileRef.current, requested, controller.signal).then(
      (image) => {
        if (proxyRevision.current !== revision) return
        setProxy({ entityId, sourceRevision, image, quarterTurn: proxyQuarterTurn })
      },
      (caught: unknown) => {
        if (proxyRevision.current !== revision) return
        if (requestWasAborted(caught, controller.signal)) return
        setProxyError('无法预览该图片。')
      },
    )
    return () => {
      controller.abort()
      if (proxyRevision.current === revision) proxyRevision.current += 1
    }
  }, [
    file.entityId,
    file.kind,
    file.modifiedNs,
    file.size,
    previewable,
    proxyQuarterTurn,
    requestImage,
    viewport,
  ])

  useEffect(() => {
    if (!previewable) {
      originalRevision.current += 1
      setOriginal(null)
      setOriginalError(null)
      return
    }
    const revision = ++originalRevision.current
    const controller = new AbortController()
    const entityId = file.entityId
    const sourceRevision = fileSourceRevision(file)
    setOriginalError(null)
    void requestImage(fileRef.current, { kind: 'original100_percent' }, controller.signal).then(
      (image) => {
        if (originalRevision.current !== revision) return
        setOriginal({ entityId, sourceRevision, image })
      },
      (caught: unknown) => {
        if (originalRevision.current !== revision) return
        if (requestWasAborted(caught, controller.signal)) return
        const budgetExceeded = commandCode(caught) === 'image_budget_exceeded'
        setOriginalError(
          budgetExceeded
            ? '原图超出安全预览限制，继续使用适窗代理。'
            : '无法加载原图，继续使用适窗代理。',
        )
        originalUnavailableRef.current(entityId, budgetExceeded ? 'budget' : 'load')
      },
    )
    return () => {
      controller.abort()
      if (originalRevision.current === revision) originalRevision.current += 1
    }
  }, [file.entityId, file.kind, file.modifiedNs, file.size, previewable, requestImage])

  useEffect(() => {
    setShowOriginal(false)
    if (original === null || transform.scale <= 1.05) return
    // Keep the fitted proxy on screen while the gesture is changing scale.
    // The full-resolution image is swapped in only after the gesture settles.
    const timer = window.setTimeout(() => setShowOriginal(true), 180)
    return () => window.clearTimeout(timer)
  }, [original, transform.scale])

  useEffect(() => {
    if (!previewable) return
    const representation = visibleRepresentation(
      proxy,
      original,
      file.entityId,
      fileSourceRevision(file),
      proxyQuarterTurn,
      showOriginal,
    )
    if (representation === null || viewport === null) return
    metricsCallbackRef.current(file.entityId, {
      imageWidth: file.imageMetadata?.width ?? representation.width,
      imageHeight: file.imageMetadata?.height ?? representation.height,
      viewportWidth: viewport.width,
      viewportHeight: viewport.height,
    })
  }, [
    file.entityId,
    file.imageMetadata?.height,
    file.imageMetadata?.width,
    original,
    proxy,
    previewable,
    proxyQuarterTurn,
    showOriginal,
    viewport,
  ])

  function beginPan(event: ReactPointerEvent<HTMLDivElement>) {
    if (event.button !== 0) return
    onActivate()
    dragStart.current = { x: event.clientX, y: event.clientY }
    event.currentTarget.setPointerCapture?.(event.pointerId)
  }

  function continuePan(event: ReactPointerEvent<HTMLDivElement>) {
    const previous = dragStart.current
    if (previous === null || geometry === null) return
    const deltaX = -(event.clientX - previous.x) / geometry.displayedWidth
    const deltaY = -(event.clientY - previous.y) / geometry.displayedHeight
    dragStart.current = { x: event.clientX, y: event.clientY }
    onPan(file.entityId, deltaX, deltaY)
  }

  function endPan(event: ReactPointerEvent<HTMLDivElement>) {
    dragStart.current = null
    if (event.currentTarget.hasPointerCapture?.(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId)
    }
  }

  const representation = previewable
    ? visibleRepresentation(
        proxy,
        original,
        file.entityId,
        fileSourceRevision(file),
        proxyQuarterTurn,
        showOriginal,
      )
    : null
  const geometry = previewable ? displayGeometry(file, representation, viewport, transform) : null
  geometryRef.current = geometry
  const imageStyle: CSSProperties =
    geometry === null
      ? {}
      : {
          width: geometry.baseWidth,
          height: geometry.baseHeight,
          maxWidth: 'none',
          maxHeight: 'none',
          transform: `translate(${cssNumber((0.5 - transform.centerX) * geometry.displayedWidth)}px, ${cssNumber((0.5 - transform.centerY) * geometry.displayedHeight)}px) rotate(${transform.rotation}deg) scale(${transform.scale})`,
        }
  const error = previewable ? (originalError ?? proxyError) : null

  useEffect(() => {
    const stage = stageRef.current
    if (stage === null) return
    const flush = () => {
      wheelFrame.current = null
      const action = pendingWheel.current
      pendingWheel.current = null
      const currentGeometry = geometryRef.current
      if (action === null || currentGeometry === null) return
      if (action.kind === 'pinch') {
        zoomAtRef.current(
          fileRef.current.entityId,
          Math.exp(-action.deltaY * 0.004),
          action.anchorX,
          action.anchorY,
        )
      } else {
        panRef.current(
          fileRef.current.entityId,
          action.deltaX / currentGeometry.displayedWidth,
          action.deltaY / currentGeometry.displayedHeight,
        )
      }
    }
    const schedule = () => {
      if (wheelFrame.current !== null) return
      wheelFrame.current = requestAnimationFrame(flush)
    }
    const onWheel = (event: WheelEvent) => {
      if (!event.cancelable) return
      const currentGeometry = geometryRef.current
      if (currentGeometry === null) return
      const bounds = stage.getBoundingClientRect()
      const measured = viewportRef.current
      const width = bounds.width > 0 ? bounds.width : (measured?.width ?? 0)
      const height = bounds.height > 0 ? bounds.height : (measured?.height ?? 0)
      if (width <= 0 || height <= 0) return
      const modeScale =
        event.deltaMode === WheelEvent.DOM_DELTA_LINE
          ? 16
          : event.deltaMode === WheelEvent.DOM_DELTA_PAGE
            ? height
            : 1
      const deltaX = event.deltaX * modeScale
      const deltaY = event.deltaY * modeScale
      if (!event.ctrlKey && !panWheelEnabled) return
      event.preventDefault()
      if (event.ctrlKey) {
        const anchorX = clamp((event.clientX - bounds.left) / width, 0, 1)
        const anchorY = clamp((event.clientY - bounds.top) / height, 0, 1)
        const previous = pendingWheel.current
        pendingWheel.current = {
          kind: 'pinch',
          deltaY: (previous?.kind === 'pinch' ? previous.deltaY : 0) + deltaY,
          anchorX,
          anchorY,
        }
      } else {
        const previous = pendingWheel.current
        pendingWheel.current = {
          kind: 'pan',
          deltaX: (previous?.kind === 'pan' ? previous.deltaX : 0) + deltaX,
          deltaY: (previous?.kind === 'pan' ? previous.deltaY : 0) + deltaY,
        }
      }
      schedule()
    }
    stage.addEventListener('wheel', onWheel, { passive: false })
    return () => {
      stage.removeEventListener('wheel', onWheel)
      if (wheelFrame.current !== null) cancelAnimationFrame(wheelFrame.current)
      wheelFrame.current = null
      pendingWheel.current = null
    }
  }, [panWheelEnabled])

  return (
    <article
      className={`compare-pane${active ? ' is-active' : ''}`}
      role="group"
      aria-label={`对比 ${file.name}`}
      tabIndex={0}
      data-active={active}
      data-scale={transform.scale}
      data-rotation={transform.rotation}
      onFocus={onActivate}
      onPointerDown={onActivate}
    >
      <header className="compare-pane-header">
        <strong title={file.name}>{file.name}</strong>
        <ViewerIconButton
          icon="x"
          label={`移除 ${file.name}`}
          tone="quiet"
          title={`从对比中移除 ${file.name}`}
          onClick={() => onRemove(file.entityId)}
        />
      </header>
      <div
        ref={stageRef}
        className="compare-pane-stage"
        onPointerDown={beginPan}
        onPointerMove={continuePan}
        onPointerUp={endPan}
        onPointerCancel={endPan}
      >
        {previewable ? (
          <>
            {representation && (
              <img src={representation.url} alt={file.name} draggable={false} style={imageStyle} />
            )}
            {representation === null && error === null && (
              <ViewerLocalFeedback tone="info" title="正在载入图片">
                正在准备 {file.name} 的预览…
              </ViewerLocalFeedback>
            )}
            {error !== null && (
              <ViewerLocalFeedback tone="danger" title="无法显示这张图片">
                {error}
              </ViewerLocalFeedback>
            )}
          </>
        ) : (
          <UnsupportedFileState file={file} />
        )}
      </div>
      <footer>
        <MarkerButtons
          reviewState={file.marker.reviewState}
          favorite={file.marker.favorite}
          disabled={readOnly}
          labelPrefix={`${file.name} `}
          showShortcuts={false}
          onSetReview={(review) => onSetReview(file.entityId, review)}
          onToggleFavorite={() => onToggleFavorite(file.entityId)}
        />
      </footer>
    </article>
  )
}

function deviceScaleMilli(): number {
  const ratio = Number.isFinite(window.devicePixelRatio) ? window.devicePixelRatio : 1
  return Math.round(Math.max(1, Math.min(4, ratio)) * 1_000)
}

function positiveDimension(value: number, fallback: number): number {
  return Number.isFinite(value) && value > 0 ? Math.round(value) : fallback
}

function commandCode(caught: unknown): string | null {
  if (typeof caught !== 'object' || caught === null || !('code' in caught)) return null
  return typeof caught.code === 'string' ? caught.code : null
}

function requestWasAborted(caught: unknown, signal: AbortSignal): boolean {
  return signal.aborted || (caught instanceof DOMException && caught.name === 'AbortError')
}

function visibleRepresentation(
  proxy: LoadedProxy | null,
  original: LoadedRepresentation | null,
  entityId: string,
  sourceRevision: string,
  quarterTurn: boolean,
  showOriginal: boolean,
): ImageRepresentation | null {
  const originalReady =
    original?.entityId === entityId && original.sourceRevision === sourceRevision
  const proxyReady =
    proxy?.entityId === entityId &&
    proxy.sourceRevision === sourceRevision &&
    proxy.quarterTurn === quarterTurn
  if (proxyReady && (!showOriginal || !originalReady)) return proxy.image
  return originalReady ? original.image : null
}

function fileSourceRevision(file: BrowserFile): string {
  return `${file.modifiedNs}:${file.size}`
}

function displayGeometry(
  file: BrowserFile,
  representation: ImageRepresentation | null,
  viewport: Viewport | null,
  transform: PaneTransform,
): DisplayGeometry | null {
  if (viewport === null) return null
  const width = file.imageMetadata?.width ?? representation?.width
  const height = file.imageMetadata?.height ?? representation?.height
  if (width === undefined || height === undefined || width <= 0 || height <= 0) return null
  const rotated = transform.rotation === 90 || transform.rotation === 270
  const rotatedWidth = rotated ? height : width
  const rotatedHeight = rotated ? width : height
  const fitScale = Math.min(1, viewport.width / rotatedWidth, viewport.height / rotatedHeight)
  if (!Number.isFinite(fitScale) || fitScale <= 0) return null
  return {
    baseWidth: width * fitScale,
    baseHeight: height * fitScale,
    displayedWidth: rotatedWidth * fitScale * transform.scale,
    displayedHeight: rotatedHeight * fitScale * transform.scale,
  }
}

function cssNumber(value: number): number {
  return Math.round(value * 1_000) / 1_000
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.max(minimum, Math.min(maximum, value))
}
