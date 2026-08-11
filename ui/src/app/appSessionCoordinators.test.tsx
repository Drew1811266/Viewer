import { act, createEvent, fireEvent, render, renderHook, screen } from '@testing-library/react'
import { type ReactNode, StrictMode, useLayoutEffect } from 'react'
import { afterEach, describe, expect, it } from 'vitest'
import type { BrowserFile } from '../api/types'
import { getAppShellStateInternals, useAppShellState } from './useAppShellState'
import { useOtherFilePanelPreference } from './useOtherFilePanelPreference'
import {
  getPreviewSessionInternals,
  type PreviewSession,
  usePreviewSession,
} from './usePreviewSession'
import { useRadialMenuContextToken, useRadialMenuSession } from './useRadialMenuSession'

const strictWrapper = ({ children }: { children: ReactNode }) => <StrictMode>{children}</StrictMode>

function file(entityId: string): BrowserFile {
  return {
    entityId,
    relativePath: `${entityId}.jpg`,
    name: `${entityId}.jpg`,
    kind: 'jpeg',
    size: 100,
    modifiedNs: '1',
    marker: { reviewState: null, favorite: false },
    imageMetadata: null,
    imageUrl: null,
  }
}

function preview(
  entityId: string,
  files: BrowserFile[] | null,
  folderOverviewIdentity: string | null,
): PreviewSession {
  return { file: file(entityId), files, folderOverviewIdentity }
}

function ShellHarness({ name }: { name: string }) {
  const shell = useAppShellState('session-1')
  const { resizeSidebarFromKeyboard } = getAppShellStateInternals(shell)
  return (
    <section aria-label={`${name} shell`}>
      <output aria-label={`${name} width`}>{shell.sidebarWidth}</output>
      <button
        type="button"
        className="sidebar-separator"
        aria-label={`${name} separator`}
        onKeyDown={resizeSidebarFromKeyboard}
      />
    </section>
  )
}

function PreviewSessionSwitchHarness({
  openOnCommit,
  projectSessionId,
}: {
  openOnCommit: boolean
  projectSessionId: string
}) {
  const state = usePreviewSession(projectSessionId)

  useLayoutEffect(() => {
    if (openOnCommit) state.openPreview(preview('new-session', null, null))
  }, [openOnCommit, state.openPreview])

  return <output aria-label="active preview">{state.activePreview?.file.name ?? 'none'}</output>
}

interface RadialContext {
  activePreview: object | null
  compareOpen: boolean
  infoOpen: boolean
  operationDialog: object | null
  organizationWorkspaceIdentity: string
  resultsBatchId: string | null
  closeBlocked: object | null
  contextRepair: object | null
}

function radialContext(overrides: Partial<RadialContext> = {}): RadialContext {
  return {
    activePreview: null,
    compareOpen: false,
    infoOpen: false,
    operationDialog: null,
    organizationWorkspaceIdentity: 'content:root:folder:browser',
    resultsBatchId: null,
    closeBlocked: null,
    contextRepair: null,
    ...overrides,
  }
}

function useRadialHarness(context: RadialContext) {
  const contextKey = useRadialMenuContextToken(context)
  return useRadialMenuSession({
    projectIdentity: 'session-1:1',
    projectStatus: 'active',
    contextKey,
  })
}

function focusButtons() {
  const returnTarget = document.createElement('button')
  const displacedTarget = document.createElement('button')
  document.body.append(returnTarget, displacedTarget)
  return { displacedTarget, returnTarget }
}

afterEach(() => {
  document.body.replaceChildren()
})

describe('App shell coordinator internals', () => {
  it('keeps keyboard resize target-local across hook instances and unrelated separators', () => {
    render(
      <StrictMode>
        <ShellHarness name="first" />
        <ShellHarness name="second" />
        <button type="button" className="sidebar-separator" aria-label="unrelated separator" />
      </StrictMode>,
    )
    const first = screen.getByRole('button', { name: 'first separator' })
    const second = screen.getByRole('button', { name: 'second separator' })
    const unrelated = screen.getByRole('button', { name: 'unrelated separator' })

    const firstKey = createEvent.keyDown(first, { key: 'ArrowLeft' })
    fireEvent(first, firstKey)
    expect(firstKey.defaultPrevented).toBe(true)
    expect(screen.getByRole('status', { name: 'first width' })).toHaveTextContent('204')
    expect(screen.getByRole('status', { name: 'second width' })).toHaveTextContent('220')

    fireEvent.keyDown(unrelated, { key: 'ArrowLeft' })
    expect(screen.getByRole('status', { name: 'first width' })).toHaveTextContent('204')
    expect(screen.getByRole('status', { name: 'second width' })).toHaveTextContent('220')

    fireEvent.keyDown(second, { key: 'ArrowRight' })
    expect(screen.getByRole('status', { name: 'first width' })).toHaveTextContent('204')
    expect(screen.getByRole('status', { name: 'second width' })).toHaveTextContent('236')
  })
})

describe('Preview session coordinator internals', () => {
  it('does not erase a preview opened during the new project session commit', () => {
    const view = render(
      <PreviewSessionSwitchHarness projectSessionId="session-1" openOnCommit={false} />,
    )

    view.rerender(<PreviewSessionSwitchHarness projectSessionId="session-2" openOnCommit={true} />)

    expect(screen.getByRole('status', { name: 'active preview' })).toHaveTextContent(
      'new-session.jpg',
    )
  })

  it('does not let a stale batched navigation reopen a closed preview', () => {
    const hook = renderHook(
      () => {
        const state = usePreviewSession('session-1')
        return { internals: getPreviewSessionInternals(state), state }
      },
      { wrapper: strictWrapper },
    )

    act(() => hook.result.current.state.openPreview(preview('old', [file('old')], 'old-folder')))
    const staleNavigate = hook.result.current.internals.navigatePreview

    act(() => {
      hook.result.current.state.closePreview()
      staleNavigate(file('next'))
    })

    expect(hook.result.current.state.activePreview).toBeNull()
  })

  it('preserves the latest preview files and identity during stale batched navigation', () => {
    const hook = renderHook(
      () => {
        const state = usePreviewSession('session-1')
        return { internals: getPreviewSessionInternals(state), state }
      },
      { wrapper: strictWrapper },
    )
    const oldFiles = [file('old'), file('old-next')]
    const latestFiles = [file('latest'), file('latest-next')]

    act(() => hook.result.current.state.openPreview(preview('old', oldFiles, 'old-folder')))
    const staleNavigate = hook.result.current.internals.navigatePreview

    act(() => {
      hook.result.current.state.openPreview(preview('latest', latestFiles, 'latest-folder'))
      staleNavigate(file('destination'))
    })

    expect(hook.result.current.state.activePreview).toEqual({
      file: file('destination'),
      files: latestFiles,
      folderOverviewIdentity: 'latest-folder',
    })
  })
})

describe('Radial menu context identity', () => {
  it('invalidates and restores focus when an equal-valued context object is replaced', () => {
    const contextRepair = {
      removedEntityIds: ['image-1'],
      suggestedEntityId: 'image-2',
      message: 'same',
    }
    const hook = renderHook(({ context }) => useRadialHarness(context), {
      initialProps: { context: radialContext({ contextRepair }) },
      wrapper: strictWrapper,
    })
    const { displacedTarget, returnTarget } = focusButtons()
    returnTarget.focus()
    act(() => {
      hook.result.current.beginRadialSession({
        files: [file('image-1')],
        origin: { x: 40, y: 50 },
        pointerId: null,
        returnFocusTarget: returnTarget,
      })
    })
    displacedTarget.focus()

    hook.rerender({
      context: radialContext({
        contextRepair: {
          removedEntityIds: ['image-1'],
          suggestedEntityId: 'image-2',
          message: 'same',
        },
      }),
    })

    expect(hook.result.current.activeRadialMenu).toBeNull()
    expect(document.activeElement).toBe(returnTarget)
  })

  it('cannot preserve a stale snapshot when delimiter-shaped context values collide', () => {
    const initialPreview = preview('c', null, null)
    const collidingPreview = preview('b:c', null, null)
    const hook = renderHook(({ context }) => useRadialHarness(context), {
      initialProps: {
        context: radialContext({
          activePreview: initialPreview,
          organizationWorkspaceIdentity: 'a:b',
        }),
      },
      wrapper: strictWrapper,
    })
    const { displacedTarget, returnTarget } = focusButtons()
    returnTarget.focus()
    act(() => {
      hook.result.current.beginRadialSession({
        files: [file('image-1')],
        origin: { x: 40, y: 50 },
        pointerId: null,
        returnFocusTarget: returnTarget,
      })
    })
    displacedTarget.focus()

    hook.rerender({
      context: radialContext({
        activePreview: collidingPreview,
        organizationWorkspaceIdentity: 'a',
      }),
    })

    expect(hook.result.current.activeRadialMenu).toBeNull()
    expect(document.activeElement).toBe(returnTarget)
  })
})

describe('Other file panel project-session preference', () => {
  it('starts collapsed, persists within one session, and resets synchronously for another', () => {
    const hook = renderHook(({ sessionId }) => useOtherFilePanelPreference(sessionId), {
      initialProps: { sessionId: 'session-1' },
      wrapper: strictWrapper,
    })

    expect(hook.result.current.expanded).toBe(false)
    act(() => hook.result.current.setExpanded(true))
    expect(hook.result.current.expanded).toBe(true)

    hook.rerender({ sessionId: 'session-2' })
    expect(hook.result.current.expanded).toBe(false)
  })

  it('ignores a stale setter captured by an earlier project session', () => {
    const hook = renderHook(({ sessionId }) => useOtherFilePanelPreference(sessionId), {
      initialProps: { sessionId: 'session-1' },
      wrapper: strictWrapper,
    })
    const staleSetExpanded = hook.result.current.setExpanded

    hook.rerender({ sessionId: 'session-2' })
    act(() => staleSetExpanded(true))

    expect(hook.result.current.expanded).toBe(false)
  })
})
