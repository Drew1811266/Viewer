import { readFileSync } from 'node:fs'
import { afterEach, expect, it } from 'vitest'
import { defined } from '../defined'

const appCss = readFileSync('src/styles/app.css', 'utf8')
const style = document.createElement('style')

afterEach(() => {
  style.remove()
  document.body.replaceChildren()
})

it('exposes the native image through the document without unlocking root scrolling', () => {
  // jsdom does not resolve custom properties; substitute the application
  // background token so the actual cascade and :has selectors are exercised.
  style.textContent = appCss
    .replace(/^@import[^;]+;/gm, '')
    .replaceAll('var(--viewer-application)', '#fafafa')
  document.head.append(style)
  document.body.innerHTML =
    '<div id="root"><div class="viewer-shell"><section class="image-preview"><div class="native-image-viewport"></div></section></div></div>'
  for (const element of [
    document.documentElement,
    document.body,
    defined(document.getElementById('root')),
  ]) {
    const computed = getComputedStyle(element)
    expect(computed.backgroundColor).toBe('rgba(0, 0, 0, 0)')
    expect(computed.overflow).toBe('hidden')
  }
  document.querySelector('.image-preview')?.remove()
  expect(getComputedStyle(document.documentElement).backgroundColor).not.toBe('rgba(0, 0, 0, 0)')
  expect(getComputedStyle(document.documentElement).overflow).toBe('hidden')
})
