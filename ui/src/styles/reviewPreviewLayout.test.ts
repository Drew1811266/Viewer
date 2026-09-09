import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const css = readFileSync(resolve(import.meta.dirname, 'review.css'), 'utf8')

describe('native image review preview layout', () => {
  it('keeps the native stage full-height while navigation floats above it', () => {
    expect(css).toMatch(
      /\.image-preview:has\(\.annotation-toolbar\) \.image-preview-stage\s*\{[^}]*margin-bottom:\s*0;/m,
    )
  })
})
