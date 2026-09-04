export type ImageRendererBackend = 'native' | 'web'
export type ImageRendererDisposition =
  | 'applied'
  | 'ignored_duplicate'
  | 'ignored_stale'
  | 'require_snapshot'

export interface ImageRendererAck {
  disposition: ImageRendererDisposition
  acceptedRevision: number
  backend: ImageRendererBackend
}

export interface OpenImageRendererRequest {
  sessionId: string
  entityId: string
  assetGeneration: number
}

export interface ImageRendererPoint {
  x: number
  y: number
}

export interface ImageRendererRect {
  left: number
  top: number
  width: number
  height: number
}

export interface ImageRendererNormalizedRect {
  x: number
  y: number
  width: number
  height: number
}

export type ImageRendererTool = 'browse' | 'point' | 'arrow' | 'brush' | 'rectangle' | 'ellipse'
export type ImageRendererRotation = 'deg0' | 'deg90' | 'deg180' | 'deg270'

export interface ImageRendererCamera {
  mode: 'fit' | 'free'
  zoom: number
  rotation: ImageRendererRotation
  offset: ImageRendererPoint
}

export type ImageRendererAnnotationGeometry =
  | { type: 'point'; position: ImageRendererPoint }
  | { type: 'arrow'; tail: ImageRendererPoint; head: ImageRendererPoint }
  | { type: 'rectangle'; rect: ImageRendererNormalizedRect }
  | { type: 'ellipse'; rect: ImageRendererNormalizedRect }
  | { type: 'stroke'; points: ImageRendererPoint[] }

export interface ImageRendererAnnotation {
  id: string
  ordinal: number
  geometry: ImageRendererAnnotationGeometry
  style: { color: [number, number, number, number]; lineWidthPx: number; dashed: boolean }
  selected: boolean
  draft: boolean
  visible: boolean
}

export interface ImageRendererScene {
  annotations: ImageRendererAnnotation[]
  draft: ImageRendererAnnotation | null
}

export interface ImageRendererViewportBinding {
  sceneRevision: number
  scene: ImageRendererScene
  tool: ImageRendererTool
  inputExclusionRevision: number
  onEvent(event: ImageRendererEvent): void
}

export type ImageRendererScenePatch =
  | { type: 'upsert'; baseRevision: number; node: ImageRendererAnnotation }
  | { type: 'remove'; baseRevision: number; id: string }
  | {
      type: 'replace_all'
      baseRevision: number
      annotations: ImageRendererAnnotation[]
      draft: ImageRendererAnnotation | null
    }
  | { type: 'set_selection'; baseRevision: number; id: string | null }
  | { type: 'set_draft'; baseRevision: number; node: ImageRendererAnnotation | null }

export type ImageRendererSessionCommand =
  | { type: 'set_surface'; surface: ImageRendererRect & { scaleFactor: number } }
  | { type: 'set_input_exclusions'; exclusions: ImageRendererRect[] }
  | { type: 'set_tool'; tool: ImageRendererTool }
  | { type: 'set_scene'; scene: ImageRendererScene }
  | { type: 'apply_scene_patch'; patch: ImageRendererScenePatch }
  | { type: 'camera'; camera: ImageRendererCamera }
  | {
      type: 'set_magnifier'
      magnifier: {
        widthPx: number
        heightPx: number
        magnification: number
        shape: 'circle' | 'rounded_rectangle'
      } | null
    }

export interface ImageRendererCommand {
  sceneRevision: number
  command: ImageRendererSessionCommand
}

export type ImageRenderCommandEnvelope = Omit<ImageRendererCommand, 'command'> & {
  sessionId: number
  assetGeneration: number
  commandId: number
  command: ImageRendererSessionCommand | { type: 'open'; entityId: string } | { type: 'close' }
}

type ImageRendererEventBase = { sessionId: string; assetGeneration: number }

export type ImageRendererEvent =
  | (ImageRendererEventBase & { type: 'ready'; width: number; height: number })
  | (ImageRendererEventBase & {
      type: 'frame_presented'
      sceneRevision: number
      frameIndex: number
    })
  | (ImageRendererEventBase & { type: 'camera_changed'; camera: ImageRendererCamera })
  | (ImageRendererEventBase & {
      type: 'draft_started' | 'draft_changed' | 'draft_completed'
      geometry: ImageRendererAnnotationGeometry
    })
  | (ImageRendererEventBase & { type: 'draft_cancelled' })
  | (ImageRendererEventBase & { type: 'selection_changed'; annotationId: string | null })
  | (ImageRendererEventBase & {
      type: 'editor_placement_changed'
      position: ImageRendererPoint
    })
  | (ImageRendererEventBase & { type: 'recovering'; reason: string })
  | (ImageRendererEventBase & {
      type: 'backend_activated'
      backend: ImageRendererBackend
    })
  | (ImageRendererEventBase & {
      type: 'failed'
      code: string
      retryable: boolean
    })

export interface ImageRendererBridge {
  imageRenderCommand(command: ImageRenderCommandEnvelope): Promise<ImageRendererAck>
  listenImageRender(handler: (event: ImageRendererEvent) => void): Promise<() => void>
}

export interface ImageRendererSession {
  readonly backend: ImageRendererBackend
  readonly sessionId: string
  readonly assetGeneration: number
  dispatch(command: ImageRendererCommand): Promise<ImageRendererAck>
  close(): Promise<void>
}

export interface ImageRendererPort {
  readonly backend: ImageRendererBackend
  open(request: OpenImageRendererRequest): Promise<ImageRendererSession>
  listen(handler: (event: ImageRendererEvent) => void): Promise<() => void>
}

export interface LegacyWebImageRendererSession {
  dispatch(command: ImageRendererCommand): Promise<void>
  close(): Promise<void>
}

export interface LegacyWebImageRendererAdapter {
  open(request: OpenImageRendererRequest): Promise<LegacyWebImageRendererSession>
  listen(handler: (event: ImageRendererEvent) => void): Promise<() => void>
}

export interface ImageRendererMigrationPolicy {
  initialBackend: ImageRendererBackend
  legacyWeb: LegacyWebImageRendererAdapter
}
