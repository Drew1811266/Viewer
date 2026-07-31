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
})
