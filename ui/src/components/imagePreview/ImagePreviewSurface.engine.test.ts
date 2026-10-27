import { describe, expect, it } from 'vitest'
import type { ImageRendererPort } from '../../rendering/imageRendererTypes'
import { shouldUseNativeViewport } from './ImagePreviewSurface'

const nativeRenderer = { backend: 'native' } as ImageRendererPort

describe('shouldUseNativeViewport', () => {
  it('uses the native viewport for raster images', () => {
    expect(shouldUseNativeViewport(nativeRenderer, { name: 'photo.jpg' })).toBe(true)
    expect(shouldUseNativeViewport(nativeRenderer, { name: 'scan.tiff' })).toBe(true)
  })

  it('keeps vector sources on the Web artifact path', () => {
    expect(shouldUseNativeViewport(nativeRenderer, { name: 'logo.svg' })).toBe(false)
    expect(shouldUseNativeViewport(nativeRenderer, { name: 'logo.svgz' })).toBe(false)
    expect(shouldUseNativeViewport(nativeRenderer, { name: 'poster.ai' })).toBe(false)
  })

  it('falls back to the Web viewport without a native renderer', () => {
    expect(shouldUseNativeViewport(undefined, { name: 'photo.jpg' })).toBe(false)
  })
})
