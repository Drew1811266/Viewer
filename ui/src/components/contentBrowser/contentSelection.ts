export function rangeSelection(
  orderedEntityIds: readonly string[],
  anchorId: string,
  targetId: string,
): string[] {
  const anchor = orderedEntityIds.indexOf(anchorId)
  const target = orderedEntityIds.indexOf(targetId)
  if (target < 0) return []
  if (anchor < 0) return [targetId]
  const start = Math.min(anchor, target)
  const end = Math.max(anchor, target)
  return orderedEntityIds.slice(start, end + 1)
}

export function toggleSelection(selectedEntityIds: readonly string[], targetId: string): string[] {
  return selectedEntityIds.includes(targetId)
    ? selectedEntityIds.filter((entityId) => entityId !== targetId)
    : [...selectedEntityIds, targetId]
}
