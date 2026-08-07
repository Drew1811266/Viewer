import { useCallback, useMemo, useState } from 'react'
import {
  clampOffset,
  displayScale,
  type ImageViewportGeometry,
  type ImageViewportState,
  type Point,
  type PreviewRotation,
  panBounds,
  type Size,
  zoomAtAnchor,
} from './imageGeometry'

const DEFAULT_STATE: ImageViewportState = {
  mode: 'fit',
  zoom: 1,
  rotation: 0,
  offset: { x: 0, y: 0 },
}

export interface ImageViewport {
  state: ImageViewportState
  geometry: ImageViewportGeometry
  scale: number
  panBounds: Point
  transform: string
  setFit: () => void
  setOriginal: () => void
  zoomBy: (factor: number, anchor: Point) => void
  panBy: (delta: Point) => void
  rotateClockwise: () => void
  setMeasurements: (stage: Size, source: Size) => void
  resetForEntity: () => void
}

export function useImageViewport(initialGeometry: ImageViewportGeometry): ImageViewport {
  const [geometry, setGeometry] = useState(initialGeometry)
  const [state, setState] = useState<ImageViewportState>(DEFAULT_STATE)

  const setFit = useCallback(() => {
    setState((current) => ({ ...current, mode: 'fit', zoom: 1, offset: { x: 0, y: 0 } }))
  }, [])

  const setOriginal = useCallback(() => {
    setState((current) => {
      const next = { ...current, mode: 'original' as const, zoom: 1 }
      return { ...next, offset: clampOffset(next.offset, panBounds(next, geometry)) }
    })
  }, [geometry])

  const zoomBy = useCallback(
    (factor: number, anchor: Point) => {
      setState((current) => zoomAtAnchor(current, geometry, factor, anchor))
    },
    [geometry],
  )

  const panBy = useCallback(
    (delta: Point) => {
      setState((current) => ({
        ...current,
        offset: clampOffset(
          { x: current.offset.x + delta.x, y: current.offset.y + delta.y },
          panBounds(current, geometry),
        ),
      }))
    },
    [geometry],
  )

  const rotateClockwise = useCallback(() => {
    setState((current) => {
      const rotation = ((current.rotation + 90) % 360) as PreviewRotation
      const next = { ...current, rotation }
      return { ...next, offset: clampOffset(next.offset, panBounds(next, geometry)) }
    })
  }, [geometry])

  const setMeasurements = useCallback(
    (stage: Size, source: Size) => {
      const nextGeometry = { stage, source, fitInset: initialGeometry.fitInset }
      setGeometry(nextGeometry)
      setState((current) => ({
        ...current,
        offset: clampOffset(current.offset, panBounds(current, nextGeometry)),
      }))
    },
    [initialGeometry.fitInset],
  )

  const resetForEntity = useCallback(() => setState(DEFAULT_STATE), [])
  const scale = displayScale(state, geometry)
  const bounds = panBounds(state, geometry)
  const transform = useMemo(
    () =>
      `translate(${state.offset.x}px, ${state.offset.y}px) rotate(${state.rotation}deg) scale(${scale})`,
    [scale, state.offset.x, state.offset.y, state.rotation],
  )

  return {
    state,
    geometry,
    scale,
    panBounds: bounds,
    transform,
    setFit,
    setOriginal,
    zoomBy,
    panBy,
    rotateClockwise,
    setMeasurements,
    resetForEntity,
  }
}
