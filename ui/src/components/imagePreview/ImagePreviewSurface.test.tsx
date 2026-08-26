import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

describe('ImagePreviewSurface architecture', () => {
  it('keeps the reusable surface review-neutral', () => {
    const source = readFileSync('src/components/imagePreview/ImagePreviewSurface.tsx', 'utf8')
    expect(source).not.toMatch(/app\/review|api\/viewer|components\/review/)
  })
})
