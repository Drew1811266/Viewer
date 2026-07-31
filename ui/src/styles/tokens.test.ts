import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import appStyles from './app.css?inline'
import tokenStyles from './tokens.css?inline'

afterEach(() => {
  document.body.replaceChildren()
  document.head.querySelectorAll('[data-viewer-token-test]').forEach((style) => {
    style.remove()
  })
})

beforeEach(() => {
  const style = document.createElement('style')
  style.dataset.viewerTokenTest = 'true'
  style.textContent = `${tokenStyles}\n${appStyles}`
  document.head.append(style)
})

describe('Viewer visual tokens', () => {
  it('applies the approved palette and geometry through the rendered cascade', () => {
    const root = getComputedStyle(document.documentElement)
    expect(root.getPropertyValue('--viewer-canvas').trim()).toBe('#f3f3f1')
    expect(root.getPropertyValue('--viewer-application').trim()).toBe('#fafaf8')
    expect(root.getPropertyValue('--viewer-surface').trim()).toBe('#ffffff')
    expect(root.getPropertyValue('--viewer-sidebar').trim()).toBe('#eff0ef')
    expect(root.getPropertyValue('--viewer-soft-surface').trim()).toBe('#f5f5f3')
    expect(root.getPropertyValue('--viewer-border').trim()).toBe('#dedfdd')
    expect(root.getPropertyValue('--viewer-text').trim()).toBe('#23262c')
    expect(root.getPropertyValue('--viewer-text-secondary').trim()).toBe('#6b7077')
    expect(root.getPropertyValue('--viewer-accent').trim()).toBe('#5869cf')
    expect(root.getPropertyValue('--viewer-accent-text').trim()).toBe('#4152b4')
    expect(root.getPropertyValue('--viewer-focus-outline').trim()).toBe(
      '2px solid var(--viewer-accent)',
    )
    expect(root.getPropertyValue('--viewer-focus-shadow').trim()).toBe(
      '0 0 0 2px var(--viewer-accent)',
    )
    expect(root.getPropertyValue('--viewer-focus-offset').trim()).toBe('2px')
    expect(root.getPropertyValue('--viewer-focus-ring').trim()).toBe('')
    expect(root.getPropertyValue('--viewer-radius-control').trim()).toBe('8px')
    expect(root.getPropertyValue('--viewer-radius-popover').trim()).toBe('12px')
    expect(root.getPropertyValue('--viewer-radius-dialog').trim()).toBe('14px')
    expect(root.getPropertyValue('--viewer-motion-fast').trim()).toBe('120ms')
    expect(root.getPropertyValue('--viewer-motion-standard').trim()).toBe('160ms')
    expect(root.getPropertyValue('--viewer-motion-slow').trim()).toBe('200ms')
    expect(root.colorScheme).toBe('light')
  })
})
