import { useCallback, useRef, useState } from 'react'

export interface OtherFilePanelPreferenceState {
  expanded: boolean
  setExpanded(expanded: boolean): void
}

interface StoredPreference {
  sessionId: string
  expanded: boolean
}

export function useOtherFilePanelPreference(
  projectSessionId: string,
): OtherFilePanelPreferenceState {
  const latestSessionId = useRef(projectSessionId)
  latestSessionId.current = projectSessionId
  const [stored, setStored] = useState<StoredPreference>(() => ({
    sessionId: projectSessionId,
    expanded: false,
  }))
  const expanded = stored.sessionId === projectSessionId ? stored.expanded : false

  const setExpanded = useCallback(
    (next: boolean) => {
      if (latestSessionId.current !== projectSessionId) return
      setStored((current) =>
        current.sessionId === projectSessionId && current.expanded === next
          ? current
          : { sessionId: projectSessionId, expanded: next },
      )
    },
    [projectSessionId],
  )

  return { expanded, setExpanded }
}
