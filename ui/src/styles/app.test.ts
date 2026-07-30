import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'
import { defined } from '../defined'

const appCss = readFileSync('src/styles/app.css', 'utf8')

describe('workspace style contracts', () => {
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

  it('gives the folder tree all remaining sidebar height', () => {
    const rules = parseRules(appCss)
    const sidebar = rules.find((rule) => rule.selector === '.folder-sidebar')
    const tree = rules.find((rule) => rule.selector === '.folder-tree')

    expect(sidebar?.declarations).toMatchObject({
      display: 'flex',
      'flex-direction': 'column',
    })
    expect(tree?.declarations).toMatchObject({
      flex: '1',
      'min-height': '0',
    })
  })

  it('keeps the sidebar collapse control compact in the flex column', () => {
    const rules = parseRules(appCss)
    const collapse = rules.find((rule) => rule.selector === '.folder-sidebar > button:first-child')

    expect(collapse?.declarations['align-self']).toBe('flex-start')
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
  })

  it('renders workspace menu summaries as stateful toolbar buttons', () => {
    const rules = parseRules(appCss)
    const baseSelectors = [
      '.search-options-panel > summary',
      '.workspace-view-menu > summary',
      '.workspace-more-menu > summary',
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
      '.workspace-view-menu > summary:hover',
      '.workspace-more-menu > summary:hover',
    ]
    for (const selector of hoverSelectors) {
      expect(winningDeclaration(rules, new Set([selector]), 'background'), selector).toBe(
        'var(--viewer-soft-surface)',
      )
      expect(winningDeclaration(rules, new Set([selector]), 'border-color'), selector).toBe(
        'var(--viewer-border)',
      )
    }

    const openSelectors = [
      '.search-options-panel[open] > summary',
      '.workspace-view-menu[open] > summary',
      '.workspace-more-menu[open] > summary',
    ]
    for (const selector of openSelectors) {
      expect(winningDeclaration(rules, new Set([selector]), 'background'), selector).toBe(
        'var(--viewer-accent-soft)',
      )
      expect(winningDeclaration(rules, new Set([selector]), 'border-color'), selector).toBe(
        'var(--viewer-accent)',
      )
    }

    const focusSelectors = [
      '.search-options-panel > summary:focus-visible',
      '.workspace-view-menu > summary:focus-visible',
      '.workspace-more-menu > summary:focus-visible',
    ]
    for (const selector of focusSelectors) {
      expect(winningDeclaration(rules, new Set([selector]), 'outline'), selector).toBe(
        'var(--viewer-focus-ring)',
      )
      expect(winningDeclaration(rules, new Set([selector]), 'outline-offset'), selector).toBe('2px')
    }
  })

  it('uses one light theme for image and text previews', () => {
    const rules = parseRules(appCss)
    const declaration = (selector: string, property: string) =>
      winningDeclaration(rules, new Set([selector]), property)

    expect(declaration('.preview-overlay', 'background')).toBe('#f0f1ef')
    expect(declaration('.preview-overlay', 'color')).toBe('var(--viewer-text)')
    expect(declaration('.preview-toolbar', 'background')).toBe('var(--viewer-surface)')
    expect(declaration('.preview-toolbar', 'border-bottom')).toBe('1px solid var(--viewer-border)')
    expect(declaration('.preview-toolbar', 'color')).toBe('var(--viewer-text)')
    expect(declaration('.image-preview-stage', 'background')).toBe('var(--preview-stage)')
    expect(declaration('.image-preview-stage img', 'border')).toBe(
      '1px solid var(--preview-border)',
    )
    expect(declaration('.image-preview-stage img', 'box-shadow')).toBe(
      'var(--preview-image-shadow)',
    )
    expect(declaration('.preview-navigation-float', 'background')).toBe('var(--viewer-surface)')
    expect(declaration('.preview-navigation-float', 'border')).toBe(
      '1px solid var(--viewer-border)',
    )
    expect(declaration('.preview-navigation-float', 'color')).toBe('var(--viewer-text)')
    expect(declaration('.preview-overlay [role="alert"]', 'color')).toBe('var(--preview-danger)')
    expect(declaration('.text-preview', 'background')).toBe('var(--preview-document-surface)')
    expect(declaration('.text-preview', 'color')).toBe('var(--preview-text)')
    expect(declaration('.text-preview .preview-toolbar', 'position')).toBe('sticky')
    expect(declaration('.text-preview .preview-toolbar', 'top')).toBe('0')
    expect(declaration('.text-preview .preview-toolbar', 'color')).toBe('var(--preview-text)')

    for (const selector of [
      '.preview-toolbar button',
      '.preview-toolbar select',
      '.preview-navigation-float button',
    ]) {
      expect(declaration(selector, 'background')).toBe('var(--preview-control-surface)')
      expect(declaration(selector, 'border')).toBe('1px solid var(--preview-control-border)')
      expect(declaration(selector, 'border-radius')).toBe('6px')
      expect(declaration(selector, 'color')).toBe('var(--preview-text)')
      expect(declaration(selector, 'min-height')).toBe('30px')
    }

    for (const selector of [
      '.preview-toolbar button:hover:not(:disabled)',
      '.preview-toolbar select:hover',
      '.preview-navigation-float button:hover:not(:disabled)',
    ]) {
      expect(declaration(selector, 'background')).toBe('var(--preview-control-hover-surface)')
      expect(declaration(selector, 'border-color')).toBe('var(--preview-control-hover-border)')
    }

    for (const selector of [
      '.preview-toolbar button:focus-visible',
      '.preview-toolbar select:focus-visible',
      '.preview-navigation-float button:focus-visible',
    ]) {
      expect(declaration(selector, 'outline')).toBe('2px solid var(--preview-accent)')
      expect(declaration(selector, 'outline-offset')).toBe('2px')
    }

    expect(contrastRatio('#1f2328', '#f5f6f8')).toBeGreaterThanOrEqual(4.5)
    expect(contrastRatio('#68717d', '#fbfcfd')).toBeGreaterThanOrEqual(4.5)
    expect(contrastRatio('#8a94a3', '#ffffff')).toBeGreaterThanOrEqual(3)
    expect(contrastRatio('#747f8e', '#f3f5f7')).toBeGreaterThanOrEqual(3)
    expect(contrastRatio('#9d1c13', '#fff3f0')).toBeGreaterThanOrEqual(4.5)
  })

  it('renders image comparison from the shared light preview theme', () => {
    const rules = parseRules(appCss)
    const declaration = (selector: string, property: string) =>
      winningDeclaration(rules, new Set([selector]), property)

    expect(declaration('.compare-workspace', 'background')).toBe('#f0f1ef')
    expect(declaration('.compare-workspace', 'border-radius')).toBe('10px')
    expect(declaration('.compare-workspace', 'border')).toBe('')
    expect(declaration('.compare-workspace', 'color')).toBe('var(--viewer-text)')
    expect(declaration('.compare-toolbar', 'background')).toBe('var(--viewer-surface)')
    expect(declaration('.compare-toolbar', 'border-bottom')).toBe('1px solid var(--viewer-border)')
    expect(declaration('.compare-toolbar-leading > span', 'color')).toBe(
      'var(--viewer-text-secondary)',
    )
    expect(declaration('.compare-layout-region', 'background')).toBe('var(--preview-surface)')
    expect(declaration('.compare-layout-region', 'overflow')).toBe('hidden')
    expect(declaration('.compare-layout-region', 'position')).toBe('relative')
    expect(declaration('.compare-fit-layout', 'height')).toBe('100%')
    expect(declaration('.compare-fit-layout', 'overflow')).toBe('hidden')
    expect(declaration('.compare-scroll-viewport[data-axis="horizontal"]', 'overflow-x')).toBe(
      'auto',
    )
    expect(declaration('.compare-scroll-viewport[data-axis="horizontal"]', 'overflow-y')).toBe(
      'hidden',
    )
    expect(declaration('.compare-scroll-viewport[data-axis="vertical"]', 'overflow-x')).toBe(
      'hidden',
    )
    expect(declaration('.compare-scroll-viewport[data-axis="vertical"]', 'overflow-y')).toBe('auto')
    expect(declaration('.compare-layout-item', 'contain')).toBe('layout paint')
    expect(declaration('.compare-layout-item', 'position')).toBe('absolute')

    expect(declaration('.compare-pane', 'background')).toBe('var(--viewer-surface)')
    expect(declaration('.compare-pane', 'border')).toBe('1px solid var(--viewer-border)')
    expect(declaration('.compare-pane[data-active="true"]', 'border-color')).toBe(
      'var(--viewer-accent)',
    )
    expect(declaration('.compare-pane-header', 'background')).toBe('var(--preview-chrome)')
    expect(declaration('.compare-pane-header', 'border-bottom')).toBe(
      '1px solid var(--preview-border)',
    )
    expect(declaration('.compare-pane-stage', 'background')).toBe('var(--preview-stage)')
    expect(declaration('.compare-pane-stage', 'overflow')).toBe('hidden')
    expect(declaration('.compare-pane-stage img', 'object-fit')).toBe('contain')
    expect(declaration('.compare-pane-stage img', 'outline')).toBe(
      '1px solid var(--preview-border)',
    )
    expect(declaration('.compare-pane-stage img', 'box-shadow')).toBe('var(--preview-image-shadow)')
    expect(declaration('.compare-pane > footer', 'background')).toBe('var(--preview-chrome)')
    expect(declaration('.compare-pane > footer', 'border-top')).toBe(
      '1px solid var(--preview-border)',
    )

    for (const selector of [
      '.compare-toolbar button',
      '.compare-pane button',
      '.compare-invalid > button',
    ]) {
      expect(declaration(selector, 'background')).toBe('var(--preview-control-surface)')
      expect(declaration(selector, 'border')).toBe('1px solid var(--preview-control-border)')
      expect(declaration(selector, 'border-radius')).toBe('5px')
      expect(declaration(selector, 'color')).toBe('var(--preview-text)')
      expect(declaration(selector, 'min-height')).toBe('28px')
    }

    expect(declaration('.compare-pane-header button', 'min-height')).toBe('24px')
    expect(declaration('.compare-pane-header button', 'min-width')).toBe('26px')
    expect(declaration('.compare-pane-header button', 'padding')).toBe('0')
    expect(declaration('.compare-pane > footer .marker-buttons button', 'min-height')).toBe('24px')
    expect(declaration('.compare-pane > footer .marker-buttons button', 'padding')).toBe('2px 5px')

    for (const selector of [
      '.compare-toolbar button:hover:not(:disabled):not([aria-pressed="true"])',
      '.compare-pane button:hover:not(:disabled):not([aria-pressed="true"])',
      '.compare-invalid > button:hover:not(:disabled)',
    ]) {
      expect(declaration(selector, 'background')).toBe('var(--preview-control-hover-surface)')
      expect(declaration(selector, 'border-color')).toBe('var(--preview-control-hover-border)')
    }

    for (const selector of [
      '.compare-toolbar button:focus-visible',
      '.compare-pane button:focus-visible',
      '.compare-invalid > button:focus-visible',
    ]) {
      expect(declaration(selector, 'outline')).toBe('2px solid var(--preview-accent)')
      expect(declaration(selector, 'outline-offset')).toBe('2px')
    }

    expect(
      winningDeclaration(
        rules,
        new Set([
          '.compare-toolbar button[aria-pressed="true"]',
          '.compare-pane .marker-buttons button[aria-pressed="true"]',
        ]),
        'background',
      ),
    ).toBe('var(--preview-accent-surface)')
    expect(declaration('.compare-toolbar button[aria-pressed="true"]', 'color')).toBe(
      'var(--preview-accent-text)',
    )
    expect(declaration('.compare-pane .marker-buttons button[aria-pressed="true"]', 'color')).toBe(
      'var(--preview-accent-text)',
    )

    for (const selector of [
      '.compare-pane > p[role="alert"]',
      '.compare-invalid > p[role="alert"]',
    ]) {
      expect(declaration(selector, 'background')).toBe('var(--preview-danger-surface)')
      expect(declaration(selector, 'color')).toBe('var(--preview-danger)')
    }
    expect(declaration('.compare-invalid > p[role="alert"]', 'font-size')).toBe('')
    expect(declaration('.compare-invalid > p[role="alert"]', 'margin')).toBe('')
    expect(declaration('.compare-invalid > p[role="alert"]', 'padding')).toBe('')

    const lightRules = parseRules(appCss)
    expect(
      winningDeclaration(
        lightRules,
        new Set(['.radial-menu-button[aria-disabled="true"]']),
        'filter',
      ),
    ).toBe('grayscale(1)')
    expect(
      winningDeclaration(
        lightRules,
        new Set(['.radial-menu-button[aria-disabled="true"]']),
        'opacity',
      ),
    ).toBe('0.38')

    const legacyColors = [
      '#171a1f',
      '#24282f',
      '#0e1013',
      '#20242a',
      '#303640',
      '#4d5663',
      '#275e9d',
      '#4e8ed8',
      '#4e94df',
      '#4b2c22',
      '#ffd5c7',
      '#b8c0ca',
    ]
    for (const rule of rules.filter((candidate) => candidate.selector.startsWith('.compare'))) {
      for (const value of Object.values(rule.declarations)) {
        for (const legacyColor of legacyColors) {
          expect(value, `${rule.selector} ${legacyColor}`).not.toContain(legacyColor)
        }
      }
    }

    expect(contrastRatio('#174f8f', '#d9e8ff')).toBeGreaterThanOrEqual(4.5)
    expect(contrastRatio('#2477d4', '#ffffff')).toBeGreaterThanOrEqual(3)
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
