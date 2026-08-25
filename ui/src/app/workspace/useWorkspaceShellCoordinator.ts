import type { KeyboardEvent as ReactKeyboardEvent } from 'react'
import { useCallback, useEffect, useRef, useState } from 'react'
import type { ContentViewCommand } from '../../components/ContentBrowser'
import type { SelectAllRequest } from '../../components/contentBrowser/adaptiveOtherFilePanelModel'
import { useAppShellState } from '../useAppShellState'
import { useOtherFilePanelPreference } from '../useOtherFilePanelPreference'
import { useToolbarPopover } from '../useToolbarPopover'
import { useVideoPanelPreference } from '../useVideoPanelPreference'
import type { WorkspaceShellPort } from './ports'

export interface WorkspaceShellOptions {
  projectSessionId: string
  videoProjectSessionId: string | null
  port: WorkspaceShellPort
}

export interface WorkspaceShellCoordinator {
  sidebarCollapsed: boolean
  sidebarWidth: number
  narrowViewport: boolean
  effectiveSidebarCollapsed: boolean
  toggleSidebar(): void
  startSidebarResize: ReturnType<typeof useAppShellState>['startSidebarResize']
  resizeSidebarFromKeyboard(event: ReactKeyboardEvent<HTMLButtonElement>): void
  infoOpen: boolean
  settingsOpen: boolean
  openInfo(): void
  closeInfo(): void
  openSettings(): void
  closeSettings(): void
  toolbarPopover: ReturnType<typeof useToolbarPopover>
  otherFilePanel: ReturnType<typeof useOtherFilePanelPreference>
  videoPanel: ReturnType<typeof useVideoPanelPreference>
  selectAllRequest: SelectAllRequest
  contentViewCommand: ContentViewCommand | null
  requestSelectAll(scope: ContentViewCommand['scope']): void
  setSelectAllRequest(request: SelectAllRequest): void
  recoveryAcknowledgedSessionId: string | null
  acknowledgeRecovery(): void
  workspaceActionError: string | null
  revealProject(): void
}

export function useWorkspaceShellCoordinator({
  projectSessionId,
  videoProjectSessionId,
  port,
}: WorkspaceShellOptions): WorkspaceShellCoordinator {
  const shellState = useAppShellState(projectSessionId)
  const otherFilePanel = useOtherFilePanelPreference(projectSessionId)
  const videoPanel = useVideoPanelPreference(videoProjectSessionId)
  const toolbarPopover = useToolbarPopover()
  const [narrowViewport, setNarrowViewport] = useState(() => window.innerWidth <= 760)
  const [infoOpen, setInfoOpen] = useState(false)
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [selectAllRequest, setSelectAllRequest] = useState<SelectAllRequest>({ kind: 'none' })
  const [contentViewCommand, setContentViewCommand] = useState<ContentViewCommand | null>(null)
  const [recoveryAcknowledgedSessionId, setRecoveryAcknowledgedSessionId] = useState<string | null>(
    null,
  )
  const [workspaceActionError, setWorkspaceActionError] = useState<string | null>(null)
  const contentViewCommandRequestId = useRef(0)

  useEffect(() => {
    const updateViewport = () => setNarrowViewport(window.innerWidth <= 760)
    window.addEventListener('resize', updateViewport)
    return () => window.removeEventListener('resize', updateViewport)
  }, [])

  useEffect(() => {
    setInfoOpen(false)
    setSettingsOpen(false)
  }, [projectSessionId])

  useEffect(() => {
    if (contentViewCommand !== null) setContentViewCommand(null)
  }, [contentViewCommand])

  const openInfo = useCallback(() => setInfoOpen(true), [])
  const closeInfo = useCallback(() => setInfoOpen(false), [])
  const openSettings = useCallback(() => setSettingsOpen(true), [])
  const closeSettings = useCallback(() => setSettingsOpen(false), [])
  const requestSelectAll = useCallback((scope: ContentViewCommand['scope']) => {
    setContentViewCommand({
      requestId: ++contentViewCommandRequestId.current,
      scope,
    })
  }, [])
  const acknowledgeRecovery = useCallback(
    () => setRecoveryAcknowledgedSessionId(projectSessionId),
    [projectSessionId],
  )
  const revealProject = useCallback(() => {
    setWorkspaceActionError(null)
    void port
      .revealProjectInFileManager()
      .catch(() => setWorkspaceActionError('请在 Finder 中手动打开当前项目文件夹。'))
  }, [port])

  return {
    sidebarCollapsed: shellState.sidebarCollapsed,
    sidebarWidth: shellState.sidebarWidth,
    narrowViewport,
    effectiveSidebarCollapsed: narrowViewport || shellState.sidebarCollapsed,
    toggleSidebar: shellState.toggleSidebar,
    startSidebarResize: shellState.startSidebarResize,
    resizeSidebarFromKeyboard: shellState.resizeSidebarFromKeyboard,
    infoOpen,
    settingsOpen,
    openInfo,
    closeInfo,
    openSettings,
    closeSettings,
    toolbarPopover,
    otherFilePanel,
    videoPanel,
    selectAllRequest,
    contentViewCommand,
    requestSelectAll,
    setSelectAllRequest,
    recoveryAcknowledgedSessionId,
    acknowledgeRecovery,
    workspaceActionError,
    revealProject,
  }
}
