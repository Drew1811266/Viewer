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

  it('strictly caps the expanded mixed shelf and lets text-only fill the body', () => {
    const rules = parseRules(adaptiveTextPanelCss)

    expect(declarationsFor(rules, '.text-file-panel')).toMatchObject({
      'box-sizing': 'border-box',
      overflow: 'hidden',
    })
    expect(
      declarationsFor(
        rules,
        '.text-file-panel--mixed-collapsed,\n.text-file-panel--mixed-expanded',
      ),
    ).toEqual({
      'border-top': '1px solid #edf0f3',
      'padding-top': '8px',
    })
    expect(declarationsFor(rules, '.text-file-panel--mixed-expanded')).toEqual({
      flex: '0 1 auto',
      'max-height': '20%',
    })
    expect(declarationsFor(rules, '.text-file-panel--text_only')).toEqual({
      flex: '1 1 auto',
      'max-height': 'none',
    })
  })

  it('keeps overflow inside the mounted text list', () => {
    const rules = parseRules(adaptiveTextPanelCss)

    expect(declarationsFor(rules, '.text-file-panel > .text-file-list')).toEqual({
      flex: '1 1 auto',
      'min-height': '0',
      overflow: 'auto',
      'overscroll-behavior': 'contain',
    })
  })

  it('renders the disclosure as a light Viewer button with focus inside the clipped shelf', () => {
    const rules = parseRules(adaptiveTextPanelCss)

    expect(declarationsFor(rules, '.text-file-disclosure')).toMatchObject({
      background: '#fff',
      border: '1px solid #c8ced6',
      'border-radius': '6px',
      'min-height': '34px',
      width: '100%',
    })
    expect(declarationsFor(rules, '.text-file-disclosure:focus-visible')).toMatchObject({
      outline: '2px solid #2477d4',
      'outline-offset': '-2px',
    })
  })

  it('anchors a compact light select-all menu beside its source control', () => {
    const rules = parseRules(adaptiveTextPanelCss)

    expect(declarationsFor(rules, '.select-all-control')).toEqual({
      position: 'relative',
    })
    expect(declarationsFor(rules, '.select-all-choice-panel')).toMatchObject({
      background: '#fff',
      border: '1px solid #c8ced6',
      'border-radius': '8px',
      'box-shadow': '0 10px 28px rgb(34 42 53 / 18%)',
      'min-width': '148px',
      position: 'absolute',
      right: 'calc(100% + 8px)',
      top: '0',
      'z-index': '24',
    })
    expect(declarationsFor(rules, '.select-all-choice-panel > button')).toMatchObject({
      'text-align': 'left',
      'white-space': 'nowrap',
    })
    expect(declarationsFor(rules, '.select-all-choice-panel > button:focus-visible')).toMatchObject(
      {
        outline: '2px solid #2477d4',
      },
    )
    expect(declarationsFor(rules, '.select-all-choice-panel > button:hover')).toMatchObject({
      background: '#eef4fc',
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
