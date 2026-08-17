import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const css = readFileSync(resolve(import.meta.dirname, 'videoPreview.css'), 'utf8')

describe('immersive video preview layout', () => {
  it('reserves exactly 50px above and 88px below the native video viewport', () => {
    expect(rule('.video-preview')).toMatchObject({
      '--video-preview-top-command-bar-height': '50px',
      '--video-preview-bottom-inspector-height': '88px',
    })
  })

  it('clips the complete player to one Viewer-light theater stage', () => {
    expect(rule('.video-preview')).toMatchObject({
      background: 'var(--viewer-application)',
      isolation: 'isolate',
      overflow: 'hidden',
    })
    expect(rule('.video-preview-stage')).toMatchObject({
      background: 'var(--viewer-soft-surface)',
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
      'z-index': '6',
    })
    expect(rule('.video-preview-bottom-chrome')).toMatchObject({
      bottom: '0',
      display: 'block',
      height: 'var(--video-preview-bottom-inspector-height)',
      position: 'absolute',
      'z-index': '5',
    })
    expect(rule('.video-preview .preview-navigation-float')).toMatchObject({
      bottom: 'auto',
      position: 'static',
      transform: 'none',
    })
    expect(rule('.video-controls')).not.toHaveProperty('bottom')
  })

  it('uses light native top and bottom chrome with a first-frame reveal veil', () => {
    expect(rule('.video-preview-stage::before')).toEqual({})
    expect(rule('.video-preview-stage::after')).toMatchObject({
      background: 'var(--video-preview-ambient-matte)',
      opacity: '1',
      'pointer-events': 'none',
    })
    expect(rule(".video-preview-stage[data-surface-visible='true']::after")).toMatchObject({
      opacity: '0',
    })
    expect(rule('.video-preview-topbar')).toMatchObject({
      background: 'var(--viewer-surface)',
      'border-bottom': '1px solid var(--viewer-border)',
      color: 'var(--viewer-text)',
      height: 'var(--video-preview-top-command-bar-height)',
    })
    expect(rule('.video-controls')).toMatchObject({
      background: 'var(--viewer-surface)',
      'border-top': '1px solid var(--viewer-border)',
      color: 'var(--viewer-text)',
      height: '100%',
      width: '100%',
    })
  })

  it('uses compact flat chrome with neutral actions and one accent playback action', () => {
    expect(rule('.video-preview-title')).toMatchObject({
      display: 'flex',
      gap: '12px',
    })
    expect(rule('.video-preview .preview-navigation-float')).toMatchObject({
      background: 'transparent',
      border: '0',
    })
    expect(rule('.video-controls')).toMatchObject({
      background: 'var(--viewer-surface)',
      border: '0',
      'border-radius': '0',
      gap: '4px',
      padding: '6px clamp(12px, 2vw, 24px)',
    })
    expect(rule('.video-controls__row')).toMatchObject({
      gap: '12px',
      'padding-top': '0',
    })
    expect(rule('.video-controls .viewer-button')).toMatchObject({
      background: 'var(--viewer-surface)',
      border: '1px solid var(--viewer-control-border)',
      'min-height': '44px',
      'min-width': '44px',
      width: '44px',
    })
    expect(rule('.video-controls .video-controls__play')).toMatchObject({
      background: 'var(--viewer-accent)',
      color: 'var(--viewer-on-accent)',
      'min-height': '44px',
      'min-width': '44px',
      width: '44px',
    })
  })

  it('covers source-authored black tails with the verified ended poster', () => {
    expect(rule('.video-preview-ended-poster')).toMatchObject({
      background: 'var(--video-preview-stage)',
      bottom: 'var(--video-preview-bottom-inspector-height)',
      position: 'absolute',
      top: 'var(--video-preview-top-command-bar-height)',
      'z-index': '4',
    })
    expect(rule('.video-preview-ended-poster > img')).toMatchObject({
      'object-fit': 'contain',
    })
  })

  it('renders external icon assets without dark-theme inversion', () => {
    expect(rule('.video-preview .viewer-icon')).toMatchObject({
      filter: 'none',
    })
  })

  it('hides the fullscreen cursor only with idle chrome', () => {
    expect(
      rule(".video-preview[data-fullscreen='true'][data-chrome-visible='false']"),
    ).toMatchObject({ cursor: 'none' })
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
