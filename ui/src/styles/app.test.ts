import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

const appCss = readFileSync('src/styles/app.css', 'utf8')

describe('workspace style contracts', () => {
  it('resolves specialized radial labels to high-contrast dark-mode colors', () => {
    const radialStart = appCss.indexOf('.radial-file-menu {')
    const darkStart = appCss.indexOf('@media (prefers-color-scheme: dark)', radialStart)
    const baseRules = parseRules(appCss.slice(radialStart, darkStart))
    const darkRules = parseRules(mediaBody(appCss, '(prefers-color-scheme: dark)'))

    const secondaryColor = winningColor(
      [...baseRules, ...darkRules],
      new Set([
        '.radial-menu-button',
        '.radial-menu-button[data-level="secondary"]',
      ]),
    )
    const destructiveColor = winningColor(
      [...baseRules, ...darkRules],
      new Set([
        '.radial-menu-button',
        '.radial-menu-button[data-tone="destructive"]',
      ]),
    )

    expect(secondaryColor).toBe('#f3f5f7')
    expect(contrastRatio(secondaryColor, '#244d7d')).toBeGreaterThanOrEqual(4.5)
    expect(destructiveColor).toBe('#ffb4ab')
    expect(contrastRatio(destructiveColor, '#2f343d')).toBeGreaterThanOrEqual(4.5)
    expect(contrastRatio(destructiveColor, '#244d7d')).toBeGreaterThanOrEqual(4.5)
  })

  it('resolves disabled radial sectors and labels to visible dark-mode treatment', () => {
    const radialStart = appCss.indexOf('.radial-file-menu {')
    const darkStart = appCss.indexOf('@media (prefers-color-scheme: dark)', radialStart)
    const rules = [
      ...parseRules(appCss.slice(radialStart, darkStart)),
      ...parseRules(mediaBody(appCss, '(prefers-color-scheme: dark)')),
    ]
    const primaryDisabled = new Set([
      '.radial-primary-shape',
      '.radial-primary-shape[data-disabled="true"]',
    ])
    const secondaryDisabled = new Set([
      '.radial-secondary-shape',
      '.radial-secondary-shape[data-disabled="true"]',
    ])
    const disabledLabel = new Set([
      '.radial-menu-button',
      '.radial-menu-button[data-level="secondary"]',
      '.radial-menu-button[aria-disabled="true"]',
    ])

    for (const selectors of [primaryDisabled, secondaryDisabled]) {
      expect(winningDeclaration(rules, selectors, 'fill')).toBe('#3f4651')
      expect(winningDeclaration(rules, selectors, 'stroke')).toBe('#818d9c')
      expect(winningDeclaration(rules, selectors, 'opacity')).toBe('1')
    }
    const label = winningDeclaration(rules, disabledLabel, 'color')
    expect(label).toBe('#c1c9d4')
    expect(winningDeclaration(rules, disabledLabel, 'filter')).toBe('none')
    expect(winningDeclaration(rules, disabledLabel, 'opacity')).toBe('1')
    expect(contrastRatio(label, '#3f4651')).toBeGreaterThanOrEqual(4.5)
  })

  it('resolves explicit header and popover gray descendants to dark contrast colors', () => {
    const darkStart = appCss.indexOf('@media (prefers-color-scheme: dark)')
    const rules = [
      ...parseRules(appCss.slice(0, darkStart)),
      ...parseRules(mediaBody(appCss, '(prefers-color-scheme: dark)')),
    ]
    const mutedSelectors = [
      '.search-field kbd',
      '.search-options-popover > label > span',
      '.project-access-status',
    ]

    for (const selector of mutedSelectors) {
      const color = winningDeclaration(rules, new Set([selector]), 'color')
      expect(color, selector).toBe('#c7ced8')
      expect(contrastRatio(color, '#24282f'), selector).toBeGreaterThanOrEqual(4.5)
    }
  })

  it('anchors the filter popover within both edges of a 720px viewport', () => {
    const narrowRules = parseRules(mediaBody(appCss, '(max-width: 800px)'))
    const popover = narrowRules.find((rule) => rule.selector === '.search-options-popover')
    expect(popover?.declarations).toMatchObject({
      position: 'fixed',
      left: '12px',
      right: '12px',
      'min-width': '0',
      width: 'auto',
    })

    const viewportWidth = 720
    const left = pixels(popover?.declarations.left)
    const right = viewportWidth - pixels(popover?.declarations.right)
    expect({ left, right, width: right - left }).toEqual({
      left: 12,
      right: 708,
      width: 696,
    })
  })

  it('keeps the complete eight-pixel separator hit target inside the clipped sidebar', () => {
    const rules = parseRules(appCss)
    const sidebar = rules.find((rule) => rule.selector === '.folder-sidebar')
    const separator = rules.find((rule) => rule.selector === '.sidebar-separator')

    expect(sidebar?.declarations.overflow).toBe('hidden')
    expect(separator?.declarations).toMatchObject({
      right: '0',
      width: '8px',
    })
  })
})

interface CssRule {
  selector: string
  declarations: Record<string, string>
  order: number
}

function mediaBody(css: string, query: string): string {
  const marker = `@media ${query}`
  const markerIndex = css.lastIndexOf(marker)
  if (markerIndex === -1) return ''
  const openingBrace = css.indexOf('{', markerIndex + marker.length)
  let depth = 1
  for (let index = openingBrace + 1; index < css.length; index += 1) {
    if (css[index] === '{') depth += 1
    else if (css[index] === '}') depth -= 1
    if (depth === 0) return css.slice(openingBrace + 1, index)
  }
  return ''
}

function parseRules(css: string): CssRule[] {
  const rules: CssRule[] = []
  const pattern = /([^{}]+)\{([^{}]*)\}/g
  for (const match of css.matchAll(pattern)) {
    const body = match[2] ?? ''
    const declarations = Object.fromEntries(
      [...body.matchAll(/([\w-]+)\s*:\s*([^;]+);/g)].map((declaration) => [
        declaration[1]!,
        declaration[2]!.trim(),
      ]),
    )
    for (const selector of (match[1] ?? '').split(',')) {
      rules.push({ selector: selector.trim(), declarations, order: rules.length })
    }
  }
  return rules
}

function winningColor(rules: CssRule[], matchingSelectors: Set<string>): string {
  return winningDeclaration(rules, matchingSelectors, 'color')
}

function winningDeclaration(
  rules: CssRule[],
  matchingSelectors: Set<string>,
  property: string,
): string {
  const candidates = rules
    .map((rule, cascadeOrder) => ({ ...rule, cascadeOrder }))
    .filter(
      (rule) => matchingSelectors.has(rule.selector) && rule.declarations[property],
    )
  candidates.sort((left, right) => {
    const specificity = compareSpecificity(
      selectorSpecificity(left.selector),
      selectorSpecificity(right.selector),
    )
    return specificity === 0 ? left.cascadeOrder - right.cascadeOrder : specificity
  })
  return candidates.at(-1)?.declarations[property] ?? ''
}

function selectorSpecificity(selector: string): [number, number, number] {
  const ids = selector.match(/#[\w-]+/g)?.length ?? 0
  const classesAndAttributes =
    selector.match(/\.[\w-]+|\[[^\]]+\]|:(?!:)[\w-]+/g)?.length ?? 0
  const elements =
    selector
      .replace(/#[\w-]+|\.[\w-]+|\[[^\]]+\]|:{1,2}[\w-]+/g, ' ')
      .match(/[a-z][\w-]*/gi)?.length ?? 0
  return [ids, classesAndAttributes, elements]
}

function compareSpecificity(
  left: [number, number, number],
  right: [number, number, number],
): number {
  for (let index = 0; index < left.length; index += 1) {
    if (left[index] !== right[index]) return left[index]! - right[index]!
  }
  return 0
}

function contrastRatio(foreground: string, background: string): number {
  const foregroundLuminance = relativeLuminance(foreground)
  const backgroundLuminance = relativeLuminance(background)
  return (
    (Math.max(foregroundLuminance, backgroundLuminance) + 0.05) /
    (Math.min(foregroundLuminance, backgroundLuminance) + 0.05)
  )
}

function relativeLuminance(color: string): number {
  const channels = [1, 3, 5].map((index) => Number.parseInt(color.slice(index, index + 2), 16) / 255)
  const [red, green, blue] = channels.map((channel) =>
    channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4,
  )
  return 0.2126 * red! + 0.7152 * green! + 0.0722 * blue!
}

function pixels(value: string | undefined): number {
  return Number.parseFloat(value ?? 'NaN')
}
