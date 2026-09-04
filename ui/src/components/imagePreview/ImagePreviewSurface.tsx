import type { MutableRefObject, ReactNode } from 'react'
import type {
  BrowserFile,
  ImageRepresentation,
  ImageRepresentationRequest,
  MagnifierPreferences,
} from '../../api/types'
import type { ImageRendererPort } from '../../rendering/imageRendererTypes'
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
}

export default function ImagePreviewSurface(props: ImagePreviewSurfaceProps) {
  if (props.renderer?.backend === 'native') {
    return <NativeImageViewport {...props} renderer={props.renderer} />
  }
  return <WebImageViewport {...props} />
}
