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

function clickToolbarControl(document: Document, label: string) {
  const button = [...document.querySelectorAll<HTMLButtonElement>('.viewer-toolbar button')].find(
    (candidate) => candidate.textContent?.trim().startsWith(label),
  )
  expect(button).not.toBeNull()
  button?.click()
}

function keydown(target: Element, key: string) {
  const KeyboardEventConstructor = target.ownerDocument.defaultView?.KeyboardEvent
  expect(KeyboardEventConstructor).toBeDefined()
  target.dispatchEvent(
    new (KeyboardEventConstructor as typeof KeyboardEvent)('keydown', { bubbles: true, key }),
  )
}

function relativeLuminance([red, green, blue]: [number, number, number]) {
  const [r, g, b] = [red, green, blue].map((channel) => {
    const normalized = channel / 255
    return normalized <= 0.03928 ? normalized / 12.92 : ((normalized + 0.055) / 1.055) ** 2.4
  }) as [number, number, number]
  return 0.2126 * r + 0.7152 * g + 0.0722 * b
}

function rgb(color: string): [number, number, number] {
  if (/^#[\da-f]{6}$/i.test(color)) {
    return [
      Number.parseInt(color.slice(1, 3), 16),
      Number.parseInt(color.slice(3, 5), 16),
      Number.parseInt(color.slice(5, 7), 16),
    ]
  }
  const channels = color.match(/\d+/g)?.slice(0, 3).map(Number)
  expect(channels).toHaveLength(3)
  return channels as [number, number, number]
}

function contrastRatio(foreground: string, background: string) {
  const foregroundLuminance = relativeLuminance(rgb(foreground))
  const backgroundLuminance = relativeLuminance(rgb(background))
  const lighter = Math.max(foregroundLuminance, backgroundLuminance)
  const darker = Math.min(foregroundLuminance, backgroundLuminance)
  return (lighter + 0.05) / (darker + 0.05)
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

  it('models the approved proportional density geometry instead of a fixed column mock', () => {
    expect(html).toMatch(/\.image-grid\s*\{[^}]*display:\s*flex;[^}]*flex-wrap:\s*wrap;/s)
    expect(html).toMatch(/\.image-grid\s+\.image-card\s*\{[^}]*flex:\s*0 0 198px;/s)
    expect(html).toMatch(
      /\.image-grid\[data-density="compact"\]\s+\.image-card\s*\{[^}]*flex-basis:\s*144px;/s,
    )
    expect(html).toMatch(
      /\.image-grid\[data-density="large"\]\s+\.image-card\s*\{[^}]*flex-basis:\s*252px;/s,
    )
    expect(html).toMatch(/\.image-stage\s*\{[^}]*height:\s*132px;/s)
    expect(html).toMatch(
      /\.image-grid\[data-density="large"\]\s+\.image-stage\s*\{[^}]*height:\s*168px;/s,
    )
    expect(html).toMatch(/\.image-meta\s*\{[^}]*min-height:\s*48px;/s)
    expect(html).toMatch(/\.image-meta strong\s*\{[^}]*font-size:\s*13px;/s)
  })

  it('exposes real shell and browsing states', () => {
    const dom = renderAtlas()
    const document = dom.window.document

    clickScreen(document, 'sidebar')
    expect(document.querySelectorAll('.image-card.selected')).toHaveLength(0)
    clickState(document, 'sidebar-collapsed')
    expect(document.querySelector('[data-viewer-sidebar]')?.getAttribute('data-mode')).toBe(
      'collapsed',
    )

    clickScreen(document, 'folder')
    clickState(document, 'structure-aggregate')
    expect(
      document.querySelector('[data-structure-state]')?.getAttribute('data-structure-state'),
    ).toBe('aggregate')

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
    expect(document.querySelector('[data-other-files]')?.getAttribute('data-mode')).toBe('expanded')
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
      expect(
        document.querySelector('[data-compare-state]')?.getAttribute('data-compare-state'),
      ).toBe(compareState.replace('compare-', ''))
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
    expect(
      document.querySelector('[data-information-state]')?.getAttribute('data-information-state'),
    ).toBe('multiple')
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
      expect(
        document.querySelector('[data-feedback-state]')?.getAttribute('data-feedback-state'),
      ).toBe(feedbackState.replace('results-', ''))
    }
    clickState(document, 'results-operation')
    expect(document.querySelector('[data-operation-results-inspector]')).not.toBeNull()

    clickScreen(document, 'launch')
    clickState(document, 'launch-no-project')
    expect(document.querySelectorAll('[data-no-project] > *')).toHaveLength(3)
    const noProject = document.querySelector<HTMLElement>('[data-no-project]')
    const noProjectTitle = noProject?.querySelector('h1')
    const noProjectCopy = noProject?.querySelector('p')
    const noProjectAction = noProject?.querySelector('button')
    expect(window.getComputedStyle(noProjectTitle as Element).fontSize).toBe('20px')
    expect(window.getComputedStyle(noProjectCopy as Element).fontSize).toBe('13px')
    expect(window.getComputedStyle(noProject as Element).display).toBe('grid')
    expect(window.getComputedStyle(noProject as Element).gap).toBe('12px')
    expect(window.getComputedStyle(noProjectCopy as Element).margin).toBe('0px')
    expect(window.getComputedStyle(noProjectAction as Element).fontSize).toBe('13px')
    expect(window.getComputedStyle(noProjectAction as Element).minHeight).toBe('36px')
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

  it('opens filters in place and derives one consistent condition count', () => {
    const dom = renderAtlas()
    const document = dom.window.document

    clickScreen(document, 'filters')
    clickState(document, 'filters-advanced')
    clickScreen(document, 'browser')
    clickToolbarControl(document, '筛选')

    expect(document.querySelector('[data-filter-popover]')).not.toBeNull()
    expect(document.querySelector('[data-screen="browser"]')?.getAttribute('aria-current')).toBe(
      'page',
    )
    const activeConditions = document.querySelectorAll('[data-active-filter-condition="true"]')
    expect(document.querySelector('[data-filter-trigger-count]')?.textContent).toBe(
      String(activeConditions.length),
    )
    expect(
      document
        .querySelector('[data-filter-condition-count]')
        ?.getAttribute('data-filter-condition-count'),
    ).toBe(String(activeConditions.length))
    expect(activeConditions).toHaveLength(6)
  })

  it('keeps filter and more-menu examples within approved product authority', () => {
    const dom = renderAtlas()
    const document = dom.window.document

    clickScreen(document, 'filters')
    clickState(document, 'filters-zero')
    expect(document.querySelector('[aria-label="关闭筛选"]')).not.toBeNull()
    expect(
      [...document.querySelectorAll('button')].some((button) => button.textContent === '完成'),
    ).toBe(true)
    expect(document.body.textContent).not.toContain('应用筛选')

    clickScreen(document, 'menus')
    clickState(document, 'menu-more')
    const menu = document.querySelector('[role="menu"]')
    expect(menu?.textContent).toContain('软件设置')
    expect(menu?.textContent).toContain('关闭项目')
    expect(menu?.textContent).not.toContain('重新扫描项目')
    expect(menu?.textContent).not.toContain('在文件管理器中显示')
  })

  it('renders a compact accessible collapsed sidebar header and visually disabled menu rows', () => {
    const dom = renderAtlas()
    const document = dom.window.document

    clickScreen(document, 'sidebar')
    clickState(document, 'sidebar-collapsed')
    const header = document.querySelector('[data-project-head="collapsed"]')
    expect(header?.querySelector('[aria-label="展开项目目录"]')).not.toBeNull()
    expect(header?.querySelector('strong')).toBeNull()
    expect(header?.textContent?.trim()).toBe('展开')

    clickScreen(document, 'menus')
    clickState(document, 'menu-readonly')
    const disabledRows = document.querySelectorAll(
      '[role="menu"] .menu-row[aria-disabled="true"].is-disabled',
    )
    expect(disabledRows.length).toBeGreaterThan(0)
  })

  it('renders distinct real radial geometry for every visual state', () => {
    const dom = renderAtlas()
    const document = dom.window.document

    clickScreen(document, 'radial')
    for (const value of [
      'click',
      'gesture',
      'mark',
      'organize',
      'disabled',
      'readonly',
      'keyboard',
    ]) {
      clickState(document, `radial-${value}`)
      expect(document.querySelector(`[data-radial-visual-state="${value}"]`)).not.toBeNull()
      expect(document.querySelector('.radial-demo-menu [data-level="primary"]')).not.toBeNull()
      expect(document.querySelectorAll('.radial-demo-menu [data-radial-sector]').length).toBe(6)
    }

    clickState(document, 'radial-mark')
    expect(document.querySelector('[data-secondary-kind="mark"]')).not.toBeNull()
    clickState(document, 'radial-organize')
    expect(document.querySelector('[data-secondary-kind="organize"]')).not.toBeNull()
    clickState(document, 'radial-readonly')
    expect(document.querySelector('.radial-demo-center')?.textContent).toContain('只读')
    clickState(document, 'radial-keyboard')
    expect(document.querySelector('[data-keyboard-active="true"]')).not.toBeNull()
  })

  it('supports keyboard selection and exposes progress and dialog semantics', () => {
    const dom = renderAtlas()
    const document = dom.window.document

    clickScreen(document, 'browser')
    const card = document.querySelector<HTMLElement>('.image-card:not(.selected)')
    expect(card?.getAttribute('aria-selected')).toBe('false')
    keydown(card as Element, 'Enter')
    expect(card?.getAttribute('aria-selected')).toBe('true')

    clickScreen(document, 'launch')
    clickState(document, 'launch-scanning')
    expect(document.querySelector('[role="progressbar"][aria-valuenow]')).not.toBeNull()
    expect(document.querySelector('[role="status"][aria-live="polite"]')).not.toBeNull()

    clickScreen(document, 'dialogs')
    expect(document.querySelector('.viewer-window > .scrim')).not.toBeNull()
    expect(document.querySelector('[role="dialog"][aria-modal="true"]')).not.toBeNull()
  })

  it('associates field labels and exposes real accessibility shell states', () => {
    const dom = renderAtlas()
    const document = dom.window.document

    clickScreen(document, 'filters')
    clickState(document, 'filters-advanced')
    const labels = document.querySelectorAll<HTMLLabelElement>('.field > label')
    expect(labels.length).toBeGreaterThan(0)
    for (const label of labels) {
      expect(label.htmlFor).not.toBe('')
      expect(document.getElementById(label.htmlFor)).not.toBeNull()
    }

    clickScreen(document, 'accessibility')
    for (const value of ['keyboard', 'restore', 'reduced', 'forced', 'zoom']) {
      clickState(document, `accessibility-${value}`)
      expect(document.querySelector(`[data-accessible-viewer-state="${value}"]`)).not.toBeNull()
      expect(document.querySelector('[data-accessible-viewer-state] .viewer-shell')).not.toBeNull()
    }
  })

  it('meets approved target size, inspector width, and small-text contrast', () => {
    const dom = renderAtlas()
    const document = dom.window.document

    clickScreen(document, 'system')
    const button = document.querySelector<HTMLElement>('.viewer-button')
    const buttonStyle = dom.window.getComputedStyle(button as Element)
    const renderedHeight = Number.parseFloat(buttonStyle.height)
    const effectiveHeight = Number.isNaN(renderedHeight)
      ? Number.parseFloat(buttonStyle.minHeight)
      : renderedHeight
    expect(effectiveHeight).toBeGreaterThanOrEqual(32)

    clickScreen(document, 'information')
    const informationLayout = document.querySelector<HTMLElement>('.information-layout')
    expect(dom.window.getComputedStyle(informationLayout as Element).gridTemplateColumns).toContain(
      '334px',
    )

    clickScreen(document, 'sidebar')
    const rootStyle = dom.window.getComputedStyle(document.documentElement)
    const tertiaryColor = rootStyle.getPropertyValue('--tertiary').trim()
    const sidebarBackground = rootStyle.getPropertyValue('--sidebar').trim()
    expect(contrastRatio(tertiaryColor, sidebarBackground)).toBeGreaterThanOrEqual(4.5)
  })
})
