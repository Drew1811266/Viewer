import { act, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { ProjectionTransition } from '../state/viewerState'
import { useDelayedProjectionProgress } from './useDelayedProjectionProgress'

const transitionA: ProjectionTransition = {
  selectedFolderId: 'folder-a',
  selectedFolderPath: 'folder-a',
  showingAggregate: false,
}

const transitionB: ProjectionTransition = {
  selectedFolderId: 'folder-b',
  selectedFolderPath: 'folder-b',
  showingAggregate: false,
}

beforeEach(() => vi.useFakeTimers())
afterEach(() => vi.useRealTimers())

describe('useDelayedProjectionProgress', () => {
  it('stays hidden until a workspace projection has remained pending for 120 ms', () => {
    const { result } = renderHook(() => useDelayedProjectionProgress(transitionA, true))

    expect(result.current).toBe(false)
    act(() => vi.advanceTimersByTime(119))
    expect(result.current).toBe(false)
    act(() => vi.advanceTimersByTime(1))
    expect(result.current).toBe(true)
  })

  it('never covers initial workspace loading and hides immediately when projection settles', () => {
    const { result, rerender } = renderHook(
      ({ transition, workspaceAvailable }) =>
        useDelayedProjectionProgress(transition, workspaceAvailable),
      {
        initialProps: {
          transition: transitionA as ProjectionTransition | null,
          workspaceAvailable: false,
        },
      },
    )

    act(() => vi.advanceTimersByTime(120))
    expect(result.current).toBe(false)
    rerender({ transition: transitionA, workspaceAvailable: true })
    act(() => vi.advanceTimersByTime(120))
    expect(result.current).toBe(true)
    rerender({ transition: null, workspaceAvailable: true })
    expect(result.current).toBe(false)
  })

  it('restarts the delay for a replacement request and clears its timer on unmount', () => {
    const { result, rerender, unmount } = renderHook(
      ({ transition }) => useDelayedProjectionProgress(transition, true),
      { initialProps: { transition: transitionA } },
    )

    act(() => vi.advanceTimersByTime(80))
    rerender({ transition: transitionB })
    act(() => vi.advanceTimersByTime(40))
    expect(result.current).toBe(false)
    act(() => vi.advanceTimersByTime(80))
    expect(result.current).toBe(true)

    rerender({ transition: transitionA })
    expect(vi.getTimerCount()).toBe(1)
    unmount()
    expect(vi.getTimerCount()).toBe(0)
  })
})
