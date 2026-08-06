import { readdirSync, readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'
import { defined } from '../defined'

const appCss = readFileSync('src/styles/app.css', 'utf8')
const viewerStyleFiles = readdirSync('src/styles').filter((file) => file.endsWith('.css'))
const viewerStyleSources = viewerStyleFiles
  .map((file) => readFileSync(`src/styles/${file}`, 'utf8'))
  .join('\n')

describe('workspace style contracts', () => {
  it('keeps the resting entry copy on the approved title and body scale', () => {
    const rules = parseRules(appCss)
    const title = rules.find((rule) => rule.selector === '.empty-project h1')
    const copy = rules.find((rule) => rule.selector === '.empty-project p')

    expect(title?.declarations['font-size']).toBe('20px')
    expect(copy?.declarations['font-size']).toBe('13px')
  })

  it('keeps the main workspace header at the approved compact density', () => {
    const rules = parseRules(appCss)
    const header = rules.find((rule) => rule.selector === '.workspace-header')

    expect(header?.declarations['min-height']).toBe('40px')
  })

  it('keeps aggregate browsing as a compact status tag instead of a full-width band', () => {
    const rules = parseRules(appCss)
    const aggregate = rules.find(
      (rule) => rule.selector === '.content-workspace-surface > .aggregate-label',
    )

    expect(aggregate?.declarations).toMatchObject({
      display: 'inline-flex',
      width: 'max-content',
      'max-width': 'calc(100% - 32px)',
      margin: '0 16px 8px',
    })
  })

  it('keeps projection progress local without concealing the workspace for thumbnails', () => {
    const rules = parseRules(appCss)
    expect(
      rules.find(
        ({ selector }) =>
          selector ===
            ".content-workspace-surface[data-thumbnail-loading='true'] > .content-browser" ||
          selector ===
            ".content-workspace-surface[data-thumbnail-loading='true'] > .workspace-loading",
      ),
    ).toBeUndefined()

    const progress = rules.find(({ selector }) => selector === '.projection-progress')
    expect(progress?.declarations).toMatchObject({
      background: 'var(--viewer-accent)',
      height: '2px',
      'inset-inline': '0',
      'pointer-events': 'none',
      position: 'absolute',
      top: '0',
    })

    const reducedProgress = parseRules(mediaBody(appCss, '(prefers-reduced-motion: reduce)')).find(
      ({ selector }) => selector === '.projection-progress',
    )
    expect(reducedProgress?.declarations.animation).toBe('none')
  })

  it('keeps folder rows at the approved compact density', () => {
    const rules = parseRules(appCss)

    expect(winningDeclaration(rules, new Set(['.folder-tree-row']), 'height')).toBe('24px')
  })

  it('separates the radial command layer from the workspace with a subtle scrim', () => {
    const rules = parseRules(appCss)
    const layer = rules.find((rule) => rule.selector === '.radial-menu-layer')
    const scrim = rules.find((rule) => rule.selector === '.radial-menu-scrim')

    expect(layer?.declarations).toMatchObject({
      position: 'fixed',
      inset: '0',
      'z-index': '25',
    })
    expect(scrim?.declarations).toMatchObject({
      position: 'absolute',
      inset: '0',
      background: 'var(--viewer-backdrop-subtle)',
      border: '0',
    })
  })

  it('paints selection around the square complete card and keyboard focus outside it', () => {
    const rules = parseRules(appCss)
    const card = rules.find((rule) => rule.selector === '.image-cell')
    const selectionOverlay = rules.find(
      (rule) => rule.selector === '.image-cell[aria-selected="true"]::after',
    )
    const selectedCard = rules.find((rule) => rule.selector === '.image-cell[aria-selected="true"]')
    const legacyOverlay = rules.find(
      (rule) => rule.selector === ".image-cell-thumbnail-frame[data-selected='true']::after",
    )
    const organizationHandle = rules.find(
      (rule) => rule.selector === '.image-cell > .organization-drag-handle',
    )
    const activeFocus = rules.find(
      (rule) =>
        rule.selector === '.aspect-virtual-grid:focus-visible .image-cell[data-active="true"]',
    )
    const forcedSelection = parseRules(mediaBody(appCss, '(forced-colors: active)')).find(
      (rule) => rule.selector === '.image-cell[aria-selected="true"]::after',
    )

    expect(card?.declarations['border-radius']).toBe('0')
    expect(selectionOverlay?.declarations).toMatchObject({
      border: '2px solid var(--viewer-accent)',
      'border-radius': '0',
      'box-sizing': 'border-box',
      inset: '0',
      'pointer-events': 'none',
      position: 'absolute',
      'z-index': '2',
    })
    expect(legacyOverlay).toBeUndefined()
    expect(selectedCard?.declarations.background).toBeUndefined()
    expect(selectedCard?.declarations['box-shadow']).toBeUndefined()
    expect(organizationHandle?.declarations['z-index']).toBe('3')
    expect(activeFocus?.declarations).toMatchObject({
      outline: 'var(--viewer-focus-outline)',
      'outline-offset': 'var(--viewer-focus-offset)',
    })
    expect(forcedSelection?.declarations).toMatchObject({
      'border-color': 'Highlight',
      'border-width': '2px',
    })
  })

  it('paints the neutral card outline above thumbnail content on every edge', () => {
    const rules = parseRules(appCss)
    const card = rules.find((rule) => rule.selector === '.image-cell')
    const outline = rules.find((rule) => rule.selector === '.image-cell::before')

    expect(card?.declarations.border).toBe('0')
    expect(outline?.declarations).toMatchObject({
      border: '1px solid var(--viewer-thumbnail-border)',
      'box-sizing': 'border-box',
      content: '""',
      inset: '0',
      'pointer-events': 'none',
      position: 'absolute',
      'z-index': '1',
    })
  })

  it('uses the approved quiet card and compact file metadata hierarchy', () => {
    const rules = parseRules(appCss)
    const card = rules.find((rule) => rule.selector === '.image-cell')
    const stage = rules.find((rule) => rule.selector === '.image-cell-preview')
    const metadata = rules.find((rule) => rule.selector === '.image-cell-meta')
    const name = rules.find((rule) => rule.selector === '.image-cell-name')
    const marker = rules.find((rule) => rule.selector === '.image-cell-meta > .file-marker')

    expect(card?.declarations).toMatchObject({
      background: 'var(--viewer-thumbnail-card-surface)',
      border: '0',
      'border-radius': '0',
    })
    expect(stage?.declarations.background).toBe('var(--viewer-thumbnail-stage-surface)')
    expect(metadata?.declarations).toMatchObject({
      'border-top': '1px solid var(--viewer-thumbnail-border)',
      display: 'flex',
      gap: '8px',
      'min-height': '48px',
      padding: '6px 8px',
    })
    expect(name?.declarations).toMatchObject({
      'font-size': '13px',
      'font-weight': '500',
      overflow: 'hidden',
      'text-overflow': 'ellipsis',
      'white-space': 'nowrap',
    })
    expect(marker?.declarations).toMatchObject({
      'border-radius': '999px',
      'font-size': '10px',
      'font-weight': '700',
      padding: '3px 6px',
    })
  })

  it('renders workspace popover commands as quiet menu rows with a real separator', () => {
    const rules = parseRules(appCss)
    const popover = rules.find(
      (rule) =>
        rule.selector === '.workspace-menu-popover' && rule.declarations['min-width'] !== undefined,
    )
    const item = rules.find(
      (rule) => rule.selector === '.workspace-menu-popover .workspace-menu-item',
    )
    const separator = rules.find((rule) => rule.selector === '.workspace-menu-separator')
    const dangerInteraction = rules.find(
      (rule) =>
        rule.selector ===
        ".workspace-menu-item[data-danger-reveal='interaction']:hover:not(:disabled)",
    )

    expect(popover?.declarations['min-width']).toBe('180px')
    expect(item?.declarations).toMatchObject({
      border: '0',
      'justify-content': 'flex-start',
      'min-height': '32px',
      'text-align': 'left',
      width: '100%',
    })
    expect(separator?.declarations).toMatchObject({
      border: '0',
      'border-top': '1px solid var(--viewer-divider-subtle)',
      margin: '6px 0',
    })
    expect(dangerInteraction?.declarations).toMatchObject({
      background: 'var(--viewer-danger-emphasis-soft)',
      color: 'var(--viewer-danger-strong)',
    })
  })

  it('anchors the formal information inspector below the 40px workspace header', () => {
    const rules = parseRules(viewerStyleSources)
    const inspector = rules.find((rule) => rule.selector === '.viewer-inspector')

    expect(inspector?.declarations).toMatchObject({
      position: 'fixed',
      top: '40px',
      right: '0',
      bottom: '0',
    })
    expect(rules.some((rule) => rule.selector === '.info-overlay')).toBe(false)
  })

  it('routes operation results through the same formal inspector rail', () => {
    const rules = parseRules(viewerStyleSources)
    const inspector = rules.find((rule) => rule.selector === '.viewer-inspector')

    expect(inspector?.declarations.top).toBe('40px')
    expect(rules.some((rule) => rule.selector === '.operation-results')).toBe(false)
    expect(rules.some((rule) => rule.selector === '.operation-results-list')).toBe(true)
  })

  it('keeps conflict controls on a readable second row instead of squeezing four columns', () => {
    const rules = parseRules(appCss)
    const row = rules.find((rule) => rule.selector === '.conflict-row')
    const policy = rules.find((rule) => rule.selector === '.conflict-policy-field')
    const applyRemaining = rules.find(
      (rule) => rule.selector === '.conflict-row > .viewer-choice-chip',
    )
    const blockedCode = rules.find(
      (rule) => rule.selector === '.conflict-row[data-state="blocked"] code',
    )

    expect(row?.declarations['grid-template-columns']).toBe('minmax(0, 1fr) auto')
    expect(policy?.declarations['grid-column']).toBe('1')
    expect(applyRemaining?.declarations).toMatchObject({
      'grid-column': '2',
      'justify-self': 'end',
      'white-space': 'nowrap',
    })
    expect(blockedCode?.declarations['grid-column']).toBe('1 / -1')
  })

  it('keeps global notices beside the formal inspector instead of covering it', () => {
    const rules = parseRules(appCss)
    const besideInspector = rules.find(
      (rule) => rule.selector === 'body:has(.viewer-inspector) .global-notice-stack',
    )

    expect(besideInspector?.declarations).toMatchObject({
      right: 'calc(clamp(320px, 24vw, 340px) + 16px)',
      width: 'min(360px, calc(100vw - clamp(320px, 24vw, 340px) - 48px))',
    })
  })

  it('consolidates task feedback into one compact surface', () => {
    const rules = parseRules(appCss)
    const surface = rules.find((rule) => rule.selector === '.task-surface')
    const list = rules.find((rule) => rule.selector === '.task-list')

    expect(surface?.declarations).toMatchObject({
      background: 'var(--viewer-surface)',
      border: '1px solid var(--viewer-border)',
      'border-radius': 'var(--viewer-radius-popover)',
      'box-shadow': 'var(--viewer-shadow-popover)',
      overflow: 'hidden',
      'pointer-events': 'auto',
    })
    expect(list?.declarations).toMatchObject({
      display: 'grid',
      gap: '0',
    })
  })

  it('keeps the one-category settings shell compact instead of reserving empty rows', () => {
    const rules = parseRules(appCss)
    const shell = rules.find((rule) => rule.selector === '.settings-dialog-shell')

    expect(shell?.declarations['min-height']).toBe('144px')
  })

  it('gives the five-stop thumbnail slider one cross-platform visual contract', () => {
    const rules = parseRules(appCss)
    const slider = rules.find((rule) => rule.selector === '.thumbnail-size-slider')
    const track = rules.find(
      (rule) => rule.selector === '.thumbnail-size-slider::-webkit-slider-runnable-track',
    )
    const thumb = rules.find(
      (rule) => rule.selector === '.thumbnail-size-slider::-webkit-slider-thumb',
    )
    const levels = rules.find((rule) => rule.selector === '.thumbnail-size-levels')
    const current = rules.find(
      (rule) => rule.selector === '.thumbnail-size-levels span[data-current="true"]',
    )

    expect(slider?.declarations).toMatchObject({
      appearance: 'none',
      background: 'transparent',
      height: '32px',
      width: '100%',
    })
    expect(track?.declarations).toMatchObject({
      'border-radius': '999px',
      height: '4px',
    })
    expect(track?.declarations.background).toContain('var(--thumbnail-level-progress)')
    expect(thumb?.declarations).toMatchObject({
      appearance: 'none',
      background: 'var(--viewer-accent)',
      'border-radius': '50%',
      height: '18px',
      width: '18px',
    })
    expect(levels?.declarations).toMatchObject({
      display: 'flex',
      'justify-content': 'space-between',
      'padding-inline': '9px',
    })
    expect(current?.declarations).toMatchObject({
      color: 'var(--viewer-accent-text)',
      'font-weight': '700',
    })

    const reduced = parseRules(mediaBody(appCss, '(prefers-reduced-motion: reduce)')).find(
      (rule) => rule.selector === '.thumbnail-size-slider',
    )
    expect(reduced?.declarations.transition).toBe('none')

    const forced = parseRules(mediaBody(appCss, '(forced-colors: active)'))
    expect(
      forced.find(
        (rule) => rule.selector === '.thumbnail-size-slider::-webkit-slider-runnable-track',
      )?.declarations.background,
    ).toBe('CanvasText')
    expect(
      forced.find((rule) => rule.selector === '.thumbnail-size-slider::-webkit-slider-thumb')
        ?.declarations.background,
    ).toBe('Highlight')
  })

  it('keeps background task feedback behind a modal so it cannot cover dialog actions', () => {
    const rules = parseRules(appCss)
    const behindDialog = rules.find(
      (rule) => rule.selector === 'body:has(.viewer-dialog-backdrop) .task-bar',
    )

    expect(behindDialog?.declarations['z-index']).toBe('29')
  })

  it('groups preview transforms and keeps completion visibly labeled', () => {
    const rules = parseRules(viewerStyleSources)
    const segmented = rules.find((rule) => rule.selector === '.viewer-segmented-control')
    const segmentedButtons = rules.find(
      (rule) => rule.selector === '.viewer-segmented-control > .viewer-button',
    )
    const complete = rules.find((rule) => rule.selector === '.preview-complete-action')

    expect(segmented?.declarations).toMatchObject({
      background: 'var(--viewer-surface)',
      border: '1px solid var(--viewer-control-border)',
      'border-radius': 'var(--viewer-radius-control)',
      overflow: 'hidden',
    })
    expect(segmentedButtons?.declarations).toMatchObject({
      border: '0',
      'border-radius': '0',
      'min-height': '30px',
    })
    expect(complete?.declarations).toMatchObject({
      'font-weight': '650',
      'min-width': '52px',
    })
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

  it('keeps the collapsed header action and navigation rail compact', () => {
    const rules = parseRules(appCss)
    const headerAction = rules.find(
      (rule) => rule.selector === '.project-identity[data-collapsed="true"] > button',
    )
    const rail = rules.find((rule) => rule.selector === '.folder-sidebar[data-collapsed="true"]')

    expect(headerAction?.declarations['white-space']).toBe('nowrap')
    expect(rail?.declarations.padding).toBe('8px 4px')
  })

  it('keeps folder identity fixed beside an independently scrolling filmstrip', () => {
    const rules = parseRules(appCss)
    const row = rules.find((rule) => rule.selector === '.folder-filmstrip-row')
    const shell = rules.find((rule) => rule.selector === '.folder-filmstrip-shell')
    const filmstrip = rules.find((rule) => rule.selector === '.folder-filmstrip')
    const track = rules.find((rule) => rule.selector === '.folder-filmstrip-track')
    const item = rules.find((rule) => rule.selector === '.folder-filmstrip-item')
    const thumbnail = rules.find((rule) => rule.selector === '.folder-filmstrip-thumbnail')
    const thumbnailHover = rules.find(
      (rule) => rule.selector === '.folder-filmstrip-thumbnail:hover',
    )
    const image = rules.find((rule) => rule.selector === '.aspect-thumbnail > img')
    const placeholder = rules.find((rule) => rule.selector === '.aspect-thumbnail-placeholder')
    const nativeScrollbar = rules.find(
      (rule) => rule.selector === '.folder-filmstrip::-webkit-scrollbar',
    )
    const scrollbar = rules.find((rule) => rule.selector === '.folder-filmstrip-scrollbar')
    const thumb = rules.find((rule) => rule.selector === '.folder-filmstrip-scrollbar-thumb')
    const visibleScrollbarSelectors = [
      '.folder-filmstrip-row:hover .folder-filmstrip-scrollbar',
      '.folder-filmstrip-row:focus-within .folder-filmstrip-scrollbar',
    ]
    const reducedScrollbar = parseRules(mediaBody(appCss, '(prefers-reduced-motion: reduce)')).find(
      (rule) => rule.selector === '.folder-filmstrip-scrollbar',
    )

    expect(row?.declarations).toMatchObject({
      display: 'grid',
      'grid-template-columns': '184px minmax(0, 1fr)',
    })
    expect(shell?.declarations).toMatchObject({
      'min-width': '0',
      position: 'relative',
    })
    expect(filmstrip?.declarations).toMatchObject({
      'min-width': '0',
      'overflow-x': 'auto',
      'overflow-y': 'hidden',
      'scrollbar-width': 'none',
    })
    expect(nativeScrollbar?.declarations.display).toBe('none')
    expect(scrollbar?.declarations).toMatchObject({
      opacity: '0',
      'pointer-events': 'none',
      transition: 'opacity 180ms ease',
    })
    expect(thumb?.declarations).toMatchObject({
      background: 'var(--viewer-text-tertiary)',
      'border-radius': '999px',
    })
    for (const selector of visibleScrollbarSelectors) {
      expect(rules.find((rule) => rule.selector === selector)?.declarations.opacity, selector).toBe(
        '1',
      )
    }
    expect(reducedScrollbar?.declarations.transition).toBe('none')
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
    expect(thumbnailHover?.declarations['box-shadow']).toBe(
      'inset 0 0 0 1px var(--viewer-thumbnail-hover-ring)',
    )
    expect(image?.declarations['object-fit']).toBe('contain')
    expect(image?.declarations.background).not.toBe('#e5e8ed')
    expect(placeholder?.declarations.background).toBe('var(--viewer-soft-surface)')
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
      outline: 'var(--viewer-focus-outline)',
      'outline-offset': 'var(--viewer-focus-offset)',
    })
    expect(thumbnailFocus?.declarations).toMatchObject({
      outline: 'var(--viewer-focus-outline)',
      'outline-offset': 'var(--viewer-focus-offset)',
    })
  })

  it('renders workspace menu summaries as stateful toolbar buttons', () => {
    const rules = parseRules(appCss)
    const baseSelectors = [
      '.search-options-panel > summary',
      '.workspace-view-menu > summary',
      '.workspace-more-menu > summary',
    ]

    for (const selector of baseSelectors) {
      expect(winningDeclaration(rules, new Set([selector]), 'background'), selector).toBe(
        'var(--viewer-surface)',
      )
      expect(winningDeclaration(rules, new Set([selector]), 'border'), selector).toBe(
        '1px solid var(--viewer-border-strong)',
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
        'var(--viewer-focus-outline)',
      )
      expect(winningDeclaration(rules, new Set([selector]), 'outline-offset'), selector).toBe(
        'var(--viewer-focus-offset)',
      )
    }
  })

  it('uses one valid focus contract for search, menus, and result controls', () => {
    const rules = parseRules(appCss)
    const searchFocus = rules.find((rule) => rule.selector === '.search-field:focus-within')
    expect(searchFocus?.declarations).toMatchObject({
      outline: 'var(--viewer-focus-outline)',
      'outline-offset': 'var(--viewer-focus-offset)',
    })

    for (const selector of [
      'button:focus-visible',
      'input:focus-visible',
      'select:focus-visible',
      'summary:focus-visible',
      '[tabindex]:focus-visible',
      '[role="menuitem"]:focus-visible',
      '[role="menuitemcheckbox"]:focus-visible',
      '[role="option"]:focus-visible',
    ]) {
      const focusRule = rules.find((rule) => rule.selector === selector)
      expect(focusRule?.declarations, selector).toMatchObject({
        outline: 'var(--viewer-focus-outline)',
        'outline-offset': 'var(--viewer-focus-offset)',
      })
    }

    expect(appCss).not.toContain('outline: var(--viewer-focus-shadow)')
    expect(appCss).not.toContain('outline: var(--viewer-focus-ring)')
  })

  it('uses semantic tokens instead of the retired cool palette', () => {
    const retiredHexValues = ['2477d4', 'd9e8ff', 'eef0f3', 'd8dce2', '59616c', '737982']
    for (const hexValue of retiredHexValues) {
      expect(viewerStyleSources.toLowerCase(), hexValue).not.toContain(`#${hexValue}`)
    }
  })

  it('requires component styles to consume semantic color roles', () => {
    const rawColorPattern =
      /#[0-9a-f]{3,8}\b|(?:rgb|hsl)a?\([^)]*\)|\b(?:white|black|red|blue|gr[ae]y|green|yellow|orange|purple|pink|brown|navy|teal|cyan|magenta|indigo|violet|gold|silver)\b/giu
    const violations = viewerStyleFiles
      .filter((file) => file !== 'tokens.css')
      .flatMap((file) => {
        const source = readFileSync(`src/styles/${file}`, 'utf8').replace(
          /\/\*[\s\S]*?\*\//g,
          (comment) => comment.replace(/[^\n]/g, ' '),
        )
        return [...source.matchAll(/([\w-]+)\s*:\s*([^;{}]+);/g)].flatMap((declaration) => {
          const matches = declaration[2]?.match(rawColorPattern) ?? []
          const line = source.slice(0, declaration.index).split('\n').length
          return matches.map((literal) => `${file}:${line} ${declaration[1]}: ${literal}`)
        })
      })

    // Transparent and currentColor are channel behavior, not palette literals.
    expect(violations).toEqual([])
  })

  it('marks selected sidebar rows with the approved leading accent indicator and text', () => {
    const rules = parseRules(appCss)
    for (const selector of [
      '.project-root-button[aria-pressed="true"]',
      '.folder-tree-row[aria-selected="true"]',
    ]) {
      const selectedRule = rules.find((rule) => rule.selector === selector)
      expect(selectedRule?.declarations, selector).toMatchObject({
        background: 'var(--viewer-accent-soft)',
        'box-shadow': 'inset 2px 0 var(--viewer-accent)',
        color: 'var(--viewer-accent-text)',
      })
    }
  })

  it('uses one light theme for image and text previews', () => {
    const rules = parseRules(viewerStyleSources)
    const declaration = (selector: string, property: string) =>
      winningDeclaration(rules, new Set([selector]), property)

    expect(declaration('.preview-overlay', 'background')).toBe('var(--preview-stage)')
    expect(declaration('.preview-overlay', 'color')).toBe('var(--viewer-text)')
    expect(declaration('.viewer-toolbar', 'background')).toBe('var(--viewer-surface)')
    expect(declaration('.viewer-toolbar', 'border-bottom')).toBe('1px solid var(--viewer-border)')
    expect(declaration('.viewer-toolbar', 'height')).toBe('52px')
    expect(declaration('.image-preview-stage', 'background')).toBe('var(--preview-stage)')
    expect(declaration('.image-preview-stage img', 'border')).toBe(
      '1px solid var(--preview-border)',
    )
    expect(declaration('.image-preview-stage img', 'box-shadow')).toBe(
      'var(--preview-image-shadow)',
    )
    for (const selector of [
      '.image-preview-stage img[data-mode="fit"]',
      '.image-preview-stage img[data-mode="free"]',
    ]) {
      expect(declaration(selector, 'height')).toBe('90%')
      expect(declaration(selector, 'width')).toBe('auto')
      expect(declaration(selector, 'max-width')).toBe('90%')
    }
    expect(declaration('.preview-navigation-float', 'background')).toBe('var(--viewer-surface)')
    expect(declaration('.preview-navigation-float', 'border')).toBe(
      '1px solid var(--viewer-border)',
    )
    expect(declaration('.preview-navigation-float', 'color')).toBe('var(--viewer-text)')
    expect(declaration('.preview-overlay [role="alert"]', 'color')).toBe('var(--preview-danger)')
    expect(declaration('.text-preview', 'background')).toBe('var(--preview-document-surface)')
    expect(declaration('.text-preview', 'color')).toBe('var(--preview-text)')
    expect(declaration('.preview-overlay > .viewer-toolbar', 'flex')).toBe('0 0 auto')
    expect(declaration('.text-encoding-field .viewer-field__control', 'width')).toBe('132px')

    expect(declaration('.preview-navigation-float .viewer-icon-button', 'min-height')).toBe('30px')
    expect(declaration('.preview-navigation-float .viewer-icon-button', 'width')).toBe('30px')

    expect(contrastRatio('#1f2328', '#f5f6f8')).toBeGreaterThanOrEqual(4.5)
    expect(contrastRatio('#68717d', '#fbfcfd')).toBeGreaterThanOrEqual(4.5)
    expect(contrastRatio('#8a94a3', '#ffffff')).toBeGreaterThanOrEqual(3)
    expect(contrastRatio('#747f8e', '#f3f5f7')).toBeGreaterThanOrEqual(3)
    expect(contrastRatio('#9d1c13', '#fff3f0')).toBeGreaterThanOrEqual(4.5)
  })

  it('renders image comparison from the shared light preview theme', () => {
    const rules = parseRules(viewerStyleSources)
    const declaration = (selector: string, property: string) =>
      winningDeclaration(rules, new Set([selector]), property)

    expect(declaration('.compare-workspace', 'background')).toBe('var(--preview-stage)')
    expect(declaration('.compare-workspace', 'border-radius')).toBe('10px')
    expect(declaration('.compare-workspace', 'border')).toBe('')
    expect(declaration('.compare-workspace', 'color')).toBe('var(--viewer-text)')
    expect(declaration('.viewer-toolbar', 'background')).toBe('var(--viewer-surface)')
    expect(declaration('.viewer-toolbar', 'border-bottom')).toBe('1px solid var(--viewer-border)')
    expect(declaration('.compare-workspace .viewer-toolbar__leading > span', 'color')).toBe(
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

    expect(declaration('.compare-pane .marker-buttons .viewer-button', 'background')).toBe(
      'var(--preview-control-surface)',
    )
    expect(declaration('.compare-pane .marker-buttons .viewer-button', 'border')).toBe(
      '1px solid var(--preview-control-border)',
    )
    expect(declaration('.viewer-button', 'border-radius')).toBe('var(--viewer-radius-control)')
    expect(declaration('.viewer-button', 'min-height')).toBe('32px')

    expect(declaration('.compare-pane-header button', 'min-height')).toBe('24px')
    expect(declaration('.compare-pane-header button', 'min-width')).toBe('26px')
    expect(declaration('.compare-pane-header button', 'padding')).toBe('0')
    expect(declaration('.compare-pane > footer .marker-buttons .viewer-button', 'min-height')).toBe(
      '24px',
    )
    expect(declaration('.compare-pane > footer .marker-buttons .viewer-button', 'padding')).toBe(
      '2px 5px',
    )

    expect(
      declaration(
        '.compare-pane .marker-buttons .viewer-button:hover:not(:disabled):not([aria-pressed="true"])',
        'background',
      ),
    ).toBe('var(--preview-control-hover-surface)')

    expect(
      declaration('.compare-pane .marker-buttons .viewer-button:focus-visible', 'outline'),
    ).toBe('var(--viewer-focus-outline)')

    expect(
      winningDeclaration(
        rules,
        new Set(['.compare-pane .marker-buttons .viewer-button[aria-pressed="true"]']),
        'background',
      ),
    ).toBe('var(--preview-accent-surface)')
    expect(declaration(".viewer-button[data-active='true']", 'background')).toBe(
      'var(--viewer-accent-soft)',
    )
    expect(
      declaration('.compare-pane .marker-buttons .viewer-button[aria-pressed="true"]', 'color'),
    ).toBe('var(--preview-accent-text)')

    expect(declaration(".viewer-local-feedback[data-tone='danger']", 'background')).toBe(
      'var(--viewer-danger-soft)',
    )
    expect(declaration('.compare-pane-stage > .viewer-local-feedback', 'position')).toBe('absolute')

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

    expect(contrastRatio('#4152b4', '#eceeff')).toBeGreaterThanOrEqual(4.5)
    expect(contrastRatio('#5869cf', '#ffffff')).toBeGreaterThanOrEqual(3)
  })

  it('keeps image metadata visible at the required 1024 px acceptance viewport', () => {
    expect(appCss).toMatch(
      /@media \(max-width: 760px\)[\s\S]*?\.image-preview \.viewer-toolbar__leading > span,[\s\S]*?display: none;/,
    )
    expect(appCss).not.toMatch(
      /@media \(max-width: 1100px\)[\s\S]*?\.image-preview \.viewer-toolbar__leading > span,[\s\S]*?display: none;/,
    )
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
