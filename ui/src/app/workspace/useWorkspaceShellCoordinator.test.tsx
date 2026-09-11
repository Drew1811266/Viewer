import { act, renderHook } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { ContentViewCommand } from '../../components/ContentBrowser'
import { useWorkspaceShellCoordinator } from './useWorkspaceShellCoordinator'

const originalInnerWidth = window.innerWidth

function setViewportWidth(width: number) {
  Object.defineProperty(window, 'innerWidth', { configurable: true, value: width })
}

afterEach(() => {
  setViewportWidth(originalInnerWidth)
  vi.restoreAllMocks()
})

describe('useWorkspaceShellCoordinator', () => {
  it('resets overlays and panel preferences by the existing project-session rules', () => {
    const port = { revealProjectInFileManager: vi.fn().mockResolvedValue(undefined) }
    const hook = renderHook(
      ({ sessionId }) =>
        useWorkspaceShellCoordinator({
          projectSessionId: sessionId,
          videoProjectSessionId: sessionId,
          port,
        }),
      { initialProps: { sessionId: 'session-1' } },
    )

    act(() => {
      hook.result.current.openInfo()
      hook.result.current.openSettings()
      hook.result.current.otherFilePanel.setExpanded(true)
      hook.result.current.videoPanel.setExpanded(false)
    })
    expect(hook.result.current.infoOpen).toBe(true)
    expect(hook.result.current.settingsOpen).toBe(true)
    expect(hook.result.current.otherFilePanel.expanded).toBe(true)
    expect(hook.result.current.videoPanel.expanded).toBe(false)

    hook.rerender({ sessionId: 'session-2' })

    expect(hook.result.current.infoOpen).toBe(false)
    expect(hook.result.current.settingsOpen).toBe(false)
    expect(hook.result.current.otherFilePanel.expanded).toBe(false)
    expect(hook.result.current.videoPanel.expanded).toBe(true)
  })

  it('uses the existing viewport threshold and sidebar resize semantics', () => {
    setViewportWidth(761)
    const hook = renderHook(() =>
      useWorkspaceShellCoordinator({
        projectSessionId: 'session-1',
        videoProjectSessionId: 'session-1',
        port: { revealProjectInFileManager: vi.fn().mockResolvedValue(undefined) },
      }),
    )

    expect(hook.result.current.narrowViewport).toBe(false)
    expect(hook.result.current.effectiveSidebarCollapsed).toBe(false)

    setViewportWidth(760)
    act(() => window.dispatchEvent(new Event('resize')))
    expect(hook.result.current.narrowViewport).toBe(true)
    expect(hook.result.current.effectiveSidebarCollapsed).toBe(true)

    setViewportWidth(761)
    act(() => window.dispatchEvent(new Event('resize')))
    const preventDefault = vi.fn()
    act(() =>
      hook.result.current.resizeSidebarFromKeyboard({
        key: 'ArrowRight',
        preventDefault,
      } as unknown as Parameters<typeof hook.result.current.resizeSidebarFromKeyboard>[0]),
    )
    expect(preventDefault).toHaveBeenCalledOnce()
    expect(hook.result.current.sidebarWidth).toBe(168)

    act(() => hook.result.current.toggleSidebar())
    expect(hook.result.current.effectiveSidebarCollapsed).toBe(true)
  })

  it('keeps toolbar popovers exclusive and select-all command ids monotonic', () => {
    const commands: ContentViewCommand[] = []
    const hook = renderHook(() => {
      const shell = useWorkspaceShellCoordinator({
        projectSessionId: 'session-1',
        videoProjectSessionId: 'session-1',
        port: { revealProjectInFileManager: vi.fn().mockResolvedValue(undefined) },
      })
      const command = shell.contentViewCommand
      if (command !== null && commands.at(-1)?.requestId !== command.requestId) {
        commands.push(command)
      }
      return shell
    })

    act(() => hook.result.current.toolbarPopover.togglePopover('filter'))
    expect(hook.result.current.toolbarPopover.openPopover).toBe('filter')
    act(() => hook.result.current.toolbarPopover.togglePopover('view'))
    expect(hook.result.current.toolbarPopover.openPopover).toBe('view')

    act(() => hook.result.current.requestSelectAll('images'))
    act(() => hook.result.current.requestSelectAll('all'))
    expect(commands).toEqual([
      { requestId: 1, scope: 'images' },
      { requestId: 2, scope: 'all' },
    ])

    act(() =>
      hook.result.current.setSelectAllRequest({
        kind: 'choice',
        scopes: ['images', 'videos'],
      }),
    )
    expect(hook.result.current.selectAllRequest).toEqual({
      kind: 'choice',
      scopes: ['images', 'videos'],
    })
  })

  it('preserves reveal errors across sessions and clears them before retrying', async () => {
    let resolveRetry!: () => void
    const retry = new Promise<void>((resolve) => {
      resolveRetry = resolve
    })
    const revealProjectInFileManager = vi
      .fn()
      .mockRejectedValueOnce(new Error('finder unavailable'))
      .mockReturnValueOnce(retry)
    const hook = renderHook(
      ({ sessionId }) =>
        useWorkspaceShellCoordinator({
          projectSessionId: sessionId,
          videoProjectSessionId: sessionId,
          port: { revealProjectInFileManager },
        }),
      { initialProps: { sessionId: 'session-1' } },
    )

    await act(async () => hook.result.current.revealProject())
    expect(hook.result.current.workspaceActionError).toBe('请在 Finder 中手动打开当前项目文件夹。')

    hook.rerender({ sessionId: 'session-2' })
    expect(hook.result.current.workspaceActionError).toBe('请在 Finder 中手动打开当前项目文件夹。')
    act(() => hook.result.current.revealProject())
    expect(hook.result.current.workspaceActionError).toBeNull()

    await act(async () => resolveRetry())
  })

  it('acknowledges recovery by session identity without force-resetting the stored id', () => {
    const hook = renderHook(
      ({ sessionId }) =>
        useWorkspaceShellCoordinator({
          projectSessionId: sessionId,
          videoProjectSessionId: sessionId,
          port: { revealProjectInFileManager: vi.fn().mockResolvedValue(undefined) },
        }),
      { initialProps: { sessionId: 'session-1' } },
    )

    act(() => hook.result.current.acknowledgeRecovery())
    expect(hook.result.current.recoveryAcknowledgedSessionId).toBe('session-1')
    hook.rerender({ sessionId: 'session-2' })
    expect(hook.result.current.recoveryAcknowledgedSessionId).toBe('session-1')
  })
})
