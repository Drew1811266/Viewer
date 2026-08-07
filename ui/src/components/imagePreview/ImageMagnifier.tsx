import type { CSSProperties } from 'react'
import { forwardRef, useCallback, useEffect, useImperativeHandle, useRef } from 'react'
import type { MagnifierArea, MagnifierMagnification, MagnifierShape } from '../../api/types'
import type { Point, PreviewRotation, Size } from './imageGeometry'
import {
  lensDimensions,
  magnifierShellPlacement,
  magnifierSourcePlacement,
} from './magnifierGeometry'
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
  stageSize: Size
  rotation: PreviewRotation
  fileName: string
  original: CurrentOriginalState
}

interface MagnifierConfiguration {
  shape: MagnifierShape
  area: MagnifierArea
  magnification: MagnifierMagnification
  stageSize: Size
  rotation: PreviewRotation
}

const ImageMagnifier = forwardRef<ImageMagnifierHandle, ImageMagnifierProps>(
  function ImageMagnifier(
    { shape, area, magnification, stageSize, rotation, fileName, original },
    forwardedRef,
  ) {
    const lens = useRef<HTMLDivElement>(null)
    const sourceImage = useRef<HTMLImageElement>(null)
    const latestPlacement = useRef<MagnifierPlacement | null>(null)
    const frame = useRef<number | null>(null)
    const configuration = useRef<MagnifierConfiguration>({
      shape,
      area,
      magnification,
      stageSize,
      rotation,
    })
    configuration.current = { shape, area, magnification, stageSize, rotation }
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
      const source = magnifierSourcePlacement(
        placement.sourcePoint,
        currentDimensions,
        current.magnification,
      )
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
      element.style.setProperty('--magnifier-scale', String(current.magnification))
      element.style.setProperty('--magnifier-rotation', `${current.rotation}deg`)
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
    }, [area, magnification, rotation, schedule, shape, stageSize])

    useEffect(
      () => () => {
        if (frame.current !== null) cancelAnimationFrame(frame.current)
      },
      [],
    )

    const style = {
      width: dimensions.width,
      height: dimensions.height,
      '--magnifier-scale': String(magnification),
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
