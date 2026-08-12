import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

const videoBrowserCss = readFileSync('src/styles/videoBrowser.css', 'utf8')

describe('video browser style contracts', () => {
  it('reserves a 16:9 cover stage before the absolutely positioned cover loads', () => {
    const rules = parseRules(videoBrowserCss)

    expect(declarationsFor(rules, '.video-card-cover-stage')).toMatchObject({
      'aspect-ratio': '16 / 9',
      overflow: 'hidden',
      position: 'relative',
      width: '100%',
    })
    expect(declarationsFor(rules, '.video-card-cover')).toMatchObject({
      inset: '0',
      opacity: '0',
      position: 'absolute',
    })
    expect(declarationsFor(rules, '.video-card-cover--loaded')).toEqual({ opacity: '1' })
  })

  it('keeps expanded videos bounded and selection visible without motion', () => {
    const rules = parseRules(videoBrowserCss)

    expect(declarationsFor(rules, '.video-card-list')).toMatchObject({
      'max-height': 'min(34vh, 320px)',
      overflow: 'auto',
    })
    expect(declarationsFor(rules, '.video-card[aria-selected="true"]')).toMatchObject({
      background: 'var(--viewer-accent-soft)',
      'border-color': 'var(--viewer-accent)',
    })
    expect(videoBrowserCss).toContain('@media (prefers-reduced-motion: reduce)')
  })
})

interface CssRule {
  selector: string
  declarations: Record<string, string>
}

function parseRules(css: string): CssRule[] {
  return [...css.matchAll(/([^{}]+)\{([^{}]*)\}/g)].map((match) => ({
    selector: match[1]?.trim() ?? '',
    declarations: Object.fromEntries(
      (match[2] ?? '')
        .split(';')
        .map((declaration) => declaration.trim())
        .filter(Boolean)
        .map((declaration) => {
          const separator = declaration.indexOf(':')
          return [declaration.slice(0, separator).trim(), declaration.slice(separator + 1).trim()]
        }),
    ),
  }))
}

function declarationsFor(rules: CssRule[], selector: string): Record<string, string> | undefined {
  return rules.find((rule) => rule.selector === selector)?.declarations
}
