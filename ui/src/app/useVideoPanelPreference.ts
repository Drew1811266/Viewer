import { useCallback, useRef, useState } from 'react'

export interface VideoPanelPreferenceState {
  expanded: boolean
  setExpanded(expanded: boolean): void
}

export function useVideoPanelPreference(
  projectSessionKey: string | null,
): VideoPanelPreferenceState {
  const latestSessionKey = useRef(projectSessionKey)
  latestSessionKey.current = projectSessionKey
  const [collapsedBySession, setCollapsedBySession] = useState<Record<string, boolean>>({})
  const collapsed =
    projectSessionKey === null ? false : (collapsedBySession[projectSessionKey] ?? false)

  const setExpanded = useCallback(
    (expanded: boolean) => {
      if (projectSessionKey === null || latestSessionKey.current !== projectSessionKey) return
      setCollapsedBySession((current) => {
        const collapsedForSession = !expanded
        if (current[projectSessionKey] === collapsedForSession) return current
        return { ...current, [projectSessionKey]: collapsedForSession }
      })
    },
    [projectSessionKey],
  )

  return { expanded: !collapsed, setExpanded }
}
