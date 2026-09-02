import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

describe('ImagePreviewSurface architecture', () => {
  it('keeps the reusable surface review-neutral', () => {
    const source = readFileSync('src/components/imagePreview/ImagePreviewSurface.tsx', 'utf8')
    expect(source).not.toMatch(/app\/review|api\/viewer|components\/review/)
  })

  it('exposes an optional generic magnifier overlay painter slot', () => {
    const source = readFileSync('src/components/imagePreview/ImagePreviewSurface.tsx', 'utf8')
    expect(source).toContain('magnifierOverlayPainter?: MagnifierOverlayPainter')
    expect(source).toContain('overlayPainter={slots?.magnifierOverlayPainter}')
  })

  it('tracks lens placement in capture phase while leaving gestures in bubble phase', () => {
    const source = readFileSync('src/components/imagePreview/ImagePreviewSurface.tsx', 'utf8')
    expect(source).toContain('onPointerDownCapture={trackMagnifierPointer}')
    expect(source).toContain('onPointerMoveCapture={trackMagnifierPointer}')
    expect(source).toContain('onPointerDown={gestures.onPointerDown}')
    expect(source).toContain('onPointerMove={gestures.onPointerMove}')
  })
})
