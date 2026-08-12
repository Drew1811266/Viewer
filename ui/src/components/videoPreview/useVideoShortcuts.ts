import { type RefObject, useEffect } from 'react'

export type VideoStepDirection = 'backward' | 'forward'

export interface UseVideoShortcutsOptions {
  active: boolean
  root: RefObject<HTMLElement | null>
  playing: boolean
  fullscreen: boolean
  onTogglePlayback(): void | Promise<void>
  onStep(direction: VideoStepDirection): void | Promise<void>
  onToggleMuted(): void | Promise<void>
  onToggleFullscreen(): void | Promise<void>
  onExitFullscreen(): void | Promise<void>
  onClose(): void
  onOwnedShortcut(): void
}

export function useVideoShortcuts({
  active,
  root,
  playing,
  fullscreen,
  onTogglePlayback,
  onStep,
  onToggleMuted,
  onToggleFullscreen,
  onExitFullscreen,
  onClose,
  onOwnedShortcut,
}: UseVideoShortcutsOptions): void {
  useEffect(() => {
    if (!active) return

    const keyDown = (event: KeyboardEvent) => {
      if (event.metaKey || event.ctrlKey || event.altKey || event.shiftKey) return
      if (shortcutSuppressed(event.target, root.current)) return
      const key = normalizeKey(event.key)
      if (!isOwnedKey(key)) return

      event.preventDefault()
      onOwnedShortcut()
      if (key === 'space') {
        void onTogglePlayback()
      } else if (key === 'arrowleft') {
        void onStep('backward')
      } else if (key === 'arrowright') {
        void onStep('forward')
      } else if (key === 'm') {
        void onToggleMuted()
      } else if (key === 'f') {
        void onToggleFullscreen()
      } else if (fullscreen) {
        void onExitFullscreen()
      } else {
        onClose()
      }
    }

    window.addEventListener('keydown', keyDown)
    return () => window.removeEventListener('keydown', keyDown)
  }, [
    active,
    fullscreen,
    onClose,
    onExitFullscreen,
    onOwnedShortcut,
    onStep,
    onToggleFullscreen,
    onToggleMuted,
    onTogglePlayback,
    playing,
    root,
  ])
}

function normalizeKey(key: string): string {
  if (key === ' ' || key === 'Spacebar') return 'space'
  return key.toLowerCase()
}

function isOwnedKey(key: string): boolean {
  return (
    key === 'space' ||
    key === 'arrowleft' ||
    key === 'arrowright' ||
    key === 'm' ||
    key === 'f' ||
    key === 'escape'
  )
}

function shortcutSuppressed(target: EventTarget | null, root: HTMLElement | null): boolean {
  if (!(target instanceof Element)) return false
  if (target.closest('input, textarea, select, [contenteditable]:not([contenteditable="false"])')) {
    return true
  }
  const dialog = target.closest('[role="dialog"], [aria-modal="true"]')
  return dialog !== null && dialog !== root && !(root?.contains(dialog) ?? false)
}
