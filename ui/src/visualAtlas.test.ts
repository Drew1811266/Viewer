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

  it('exposes viewing and radial states', () => {
    const dom = renderAtlas()
    const document = dom.window.document

    clickScreen(document, 'radial')
    for (const radialState of [
      'radial-click',
      'radial-gesture',
      'radial-mark',
      'radial-organize',
      'radial-disabled',
      'radial-readonly',
      'radial-keyboard',
    ]) {
      clickState(document, radialState)
      expect(document.querySelector('[data-radial-state]')?.getAttribute('data-radial-state')).toBe(
        radialState.replace('radial-', ''),
      )
      expect(document.querySelector('[role="menu"] [role="menuitem"]')).not.toBeNull()
    }

    clickScreen(document, 'preview')
    for (const previewState of [
      'preview-fit',
      'preview-100',
      'preview-zoom',
      'preview-rotate',
      'preview-loading',
      'preview-error',
      'preview-navigation',
    ]) {
      clickState(document, previewState)
      expect(
        document.querySelector('[data-preview-state]')?.getAttribute('data-preview-state'),
      ).toBe(previewState.replace('preview-', ''))
    }

    clickScreen(document, 'compare')
    for (const compareState of ['compare-2', 'compare-3', 'compare-4', 'compare-many']) {
      clickState(document, compareState)
      expect(document.querySelector('[data-compare-state]')?.getAttribute('data-compare-state')).toBe(
        compareState.replace('compare-', ''),
      )
    }

    clickScreen(document, 'text')
    for (const documentState of [
      'document-markdown',
      'document-plain',
      'document-encoding',
      'document-truncated',
      'document-dual',
      'document-unsupported',
      'document-unavailable',
    ]) {
      clickState(document, documentState)
      expect(
        document.querySelector('[data-document-state]')?.getAttribute('data-document-state'),
      ).toBe(documentState.replace('document-', ''))
    }

    clickScreen(document, 'information')
    clickState(document, 'info-multiple')
    expect(document.querySelector('[data-information-state]')?.getAttribute('data-information-state')).toBe(
      'multiple',
    )
  })

  it('exposes dialogs feedback and recovery states', () => {
    const dom = renderAtlas()
    const document = dom.window.document

    clickScreen(document, 'dialogs')
    for (const dialogState of [
      'dialog-settings',
      'dialog-single-rename',
      'dialog-batch-rename',
      'dialog-destination',
      'dialog-conflict',
      'dialog-trash',
      'dialog-close',
    ]) {
      clickState(document, dialogState)
      expect(document.querySelector('[data-dialog-state]')?.getAttribute('data-dialog-state')).toBe(
        dialogState.replace('dialog-', ''),
      )
    }

    clickScreen(document, 'feedback')
    for (const taskState of [
      'task-running',
      'task-success',
      'task-failure',
      'task-cancelled',
      'task-result',
    ]) {
      clickState(document, taskState)
      expect(document.querySelector('[data-task-state]')?.getAttribute('data-task-state')).toBe(
        taskState.replace('task-', ''),
      )
    }

    clickScreen(document, 'results')
    for (const feedbackState of [
      'results-operation',
      'results-notice',
      'results-local-error',
      'results-readonly',
      'results-recovery',
    ]) {
      clickState(document, feedbackState)
      expect(document.querySelector('[data-feedback-state]')?.getAttribute('data-feedback-state')).toBe(
        feedbackState.replace('results-', ''),
      )
    }

    clickScreen(document, 'launch')
    clickState(document, 'launch-no-project')
    expect(document.querySelectorAll('[data-no-project] > *')).toHaveLength(3)
    for (const launchState of [
      'launch-drag',
      'launch-invalid',
      'launch-opening',
      'launch-scanning',
      'launch-thumbnails',
      'launch-empty',
      'launch-error',
      'launch-recovery',
    ]) {
      clickState(document, launchState)
      expect(document.querySelector('[data-launch-state]')?.getAttribute('data-launch-state')).toBe(
        launchState.replace('launch-', ''),
      )
    }

    clickScreen(document, 'accessibility')
    for (const accessibilityState of [
      'accessibility-keyboard',
      'accessibility-restore',
      'accessibility-reduced',
      'accessibility-forced',
      'accessibility-zoom',
    ]) {
      clickState(document, accessibilityState)
      expect(
        document
          .querySelector('[data-accessibility-state]')
          ?.getAttribute('data-accessibility-state'),
      ).toBe(accessibilityState.replace('accessibility-', ''))
    }
  })
})
