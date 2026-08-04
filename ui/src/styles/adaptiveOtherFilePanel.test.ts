import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

const adaptiveOtherFilePanelCss = readFileSync('src/styles/adaptiveOtherFilePanel.css', 'utf8')

describe('adaptive other-file panel layout contracts', () => {
  it('bounds the active content workspace and its compare surface', () => {
    const rules = parseRules(adaptiveOtherFilePanelCss)

    expect(declarationsFor(rules, '.workspace.workspace--content')).toEqual({
      display: 'flex',
      'flex-direction': 'column',
      'min-height': '0',
      overflow: 'hidden',
      padding: '0',
    })
    expect(
      declarationsFor(
        rules,
        '.workspace--content > .content-workspace-surface,\n.workspace--content > .compare-workspace',
      ),
    ).toEqual({
      flex: '1 1 auto',
      'min-height': '0',
    })
    expect(declarationsFor(rules, '.workspace--content > .compare-workspace')).toEqual({
      height: 'auto',
    })
  })

  it('strictly caps the expanded mixed shelf and lets other-only fill the body', () => {
    const rules = parseRules(adaptiveOtherFilePanelCss)

    expect(declarationsFor(rules, '.other-file-panel')).toMatchObject({
      'box-sizing': 'border-box',
      overflow: 'hidden',
    })
    expect(declarationsFor(rules, '.other-file-panel--mixed-expanded')).toEqual({
      flex: '0 1 auto',
      'max-height': '20%',
    })
    expect(declarationsFor(rules, '.other-file-panel--other_only')).toEqual({
      flex: '1 1 auto',
      'max-height': 'none',
    })
  })

  it('keeps overflow inside the mounted virtualized other-file list', () => {
    const rules = parseRules(adaptiveOtherFilePanelCss)

    expect(declarationsFor(rules, '.other-file-listbox')).toMatchObject({
      flex: '1 1 auto',
      'min-height': '0',
      overflow: 'hidden',
    })
    expect(declarationsFor(rules, '.other-file-virtual-list')).toMatchObject({
      'min-height': '0',
      'overscroll-behavior': 'contain',
    })
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
