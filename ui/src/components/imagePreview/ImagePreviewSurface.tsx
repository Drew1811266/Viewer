import type { MutableRefObject, ReactNode } from 'react'
import type {
  BrowserFile,
  ImageRepresentation,
  ImageRepresentationRequest,
  MagnifierPreferences,
} from '../../api/types'
import { isVectorImage } from '../../fileKinds'
import type {
  ImageRendererPort,
  ImageRendererViewportBinding,
} from '../../rendering/imageRendererTypes'
import type { Point } from './imageGeometry'
import type { ImagePreviewProjection } from './imagePreviewProjection'
import type { MagnifierOverlayPainter } from './magnifierOverlay'
import NativeImageViewport from './NativeImageViewport'
import WebImageViewport from './WebImageViewport'

export type { ImagePreviewProjection } from './imagePreviewProjection'

export interface ImagePreviewSurfaceSlots {
  toolbarLeading?: ReactNode
  toolbarActions?: ReactNode
  stageOverlay?: (projection: ImagePreviewProjection) => ReactNode
  nativeStageOverlay?: (projection: ImagePreviewProjection) => ReactNode
  sidePanel?: ReactNode
  magnifierOverlayPainter?: MagnifierOverlayPainter
}

export interface ImagePreviewSurfaceProps {
  file: BrowserFile
  files: BrowserFile[]
  magnifier: MagnifierPreferences
  pointerClientPoint: MutableRefObject<Point | null>
  unavailableEntityIds?: ReadonlySet<string>
  prefetchFit?: boolean
  requestImage: (
    file: BrowserFile,
    representation: ImageRepresentationRequest,
    signal?: AbortSignal,
  ) => Promise<ImageRepresentation>
  onNavigate: (file: BrowserFile) => void
  onDimensions?: (entityId: string, width: number, height: number) => void
  ariaLabel?: string
  toolbarLabel?: string
  onEscape?: () => void
  slots?: ImagePreviewSurfaceSlots
  renderer?: ImageRendererPort
  nativeBinding?: ImageRendererViewportBinding
}

export function shouldUseNativeViewport(
  renderer: ImageRendererPort | undefined,
  file: Pick<BrowserFile, 'name'>,
): boolean {
  // The native provider decodes raster sources through CGImageSource. Vector
  // sources are rasterized by the platform image port, so they stay on the
  // Web artifact path until the native provider gains a vector decode step.
  return renderer?.backend === 'native' && !isVectorImage(file)
}

export default function ImagePreviewSurface(props: ImagePreviewSurfaceProps) {
  const renderer = props.renderer
  if (shouldUseNativeViewport(renderer, props.file) && renderer !== undefined) {
    return <NativeImageViewport {...props} renderer={renderer} />
  }
  return <WebImageViewport {...props} />
}
