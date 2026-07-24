const INTERACTIVE_SELECTOR = [
  'input',
  'textarea',
  'select',
  'button',
  'a',
  'summary',
  '[role="button"]',
  '[role="dialog"]',
  '[aria-modal="true"]',
  '[contenteditable]:not([contenteditable="false"])',
].join(', ')

export function organizationShortcutIsOwned(event: KeyboardEvent, disabled = false): boolean {
  if (disabled || event.defaultPrevented) return true
  if (document.querySelector('[aria-modal="true"], [role="dialog"]') !== null) return true
  if (hasTextSelection()) return true
  return isInteractive(event.target) || isInteractive(document.activeElement)
}

function isInteractive(target: EventTarget | null): boolean {
  return target instanceof Element && target.closest(INTERACTIVE_SELECTOR) !== null
}

function hasTextSelection(): boolean {
  const selection = window.getSelection()
  return selection !== null && !selection.isCollapsed && selection.toString().length > 0
}
