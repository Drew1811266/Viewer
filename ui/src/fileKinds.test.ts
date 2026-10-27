import { describe, expect, it } from 'vitest'
import { isVectorImage } from './fileKinds'

describe('isVectorImage', () => {
  it.each(['logo.svg', 'LOGO.SVG', 'logo.svgz', 'poster.ai'])('detects %s', (name) => {
    expect(isVectorImage({ name })).toBe(true)
  })

  it.each(['photo.jpg', 'photo.png', 'vector', '.svg', 'photo.svg.backup'])(
    'leaves %s on the raster path',
    (name) => {
      expect(isVectorImage({ name })).toBe(false)
    },
  )
})
