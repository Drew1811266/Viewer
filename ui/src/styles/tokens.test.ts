import { readdirSync, readFileSync } from 'node:fs'
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
  it('defines every viewer token consumed by the stylesheets', () => {
    const sources = readdirSync('src/styles')
      .filter((file) => file.endsWith('.css'))
      .map((file) => readFileSync(`src/styles/${file}`, 'utf8'))
      .join('\n')
    const defined = new Set(
      [...sources.matchAll(/(--viewer-[a-z0-9-]+)\s*:/g)]
        .map((match) => match[1])
        .filter((token): token is string => token !== undefined),
    )
    // Set from React inline styles rather than a stylesheet.
    const dynamic = new Set([
      '--viewer-progress',
      '--viewer-dialog-footer',
      '--viewer-sidebar-width',
    ])
    const used = new Set(
      [...sources.matchAll(/var\((--viewer-[a-z0-9-]+)/g)]
        .map((match) => match[1])
        .filter((token): token is string => token !== undefined),
    )
    expect([...used].filter((token) => !defined.has(token) && !dynamic.has(token))).toEqual([])
  })

  it('applies the approved palette and geometry through the rendered cascade', () => {
    const root = getComputedStyle(document.documentElement)
    expect(root.getPropertyValue('--viewer-canvas').trim()).toBe('#fbfbfb')
    expect(root.getPropertyValue('--viewer-application').trim()).toBe('#fbfbfb')
    expect(root.getPropertyValue('--viewer-surface').trim()).toBe('#ffffff')
    expect(root.getPropertyValue('--viewer-sidebar').trim()).toBe('#ffffff')
    expect(root.getPropertyValue('--viewer-soft-surface').trim()).toBe('#fbfbfb')
    expect(root.getPropertyValue('--viewer-border').trim()).toBe('#cdd7e7')
    expect(root.getPropertyValue('--viewer-text').trim()).toBe('#171a21')
    expect(root.getPropertyValue('--viewer-text-secondary').trim()).toBe('#3f4757')
    expect(root.getPropertyValue('--viewer-accent').trim()).toBe('#c5baff')
    expect(root.getPropertyValue('--viewer-accent-text').trim()).toBe('#241a4d')
    const stateColorRoles = ['--viewer-warning', '--viewer-danger', '--viewer-review'].map(
      (property) => root.getPropertyValue(property).trim(),
    )
    expect(stateColorRoles.every(Boolean)).toBe(true)
    expect(new Set(stateColorRoles).size).toBe(3)
    expect(root.getPropertyValue('--viewer-focus-outline').trim()).toBe(
      '2px solid var(--viewer-accent)',
    )
    expect(root.getPropertyValue('--viewer-focus-shadow').trim()).toBe(
      '0 0 0 2px var(--viewer-accent)',
    )
    expect(root.getPropertyValue('--viewer-focus-offset').trim()).toBe('2px')
    expect(root.getPropertyValue('--viewer-focus-ring').trim()).toBe('')
    expect(root.getPropertyValue('--viewer-radius-control').trim()).toBe('8px')
    expect(root.getPropertyValue('--viewer-radius-popover').trim()).toBe('16px')
    expect(root.getPropertyValue('--viewer-radius-dialog').trim()).toBe('28px')
    expect(root.getPropertyValue('--viewer-motion-fast').trim()).toBe('120ms')
    expect(root.getPropertyValue('--viewer-motion-standard').trim()).toBe('160ms')
    expect(root.getPropertyValue('--viewer-motion-slow').trim()).toBe('200ms')
    expect(root.colorScheme).toBe('light')
  })
})
