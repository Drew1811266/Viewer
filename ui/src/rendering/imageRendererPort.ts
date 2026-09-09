import type { ImageRendererBridge, ImageRendererPort } from './imageRendererTypes'
import { NativeImageRenderer } from './nativeImageRenderer'

export type CreateImageRendererPortOptions = {
  bridge: ImageRendererBridge
}

export function createImageRendererPort({
  bridge,
}: CreateImageRendererPortOptions): ImageRendererPort {
  // Stable production path: native wgpu/Metal owns the single image renderer.
  // The legacy Web adapter remains available only to isolated migration tests
  // until their fixtures are retired; it is not constructed or bundled here.
  return new NativeImageRenderer(bridge)
}
