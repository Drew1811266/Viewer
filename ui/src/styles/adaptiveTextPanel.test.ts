import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

const adaptiveTextPanelCss = readFileSync('src/styles/adaptiveTextPanel.css', 'utf8')

describe('adaptive text panel layout contracts', () => {
  it('bounds the active content workspace and its compare surface', () => {
    const rules = parseRules(adaptiveTextPanelCss)

    expect(declarationsFor(rules, '.workspace.workspace--content')).toEqual({
      display: 'flex',
      'flex-direction': 'column',
      'min-height': '0',
      overflow: 'hidden',
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

  it('gives the content browser a bounded flex body and image slot', () => {
    const rules = parseRules(adaptiveTextPanelCss)

    expect(declarationsFor(rules, '.content-workspace-surface')).toEqual({
      display: 'flex',
      'flex-direction': 'column',
      'min-height': '0',
    })
    expect(declarationsFor(rules, '.content-workspace-surface[hidden]')).toEqual({
      display: 'none',
    })
    expect(declarationsFor(rules, '.content-browser')).toEqual({
      display: 'flex',
      flex: '1 1 auto',
      'flex-direction': 'column',
      height: '100%',
      'min-height': '0',
    })
    expect(declarationsFor(rules, '.content-browser > .content-toolbar')).toEqual({
      flex: '0 0 auto',
    })
    expect(declarationsFor(rules, '.content-browser-body')).toEqual({
      display: 'flex',
      flex: '1 1 auto',
      'flex-direction': 'column',
      'min-height': '0',
    })
    expect(declarationsFor(rules, '.content-browser-image-slot')).toEqual({
      flex: '1 1 auto',
      'min-height': '0',
      overflow: 'hidden',
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
