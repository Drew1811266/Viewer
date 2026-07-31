import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { JSDOM } from 'jsdom'
import { describe, expect, it } from 'vitest'

const htmlPath = resolve(
  import.meta.dirname,
  '../../docs/prototypes/viewer-complete-ui-visual-atlas.html',
)
const html = readFileSync(htmlPath, 'utf8')

function renderAtlas() {
  return new JSDOM(html, {
    runScripts: 'dangerously',
    url: `file://${htmlPath}`,
    beforeParse(window) {
      window.ResizeObserver = class {
        observe() {}
        disconnect() {}
        unobserve() {}
      }
    },
  })
}

function clickScreen(document: Document, screen: string) {
  const button = document.querySelector<HTMLButtonElement>(`[data-screen="${screen}"]`)
  expect(button).not.toBeNull()
  button?.click()
}

function clickState(document: Document, sceneState: string) {
  const button = document.querySelector<HTMLButtonElement>(`[data-state="${sceneState}"]`)
  expect(button).not.toBeNull()
  button?.click()
}

describe('complete Viewer visual atlas', () => {
  it('publishes every approved state group and no external asset paths', () => {
    const dom = renderAtlas()
    const document = dom.window.document

    expect(document.querySelectorAll('[data-coverage]')).toHaveLength(17)
    expect(document.querySelectorAll('[data-screen]').length).toBeGreaterThanOrEqual(17)
    expect(document.querySelector('[data-coverage="radial"]')).not.toBeNull()
    expect(document.querySelector('[data-coverage="unsupported-files"]')).not.toBeNull()
    expect(document.querySelector('[data-coverage="operation-results"]')).not.toBeNull()
    expect(html).not.toContain('const assetBase = "/files/"')
    expect(document.querySelectorAll('img[src^="/"], img[src^="http"]')).toHaveLength(0)
  })

  it('exposes real shell and browsing states', () => {
    const dom = renderAtlas()
    const document = dom.window.document

    clickScreen(document, 'sidebar')
    clickState(document, 'sidebar-collapsed')
    expect(
      document.querySelector('[data-viewer-sidebar]')?.getAttribute('data-mode'),
    ).toBe('collapsed')

    clickScreen(document, 'folder')
    clickState(document, 'structure-aggregate')
    expect(document.querySelector('[data-structure-state]')?.getAttribute('data-structure-state')).toBe(
      'aggregate',
    )

    clickScreen(document, 'browser')
    clickState(document, 'density-compact')
    expect(document.querySelector('[data-thumbnail-grid]')?.getAttribute('data-density')).toBe(
      'compact',
    )
    clickState(document, 'selection-multiple')
    expect(document.querySelectorAll('.image-card.selected').length).toBeGreaterThan(1)
    clickState(document, 'selection-focus')
    expect(document.querySelector('[data-keyboard-focus="true"]')).not.toBeNull()

    clickScreen(document, 'otherFiles')
    clickState(document, 'other-expanded')
    expect(document.querySelector('[data-other-files]')?.getAttribute('data-mode')).toBe(
      'expanded',
    )
    clickState(document, 'organization-drag')
    expect(document.querySelector('[data-drag-preview]')).not.toBeNull()
    expect(document.querySelector('[data-drop-target="valid"]')).not.toBeNull()

    clickScreen(document, 'search')
    for (const searchState of [
      'search-grouped',
      'search-flat',
      'search-indexing',
      'search-paging',
      'search-empty',
    ]) {
      clickState(document, searchState)
      expect(document.querySelector('[data-search-state]')?.getAttribute('data-search-state')).toBe(
        searchState.replace('search-', ''),
      )
    }

    clickScreen(document, 'filters')
    for (const filterState of [
      'filters-zero',
      'filters-one',
      'filters-multiple',
      'filters-advanced',
    ]) {
      clickState(document, filterState)
      expect(document.querySelector('[data-filter-state]')?.getAttribute('data-filter-state')).toBe(
        filterState.replace('filters-', ''),
      )
    }

    clickScreen(document, 'menus')
    clickState(document, 'menu-more')
    expect(document.querySelector('[role="menu"]')?.getAttribute('data-menu-kind')).toBe('more')
    clickState(document, 'menu-readonly')
    expect(document.querySelector('[role="menu"] [aria-disabled="true"]')).not.toBeNull()
  })
})
