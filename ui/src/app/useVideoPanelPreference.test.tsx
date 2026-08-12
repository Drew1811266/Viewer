import { act, renderHook } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { useVideoPanelPreference } from './useVideoPanelPreference'

describe('useVideoPanelPreference', () => {
  it('defaults each project session to expanded and restores its in-memory preference', () => {
    const hook = renderHook(({ sessionKey }) => useVideoPanelPreference(sessionKey), {
      initialProps: { sessionKey: 'session-1' as string | null },
    })

    expect(hook.result.current.expanded).toBe(true)
    act(() => hook.result.current.setExpanded(false))
    expect(hook.result.current.expanded).toBe(false)

    hook.rerender({ sessionKey: 'session-2' })
    expect(hook.result.current.expanded).toBe(true)
    hook.rerender({ sessionKey: 'session-1' })
    expect(hook.result.current.expanded).toBe(false)
  })

  it('does not persist a presentation preference without an active session', () => {
    const hook = renderHook(({ sessionKey }) => useVideoPanelPreference(sessionKey), {
      initialProps: { sessionKey: null as string | null },
    })

    act(() => hook.result.current.setExpanded(false))
    expect(hook.result.current.expanded).toBe(true)
  })
})
