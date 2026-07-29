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
    const rules = parseRules(adaptiveOtherFilePanelCss)

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

  it('strictly caps the expanded mixed shelf and lets other-only fill the body', () => {
    const rules = parseRules(adaptiveOtherFilePanelCss)

    expect(declarationsFor(rules, '.other-file-panel')).toMatchObject({
      'box-sizing': 'border-box',
      overflow: 'hidden',
    })
    expect(
      declarationsFor(
        rules,
        '.other-file-panel--mixed-collapsed,\n.other-file-panel--mixed-expanded',
      ),
    ).toEqual({
      'border-top': '1px solid #edf0f3',
      'padding-top': '8px',
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

  it('renders the disclosure as a light Viewer button with focus inside the clipped shelf', () => {
    const rules = parseRules(adaptiveOtherFilePanelCss)

    expect(declarationsFor(rules, '.other-file-disclosure')).toMatchObject({
      background: '#fff',
      border: '1px solid #c8ced6',
      'border-radius': '6px',
      'min-height': '34px',
      width: '100%',
    })
    expect(declarationsFor(rules, '.other-file-disclosure:focus-visible')).toMatchObject({
      outline: '2px solid #2477d4',
      'outline-offset': '-2px',
    })
  })

  it('keeps the compact menu inside the minimum workspace beside a maximum sidebar', () => {
    const rules = parseRules(adaptiveOtherFilePanelCss)

    expect(declarationsFor(rules, '.select-all-control')).toEqual({
      position: 'relative',
    })
    const panel = declarationsFor(rules, '.select-all-choice-panel')
    expect(panel).toMatchObject({
      background: '#fff',
      border: '1px solid #c8ced6',
      'border-radius': '8px',
      'box-shadow': '0 10px 28px rgb(34 42 53 / 18%)',
      'min-width': '148px',
      position: 'absolute',
      right: '0',
      top: 'calc(100% + 8px)',
      'z-index': '24',
    })
    expect(panel).not.toHaveProperty('left')

    const minimumWorkspaceContentWidth = 720 - 420 - 2 * 16
    const viewPopoverBorderBoxWidth = 190
    const viewPopoverContentWidth = viewPopoverBorderBoxWidth - 2 * 10
    const panelWorstCaseOuterWidth =
      pixels(panel?.['min-width']) + 2 * pixels(panel?.padding) + 2 * pixels(panel?.border)
    expect(minimumWorkspaceContentWidth).toBe(268)
    expect(viewPopoverBorderBoxWidth).toBeLessThanOrEqual(minimumWorkspaceContentWidth)
    expect(panelWorstCaseOuterWidth).toBeLessThanOrEqual(viewPopoverContentWidth)

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

function pixels(value: string | undefined): number {
  return Number.parseInt(value ?? '', 10)
}
