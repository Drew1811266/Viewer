import type { KeyboardEvent, MutableRefObject, PointerEvent, ReactNode } from 'react'
import { useCallback, useEffect, useRef, useState } from 'react'
import type {
  BrowserFile,
  ImageRepresentation,
  ImageRepresentationRequest,
  MagnifierPreferences,
} from '../api/types'
import { isPreviewableImage } from '../fileKinds'
import ImageMagnifier, { type ImageMagnifierHandle } from './imagePreview/ImageMagnifier'
import {
  type Point,
  remapSourcePoint,
  type Size,
  sourcePointAtStagePoint,
} from './imagePreview/imageGeometry'
import { type CurrentOriginalState, useCurrentOriginal } from './imagePreview/useCurrentOriginal'
import { useImageViewport } from './imagePreview/useImageViewport'
import { usePreviewGestures } from './imagePreview/usePreviewGestures'
import UnsupportedFileState from './UnsupportedFileState'
import ViewerButton, { ViewerIconButton } from './ui/ViewerButton'
import ViewerLocalFeedback from './ui/ViewerLocalFeedback'
import ViewerSegmentedControl from './ui/ViewerSegmentedControl'
import ViewerToolbar from './ui/ViewerToolbar'

interface ImagePreviewProps {
  file: BrowserFile
  files: BrowserFile[]
  magnifier: MagnifierPreferences
  pointerClientPoint: MutableRefObject<Point | null>
  unavailableEntityIds?: ReadonlySet<string>
  requestImage: (
    file: BrowserFile,
    representation: ImageRepresentationRequest,
    signal?: AbortSignal,
  ) => Promise<ImageRepresentation>
  onNavigate: (file: BrowserFile) => void
  onClose: () => void
  onDimensions?: (entityId: string, width: number, height: number) => void
}

const EMPTY_ENTITY_IDS: ReadonlySet<string> = new Set()
const EMPTY_STAGE: Size = { width: 0, height: 0 }
const DEFAULT_STAGE: Size = { width: 640, height: 480 }

export default function ImagePreview({
  file,
  files,
  magnifier,
  pointerClientPoint,
  unavailableEntityIds = EMPTY_ENTITY_IDS,
  requestImage,
  onNavigate,
  onClose,
  onDimensions,
}: ImagePreviewProps) {
  const fitCache = useRef(new Map<string, ImageRepresentation>())
  const dialog = useRef<HTMLElement>(null)
  const stage = useRef<HTMLDivElement>(null)
  const magnifierHandle = useRef<ImageMagnifierHandle>(null)
  const pendingFit = useRef(new Map<string, Promise<ImageRepresentation>>())
  const allowedWindow = useRef(new Set<string>())
  const lastStagePoint = useRef<Point | null>(null)
  const [, refresh] = useState(0)
  const [stageSize, setStageSize] = useState(EMPTY_STAGE)
  const [magnifierEnabled, setMagnifierEnabled] = useState(false)
  const [magnifierAnnounced, setMagnifierAnnounced] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const currentIndex = files.findIndex((candidate) => candidate.entityId === file.entityId)
  const unavailable = unavailableEntityIds.has(file.entityId)
  const transformsDisabled = unavailable || !isPreviewableImage(file)
  const viewport = useImageViewport({ stage: EMPTY_STAGE, source: EMPTY_STAGE, fitInset: 0.9 })
  const scalePercent = Math.round((viewport.state.mode === 'free' ? viewport.state.zoom : 1) * 100)
  const [announcedScalePercent, setAnnouncedScalePercent] = useState(scalePercent)
  const original = useCurrentOriginal({
    file,
    available: !transformsDisabled,
    requestImage,
  })
  const fitRepresentation = transformsDisabled ? undefined : fitCache.current.get(file.entityId)
  const currentOriginal: CurrentOriginalState =
    original.entityId === file.entityId
      ? original
      : { status: 'loading', entityId: file.entityId, representation: null }
  const originalRepresentation =
    currentOriginal.status === 'ready' ? currentOriginal.representation : null
  const representation = originalRepresentation ?? fitRepresentation
  const sourceDimensions: Size =
    file.imageMetadata ?? originalRepresentation ?? fitRepresentation ?? EMPTY_STAGE
  const gestures = usePreviewGestures({
    stage,
    disabled: transformsDisabled || representation == null,
    panBounds: viewport.panBounds,
    zoomBy: viewport.zoomBy,
    panBy: viewport.panBy,
  })

  useEffect(() => {
    const previous = document.activeElement
    dialog.current?.focus()
    return () => {
      if (previous instanceof HTMLElement && previous.isConnected) previous.focus()
    }
  }, [])

  useEffect(() => {
    viewport.resetForEntity()
    setError(null)
    lastStagePoint.current = null
    hideMagnifier(magnifierHandle, stage)
  }, [file.entityId, viewport.resetForEntity])

  useEffect(() => {
    const timer = window.setTimeout(() => setAnnouncedScalePercent(scalePercent), 300)
    return () => window.clearTimeout(timer)
  }, [scalePercent])

  useEffect(() => {
    const element = stage.current
    if (element === null) return
    const publish = (size: Size) => {
      if (size.width > 0 && size.height > 0) {
        setStageSize({ width: Math.round(size.width), height: Math.round(size.height) })
      }
    }
    const bounds = element.getBoundingClientRect()
    publish({
      width: bounds.width || element.clientWidth || DEFAULT_STAGE.width,
      height: bounds.height || element.clientHeight || DEFAULT_STAGE.height,
    })
    if (typeof ResizeObserver === 'undefined') return
    const observer = new ResizeObserver((entries) => {
      const content = entries[0]?.contentRect
      if (content) publish(content)
    })
    observer.observe(element)
    return () => observer.disconnect()
  }, [])

  useEffect(() => {
    const windowFiles = files.slice(Math.max(0, currentIndex - 1), currentIndex + 2)
    const allowed = new Set(
      windowFiles
        .filter(
          (candidate) =>
            isPreviewableImage(candidate) && !unavailableEntityIds.has(candidate.entityId),
        )
        .map((candidate) => candidate.entityId),
    )
    allowedWindow.current = allowed
    for (const entityId of fitCache.current.keys()) {
      if (!allowed.has(entityId)) fitCache.current.delete(entityId)
    }
    for (const candidate of windowFiles) {
      if (
        !isPreviewableImage(candidate) ||
        unavailableEntityIds.has(candidate.entityId) ||
        fitCache.current.has(candidate.entityId) ||
        pendingFit.current.has(candidate.entityId)
      ) {
        continue
      }
      const request = requestImage(candidate, {
        kind: 'fit_preview',
        maxWidth: 2_400,
        maxHeight: 2_400,
        scaleMilli: 1_000,
      })
      pendingFit.current.set(candidate.entityId, request)
      void request.then(
        (loaded) => {
          pendingFit.current.delete(candidate.entityId)
          if (!allowedWindow.current.has(candidate.entityId)) return
          fitCache.current.set(candidate.entityId, loaded)
          refresh((value) => value + 1)
        },
        () => {
          pendingFit.current.delete(candidate.entityId)
          if (candidate.entityId === file.entityId) setError('无法预览该图片。')
        },
      )
    }
  }, [currentIndex, file.entityId, files, requestImage, unavailableEntityIds])

  useEffect(() => {
    viewport.setMeasurements(stageSize, sourceDimensions)
  }, [sourceDimensions, stageSize, viewport.setMeasurements])

  useEffect(() => {
    if (originalRepresentation !== null) {
      onDimensions?.(file.entityId, originalRepresentation.width, originalRepresentation.height)
    }
  }, [file.entityId, onDimensions, originalRepresentation])

  const placeMagnifier = useCallback(
    (stagePoint: Point) => {
      const sourcePoint = sourcePointAtStagePoint(stagePoint, viewport.state, viewport.geometry)
      if (
        !magnifierEnabled ||
        transformsDisabled ||
        representation == null ||
        sourcePoint === null
      ) {
        hideMagnifier(magnifierHandle, stage)
        return
      }
      const originalSize = originalRepresentation ?? file.imageMetadata ?? representation
      magnifierHandle.current?.place({
        stagePoint,
        sourcePoint: remapSourcePoint(sourcePoint, viewport.geometry.source, originalSize),
      })
      if (stage.current) stage.current.dataset.magnifierOverImage = 'true'
    },
    [
      file.imageMetadata,
      magnifierEnabled,
      originalRepresentation,
      representation,
      transformsDisabled,
      viewport.geometry,
      viewport.state,
    ],
  )

  const placeLatestMagnifier = useCallback(() => {
    const element = stage.current
    const clientPoint = pointerClientPoint.current
    if (element === null || clientPoint === null) {
      hideMagnifier(magnifierHandle, stage)
      return
    }
    const bounds = element.getBoundingClientRect()
    const stagePoint = {
      x: clientPoint.x - bounds.left,
      y: clientPoint.y - bounds.top,
    }
    lastStagePoint.current = stagePoint
    placeMagnifier(stagePoint)
  }, [placeMagnifier, pointerClientPoint])

  useEffect(() => {
    if (!magnifierEnabled || transformsDisabled || representation == null) {
      hideMagnifier(magnifierHandle, stage)
      return
    }
    placeLatestMagnifier()
  }, [
    magnifier.area,
    magnifier.magnification,
    magnifier.shape,
    magnifierEnabled,
    placeLatestMagnifier,
    representation,
    stageSize,
    transformsDisabled,
    viewport.state.rotation,
  ])

  function navigate(delta: number) {
    const next = files[currentIndex + delta]
    if (next) onNavigate(next)
  }

  function toggleMagnifier() {
    setMagnifierAnnounced(true)
    setMagnifierEnabled((current) => !current)
  }

  function keyboard(event: KeyboardEvent<HTMLElement>) {
    if (ownsMagnifierShortcut(event) && !transformsDisabled) {
      event.preventDefault()
      toggleMagnifier()
      return
    }
    if (event.key === 'Escape') {
      event.preventDefault()
      onClose()
    }
    if (event.key === 'ArrowLeft') {
      event.preventDefault()
      navigate(-1)
    }
    if (event.key === 'ArrowRight') {
      event.preventDefault()
      navigate(1)
    }
  }

  function zoomFromToolbar(factor: number) {
    viewport.zoomBy(factor, {
      x: viewport.geometry.stage.width / 2,
      y: viewport.geometry.stage.height / 2,
    })
  }

  function sampleMagnifier(event: PointerEvent<HTMLDivElement>) {
    gestures.onPointerMove(event)
    const point = recordPointer(event)
    lastStagePoint.current = point
    placeMagnifier(point)
  }

  function startPointer(event: PointerEvent<HTMLDivElement>) {
    gestures.onPointerDown(event)
    const point = recordPointer(event)
    lastStagePoint.current = point
    placeMagnifier(point)
  }

  function recordPointer(event: PointerEvent<HTMLDivElement>): Point {
    pointerClientPoint.current = { x: event.clientX, y: event.clientY }
    const bounds = event.currentTarget.getBoundingClientRect()
    return { x: event.clientX - bounds.left, y: event.clientY - bounds.top }
  }

  function stopMagnifier(event: PointerEvent<HTMLDivElement>) {
    pointerClientPoint.current = { x: event.clientX, y: event.clientY }
    lastStagePoint.current = null
    hideMagnifier(magnifierHandle, stage)
  }

  const previewDimensions = file.imageMetadata ?? originalRepresentation ?? fitRepresentation
  const previewMetadata = [
    previewDimensions ? `${previewDimensions.width} × ${previewDimensions.height} px` : null,
    formatBytes(file.size),
  ]
    .filter((value): value is string => value !== null)
    .join(' · ')
  const previewIdentity: ReactNode = (
    <>
      <strong>{file.name}</strong>
      <span>{previewMetadata}</span>
    </>
  )
  const displayControls: ReactNode = (
    <ViewerSegmentedControl label="图片显示控制">
      <ViewerButton
        active={viewport.state.mode === 'fit'}
        disabled={transformsDisabled}
        onClick={() => {
          setError(null)
          viewport.setFit()
        }}
      >
        适应窗口
      </ViewerButton>
      <ViewerIconButton
        icon="minus"
        label="缩小"
        disabled={transformsDisabled}
        onClick={() => zoomFromToolbar(0.8)}
      />
      <span className="preview-scale-label">{scalePercent}%</span>
      <span className="visually-hidden" data-testid="preview-scale-announcement" aria-live="polite">
        缩放比例 {announcedScalePercent}%
      </span>
      <ViewerIconButton
        icon="plus"
        label="放大"
        disabled={transformsDisabled}
        onClick={() => zoomFromToolbar(1.25)}
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
        onClick={viewport.rotateClockwise}
      />
      <ViewerButton
        tone="quiet"
        className="preview-complete-action"
        aria-label="关闭预览"
        onClick={onClose}
      >
        完成
      </ViewerButton>
    </>
  )
  const previewStage: ReactNode = (
    <div
      ref={stage}
      className="image-preview-stage"
      onPointerDown={startPointer}
      onPointerMove={sampleMagnifier}
      onPointerEnter={sampleMagnifier}
      onPointerLeave={stopMagnifier}
      onPointerUp={gestures.onPointerUp}
      onPointerCancel={gestures.onPointerCancel}
      onLostPointerCapture={gestures.onLostPointerCapture}
    >
      {unavailable ? (
        <UnsupportedFileState file={file} unavailable />
      ) : !isPreviewableImage(file) ? (
        <UnsupportedFileState file={file} />
      ) : representation ? (
        <img
          className="image-preview-image"
          src={representation.url}
          alt={file.name}
          width={sourceDimensions.width}
          height={sourceDimensions.height}
          draggable={false}
          data-mode={viewport.state.mode}
          data-representation={originalRepresentation === null ? 'fit' : 'original'}
          style={{ transform: viewport.transform }}
        />
      ) : null}
      {!unavailable &&
        isPreviewableImage(file) &&
        (representation === undefined || representation === null) &&
        !isFatalImageFailure(error, currentOriginal.status) && (
          <ViewerLocalFeedback tone="info" title="正在载入图片">
            正在准备高分辨率预览…
          </ViewerLocalFeedback>
        )}
      {fitRepresentation !== undefined && originalFallbackCopy(currentOriginal.status) !== null && (
        <ViewerLocalFeedback tone="warning" title="正在使用适窗预览">
          {originalFallbackCopy(currentOriginal.status)}
        </ViewerLocalFeedback>
      )}
      {representation == null && isFatalImageFailure(error, currentOriginal.status) && (
        <ViewerLocalFeedback tone="danger" title="无法显示这张图片">
          {error}
        </ViewerLocalFeedback>
      )}
      <ImageMagnifier
        ref={magnifierHandle}
        shape={magnifier.shape}
        area={magnifier.area}
        magnification={magnifier.magnification}
        sourceScale={viewport.scale}
        stageSize={stageSize}
        rotation={viewport.state.rotation}
        fileName={file.name}
        original={currentOriginal}
      />
      {magnifierAnnouncement(magnifierEnabled, currentOriginal.status)}
      {magnifierAnnounced && (
        <span className="visually-hidden" aria-live="polite">
          {magnifierEnabled ? '放大镜已开启' : '放大镜已关闭'}
        </span>
      )}
    </div>
  )
  const previewNavigation: ReactNode = (
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
    <section
      ref={dialog}
      className="preview-overlay image-preview"
      role="dialog"
      aria-label={`图片预览 ${file.name}`}
      tabIndex={-1}
      onKeyDown={keyboard}
    >
      <ViewerToolbar
        label="图片预览工具"
        leading={previewIdentity}
        center={displayControls}
        actions={previewActions}
      />
      {previewStage}
      <nav className="preview-navigation-float" aria-label="图片导航">
        {previewNavigation}
      </nav>
    </section>
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

function hideMagnifier(
  magnifier: { current: ImageMagnifierHandle | null },
  stage: { current: HTMLElement | null },
) {
  magnifier.current?.hide()
  stage.current?.removeAttribute('data-magnifier-over-image')
}

function magnifierAnnouncement(
  enabled: boolean,
  status: ReturnType<typeof useCurrentOriginal>['status'],
): ReactNode {
  if (!enabled || (status !== 'budget_error' && status !== 'error')) return null
  return (
    <span className="visually-hidden" role="status" aria-live="polite">
      {status === 'budget_error' ? '放大镜原图超出安全预览限制' : '放大镜无法载入原图'}
    </span>
  )
}

function originalFallbackCopy(status: CurrentOriginalState['status']): string | null {
  if (status === 'budget_error') return '原图超出安全预览限制，已继续使用适窗预览。'
  if (status === 'error') return '无法加载原图，已继续使用适窗预览。'
  return null
}

function isFatalImageFailure(
  fitError: string | null,
  originalStatus: CurrentOriginalState['status'],
): boolean {
  return fitError !== null && originalFallbackCopy(originalStatus) !== null
}

function formatBytes(bytes: number): string {
  if (bytes < 1_024) return `${bytes} B`
  if (bytes < 1_024 ** 2) return `${formatUnit(bytes / 1_024)} KiB`
  if (bytes < 1_024 ** 3) return `${formatUnit(bytes / 1_024 ** 2)} MiB`
  return `${formatUnit(bytes / 1_024 ** 3)} GiB`
}

function formatUnit(value: number): string {
  return value >= 10 ? value.toFixed(0) : value.toFixed(1).replace(/\.0$/, '')
}
