import type { CSSProperties } from 'react'
import { forwardRef, useCallback, useEffect, useImperativeHandle, useRef } from 'react'
import type { MagnifierArea, MagnifierMagnification, MagnifierShape } from '../../api/types'
import type { Point, PreviewRotation, Size } from './imageGeometry'
import {
  createMagnifierContentProjection,
  lensDimensions,
  magnifierShellPlacement,
  magnifierSourcePlacement,
} from './magnifierGeometry'
import type { MagnifierOverlayPainter } from './magnifierOverlay'
import type { CurrentOriginalState } from './useCurrentOriginal'

export interface MagnifierPlacement {
  stagePoint: Point
  sourcePoint: Point
}

export interface ImageMagnifierHandle {
  place: (placement: MagnifierPlacement) => void
  hide: () => void
}

interface ImageMagnifierProps {
  shape: MagnifierShape
  area: MagnifierArea
  magnification: MagnifierMagnification
  sourceScale: number
  stageSize: Size
  rotation: PreviewRotation
  fileName: string
  original: CurrentOriginalState
  overlayPainter?: MagnifierOverlayPainter
}

interface MagnifierConfiguration {
  shape: MagnifierShape
  area: MagnifierArea
  magnification: MagnifierMagnification
  sourceScale: number
  stageSize: Size
  rotation: PreviewRotation
}

const ImageMagnifier = forwardRef<ImageMagnifierHandle, ImageMagnifierProps>(
  function ImageMagnifier(
    {
      shape,
      area,
      magnification,
      sourceScale,
      stageSize,
      rotation,
      fileName,
      original,
      overlayPainter,
    },
    forwardedRef,
  ) {
    const lens = useRef<HTMLDivElement>(null)
    const sourceImage = useRef<HTMLImageElement>(null)
    const overlayCanvas = useRef<HTMLCanvasElement>(null)
    const latestPlacement = useRef<MagnifierPlacement | null>(null)
    const latestOriginal = useRef(original)
    const latestOverlayPainter = useRef(overlayPainter)
    const frame = useRef<number | null>(null)
    const configuration = useRef<MagnifierConfiguration>({
      shape,
      area,
      magnification,
      sourceScale,
      stageSize,
      rotation,
    })
    latestOriginal.current = original
    latestOverlayPainter.current = overlayPainter
    configuration.current = { shape, area, magnification, sourceScale, stageSize, rotation }
    const dimensions = lensDimensions(shape, area)

    const flush = useCallback(() => {
      frame.current = null
      const placement = latestPlacement.current
      const element = lens.current
      if (placement === null || element === null) return
      const current = configuration.current
      const currentDimensions = lensDimensions(current.shape, current.area)
      const shell = magnifierShellPlacement(
        placement.stagePoint,
        current.stageSize,
        currentDimensions,
      )
      const source = magnifierSourcePlacement(placement.sourcePoint, currentDimensions)
      setPixels(element, '--magnifier-x', shell.center.x)
      setPixels(element, '--magnifier-y', shell.center.y)
      setPixels(element, '--magnifier-shell-origin-x', shell.origin.x)
      setPixels(element, '--magnifier-shell-origin-y', shell.origin.y)
      setPixels(element, '--magnifier-source-left', source.left)
      setPixels(element, '--magnifier-source-top', source.top)
      if (sourceImage.current !== null) {
        setPixels(sourceImage.current, '--magnifier-source-left', source.left)
        setPixels(sourceImage.current, '--magnifier-source-top', source.top)
      }
      setPixels(element, '--magnifier-transform-origin-x', source.transformOriginX)
      setPixels(element, '--magnifier-transform-origin-y', source.transformOriginY)
      element.style.setProperty(
        '--magnifier-scale',
        String(current.sourceScale * current.magnification),
      )
      element.style.setProperty('--magnifier-rotation', `${current.rotation}deg`)
      paintOverlay(
        overlayCanvas.current,
        latestOverlayPainter.current,
        latestOriginal.current,
        placement.sourcePoint,
        currentDimensions,
        current.sourceScale * current.magnification,
        current.magnification,
        current.rotation,
      )
      element.dataset.visible = 'true'
    }, [])

    const schedule = useCallback(() => {
      if (frame.current === null) frame.current = requestAnimationFrame(flush)
    }, [flush])

    const place = useCallback(
      (placement: MagnifierPlacement) => {
        latestPlacement.current = placement
        schedule()
      },
      [schedule],
    )

    const hide = useCallback(() => {
      latestPlacement.current = null
      lens.current?.removeAttribute('data-visible')
      if (frame.current !== null) cancelAnimationFrame(frame.current)
      frame.current = null
    }, [])

    useImperativeHandle(forwardedRef, () => ({ place, hide }), [hide, place])

    useEffect(() => {
      if (latestPlacement.current !== null && lens.current?.dataset.visible === 'true') schedule()
    }, [area, magnification, rotation, schedule, shape, sourceScale, stageSize])

    useEffect(() => {
      clearOverlay(overlayCanvas.current)
      if (latestPlacement.current !== null && lens.current?.dataset.visible === 'true') schedule()
    }, [
      original.entityId,
      original.representation?.cacheKey,
      original.status,
      overlayPainter,
      schedule,
    ])

    useEffect(
      () => () => {
        if (frame.current !== null) cancelAnimationFrame(frame.current)
      },
      [],
    )

    const style = {
      width: dimensions.width,
      height: dimensions.height,
      '--magnifier-scale': String(sourceScale * magnification),
      '--magnifier-rotation': `${rotation}deg`,
    } as CSSProperties

    return (
      <div
        ref={lens}
        className="image-magnifier"
        data-testid="image-magnifier"
        data-shape={shape}
        aria-hidden="true"
        style={style}
      >
        {original.status === 'ready' && original.representation !== null ? (
          <img
            ref={sourceImage}
            className="image-magnifier__source"
            data-testid="image-magnifier-source"
            data-source-name={fileName}
            src={original.representation.url}
            width={original.representation.width}
            height={original.representation.height}
            draggable={false}
            aria-hidden="true"
            alt=""
          />
        ) : (
          <span className="image-magnifier__status">{statusCopy(original.status)}</span>
        )}
        {overlayPainter !== undefined && (
          <canvas
            ref={overlayCanvas}
            className="image-magnifier__overlay"
            data-testid="image-magnifier-overlay"
          />
        )}
      </div>
    )
  },
)

export default ImageMagnifier

function setPixels(element: HTMLElement, property: string, value: number) {
  element.style.setProperty(property, `${value}px`)
}

function statusCopy(status: CurrentOriginalState['status']): string {
  if (status === 'budget_error') return '原图超出安全预览限制'
  if (status === 'error') return '无法载入原图'
  return '正在载入原图'
}

function paintOverlay(
  canvas: HTMLCanvasElement | null,
  painter: MagnifierOverlayPainter | undefined,
  original: CurrentOriginalState,
  sourcePoint: Point,
  lensSize: Size,
  contentScale: number,
  magnification: MagnifierMagnification,
  rotation: PreviewRotation,
) {
  if (
    canvas === null ||
    painter === undefined ||
    original.status !== 'ready' ||
    original.representation === null
  ) {
    clearOverlay(canvas)
    return
  }
  const ratio = canvasPixelRatio()
  canvas.style.width = `${lensSize.width}px`
  canvas.style.height = `${lensSize.height}px`
  canvas.width = Math.max(1, Math.round(lensSize.width * ratio))
  canvas.height = Math.max(1, Math.round(lensSize.height * ratio))
  let context: CanvasRenderingContext2D | null = null
  try {
    context = canvas.getContext('2d')
    if (context === null) {
      canvas.removeAttribute('data-has-content')
      return
    }
    context.setTransform(ratio, 0, 0, ratio, 0, 0)
    context.clearRect(0, 0, lensSize.width, lensSize.height)
    const count = painter(context, {
      projection: createMagnifierContentProjection({
        sourceSize: original.representation,
        sourcePoint,
        lensSize,
        contentScale,
        rotation,
      }),
      magnification,
      pixelRatio: ratio,
    })
    if (count > 0) canvas.dataset.hasContent = 'true'
    else canvas.removeAttribute('data-has-content')
  } catch {
    canvas.removeAttribute('data-has-content')
    try {
      context?.setTransform(1, 0, 0, 1, 0, 0)
      context?.clearRect(0, 0, canvas.width, canvas.height)
    } catch {
      // The image-only lens remains usable when Canvas is unavailable.
    }
  }
}

function clearOverlay(canvas: HTMLCanvasElement | null) {
  if (canvas === null) return
  canvas.removeAttribute('data-has-content')
  try {
    const context = canvas.getContext('2d')
    context?.setTransform(1, 0, 0, 1, 0, 0)
    context?.clearRect(0, 0, canvas.width, canvas.height)
  } catch {
    // Clearing is best-effort; stale identity is still hidden by the data attribute.
  }
}

function canvasPixelRatio(): number {
  const ratio = Number.isFinite(window.devicePixelRatio) ? window.devicePixelRatio : 1
  return Math.min(4, Math.max(1, ratio))
}
