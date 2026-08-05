import { describe, expect, it } from 'vitest'
import { workspaceVisualsSettled } from './sceneHarness'

describe('acceptance product scene readiness', () => {
  it('waits for real thumbnail content instead of capturing workspace skeletons', () => {
    const root = document.createElement('div')
    root.innerHTML = `
      <main class="viewer-shell">
        <span class="aspect-thumbnail" data-thumbnail-state="loading">
          <span class="aspect-thumbnail-placeholder" aria-label="缩略图加载中"></span>
        </span>
      </main>
    `
    expect(workspaceVisualsSettled(root)).toBe(false)

    root.innerHTML = `
      <main class="viewer-shell">
        <span class="aspect-thumbnail" data-thumbnail-state="ready"><img alt="" /></span>
      </main>
    `
    expect(workspaceVisualsSettled(root)).toBe(true)
  })

  it('accepts completed empty overviews and terminal thumbnail failures', () => {
    const root = document.createElement('div')
    root.innerHTML = `
      <main class="viewer-shell">
        <section class="folder-overview"><div class="folder-filmstrip-list"></div></section>
      </main>
    `
    expect(workspaceVisualsSettled(root)).toBe(true)

    root.innerHTML = `
      <main class="viewer-shell">
        <section class="content-browser">
          <span class="aspect-thumbnail" data-thumbnail-state="failed">
            <span class="aspect-thumbnail-placeholder" aria-label="缩略图不可用"></span>
          </span>
        </section>
      </main>
    `
    expect(workspaceVisualsSettled(root)).toBe(true)
  })

  it('does not accept an overview while its rows are still skeletons', () => {
    const root = document.createElement('div')
    root.innerHTML = `
      <main class="viewer-shell">
        <section class="folder-overview">
          <div class="folder-filmstrip-skeleton"></div>
        </section>
      </main>
    `
    expect(workspaceVisualsSettled(root)).toBe(false)
  })
})
