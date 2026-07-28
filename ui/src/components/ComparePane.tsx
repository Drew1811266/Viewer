import type { CSSProperties, PointerEvent as ReactPointerEvent } from 'react'
import { useEffect, useRef, useState } from 'react'
import type {
  BrowserFile,
  ImageRepresentation,
  ImageRepresentationRequest,
  ReviewState,
} from '../api/types'
import type { PaneMetrics, PaneTransform } from '../state/compareModel'
import { MarkerButtons } from './MarkerControls'

interface ComparePaneProps {
  file: BrowserFile
  transform: PaneTransform
  active: boolean
  useOriginal: boolean
  readOnly: boolean
  requestImage: (
    file: BrowserFile,
    representation: ImageRepresentationRequest,
    signal?: AbortSignal,
  ) => Promise<ImageRepresentation>
  onActivate: () => void
  onMetrics: (entityId: string, metrics: PaneMetrics) => void
  onPan: (entityId: string, deltaX: number, deltaY: number) => void
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
  useOriginal,
  readOnly,
  requestImage,
  onActivate,
  onMetrics,
  onPan,
  onRemove,
  onSetReview,
  onToggleFavorite,
  onOriginalUnavailable,
}: ComparePaneProps) {
  const stageRef = useRef<HTMLDivElement>(null)
  const proxyRevision = useRef(0)
  const originalRevision = useRef(0)
  const dragStart = useRef<{ x: number; y: number } | null>(null)
  const fileRef = useRef(file)
  const metricsCallbackRef = useRef(onMetrics)
  const originalUnavailableRef = useRef(onOriginalUnavailable)
  fileRef.current = file
  metricsCallbackRef.current = onMetrics
  originalUnavailableRef.current = onOriginalUnavailable
  const [viewport, setViewport] = useState<Viewport | null>(null)
  const [proxy, setProxy] = useState<LoadedProxy | null>(null)
  const [original, setOriginal] = useState<LoadedRepresentation | null>(null)
  const [proxyError, setProxyError] = useState<string | null>(null)
  const [originalError, setOriginalError] = useState<string | null>(null)

  useEffect(() => {
    setProxy(null)
    setOriginal(null)
    setProxyError(null)
    setOriginalError(null)
  }, [file.entityId, file.modifiedNs, file.size])

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
    if (viewport === null) return
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
  }, [file.entityId, file.modifiedNs, file.size, proxyQuarterTurn, requestImage, viewport])

  useEffect(() => {
    if (!useOriginal) {
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
  }, [file.entityId, file.modifiedNs, file.size, requestImage, useOriginal])

  useEffect(() => {
    const representation = visibleRepresentation(
      proxy,
      original,
      file.entityId,
      fileSourceRevision(file),
      useOriginal,
      proxyQuarterTurn,
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
    proxyQuarterTurn,
    useOriginal,
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

  const representation = visibleRepresentation(
    proxy,
    original,
    file.entityId,
    fileSourceRevision(file),
    useOriginal,
    proxyQuarterTurn,
  )
  const geometry = displayGeometry(file, representation, viewport, transform)
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
  const error = useOriginal ? (originalError ?? proxyError) : proxyError

  return (
    <article
      className={`compare-pane${active ? ' is-active' : ''}`}
      aria-label={`对比 ${file.name}`}
      tabIndex={0}
      data-scale={transform.scale}
      data-rotation={transform.rotation}
      onFocus={onActivate}
      onPointerDown={onActivate}
    >
      <header className="compare-pane-header">
        <strong title={file.name}>{file.name}</strong>
        <button
          type="button"
          aria-label={`移除 ${file.name}`}
          title={`从对比中移除 ${file.name}`}
          onClick={() => onRemove(file.entityId)}
        >
          ×
        </button>
      </header>
      <div
        ref={stageRef}
        className="compare-pane-stage"
        onPointerDown={beginPan}
        onPointerMove={continuePan}
        onPointerUp={endPan}
        onPointerCancel={endPan}
      >
        {representation && (
          <img src={representation.url} alt={file.name} draggable={false} style={imageStyle} />
        )}
        {representation === null && error === null && <span role="status">正在载入…</span>}
      </div>
      {error && <p role="alert">{error}</p>}
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
  useOriginal: boolean,
  quarterTurn: boolean,
): ImageRepresentation | null {
  if (
    useOriginal &&
    original?.entityId === entityId &&
    original.sourceRevision === sourceRevision
  ) {
    return original.image
  }
  return proxy?.entityId === entityId &&
    proxy.sourceRevision === sourceRevision &&
    proxy.quarterTurn === quarterTurn
    ? proxy.image
    : null
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
