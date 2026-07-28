import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'
import { defined } from '../defined'

const appCss = readFileSync('src/styles/app.css', 'utf8')

describe('workspace style contracts', () => {
  it('resolves specialized radial labels to high-contrast dark-mode colors', () => {
    const radialStart = appCss.indexOf('.radial-file-menu {')
    const darkStart = appCss.indexOf('@media (prefers-color-scheme: dark)', radialStart)
    const baseRules = parseRules(appCss.slice(radialStart, darkStart))
    const darkRules = parseRules(mediaBody(appCss, '(prefers-color-scheme: dark)'))

    const secondaryColor = winningColor(
      [...baseRules, ...darkRules],
      new Set(['.radial-menu-button', '.radial-menu-button[data-level="secondary"]']),
    )
    const destructiveColor = winningColor(
      [...baseRules, ...darkRules],
      new Set(['.radial-menu-button', '.radial-menu-button[data-tone="destructive"]']),
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

  it('keeps folder identity fixed beside an independently scrolling filmstrip', () => {
    const rules = parseRules(appCss)
    const row = rules.find((rule) => rule.selector === '.folder-filmstrip-row')
    const filmstrip = rules.find((rule) => rule.selector === '.folder-filmstrip')
    const track = rules.find((rule) => rule.selector === '.folder-filmstrip-track')
    const item = rules.find((rule) => rule.selector === '.folder-filmstrip-item')
    const thumbnail = rules.find((rule) => rule.selector === '.folder-filmstrip-thumbnail')
    const thumbnailHover = rules.find(
      (rule) => rule.selector === '.folder-filmstrip-thumbnail:hover',
    )
    const image = rules.find((rule) => rule.selector === '.aspect-thumbnail > img')
    const placeholder = rules.find((rule) => rule.selector === '.aspect-thumbnail-placeholder')

    expect(row?.declarations).toMatchObject({
      display: 'grid',
      'grid-template-columns': '184px minmax(0, 1fr)',
    })
    expect(filmstrip?.declarations).toMatchObject({
      'min-width': '0',
      'overflow-x': 'auto',
      'overflow-y': 'hidden',
      'scrollbar-gutter': 'stable',
    })
    expect(track?.declarations).toMatchObject({
      position: 'relative',
    })
    expect(track?.declarations.height).toBeUndefined()
    expect(item?.declarations).toMatchObject({
      position: 'absolute',
      top: '0',
    })
    expect(item?.declarations.width).toBeUndefined()
    expect(item?.declarations.height).toBeUndefined()
    expect(thumbnail?.declarations).toMatchObject({
      background: 'transparent',
      border: '0',
      overflow: 'visible',
      padding: '0',
    })
    expect(thumbnailHover?.declarations['box-shadow']).toBe('inset 0 0 0 1px #8cb8ea')
    expect(image?.declarations['object-fit']).toBe('contain')
    expect(image?.declarations.background).not.toBe('#e5e8ed')
    expect(placeholder?.declarations.background).toBe('#e5e8ed')
  })

  it('paints thumbnail keyboard focus above every thumbnail child state', () => {
    const rules = parseRules(appCss)
    const identityFocus = rules.find(
      (rule) => rule.selector === '.folder-filmstrip-identity:focus-visible',
    )
    const thumbnailFocus = rules.find(
      (rule) => rule.selector === '.folder-filmstrip-thumbnail:focus-visible',
    )

    expect(identityFocus?.declarations).toMatchObject({
      'box-shadow': 'inset 0 0 0 2px #2477d4',
      outline: 'none',
    })
    expect(thumbnailFocus?.declarations).toMatchObject({
      outline: '3px solid #2477d4',
      'outline-offset': '-3px',
    })
    expect(thumbnailFocus?.declarations['box-shadow']).toBeUndefined()

    const darkStart = appCss.indexOf('@media (prefers-color-scheme: dark)')
    const darkRules = [
      ...parseRules(appCss.slice(0, darkStart)),
      ...parseRules(mediaBody(appCss, '(prefers-color-scheme: dark)')),
    ]
    const darkOutline = winningDeclaration(
      darkRules,
      new Set(['.folder-filmstrip-thumbnail:focus-visible']),
      'outline-color',
    )
    expect(darkOutline).toBe('#8ec8ff')
    expect(contrastRatio(darkOutline, '#343a43')).toBeGreaterThanOrEqual(3)
  })

  it('reserves content-grid gray for loading placeholders instead of successful images', () => {
    const rules = parseRules(appCss)
    const cell = rules.find((rule) => rule.selector === '.image-cell')
    const selected = rules.find((rule) => rule.selector === '.image-cell[aria-selected="true"]')
    const preview = rules.find((rule) => rule.selector === '.image-cell-preview')
    const image = rules.find((rule) => rule.selector === '.aspect-thumbnail > img')
    const placeholder = rules.find((rule) => rule.selector === '.aspect-thumbnail-placeholder')

    expect(cell?.declarations).toMatchObject({
      border: '0',
      overflow: 'visible',
      padding: '0',
    })
    expect(selected?.declarations['box-shadow']).toBe('inset 0 0 0 2px #2477d4')
    expect(preview?.declarations.background).toBe('transparent')
    expect(preview?.declarations.height).toBeUndefined()
    expect(image?.declarations).toMatchObject({
      background: 'transparent',
      'object-fit': 'contain',
    })
    expect(placeholder?.declarations.background).toBe('#e5e8ed')
  })

  it('renders workspace menu summaries as stateful toolbar buttons', () => {
    const rules = parseRules(appCss)
    const baseSelectors = [
      '.search-options-panel > summary',
      '.search-view-panel > summary',
      '.project-menu > summary',
    ]

    for (const selector of baseSelectors) {
      expect(winningDeclaration(rules, new Set([selector]), 'background'), selector).toBe('#fff')
      expect(winningDeclaration(rules, new Set([selector]), 'border'), selector).toBe(
        '1px solid #8a94a3',
      )
      expect(contrastRatio('#8a94a3', '#ffffff'), selector).toBeGreaterThanOrEqual(3)
      expect(winningDeclaration(rules, new Set([selector]), 'border-radius'), selector).toBe('6px')
      expect(winningDeclaration(rules, new Set([selector]), 'cursor'), selector).toBe('pointer')
      expect(winningDeclaration(rules, new Set([selector]), 'min-height'), selector).toBe('30px')
    }

    const hoverSelectors = [
      '.search-options-panel > summary:hover',
      '.search-view-panel > summary:hover',
      '.project-menu > summary:hover',
    ]
    for (const selector of hoverSelectors) {
      expect(winningDeclaration(rules, new Set([selector]), 'background'), selector).toBe('#f3f5f7')
      expect(winningDeclaration(rules, new Set([selector]), 'border-color'), selector).toBe(
        '#747f8e',
      )
      expect(contrastRatio('#747f8e', '#f3f5f7'), selector).toBeGreaterThanOrEqual(3)
    }

    const openSelectors = [
      '.search-options-panel[open] > summary',
      '.search-view-panel[open] > summary',
      '.project-menu[open] > summary',
    ]
    for (const selector of openSelectors) {
      expect(winningDeclaration(rules, new Set([selector]), 'background'), selector).toBe('#d9e8ff')
      expect(winningDeclaration(rules, new Set([selector]), 'border-color'), selector).toBe(
        '#2477d4',
      )
    }

    const focusSelectors = [
      '.search-options-panel > summary:focus-visible',
      '.search-view-panel > summary:focus-visible',
      '.project-menu > summary:focus-visible',
    ]
    for (const selector of focusSelectors) {
      expect(winningDeclaration(rules, new Set([selector]), 'outline'), selector).toBe(
        '2px solid #2477d4',
      )
      expect(winningDeclaration(rules, new Set([selector]), 'outline-offset'), selector).toBe('2px')
    }

    expect(winningDeclaration(rules, new Set(['.project-menu > summary']), 'min-width')).toBe(
      '30px',
    )

    const darkRules = [...rules, ...parseRules(mediaBody(appCss, '(prefers-color-scheme: dark)'))]
    for (const selector of baseSelectors) {
      expect(winningDeclaration(darkRules, new Set([selector]), 'background'), selector).toBe(
        '#24282f',
      )
      expect(winningDeclaration(darkRules, new Set([selector]), 'border-color'), selector).toBe(
        '#788596',
      )
      expect(winningDeclaration(darkRules, new Set([selector]), 'color'), selector).toBe('#f3f5f7')
      expect(contrastRatio('#788596', '#24282f'), selector).toBeGreaterThanOrEqual(3)
    }
    for (const selector of hoverSelectors) {
      expect(winningDeclaration(darkRules, new Set([selector]), 'background'), selector).toBe(
        '#2f343d',
      )
      expect(winningDeclaration(darkRules, new Set([selector]), 'border-color'), selector).toBe(
        '#8896a8',
      )
      expect(contrastRatio('#8896a8', '#2f343d'), selector).toBeGreaterThanOrEqual(3)
    }
    for (const selector of openSelectors) {
      expect(winningDeclaration(darkRules, new Set([selector]), 'background'), selector).toBe(
        '#244d7d',
      )
      expect(winningDeclaration(darkRules, new Set([selector]), 'border-color'), selector).toBe(
        '#5d9ee8',
      )
    }
    for (const selector of focusSelectors) {
      expect(winningDeclaration(darkRules, new Set([selector]), 'outline-color'), selector).toBe(
        '#8ec8ff',
      )
    }
  })

  it('renders image and text previews as shared light surfaces', () => {
    const rules = parseRules(appCss)
    const declaration = (selector: string, property: string) =>
      winningDeclaration(rules, new Set([selector]), property)

    expect(declaration('.preview-overlay', 'background')).toBe('#f5f6f8')
    expect(declaration('.preview-overlay', 'color')).toBe('#1f2328')

    expect(declaration('.preview-toolbar', 'background')).toBe('#fbfcfd')
    expect(declaration('.preview-toolbar', 'border-bottom')).toBe('1px solid #d8dce2')
    expect(declaration('.preview-toolbar', 'color')).toBe('#1f2328')

    expect(declaration('.image-preview-stage', 'background')).toBe('#edf0f3')
    expect(declaration('.image-preview-stage img', 'border')).toBe('1px solid #d8dce2')
    expect(declaration('.image-preview-stage img', 'box-shadow')).toBe(
      '0 8px 28px rgb(34 42 53 / 14%)',
    )

    expect(declaration('.image-preview > footer', 'background')).toBe('#fbfcfd')
    expect(declaration('.image-preview > footer', 'border-top')).toBe('1px solid #d8dce2')
    expect(declaration('.image-preview > footer', 'color')).toBe('#1f2328')
    expect(declaration('.preview-overlay [role="alert"]', 'color')).toBe('#9d1c13')

    expect(declaration('.text-preview', 'background')).toBe('#f7f7f8')
    expect(declaration('.text-preview', 'color')).toBe('#1f2328')
    expect(declaration('.text-preview .preview-toolbar', 'position')).toBe('sticky')
    expect(declaration('.text-preview .preview-toolbar', 'top')).toBe('0')
    expect(declaration('.text-preview .preview-toolbar', 'color')).toBe('#1f2328')

    for (const selector of [
      '.preview-toolbar button',
      '.preview-toolbar select',
      '.image-preview > footer button',
    ]) {
      expect(declaration(selector, 'background')).toBe('#fff')
      expect(declaration(selector, 'border')).toBe('1px solid #8a94a3')
      expect(declaration(selector, 'border-radius')).toBe('6px')
      expect(declaration(selector, 'color')).toBe('#1f2328')
      expect(declaration(selector, 'min-height')).toBe('30px')
    }

    expect(contrastRatio('#8a94a3', '#ffffff')).toBeGreaterThanOrEqual(3)

    for (const selector of [
      '.preview-toolbar button:hover:not(:disabled)',
      '.preview-toolbar select:hover',
      '.image-preview > footer button:hover:not(:disabled)',
    ]) {
      expect(declaration(selector, 'background')).toBe('#f3f5f7')
      expect(declaration(selector, 'border-color')).toBe('#747f8e')
    }

    expect(contrastRatio('#747f8e', '#f3f5f7')).toBeGreaterThanOrEqual(3)

    for (const selector of [
      '.preview-toolbar button:focus-visible',
      '.preview-toolbar select:focus-visible',
      '.image-preview > footer button:focus-visible',
    ]) {
      expect(declaration(selector, 'outline')).toBe('2px solid #2477d4')
      expect(declaration(selector, 'outline-offset')).toBe('2px')
    }
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
  let depth = 0
  let selectorStart = 0
  let bodyStart = -1

  for (let index = 0; index < css.length; index += 1) {
    if (css[index] === '{') {
      depth += 1
      if (depth === 1) bodyStart = index + 1
      continue
    }
    if (css[index] !== '}') continue

    if (depth === 1 && bodyStart !== -1) {
      const selectorText = css.slice(selectorStart, bodyStart - 1).trim()
      const body = css.slice(bodyStart, index)
      if (!selectorText.startsWith('@')) {
        const declarations = Object.fromEntries(
          [...body.matchAll(/([\w-]+)\s*:\s*([^;]+);/g)].map((declaration) => [
            defined(declaration[1], 'Expected CSS declaration property'),
            defined(declaration[2], 'Expected CSS declaration value').trim(),
          ]),
        )
        for (const selector of selectorText.split(',')) {
          rules.push({ selector: selector.trim(), declarations, order: rules.length })
        }
      }
      selectorStart = index + 1
      bodyStart = -1
    }
    depth -= 1
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
    .filter((rule) => matchingSelectors.has(rule.selector) && rule.declarations[property])
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
  const classesAndAttributes = selector.match(/\.[\w-]+|\[[^\]]+\]|:(?!:)[\w-]+/g)?.length ?? 0
  const elements =
    selector.replace(/#[\w-]+|\.[\w-]+|\[[^\]]+\]|:{1,2}[\w-]+/g, ' ').match(/[a-z][\w-]*/gi)
      ?.length ?? 0
  return [ids, classesAndAttributes, elements]
}

function compareSpecificity(
  left: [number, number, number],
  right: [number, number, number],
): number {
  for (let index = 0; index < left.length; index += 1) {
    if (left[index] !== right[index]) {
      return (
        defined(left[index], `Expected left specificity component ${index}`) -
        defined(right[index], `Expected right specificity component ${index}`)
      )
    }
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
  const channels = [1, 3, 5].map(
    (index) => Number.parseInt(color.slice(index, index + 2), 16) / 255,
  )
  const [red, green, blue] = channels.map((channel) =>
    channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4,
  )
  return (
    0.2126 * defined(red, 'Expected red luminance channel') +
    0.7152 * defined(green, 'Expected green luminance channel') +
    0.0722 * defined(blue, 'Expected blue luminance channel')
  )
}

function pixels(value: string | undefined): number {
  return Number.parseFloat(value ?? 'NaN')
}
