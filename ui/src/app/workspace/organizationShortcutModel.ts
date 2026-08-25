export type OrganizationShortcutAction = 'undo' | 'preview' | 'compare' | 'rename' | 'trash'

export function isToggleInfoShortcut(event: KeyboardEvent): boolean {
  return (
    event.metaKey &&
    !event.ctrlKey &&
    !event.altKey &&
    !event.shiftKey &&
    event.key.toLowerCase() === 'i'
  )
}

export function organizationShortcutAction(
  event: KeyboardEvent,
  selectedCount: number,
  canMutateSelection: boolean,
  operationBusy: boolean,
): OrganizationShortcutAction | null {
  if (isUndoShortcut(event)) return operationBusy || event.repeat ? null : 'undo'
  if (event.metaKey || event.ctrlKey || event.altKey || event.shiftKey) return null
  if (isPreviewShortcut(event)) return selectedCount === 1 ? 'preview' : null

  const key = event.key.toLowerCase()
  if (key === 'c') return 'compare'
  if (event.key === 'Enter') return canMutateSelection ? 'rename' : null
  if (event.key === 'Delete' || event.key === 'Backspace') {
    return canMutateSelection ? 'trash' : null
  }
  return null
}

function isUndoShortcut(event: KeyboardEvent): boolean {
  return (
    event.metaKey &&
    !event.ctrlKey &&
    !event.altKey &&
    !event.shiftKey &&
    event.key.toLowerCase() === 'z'
  )
}

function isPreviewShortcut(event: KeyboardEvent): boolean {
  return event.key === ' ' || event.key === 'Spacebar' || event.code === 'Space'
}
