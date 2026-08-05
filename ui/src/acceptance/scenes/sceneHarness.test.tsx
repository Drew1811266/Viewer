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
})
