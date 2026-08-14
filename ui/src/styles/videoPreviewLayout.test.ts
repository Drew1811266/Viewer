import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const css = readFileSync(resolve(import.meta.dirname, 'videoPreview.css'), 'utf8')

describe('immersive video preview layout', () => {
  it('clips the complete player to one dark theater stage', () => {
    expect(rule('.video-preview')).toMatchObject({
      background: 'var(--video-preview-stage)',
      isolation: 'isolate',
      overflow: 'hidden',
    })
    expect(rule('.video-preview-stage')).toMatchObject({
      background: 'var(--video-preview-stage)',
      inset: '0',
      overflow: 'hidden',
      position: 'absolute',
    })
    expect(rule(".video-preview-stage[data-surface-visible='true']")).toMatchObject({
      background: 'transparent',
    })
  })

  it('keeps title, navigation, and controls inside the stage safe area', () => {
    expect(rule('.video-preview-topbar')).toMatchObject({
      position: 'absolute',
      top: '0',
      'z-index': '5',
    })
    expect(rule('.video-preview .preview-navigation-float')).toMatchObject({
      bottom: '154px',
      'z-index': '6',
    })
    expect(rule('.video-controls')).toMatchObject({
      bottom: '24px',
      'z-index': '5',
    })
  })
})

function rule(selector: string): Record<string, string> {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const match = css.match(new RegExp(`(?:^|\\n)${escaped}\\s*\\{([^}]*)\\}`, 'm'))
  if (match?.[1] === undefined) return {}
  return Object.fromEntries(
    match[1]
      .split(';')
      .map((declaration) => declaration.trim())
      .filter(Boolean)
      .map((declaration) => {
        const separator = declaration.indexOf(':')
        return [declaration.slice(0, separator).trim(), declaration.slice(separator + 1).trim()]
      }),
  )
}
