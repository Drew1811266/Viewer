import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const css = readFileSync(resolve(import.meta.dirname, 'videoPreview.css'), 'utf8')

describe('immersive video preview layout', () => {
  it('overlays compact chrome without reserving native video viewport space', () => {
    expect(rule('.video-preview')).toMatchObject({
      '--video-preview-top-overlay-height': '44px',
      '--video-preview-bottom-controller-height': '62px',
    })
    expect(rule('.video-preview-topbar')).toMatchObject({
      height: 'var(--video-preview-top-overlay-height)',
      inset: '20px 22px auto',
      position: 'absolute',
    })
    expect(rule('.video-preview-bottom-chrome')).toMatchObject({
      bottom: '22px',
      height: 'var(--video-preview-bottom-controller-height)',
      left: '50%',
      position: 'absolute',
      transform: 'translateX(-50%)',
      width: 'min(920px, calc(100% - 44px))',
    })
    expect(rule('.video-controls .viewer-button')).toMatchObject({
      height: '40px',
      width: '40px',
    })
    expect(rule('.video-controls .viewer-button .viewer-icon')).toMatchObject({
      height: '22px',
      width: '22px',
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

  it('freezes preview scrolling at the fixed theater boundary', () => {
    expect(rule('.video-preview')).toMatchObject({
      inset: '0',
      overflow: 'hidden',
      'overscroll-behavior': 'none',
      position: 'fixed',
    })
    expect(rule(":root:has(.viewer-shell[data-video-preview-open='true'])")).toMatchObject({
      overflow: 'hidden',
      'overscroll-behavior': 'none',
    })
    expect(rule("body:has(.viewer-shell[data-video-preview-open='true'])")).toMatchObject({
      overflow: 'hidden',
      'overscroll-behavior': 'none',
    })
  })

  it('keeps title, navigation, and controls inside the stage safe area', () => {
    expect(rule('.video-preview-topbar')).toMatchObject({
      position: 'absolute',
      'z-index': '6',
    })
    expect(rule('.video-preview-bottom-chrome')).toMatchObject({
      display: 'block',
      height: 'var(--video-preview-bottom-controller-height)',
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

  it('uses three independent top surfaces and one compact bottom controller', () => {
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
      'backdrop-filter': 'none',
      background: 'transparent',
      border: '0',
      'box-shadow': 'none',
      color: 'var(--viewer-text)',
      height: 'var(--video-preview-top-overlay-height)',
    })
    expect(rule('.video-preview-topbar .viewer-toolbar__leading')).toMatchObject({
      background: 'var(--video-preview-chrome-surface)',
      border: '1px solid var(--viewer-border)',
      'border-radius': '12px',
      'min-height': '44px',
      padding: '0 14px',
    })
    expect(rule('.video-preview .preview-navigation-float')).toMatchObject({
      background: 'var(--video-preview-chrome-surface)',
      border: '1px solid var(--viewer-border)',
      'border-radius': '13px',
      padding: '3px',
    })
    expect(rule('.video-preview-topbar .viewer-toolbar__actions')).toMatchObject({
      background: 'var(--video-preview-chrome-surface)',
      border: '1px solid var(--viewer-border)',
      'border-radius': '12px',
      'min-height': '44px',
      padding: '3px',
    })
    expect(rule('.video-controls')).toMatchObject({
      background: 'var(--video-preview-chrome-surface)',
      border: '1px solid var(--viewer-border)',
      color: 'var(--viewer-text)',
      height: '100%',
      width: '100%',
    })
  })

  it('uses compact flat chrome with neutral actions and one accent playback action', () => {
    expect(rule('.video-preview-title')).toMatchObject({
      display: 'flex',
      gap: '8px',
    })
    expect(rule('.video-controls')).toMatchObject({
      background: 'var(--video-preview-chrome-surface)',
      border: '1px solid var(--viewer-border)',
      'border-radius': '17px',
      gap: '14px',
      'grid-template-columns': 'auto minmax(160px, 1fr) auto',
      'grid-template-rows': '1fr',
      padding: '8px 10px',
    })
    expect(rule('.video-controls__timeline')).toMatchObject({
      'min-width': '0',
    })
    expect(rule('.video-controls .viewer-button')).toMatchObject({
      background: 'transparent',
      border: '0',
      height: '40px',
      'min-height': '40px',
      'min-width': '40px',
      width: '40px',
    })
    expect(rule('.video-controls .viewer-button::before')).toMatchObject({
      background: 'transparent',
      border: '0',
      inset: '0',
      position: 'absolute',
    })
    expect(rule('.video-controls .video-controls__play')).toMatchObject({
      background: 'transparent',
      color: 'var(--viewer-on-accent)',
      height: '44px',
      'min-height': '44px',
      'min-width': '44px',
      width: '44px',
    })
    expect(rule('.video-controls .video-controls__play::before')).toMatchObject({
      background: 'var(--viewer-accent)',
      'border-color': 'var(--viewer-accent)',
      inset: '-2px',
    })
  })

  it('keeps a 44px timeline hit region around the compact visible track', () => {
    const timeline = rule('.video-timeline')
    const slider = rule('.video-timeline__slider')
    const track = rule('.video-timeline__track')

    expect(timeline).toMatchObject({ height: '44px' })
    expect(Number.parseFloat(slider.height ?? '0')).toBeGreaterThanOrEqual(44)
    expect(Number.parseFloat(track.height ?? '0')).toBeLessThanOrEqual(4)
    expect(track).toMatchObject({ top: '20px' })
    expect(rule('.video-preview-bottom-chrome')).toMatchObject({
      height: 'var(--video-preview-bottom-controller-height)',
    })
  })

  it('gives controls explicit hover, pressed, focus, active, and disabled states', () => {
    expect(rule('.video-controls .viewer-button:hover:not(:disabled)::before')).toMatchObject({
      background: 'var(--video-preview-control-hover-surface)',
      'border-color': 'var(--video-preview-control-hover-border)',
    })
    expect(rule('.video-controls .viewer-button:active:not(:disabled)::before')).toMatchObject({
      background: 'var(--viewer-accent-soft)',
      'border-color': 'var(--viewer-accent)',
    })
    expect(rule(".video-controls .viewer-button[data-active='true']::before")).toMatchObject({
      background: 'var(--viewer-accent-soft)',
      'border-color': 'var(--viewer-accent)',
    })
    expect(rule('.video-controls .viewer-button:focus-visible')).toMatchObject({
      outline: 'var(--viewer-focus-outline)',
      'outline-offset': 'var(--viewer-focus-offset)',
    })
    expect(rule('.video-controls .viewer-button:disabled')).toMatchObject({
      opacity: '0.42',
    })
  })

  it('keeps the rounded speed menu visible above both control layouts', () => {
    expect(rule('.video-rate-menu__popover')).toMatchObject({
      'border-radius': '13px',
      bottom: 'calc(100% + 8px)',
    })
    expect(rule('.video-rate-menu__option')).toMatchObject({
      'border-radius': '9px',
    })
    expect(rule('.video-controls-more__popover')).toMatchObject({
      overflow: 'visible',
    })
  })

  it('covers source-authored black tails with the verified ended poster', () => {
    expect(rule('.video-preview-ended-poster')).toMatchObject({
      background: 'var(--video-preview-stage)',
      inset: '0',
      position: 'absolute',
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
