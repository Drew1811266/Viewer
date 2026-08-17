import { readdirSync, readFileSync, statSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const primitivesCss = readFileSync(resolve(import.meta.dirname, 'primitives.css'), 'utf8')
const appCss = readFileSync(resolve(import.meta.dirname, 'app.css'), 'utf8')
const videoPreviewCss = readFileSync(resolve(import.meta.dirname, 'videoPreview.css'), 'utf8')
const forcedColors = `${mediaBody(primitivesCss, '(forced-colors: active)')}\n${mediaBody(
  appCss,
  '(forced-colors: active)',
)}`

describe('Viewer visual accessibility contracts', () => {
  it('uses system colors and structural state cues in forced-colors mode', () => {
    for (const systemColor of ['Canvas', 'CanvasText', 'Highlight', 'HighlightText', 'GrayText']) {
      expect(forcedColors).toContain(systemColor)
    }
    for (const selector of [
      '.image-cell[aria-selected="true"]::after',
      ".viewer-menu-row[data-current='true']",
      ".viewer-button[data-tone='danger']",
      ".viewer-choice-chip[data-disabled='true']",
      ':focus-visible',
    ]) {
      expect(forcedColors).toContain(selector)
    }
    expect(forcedColors).toMatch(/border-(?:left-)?width:\s*(?:2px|3px)/)
    expect(forcedColors).toContain('outline: 2px solid Highlight')
  })

  it('removes motion while retaining the formal progress surface', () => {
    const reducedMotion = `${mediaBody(
      primitivesCss,
      '(prefers-reduced-motion: reduce)',
    )}\n${mediaBody(appCss, '(prefers-reduced-motion: reduce)')}`
    expect(reducedMotion).toContain('.viewer-task-surface__fill')
    expect(reducedMotion).toContain('transition: none')
    expect(reducedMotion).toContain('animation: none')
    expect(primitivesCss).toContain('.viewer-task-surface__track')
  })

  it('keeps video controls immediate in reduced motion and structural in forced colors', () => {
    const reducedMotion = mediaBody(videoPreviewCss, '(prefers-reduced-motion: reduce)')
    expect(reducedMotion).toContain('.video-controls')
    expect(reducedMotion).toContain('.video-preview-bottom-chrome')
    expect(reducedMotion).toContain('.video-preview-stage::after')
    expect(reducedMotion).toContain('transition: none')
    expect(reducedMotion).toContain('.video-preview-loading__progress > span')
    expect(reducedMotion).toContain('animation: none')

    const forcedColors = mediaBody(videoPreviewCss, '(forced-colors: active)')
    expect(forcedColors).toContain('.video-controls')
    expect(forcedColors).toContain('.video-controls-more__popover')
    expect(forcedColors).toContain('.video-preview-matte--top')
    expect(forcedColors).toContain('.video-preview-stage::after')
    expect(forcedColors).toContain('CanvasText')
    expect(forcedColors).toContain('Highlight')
  })

  it('clears every web document canvas layer after the native first frame', () => {
    for (const selector of [
      ":root:has(.viewer-shell[data-video-preview-open='true'])",
      "body:has(.viewer-shell[data-video-preview-open='true'])",
      "#root:has(.viewer-shell[data-video-preview-open='true'])",
    ]) {
      expect(videoPreviewCss).toMatch(
        new RegExp(`${escapeRegExp(selector)}\\s*\\{[^}]*background:\\s*transparent`, 's'),
      )
    }
  })

  it('contains no superseded visible UI glyphs in production source', () => {
    const productionSource = sourceFiles(resolve(import.meta.dirname, '..'))
      .map((path) => readFileSync(path, 'utf8'))
      .join('\n')
    const forbiddenGlyphs = ['⌕', '↻', '▾', '▸', '⇄', 'ⓘ', '◉', '✎', '⧉', '⌫', '★']
    for (const glyph of forbiddenGlyphs) expect(productionSource).not.toContain(glyph)
  })
})

function sourceFiles(root: string): string[] {
  return readdirSync(root).flatMap((entry) => {
    const path = resolve(root, entry)
    if (statSync(path).isDirectory()) return sourceFiles(path)
    return /\.(ts|tsx)$/.test(path) && !path.endsWith('.test.ts') && !path.endsWith('.test.tsx')
      ? [path]
      : []
  })
}

function mediaBody(source: string, query: string): string {
  const marker = `@media ${query}`
  const start = source.indexOf(marker)
  if (start === -1) return ''
  const opening = source.indexOf('{', start)
  let depth = 0
  for (let index = opening; index < source.length; index += 1) {
    if (source[index] === '{') depth += 1
    if (source[index] === '}') depth -= 1
    if (depth === 0) return source.slice(opening + 1, index)
  }
  return ''
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}
