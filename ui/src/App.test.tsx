import {
  act,
  createEvent,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from '@testing-library/react'
import { afterEach, describe, expect, expectTypeOf, it, vi } from 'vitest'
import App from './App'
import type {
  ReviewHistorySelector,
  ReviewHistoryView,
  ReviewWorkspaceView,
} from './api/reviewWorkspaceTypes'
import type { FolderWorkspace, ReviewSessionSnapshot } from './api/types'
import type { ProjectDropEvent, ViewerBridge } from './api/viewer'
import {
  migrationInspection,
  reviewPort,
  workspace,
} from './app/review/continuousReviewTestFixtures'
import { type AppShellState, useAppShellState } from './app/useAppShellState'
import {
  type OperationDialogsState,
  type UseOperationDialogsOptions,
  useOperationDialogs,
} from './app/useOperationDialogs'
import {
  type OtherFilePanelPreferenceState,
  useOtherFilePanelPreference,
} from './app/useOtherFilePanelPreference'
import { type PreviewSessionState, usePreviewSession } from './app/usePreviewSession'
import {
  type RadialMenuSessionState,
  type UseRadialMenuSessionOptions,
  useRadialMenuSession,
} from './app/useRadialMenuSession'
import { defined } from './defined'
import './styles/app.css'

let restorePreviewStageBounds: (() => void) | null = null

afterEach(() => {
  vi.useRealTimers()
  restorePreviewStageBounds?.()
  restorePreviewStageBounds = null
  vi.unstubAllGlobals()
})

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((next, fail) => {
    resolve = next
    reject = fail
  })
  return { promise, resolve, reject }
}

function openRadialMenu(file: HTMLElement, pointerId = 90) {
  fireEvent.pointerDown(file, {
    pointerId,
    button: 2,
    clientX: 420,
    clientY: 260,
  })
  fireEvent.pointerMove(window, {
    pointerId,
    buttons: 2,
    clientX: 429,
    clientY: 260,
  })
}

function closeProjectFromMenu() {
  fireEvent.click(screen.getByRole('button', { name: '更多' }))
  fireEvent.click(screen.getByRole('button', { name: '关闭项目' }))
}

function openSettingsFromMenu() {
  fireEvent.click(screen.getByRole('button', { name: '更多' }))
  fireEvent.click(screen.getByRole('button', { name: '软件设置' }))
}

function bridge(access: 'read_write' | 'read_only' = 'read_write'): ViewerBridge {
  return {
    chooseProject: vi.fn().mockResolvedValue('/fixture/project'),
    openProject: vi.fn().mockResolvedValue({
      projectId: 'project-1',
      sessionId: 'session-1',
      generation: 1,
      displayName: 'Catalog',
      access,
    }),
    closeProject: vi.fn().mockResolvedValue('closed'),
    projectSnapshot: vi.fn().mockResolvedValue(null),
    getViewerSettings: vi.fn().mockResolvedValue({
      schemaVersion: 4,
      thumbnailDensity: 'standard',
      magnifier: { shape: 'circle', magnification: 2, area: 'small' },
    }),
    updateViewerSettings: vi.fn().mockImplementation(async (settings) => ({
      schemaVersion: 4,
      ...settings,
    })),
    folderTree: vi.fn().mockResolvedValue([]),
    queryFolder: vi.fn().mockResolvedValue({ workspace: 'empty' }),
    requestImage: vi.fn().mockResolvedValue({
      cacheKey: 'test-image',
      url: 'viewer-image://localhost/session/test-image',
      width: 800,
      height: 600,
      backend: 'quick_look',
    }),
    previewText: vi.fn(),
    openExternalLink: vi.fn(),
    revealProjectInFileManager: vi.fn().mockResolvedValue(undefined),
    cancelTask: vi.fn().mockResolvedValue(false),
    searchProject: vi.fn(),
    searchTextSnippet: vi.fn(),
    setReviewState: vi.fn(),
    toggleFavorite: vi.fn(),
    selectionInfo: vi.fn().mockResolvedValue({
      relativePaths: [],
      totalSize: 0,
      types: { folders: 0, images: 0, videos: 0, otherFiles: 0 },
      commonReview: { state: 'none_selected' },
      commonFavorite: { state: 'none_selected' },
    }),
    previewRename: vi.fn().mockResolvedValue({ rows: [], executable: false }),
    preflightFileCommand: vi.fn().mockResolvedValue({ rows: [], executable: false }),
    executeFileCommand: vi.fn().mockResolvedValue({ batchId: 'batch-1' }),
    operationStatus: vi.fn().mockResolvedValue({
      sessionId: 'session-1',
      generation: 1,
      batchId: 'batch-1',
      target: 'project',
      lifecycle: 'queued',
      requested: 1,
      completed: 0,
      failed: 0,
      skipped: 0,
      cancelled: 0,
      activeEntityId: null,
    }),
    operationResults: vi.fn().mockResolvedValue({ total: 0, offset: 0, items: [] }),
    cancelOperation: vi.fn().mockResolvedValue(false),
    undoLastOperation: vi.fn().mockResolvedValue(null),
    beginFinderDrag: vi.fn().mockResolvedValue({ fileCount: 1 }),
    reviewStatus: vi.fn().mockResolvedValue({
      phase: 'idle',
      resume: null,
      reviewStreamId: null,
      reviewRoundId: null,
      revision: 0,
      members: [],
      feedback: [],
      unreviewable: [],
      conflicts: [],
      counts: { total: 0, feedbackItems: 0, revise: 0, unreviewable: 0, pass: 0 },
      error: null,
    }),
    reviewPreviewStart: vi.fn(),
    reviewStart: vi.fn(),
    reviewStartWithFeedback: vi.fn(),
    reviewResume: vi.fn(),
    reviewAddFeedback: vi.fn(),
    reviewUpdateFeedback: vi.fn(),
    reviewUpdateFeedbackText: vi.fn(),
    reviewReplaceFeedbackAnchor: vi.fn(),
    reviewDeleteFeedback: vi.fn(),
    reviewRestoreDeletedFeedback: vi.fn(),
    reviewCompletionSummary: vi.fn(),
    reviewComplete: vi.fn(),
    reviewAbandon: vi.fn(),
    reviewCancelTask: vi.fn().mockResolvedValue(false),
    videoOpen: vi.fn(),
    videoCancelOpen: vi.fn().mockResolvedValue(true),
    videoClose: vi.fn(),
    videoPlay: vi.fn(),
    videoPause: vi.fn(),
    videoSeek: vi.fn(),
    videoStep: vi.fn(),
    videoSetVolume: vi.fn(),
    videoSetMuted: vi.fn(),
    videoSetRate: vi.fn(),
    videoSetFullscreen: vi.fn(),
    videoRequestCover: vi.fn().mockResolvedValue('viewer-image://localhost/session/cover'),
    videoRequestThumbnail: vi.fn(),
    videoCacheStats: vi.fn().mockResolvedValue({
      bytesUsed: 268_435_456,
      budgetBytes: 1_073_741_824,
      entryCount: 24,
    }),
    videoCacheClear: vi.fn().mockResolvedValue({
      bytesUsed: 0,
      budgetBytes: 1_073_741_824,
      entryCount: 0,
    }),
    openPermissionSettings: vi.fn().mockResolvedValue(undefined),
    listenScan: vi.fn().mockResolvedValue(() => undefined),
    listenIndexProgress: vi.fn().mockResolvedValue(() => undefined),
    listenOperationProgress: vi.fn().mockResolvedValue(() => undefined),
    listenProjectChanged: vi.fn().mockResolvedValue(() => undefined),
    listenCloseBlocked: vi.fn().mockResolvedValue(() => undefined),
    listenVideo: vi.fn().mockResolvedValue(() => undefined),
    listenReviewProgress: vi.fn().mockResolvedValue(() => undefined),
    listenProjectClosed: vi.fn().mockResolvedValue(() => undefined),
    listenProjectDrops: vi.fn().mockResolvedValue(() => undefined),
    listenProjectDropEvents: vi.fn().mockResolvedValue(() => undefined),
  }
}

describe('App-local session coordinator contracts', () => {
  it('exposes the shell and preview state shapes keyed by backend session', () => {
    expect(useAppShellState).toBeTypeOf('function')
    expect(usePreviewSession).toBeTypeOf('function')
    expect(useOtherFilePanelPreference).toBeTypeOf('function')
    expectTypeOf(useAppShellState).parameter(0).toEqualTypeOf<string>()
    expectTypeOf(usePreviewSession).parameter(0).toEqualTypeOf<string>()
    expectTypeOf(useOtherFilePanelPreference).parameter(0).toEqualTypeOf<string>()
    expectTypeOf<ReturnType<typeof useAppShellState>>().toMatchTypeOf<AppShellState>()
    expectTypeOf<ReturnType<typeof usePreviewSession>>().toMatchTypeOf<PreviewSessionState>()
    expectTypeOf<
      ReturnType<typeof useOtherFilePanelPreference>
    >().toMatchTypeOf<OtherFilePanelPreferenceState>()
    expectTypeOf<keyof ReturnType<typeof useAppShellState>>().toEqualTypeOf<keyof AppShellState>()
    expectTypeOf<keyof ReturnType<typeof usePreviewSession>>().toEqualTypeOf<
      keyof PreviewSessionState
    >()
    expectTypeOf<keyof ReturnType<typeof useOtherFilePanelPreference>>().toEqualTypeOf<
      keyof OtherFilePanelPreferenceState
    >()
  })

  it('exposes dialog validity and radial context reset inputs', () => {
    expect(useOperationDialogs).toBeTypeOf('function')
    expect(useRadialMenuSession).toBeTypeOf('function')
    expectTypeOf(useOperationDialogs).parameter(0).toEqualTypeOf<UseOperationDialogsOptions>()
    expectTypeOf(useRadialMenuSession).parameter(0).toEqualTypeOf<UseRadialMenuSessionOptions>()
    expectTypeOf<ReturnType<typeof useOperationDialogs>>().toMatchTypeOf<OperationDialogsState>()
    expectTypeOf<ReturnType<typeof useRadialMenuSession>>().toMatchTypeOf<RadialMenuSessionState>()
  })
})

describe('Viewer empty state', () => {
  it('asks the user to import one project folder', () => {
    render(<App />)
    expect(screen.getByRole('heading', { name: 'Viewer' })).toBeVisible()
    expect(screen.getByText('选择或拖入一个项目文件夹')).toBeVisible()
    expect(screen.queryByRole('button', { name: '软件设置' })).not.toBeInTheDocument()
  })

  it('consolidates project actions in the Viewer toolbar after opening', async () => {
    const viewer = bridge()
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    await screen.findByRole('heading', { name: 'Catalog' })

    const toolbar = await screen.findByRole('toolbar', { name: 'Viewer 工具栏' })
    expect(within(toolbar).getByRole('button', { name: /^筛选/ })).toBeVisible()
    expect(within(toolbar).getByRole('button', { name: '开始评审' })).toBeVisible()
    expect(within(toolbar).getByRole('button', { name: '视图' })).toBeVisible()
    expect(within(toolbar).getByRole('button', { name: '更多' })).toBeVisible()
    expect(within(toolbar).queryByRole('button', { name: '软件设置' })).not.toBeInTheDocument()
    expect(within(toolbar).queryByRole('button', { name: '项目菜单' })).not.toBeInTheDocument()
  })

  it('places ready continuous review controls inside the Viewer toolbar', async () => {
    const viewer = bridge()
    render(<App bridge={viewer} reviewWorkspacePort={reviewPort(workspace())} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))

    const toolbar = await screen.findByRole('toolbar', { name: 'Viewer 工具栏' })
    const reviewControls = await within(toolbar).findByRole('region', {
      name: '持续评审上下文',
    })

    expect(within(reviewControls).getByLabelText('当前意见 0')).toBeVisible()
    expect(within(reviewControls).getByRole('button', { name: '返回素材' })).toBeVisible()
    expect(within(reviewControls).getByRole('button', { name: '历史' })).toBeVisible()
    expect(within(reviewControls).getByRole('button', { name: '导入声明' })).toBeVisible()
    expect(within(reviewControls).getByRole('button', { name: '存档意见' })).toBeVisible()
    expect(screen.getAllByRole('region', { name: '持续评审上下文' })).toEqual([reviewControls])
  })

  it('routes a verified legacy project to explicit migration without exposing legacy completion', async () => {
    const viewer = bridge()
    const inspection = migrationInspection()
    const reviewWorkspace = reviewPort({
      ...workspace(),
      migration: inspection,
      capabilities: { continuousEditing: false, usageImport: false, migration: true },
    })
    render(<App bridge={viewer} reviewWorkspacePort={reviewWorkspace} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))

    expect(await screen.findByRole('dialog', { name: '迁移旧评审记录' })).toBeVisible()
    expect(screen.queryByText('持续评审为只读')).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '开始评审' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '完成本轮评审' })).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: '暂不迁移' }))
    expect(screen.queryByRole('dialog', { name: '迁移旧评审记录' })).not.toBeInTheDocument()
    expect(reviewWorkspace.prepareCommand).not.toHaveBeenCalled()
    expect(reviewWorkspace.applyCommand).not.toHaveBeenCalled()
  })

  it('opens persisted history from the typed production workspace view without presentation injection', async () => {
    const viewer = bridge()
    const selector: ReviewHistorySelector = { kind: 'archive', archiveId: 'archive-1' }
    const reviewWorkspace = reviewPort(workspaceWithHistorySelectors([selector]))
    vi.mocked(reviewWorkspace.getHistory).mockResolvedValue(emptyHistory(selector))

    render(<App bridge={viewer} reviewWorkspacePort={reviewWorkspace} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))

    const history = await screen.findByRole('button', { name: '历史' })
    expect(history).toBeEnabled()
    fireEvent.click(history)

    expect(await screen.findByRole('dialog', { name: '历史意见' })).toBeVisible()
    expect(reviewWorkspace.getHistory).toHaveBeenCalledWith({
      sessionId: 'session-1',
      generation: 1,
      selector,
    })
  })

  it('keeps persisted history disabled when the typed workspace catalog is empty', async () => {
    const viewer = bridge()
    const reviewWorkspace = reviewPort(workspaceWithHistorySelectors([]))

    render(<App bridge={viewer} reviewWorkspacePort={reviewWorkspace} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))

    expect(await screen.findByRole('button', { name: '历史' })).toBeDisabled()
    expect(reviewWorkspace.getHistory).not.toHaveBeenCalled()
  })

  it('requires an explicit visible choice and reads the exact selected history entry', async () => {
    const viewer = bridge()
    const archive: ReviewHistorySelector = { kind: 'archive', archiveId: 'archive-1' }
    const legacy: ReviewHistorySelector = { kind: 'legacy', roundId: 'round-2' }
    const reviewWorkspace = reviewPort(workspaceWithHistorySelectors([archive, legacy]))
    vi.mocked(reviewWorkspace.getHistory).mockResolvedValue(emptyHistory(legacy))

    render(<App bridge={viewer} reviewWorkspacePort={reviewWorkspace} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    fireEvent.click(await screen.findByRole('button', { name: '历史' }))

    const choice = await screen.findByRole('dialog', { name: '选择历史记录' })
    expect(within(choice).getByRole('radio', { name: '存档历史 1' })).toBeVisible()
    expect(within(choice).getByRole('radio', { name: '旧评审记录 2' })).toBeVisible()
    expect(reviewWorkspace.getHistory).not.toHaveBeenCalled()

    fireEvent.click(within(choice).getByRole('radio', { name: '旧评审记录 2' }))
    fireEvent.click(within(choice).getByRole('button', { name: '查看所选历史' }))

    expect(await screen.findByRole('dialog', { name: '历史意见' })).toBeVisible()
    expect(reviewWorkspace.getHistory).toHaveBeenCalledOnce()
    expect(reviewWorkspace.getHistory).toHaveBeenCalledWith({
      sessionId: 'session-1',
      generation: 1,
      selector: legacy,
    })
  })

  it('keeps explicit migration available when migration preparation is interrupted', async () => {
    const viewer = bridge()
    const reviewWorkspace = reviewPort({
      ...workspace(),
      migration: migrationInspection(),
      capabilities: { continuousEditing: false, usageImport: false, migration: true },
    })
    vi.mocked(reviewWorkspace.prepareCommand).mockRejectedValue({
      code: 'io',
      message: 'migration preparation interrupted',
      retryable: true,
      committedReceipt: null,
    })
    render(<App bridge={viewer} reviewWorkspacePort={reviewWorkspace} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))

    fireEvent.click(
      within(await screen.findByRole('dialog', { name: '迁移旧评审记录' })).getByRole('button', {
        name: '仅保留历史',
      }),
    )

    expect(await screen.findByText('migration preparation interrupted')).toBeVisible()
    expect(screen.getByRole('dialog', { name: '迁移旧评审记录' })).toBeVisible()
    expect(reviewWorkspace.applyCommand).not.toHaveBeenCalled()
  })

  it('routes every writable image through the verified continuous adapter without starting a legacy round', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    const reviewWorkspace = reviewPort(workspace('snapshot-1'))
    vi.mocked(reviewWorkspace.prepareAssets).mockResolvedValue([
      {
        asset: {
          id: 'asset-1',
          sourceEntityId: 'image-1',
          relativePath: 'front.jpg',
          evidence: { sizeBytes: 10, modifiedNs: '1', blake3: '12'.repeat(32) },
          media: { kind: 'image', width: 640, height: 480 },
          producerAssetId: null,
          parentAssetVersionId: null,
        },
        preview: null,
      },
    ])
    render(<App bridge={viewer} reviewWorkspacePort={reviewWorkspace} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    fireEvent.doubleClick(await screen.findByRole('option', { name: 'front.jpg' }))

    await waitFor(() =>
      expect(reviewWorkspace.prepareAssets).toHaveBeenCalledWith({
        sessionId: 'session-1',
        generation: 1,
        entityIds: ['image-1'],
      }),
    )
    expect(await screen.findByRole('toolbar', { name: '图片评审工具' })).toBeVisible()
    expect(screen.queryByRole('button', { name: '开始评审' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '完成本轮评审' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '放弃本轮' })).not.toBeInTheDocument()
    expect(viewer.reviewPreviewStart).not.toHaveBeenCalled()
  })

  it('hydrates four saved continuous targets into the fresh App workbench rail and geometry', async () => {
    installPreviewStageBounds()
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    const current = continuousWorkspaceWithFeedback(4, 'snapshot-restarted')
    if (current.current === null) throw new Error('Expected current continuous review fixture')
    const anchors = [
      { kind: 'image_rect' as const, x: 0.1, y: 0.1, width: 0.2, height: 0.2 },
      { kind: 'image_rect' as const, x: 0.5, y: 0.1, width: 0.2, height: 0.2 },
      {
        kind: 'image_stroke' as const,
        points: [
          { x: 0.2, y: 0.6 },
          { x: 0.4, y: 0.7 },
        ],
      },
      {
        kind: 'image_stroke' as const,
        points: [
          { x: 0.6, y: 0.6 },
          { x: 0.8, y: 0.7 },
        ],
      },
    ]
    current.current.state.feedback.forEach((feedback, index) => {
      const target = feedback.targets[0]
      const anchor = anchors[index]
      if (target === undefined || anchor === undefined) throw new Error('Missing feedback fixture')
      target.anchor = anchor
    })
    const reviewWorkspace = reviewPort(current)
    vi.mocked(reviewWorkspace.prepareAssets).mockResolvedValue([preparedFrontAsset()])

    render(<App bridge={viewer} reviewWorkspacePort={reviewWorkspace} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    fireEvent.doubleClick(await screen.findByRole('option', { name: 'front.jpg' }))

    const rail = await screen.findByRole('complementary', { name: '评审意见' })
    expect(rail).toHaveTextContent('4 条')
    expect(within(rail).getByRole('button', { name: '选择意见 1：持续意见 1' })).toBeVisible()
    expect(within(rail).getByRole('button', { name: '选择意见 4：持续意见 4' })).toBeVisible()
    expect(await screen.findAllByTestId('annotation-marker')).toHaveLength(4)
  })

  it('does not silently bind saved targets to a different prepared asset version', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    const reviewWorkspace = reviewPort(continuousWorkspaceWithFeedback(4, 'snapshot-restarted'))
    const replacement = preparedFrontAsset()
    replacement.asset.id = 'asset-replaced'
    replacement.asset.evidence.blake3 = '34'.repeat(32)
    replacement.preview.assetVersionId = 'asset-replaced'
    replacement.preview.url = 'viewer-review-image://localhost/asset-replaced'
    vi.mocked(reviewWorkspace.prepareAssets).mockResolvedValue([replacement])

    render(<App bridge={viewer} reviewWorkspacePort={reviewWorkspace} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    fireEvent.doubleClick(await screen.findByRole('option', { name: 'front.jpg' }))

    const rail = await screen.findByRole('complementary', { name: '评审意见' })
    expect(rail).toHaveTextContent('0 条')
    expect(screen.queryByTestId('annotation-marker')).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: '画笔' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '矩形' })).toBeDisabled()
    expect(screen.getByText('素材来源或版本需要确认，刷新确认后可继续评审。')).toBeVisible()
  })

  it('keeps mixed referenced asset versions fail-closed when opening the prepared current version', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    const current = continuousWorkspaceWithFeedback(4, 'snapshot-mixed-versions')
    if (current.current === null) throw new Error('Expected current continuous review fixture')
    const replacement = preparedFrontAsset()
    replacement.asset.id = 'asset-replaced'
    replacement.asset.evidence.blake3 = '34'.repeat(32)
    replacement.preview.assetVersionId = 'asset-replaced'
    replacement.preview.url = 'viewer-review-image://localhost/asset-replaced'
    current.current.state.assets.push(replacement.asset)
    const reviewWorkspace = reviewPort(current)
    vi.mocked(reviewWorkspace.prepareAssets).mockResolvedValue([replacement])

    render(<App bridge={viewer} reviewWorkspacePort={reviewWorkspace} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    fireEvent.doubleClick(await screen.findByRole('option', { name: 'front.jpg' }))

    const rail = await screen.findByRole('complementary', { name: '评审意见' })
    expect(rail).toHaveTextContent('0 条')
    expect(screen.queryByTestId('annotation-marker')).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: '画笔' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '矩形' })).toBeDisabled()
    expect(screen.getByText('素材来源或版本需要确认，刷新确认后可继续评审。')).toBeVisible()
  })

  it('uses only the continuous current snapshot as the visible feedback-count owner', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    vi.mocked(viewer.reviewStatus).mockResolvedValue(legacySnapshotWithFeedback(2))
    const reviewWorkspace = reviewPort(continuousWorkspaceWithFeedback(1, 'snapshot-1'))

    render(<App bridge={viewer} reviewWorkspacePort={reviewWorkspace} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))

    expect(await screen.findByText('返工 · 1 条')).toBeVisible()
    expect(screen.queryByText('返工 · 2 条')).not.toBeInTheDocument()
    expect(viewer.reviewStatus).not.toHaveBeenCalled()
  })

  it('retains legacy count ownership when no continuous desktop boundary is present', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    vi.mocked(viewer.reviewStatus).mockResolvedValue(legacySnapshotWithFeedback(2))

    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))

    expect(await screen.findByText('返工 · 2 条')).toBeVisible()
    expect(viewer.reviewStatus).toHaveBeenCalled()
  })

  it('updates the grid count from a continuous image save without starting legacy ownership', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    const reviewWorkspace = reviewPort(continuousWorkspaceWithFeedback(0, 'snapshot-1'))
    vi.mocked(reviewWorkspace.prepareAssets).mockResolvedValue([preparedFrontAsset()])
    vi.mocked(reviewWorkspace.applyCommand).mockImplementation(async ({ envelope }) => ({
      receipt: {
        commandId: envelope.commandId,
        payloadDigest: envelope.payloadDigest,
        snapshot: { snapshotId: 'snapshot-2', blake3: '56'.repeat(32) },
      },
      view: continuousWorkspaceWithFeedback(1, 'snapshot-2'),
    }))

    render(<App bridge={viewer} reviewWorkspacePort={reviewWorkspace} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    fireEvent.doubleClick(await screen.findByRole('option', { name: 'front.jpg' }))
    const wholeImageFeedback = await screen.findByRole('button', { name: '整图意见' })
    await waitFor(() => expect(wholeImageFeedback).toBeEnabled())
    fireEvent.click(wholeImageFeedback)
    fireEvent.change(screen.getByRole('textbox', { name: '标注意见' }), {
      target: { value: '持续评审意见' },
    })
    fireEvent.click(
      within(screen.getByRole('region', { name: '整图意见编辑器' })).getByRole('button', {
        name: '保存',
      }),
    )

    await waitFor(() => expect(reviewWorkspace.applyCommand).toHaveBeenCalledOnce())
    fireEvent.click(screen.getByRole('button', { name: '返回网格' }))
    expect(await screen.findByText('返工 · 1 条')).toBeVisible()
    expect(viewer.reviewStatus).not.toHaveBeenCalled()
  })

  it('does not leak legacy counts when switching from a current project to a migration project', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    vi.mocked(viewer.reviewStatus).mockResolvedValue(legacySnapshotWithFeedback(2))
    vi.mocked(viewer.openProject)
      .mockResolvedValueOnce({
        projectId: 'project-new',
        sessionId: 'session-new',
        generation: 1,
        displayName: 'New project',
        access: 'read_write',
      })
      .mockResolvedValueOnce({
        projectId: 'project-old',
        sessionId: 'session-old',
        generation: 2,
        displayName: 'Old project',
        access: 'read_write',
      })
    const current = continuousWorkspaceWithFeedback(1, 'snapshot-new')
    const migration = {
      ...workspace(),
      migration: migrationInspection(),
      capabilities: { continuousEditing: false, usageImport: false, migration: true },
    }
    const reviewWorkspace = reviewPort()
    vi.mocked(reviewWorkspace.getWorkspace).mockImplementation(async ({ sessionId }) =>
      sessionId === 'session-new' ? current : migration,
    )

    render(<App bridge={viewer} reviewWorkspacePort={reviewWorkspace} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    expect(await screen.findByText('返工 · 1 条')).toBeVisible()

    closeProjectFromMenu()
    await screen.findByRole('button', { name: '选择项目文件夹' })
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))

    expect(await screen.findByRole('dialog', { name: '迁移旧评审记录' })).toBeVisible()
    expect(screen.queryByText('返工 · 1 条')).not.toBeInTheDocument()
    expect(screen.queryByText('返工 · 2 条')).not.toBeInTheDocument()
    expect(viewer.reviewStatus).not.toHaveBeenCalled()
  })

  it('blocks all review writes when the verified continuous project is read-only', async () => {
    const viewer = bridge('read_only')
    const readOnlyView = workspace('snapshot-1')
    readOnlyView.capabilities = { continuousEditing: false, usageImport: false, migration: false }
    render(<App bridge={viewer} reviewWorkspacePort={reviewPort(readOnlyView)} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))

    expect(await screen.findByText('持续评审为只读')).toBeVisible()
    expect(screen.queryByRole('button', { name: '开始评审' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '导入声明' })).not.toBeInTheDocument()
  })

  it('does not fall back to legacy writes for an unknown future review protocol', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    const reviewWorkspace = reviewPort()
    vi.mocked(reviewWorkspace.getWorkspace).mockRejectedValue({
      code: 'unsupported_protocol',
      message: 'viewer.review/99 is not supported',
      retryable: false,
      committedReceipt: null,
    })
    render(<App bridge={viewer} reviewWorkspacePort={reviewWorkspace} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))

    expect(await screen.findByText('评审协议版本暂不支持')).toBeVisible()
    expect(screen.queryByRole('button', { name: '开始评审' })).not.toBeInTheDocument()
    fireEvent.doubleClick(screen.getByRole('option', { name: 'front.jpg' }))
    expect(await screen.findByRole('dialog', { name: '图片预览 front.jpg' })).toBeVisible()
    expect(screen.queryByRole('toolbar', { name: '图片评审工具' })).not.toBeInTheDocument()
    expect(reviewWorkspace.prepareAssets).not.toHaveBeenCalled()
    expect(viewer.reviewPreviewStart).not.toHaveBeenCalled()
  })

  it('previews a selected fixed review scope through the typed bridge', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    vi.mocked(viewer.reviewPreviewStart).mockResolvedValue({
      proposalId: 7,
      resolution: { candidateCount: 1, imageCount: 1, videoCount: 0, excludedCount: 0 },
    })
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    fireEvent.click(await screen.findByRole('option', { name: 'front.jpg' }))

    fireEvent.click(screen.getByRole('button', { name: '开始评审' }))

    await waitFor(() =>
      expect(viewer.reviewPreviewStart).toHaveBeenCalledWith({
        sessionId: 'session-1',
        generation: 1,
        scope: { kind: 'selection', entityIds: ['image-1'] },
      }),
    )
    expect(await screen.findByRole('dialog', { name: '确认本轮评审范围' })).toBeVisible()
  })

  it('treats folder navigation as a folder scope instead of a selected material', async () => {
    const viewer = bridge()
    vi.mocked(viewer.folderTree).mockResolvedValue([
      {
        entityId: 'folder-1',
        parentEntityId: null,
        relativePath: 'folder-1',
        name: 'folder-1',
        marker: { reviewState: null, favorite: false },
      },
    ])
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    vi.mocked(viewer.reviewPreviewStart).mockResolvedValue({
      proposalId: 8,
      resolution: { candidateCount: 1, imageCount: 1, videoCount: 0, excludedCount: 0 },
    })
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    fireEvent.click(await screen.findByRole('treeitem', { name: 'folder-1' }))

    fireEvent.click(screen.getByRole('button', { name: '开始评审' }))

    await waitFor(() =>
      expect(viewer.reviewPreviewStart).toHaveBeenCalledWith({
        sessionId: 'session-1',
        generation: 1,
        scope: { kind: 'folder', folderId: 'folder-1', includeDescendants: false },
      }),
    )
  })

  it('guards project close while a review opinion is nonempty and unsaved', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    vi.mocked(viewer.reviewStatus).mockResolvedValue({
      phase: 'active',
      resume: null,
      reviewStreamId: 'stream-1',
      reviewRoundId: 'round-1',
      revision: 1,
      members: [
        {
          assetVersionId: 'asset-1',
          entityId: 'image-1',
          relativePath: 'id/front.jpg',
          displayName: 'front.jpg',
          kind: 'image',
          feedbackItems: 0,
        },
      ],
      feedback: [],
      restorableFeedbackId: null,
      unreviewable: [],
      conflicts: [],
      counts: { total: 1, feedbackItems: 0, revise: 0, unreviewable: 0, pass: 0 },
      error: null,
    })
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    fireEvent.click(await screen.findByRole('option', { name: 'front.jpg' }))
    fireEvent.click(await screen.findByRole('button', { name: '评审意见' }))
    fireEvent.change(screen.getByRole('textbox', { name: '返工意见' }), {
      target: { value: '保留这条未保存意见' },
    })

    closeProjectFromMenu()

    expect(screen.getByRole('dialog', { name: '放弃未保存的意见？' })).toBeVisible()
    expect(viewer.closeProject).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: '放弃未保存内容' }))
    await waitFor(() => expect(viewer.closeProject).toHaveBeenCalledOnce())
  })

  it('routes active-round member images to the workbench and nonmembers to fixed-scope preview', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(twoImageContentWorkspace())
    vi.mocked(viewer.reviewStatus).mockResolvedValue(activeImageReviewSnapshot())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    fireEvent.doubleClick(await screen.findByRole('option', { name: 'front.jpg' }))

    expect(await screen.findByRole('toolbar', { name: '图片评审工具' })).toBeVisible()
    const workbench = screen.getByRole('dialog', { name: '图片评审 front.jpg' })
    fireEvent.keyDown(workbench, { key: 'ArrowRight' })

    expect(await screen.findByText('这张图片不在当前评审范围内')).toBeVisible()
    expect(screen.queryByRole('toolbar', { name: '图片评审工具' })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: '返回本轮素材' })).toBeVisible()
    expect(screen.getByRole('button', { name: '完成当前评审' })).toBeVisible()
    expect(screen.getByRole('button', { name: '放弃当前草稿后重新开始' })).toBeVisible()
  })

  it('uses the image annotation leave guard before project close', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(twoImageContentWorkspace())
    vi.mocked(viewer.reviewStatus).mockResolvedValue(activeImageReviewSnapshot())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    fireEvent.doubleClick(await screen.findByRole('option', { name: 'front.jpg' }))
    fireEvent.click(await screen.findByRole('button', { name: '整图意见' }))

    closeProjectFromMenu()

    expect(screen.getByRole('dialog', { name: '放弃未保存的标注？' })).toBeVisible()
    expect(viewer.closeProject).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: '放弃未保存内容' }))
    await waitFor(() => expect(viewer.closeProject).toHaveBeenCalledOnce())
  })

  it('renders a formal empty state when the selected folder has no supported files', async () => {
    const viewer = bridge()
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))

    expect(
      await screen.findByRole('heading', { name: '这个项目中还没有可显示的文件' }),
    ).toBeVisible()
    expect(screen.getByText('Viewer 会显示支持的图片、视频、Markdown 与文本文件。')).toBeVisible()
    fireEvent.click(screen.getByRole('button', { name: '在文件管理器中显示' }))
    expect(viewer.revealProjectInFileManager).toHaveBeenCalledOnce()
    expect(screen.queryByText('此文件夹中没有支持的文件。')).not.toBeInTheDocument()
  })

  it('keeps the approved recovery summary in the workspace until the user continues', async () => {
    const viewer = bridge()
    vi.mocked(viewer.openProject).mockResolvedValue({
      projectId: 'project-1',
      sessionId: 'session-1',
      generation: 1,
      displayName: 'Catalog',
      access: 'read_write',
      recoveryReport: { recovered: 8, needsUserReview: 1 },
    })
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    render(<App bridge={viewer} />)

    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))

    expect(await screen.findByRole('heading', { name: '项目状态已恢复' })).toBeVisible()
    expect(screen.getByText('8 项操作已经恢复，1 项需要检查。')).toBeVisible()
    expect(screen.queryByRole('option', { name: 'front.jpg' })).not.toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: '继续浏览项目' }))
    await waitFor(() => expect(screen.getByRole('option', { name: 'front.jpg' })).toBeVisible())
    expect(screen.queryByRole('heading', { name: '项目状态已恢复' })).not.toBeInTheDocument()
  })

  it('keeps the project-root row out of the approved launch loading skeleton', async () => {
    const viewer = bridge()
    const workspace = deferred<Awaited<ReturnType<ViewerBridge['queryFolder']>>>()
    vi.mocked(viewer.queryFolder).mockImplementation(() => workspace.promise)
    render(<App bridge={viewer} />)

    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    await screen.findByRole('heading', { name: 'Catalog' })

    const sidebar = screen.getByRole('complementary', { name: '文件夹栏' })
    expect(within(sidebar).queryByRole('button', { name: 'Catalog' })).not.toBeInTheDocument()
    expect(sidebar.querySelectorAll('.folder-tree-skeleton-row')).toHaveLength(10)

    await act(async () => {
      workspace.resolve({ workspace: 'empty' })
      await Promise.resolve()
    })
    expect(within(sidebar).getByRole('button', { name: 'Catalog' })).toBeVisible()
  })

  it('keeps the project structure and item placeholders visible while thumbnails are pending', async () => {
    const viewer = bridge()
    const image = deferred<Awaited<ReturnType<ViewerBridge['requestImage']>>>()
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    vi.mocked(viewer.requestImage).mockReturnValue(image.promise)
    render(<App bridge={viewer} />)

    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))

    await waitFor(() => expect(viewer.requestImage).toHaveBeenCalled())
    expect(screen.queryByRole('status', { name: '项目内容加载中' })).not.toBeInTheDocument()
    const sidebar = screen.getByRole('complementary', { name: '文件夹栏' })
    expect(within(sidebar).getByRole('button', { name: 'Catalog' })).toBeVisible()
    expect(sidebar.querySelectorAll('.folder-tree-skeleton-row')).toHaveLength(0)
    expect(screen.getByRole('option', { name: 'front.jpg' })).toBeVisible()
    expect(screen.getAllByLabelText('缩略图加载中').length).toBeGreaterThan(0)
    await waitFor(() => expect(screen.getByText('正在生成缩略图')).toBeVisible())

    await act(async () => {
      image.resolve({
        cacheKey: 'test-image',
        url: 'viewer-image://localhost/session/test-image',
        width: 800,
        height: 600,
        backend: 'quick_look',
      })
      await image.promise
    })

    await waitFor(() =>
      expect(screen.queryByRole('status', { name: '项目内容加载中' })).not.toBeInTheDocument(),
    )
  })

  it('selects the target immediately and delays non-blocking folder progress for 120 ms', async () => {
    const viewer = bridge()
    vi.mocked(viewer.folderTree).mockResolvedValue([
      {
        entityId: 'folder-b',
        parentEntityId: null,
        relativePath: 'folder-b',
        name: 'folder-b',
        marker: { reviewState: null, favorite: false },
      },
    ])
    const folderB = deferred<Awaited<ReturnType<ViewerBridge['queryFolder']>>>()
    vi.mocked(viewer.queryFolder)
      .mockResolvedValueOnce(contentWorkspace())
      .mockImplementation((entityId) =>
        entityId === 'folder-b' ? folderB.promise : Promise.resolve(contentWorkspace()),
      )
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    await screen.findByRole('option', { name: 'front.jpg' })
    vi.useFakeTimers()

    fireEvent.click(screen.getByRole('treeitem', { name: 'folder-b' }))

    expect(screen.getByRole('treeitem', { name: 'folder-b' })).toHaveAttribute(
      'aria-selected',
      'true',
    )
    expect(screen.getByRole('option', { name: 'front.jpg' })).toBeVisible()
    act(() => vi.advanceTimersByTime(119))
    expect(screen.queryByRole('progressbar', { name: '正在切换文件夹' })).not.toBeInTheDocument()
    act(() => vi.advanceTimersByTime(1))
    expect(screen.getByRole('progressbar', { name: '正在切换文件夹' })).toBeVisible()

    const nextWorkspace = contentWorkspace()
    nextWorkspace.images = nextWorkspace.images.map((file) => ({
      ...file,
      entityId: 'image-b',
      relativePath: 'folder-b/folder-b.jpg',
      name: 'folder-b.jpg',
    }))
    await act(async () => {
      folderB.resolve(nextWorkspace)
      await Promise.resolve()
      await Promise.resolve()
    })

    expect(screen.getByRole('option', { name: 'folder-b.jpg' })).toBeVisible()
    expect(screen.queryByRole('option', { name: 'front.jpg' })).not.toBeInTheDocument()
    expect(screen.queryByRole('progressbar', { name: '正在切换文件夹' })).not.toBeInTheDocument()
  })

  it('routes contextual content selection through the shared View menu', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(mixedContentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))

    const toolbar = await screen.findByRole('toolbar', { name: 'Viewer 工具栏' })
    fireEvent.click(within(toolbar).getByRole('button', { name: '视图' }))
    fireEvent.click(within(toolbar).getByRole('button', { name: '全选全部文件' }))

    expect(screen.getByRole('option', { name: 'front.jpg' })).toHaveAttribute(
      'aria-selected',
      'true',
    )
    fireEvent.click(screen.getByRole('button', { name: /其它文件 · 1 · 已选 1/ }))
    expect(screen.getByRole('option', { name: 'root-notes.txt' })).toHaveAttribute(
      'aria-selected',
      'true',
    )
  })

  it('shows aggregate browsing as a formal status tag after enabling descendants', async () => {
    const viewer = bridge()
    vi.mocked(viewer.folderTree).mockResolvedValue([
      {
        entityId: 'folder-a',
        parentEntityId: null,
        relativePath: 'id',
        name: 'id',
        marker: { reviewState: null, favorite: false },
      },
    ])
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))

    fireEvent.click(await screen.findByRole('treeitem', { name: 'id' }))
    await waitFor(() => expect(viewer.queryFolder).toHaveBeenCalledWith('folder-a', false))

    const toolbar = await screen.findByRole('toolbar', { name: 'Viewer 工具栏' })
    fireEvent.click(within(toolbar).getByRole('button', { name: '视图' }))
    fireEvent.click(within(toolbar).getByRole('button', { name: '显示全部后代文件' }))

    const aggregate = await screen.findByText('全部后代文件')
    expect(aggregate).toHaveClass('viewer-status-tag')
    expect(aggregate).toHaveAttribute('data-tone', 'info')
    expect(viewer.queryFolder).toHaveBeenLastCalledWith('folder-a', true)
  })

  it('opens the shared View menu for Command-A in a mixed folder', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(mixedContentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))

    const grid = await screen.findByRole('listbox', { name: '图片文件' })
    fireEvent.keyDown(grid, { key: 'a', metaKey: true })

    expect(screen.getByRole('button', { name: '全选图片' })).toBeVisible()
    expect(screen.getByRole('button', { name: '全选其它文件' })).toBeVisible()
    expect(screen.getByRole('button', { name: '全选全部文件' })).toBeVisible()
  })

  it('updates thumbnail size optimistically and restores trigger focus when the dialog closes', async () => {
    const viewer = bridge()
    const save = deferred<Awaited<ReturnType<ViewerBridge['updateViewerSettings']>>>()
    vi.mocked(viewer.updateViewerSettings).mockReturnValue(save.promise)
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const trigger = await screen.findByRole('button', { name: '更多' })
    trigger.focus()
    openSettingsFromMenu()

    const slider = screen.getByRole('slider', { name: '缩略图大小' })
    fireEvent.change(slider, { target: { value: '3' } })
    expect(slider).toHaveValue('3')
    fireEvent.click(screen.getByRole('button', { name: '关闭' }))

    expect(screen.queryByRole('dialog', { name: '软件设置' })).not.toBeInTheDocument()
    expect(trigger).toHaveFocus()
  })

  it('restores focus to More after opening settings from its focused command', async () => {
    const viewer = bridge()
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))

    const more = await screen.findByRole('button', { name: '更多' })
    fireEvent.click(more)
    const settings = screen.getByRole('button', { name: '软件设置' })
    settings.focus()
    fireEvent.click(settings)
    fireEvent.click(screen.getByRole('button', { name: '关闭' }))

    expect(more).toHaveFocus()
  })

  it('keeps the sidebar collapse control in the aligned project header and uses a 52px rail', async () => {
    const viewer = bridge()
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))

    const projectHeading = await screen.findByRole('heading', { name: 'Catalog' })
    const projectIdentity = projectHeading.parentElement
    expect(projectIdentity).not.toBeNull()
    const collapse = within(projectIdentity as HTMLElement).getByRole('button', {
      name: '折叠文件夹栏',
    })
    expect(collapse.querySelector('img')).toHaveAttribute('aria-hidden', 'true')
    expect(collapse).not.toHaveTextContent('收起')
    const sidebar = screen.getByRole('complementary', { name: '文件夹栏' })

    expect(within(sidebar).getByText('项目目录')).toBeVisible()
    expect(within(sidebar).getByRole('button', { name: 'Catalog' })).toHaveAttribute(
      'aria-pressed',
      'true',
    )
    expect(within(sidebar).queryByRole('button', { name: '项目根目录' })).not.toBeInTheDocument()

    fireEvent.click(collapse)

    expect(sidebar).toHaveStyle({ width: '52px' })
    expect(sidebar).toHaveAttribute('data-collapsed', 'true')
    expect(within(sidebar).getByText('项目目录')).toBeVisible()
    expect(within(sidebar).getByRole('button', { name: 'Catalog' })).toBeVisible()
    expect(projectIdentity).not.toHaveTextContent('Catalog')
    const expand = within(projectIdentity as HTMLElement).getByRole('button', {
      name: '展开文件夹栏',
    })
    expect(expand).toBeVisible()
    expect(expand.querySelector('img')).toHaveAttribute('aria-hidden', 'true')
    expect(expand).not.toHaveTextContent('展开')
  })

  it('keeps compact toolbar popovers reachable and viewport-contained at the supported narrow width', async () => {
    vi.stubGlobal('innerWidth', 500)
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))

    const sidebar = await screen.findByRole('complementary', { name: '文件夹栏' })
    expect(sidebar).toHaveStyle({ width: '52px' })
    const projectIdentity = document.querySelector<HTMLElement>('.project-identity')
    expect(projectIdentity).not.toBeNull()
    expect(
      within(projectIdentity as HTMLElement).getByRole('button', {
        name: '窄窗口中已折叠文件夹栏',
      }),
    ).toBeDisabled()
    await screen.findByRole('option', { name: 'front.jpg' })

    const toolbar = screen.getByRole('toolbar', { name: 'Viewer 工具栏' })
    const view = within(toolbar).getByRole('button', { name: '视图' })
    fireEvent.click(view)
    const viewPopover = view
      .closest('details')
      ?.querySelector<HTMLElement>('.workspace-menu-popover')
    expect(viewPopover).toBeVisible()
    expect(viewPopover).toHaveStyle({ maxWidth: '484px' })
    expect(within(toolbar).getByRole('button', { name: '全选图片' })).toBeVisible()
    fireEvent.click(view)

    const more = within(toolbar).getByRole('button', { name: '更多' })
    fireEvent.click(more)
    const morePopover = more
      .closest('details')
      ?.querySelector<HTMLElement>('.workspace-menu-popover')
    expect(morePopover).toBeVisible()
    expect(morePopover).toHaveStyle({ maxWidth: '484px' })
    expect(within(toolbar).getByRole('button', { name: '软件设置' })).toBeVisible()
  })

  it('keeps filter, View, and More popovers mutually exclusive', async () => {
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => undefined)
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))

    const toolbar = await screen.findByRole('toolbar', { name: 'Viewer 工具栏' })
    const view = within(toolbar).getByRole('button', { name: '视图' })
    const filter = within(toolbar).getByRole('button', { name: '筛选' })
    const more = within(toolbar).getByRole('button', { name: '更多' })

    fireEvent.click(view)
    expect(view).toHaveAttribute('aria-expanded', 'true')
    fireEvent.click(filter)
    expect(view).toHaveAttribute('aria-expanded', 'false')
    expect(filter).toHaveAttribute('aria-expanded', 'true')
    fireEvent.click(more)
    expect(filter).toHaveAttribute('aria-expanded', 'false')
    expect(more).toHaveAttribute('aria-expanded', 'true')
    expect(
      consoleError.mock.calls.some((call) =>
        call.some((value) =>
          String(value).includes('Cannot update a component while rendering a different component'),
        ),
      ),
    ).toBe(false)
    consoleError.mockRestore()
  })

  it('shows the latest settings save failure and rolls back the thumbnail slider', async () => {
    const viewer = bridge()
    const save = deferred<Awaited<ReturnType<ViewerBridge['updateViewerSettings']>>>()
    vi.mocked(viewer.updateViewerSettings).mockReturnValue(save.promise)
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    await screen.findByRole('button', { name: '更多' })
    openSettingsFromMenu()
    fireEvent.change(screen.getByRole('slider', { name: '缩略图大小' }), {
      target: { value: '3' },
    })
    await waitFor(() =>
      expect(viewer.updateViewerSettings).toHaveBeenCalledWith({
        thumbnailDensity: 'large',
        magnifier: { shape: 'circle', magnification: 2, area: 'small' },
      }),
    )

    save.reject({ userMessage: '设置未能保存' })

    expect(await screen.findByRole('alert')).toHaveTextContent('设置未能保存')
    expect(screen.getByRole('slider', { name: '缩略图大小' })).toHaveValue('2')
  })

  it('moves close-project into the compact project menu', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const file = await screen.findByRole('option', { name: 'front.jpg' })
    expect(screen.queryByRole('button', { name: '关闭项目' })).not.toBeInTheDocument()
    openRadialMenu(file, 105)
    expect(await screen.findByRole('menu', { name: '文件操作' })).toBeVisible()
    closeProjectFromMenu()
    await waitFor(() => expect(viewer.closeProject).toHaveBeenCalled())
    expect(screen.queryByRole('menu', { name: '文件操作' })).not.toBeInTheDocument()
  })

  it('keeps the mixed text shelf preference while navigating inside one project session', async () => {
    const viewer = bridge()
    vi.mocked(viewer.folderTree).mockResolvedValue([
      {
        entityId: 'folder-b',
        parentEntityId: null,
        relativePath: 'folder-b',
        name: 'folder-b',
        marker: { reviewState: null, favorite: false },
      },
    ])
    vi.mocked(viewer.queryFolder).mockImplementation(async (entityId) =>
      entityId === 'folder-b'
        ? mixedContentWorkspace({
            entityId: 'text-folder-b',
            relativePath: 'folder-b/folder-notes.txt',
            name: 'folder-notes.txt',
          })
        : mixedContentWorkspace(),
    )
    render(<App bridge={viewer} />)

    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const disclosure = await screen.findByRole('button', { name: /其它文件 · 1/ })
    expect(disclosure).toHaveAttribute('aria-expanded', 'false')
    fireEvent.click(disclosure)
    expect(disclosure).toHaveAttribute('aria-expanded', 'true')

    fireEvent.click(await screen.findByRole('treeitem', { name: 'folder-b' }))
    await waitFor(() => expect(viewer.queryFolder).toHaveBeenCalledWith('folder-b', false))
    expect(await screen.findByText('folder-b/folder-notes.txt')).toBeVisible()
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /其它文件 · 1/ })).toHaveAttribute(
        'aria-expanded',
        'true',
      ),
    )
  })

  it('exposes read-only status in the keyboard-operable project menu and keeps the banner', async () => {
    const viewer = bridge('read_only')
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    expect(await screen.findByRole('status', { name: '只读模式' })).toBeVisible()
    const projectMenu = screen.getByRole('button', { name: '更多' })
    expect(projectMenu).toHaveAttribute('aria-expanded', 'false')

    fireEvent.keyDown(projectMenu, { key: 'Enter' })
    expect(projectMenu).toHaveAttribute('aria-expanded', 'true')
    const accessStatus = screen.getByRole('button', { name: /访问权限/ })
    expect(accessStatus).toBeDisabled()
    expect(accessStatus).toHaveTextContent('只读')
    expect(accessStatus).toHaveTextContent('修改命令已停用')
    fireEvent.keyDown(projectMenu, { key: ' ' })
    expect(projectMenu).toHaveAttribute('aria-expanded', 'false')
    expect(screen.queryByRole('button', { name: /访问权限/ })).not.toBeInTheDocument()
    expect(screen.getByRole('status', { name: '只读模式' })).toBeVisible()
  })

  it('resizes the sidebar through pointer and keyboard separator input', async () => {
    const viewer = bridge()
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    await screen.findByRole('heading', { name: 'Catalog' })
    const separator = screen.getByRole('separator', { name: '调整文件夹栏宽度' })
    expect(separator).toHaveAttribute('aria-valuemin', '200')
    expect(separator).toHaveAttribute('aria-valuemax', '420')
    expect(separator).toHaveAttribute('aria-valuenow', '220')
    fireEvent.pointerDown(separator, { pointerId: 101, button: 0, clientX: 220 })
    fireEvent.pointerMove(window, { pointerId: 101, clientX: 300 })
    fireEvent.pointerUp(window, { pointerId: 101, clientX: 300 })
    expect(screen.getByLabelText('文件夹栏')).toHaveStyle({ width: '300px' })
    expect(separator).toHaveAttribute('aria-valuenow', '300')
    fireEvent.keyDown(separator, { key: 'ArrowLeft' })
    expect(screen.getByLabelText('文件夹栏')).toHaveStyle({ width: '284px' })
    expect(separator).toHaveAttribute('aria-valuenow', '284')
    fireEvent.keyDown(separator, { key: 'ArrowRight' })
    expect(screen.getByLabelText('文件夹栏')).toHaveStyle({ width: '300px' })
    expect(separator).toHaveAttribute('aria-valuenow', '300')

    fireEvent.pointerDown(separator, { pointerId: 102, button: 0, clientX: 300 })
    fireEvent.pointerMove(window, { pointerId: 102, clientX: -1000 })
    fireEvent.pointerUp(window, { pointerId: 102, clientX: -1000 })
    fireEvent.keyDown(separator, { key: 'ArrowLeft' })
    expect(screen.getByLabelText('文件夹栏')).toHaveStyle({ width: '200px' })
    expect(separator).toHaveAttribute('aria-valuenow', '200')

    fireEvent.pointerDown(separator, { pointerId: 103, button: 0, clientX: 200 })
    fireEvent.pointerMove(window, { pointerId: 103, clientX: 1000 })
    fireEvent.pointerUp(window, { pointerId: 103, clientX: 1000 })
    fireEvent.keyDown(separator, { key: 'ArrowRight' })
    expect(screen.getByLabelText('文件夹栏')).toHaveStyle({ width: '420px' })
    expect(separator).toHaveAttribute('aria-valuenow', '420')
  })

  it('stops sidebar resize tracking when the pointer is cancelled', async () => {
    const viewer = bridge()
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    await screen.findByRole('heading', { name: 'Catalog' })
    const sidebar = screen.getByLabelText('文件夹栏')
    const separator = screen.getByRole('separator', { name: '调整文件夹栏宽度' })

    fireEvent.pointerDown(separator, { pointerId: 104, button: 0, clientX: 220 })
    fireEvent.pointerMove(window, { pointerId: 104, clientX: 240 })
    expect(sidebar).toHaveStyle({ width: '240px' })
    fireEvent.pointerCancel(window, { pointerId: 104, clientX: 240 })
    fireEvent.pointerMove(window, { pointerId: 104, clientX: 340 })
    const widthAfterCancelledMove = sidebar.style.width
    fireEvent.pointerUp(window, { pointerId: 104, clientX: 340 })

    expect(widthAfterCancelledMove).toBe('240px')
  })

  it('previews a filmstrip image in row order while keeping the category overview', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder)
      .mockResolvedValueOnce(categoryWorkspace())
      .mockResolvedValueOnce(compareContentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))

    const front = await screen.findByRole('button', { name: '预览 front.jpg' })
    front.focus()
    fireEvent.doubleClick(front)

    const preview = screen.getByRole('dialog', { name: /^图片评审 / })
    expect(preview).toHaveTextContent('front.jpg')
    expect(preview).toHaveTextContent('1 / 2')
    expect(screen.getByRole('region', { name: 'B01 图片' })).toBeInTheDocument()

    fireEvent.keyDown(preview, { key: 'ArrowRight' })
    await waitFor(() => expect(screen.getByRole('dialog')).toHaveTextContent('back.jpg'))
    expect(screen.getByRole('dialog')).toHaveTextContent('2 / 2')

    fireEvent.click(screen.getByRole('button', { name: '返回网格' }))
    expect(front).toHaveFocus()
    expect(viewer.queryFolder).toHaveBeenNthCalledWith(2, 'folder-b01', false)
  })

  it('applies global density and proportional request sizing to folder filmstrips', async () => {
    const viewer = bridge()
    const returned = compareContentWorkspace()
    vi.mocked(viewer.queryFolder)
      .mockResolvedValueOnce(categoryWorkspace())
      .mockResolvedValueOnce({
        ...returned,
        images: returned.images.map((file, index) => ({
          ...file,
          imageMetadata: index === 0 ? { width: 3, height: 2 } : { width: 2, height: 3 },
        })),
      })
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))

    const front = await screen.findByRole('button', { name: '预览 front.jpg' })
    await waitFor(() =>
      expect(viewer.requestImage).toHaveBeenCalledWith({
        entityId: 'image-1',
        representation: { kind: 'thumbnail', maxPixels: 198, scaleMilli: 1_000 },
      }),
    )
    expect(front.closest('[role="listitem"]')).toHaveStyle({
      width: '198px',
      height: '132px',
    })

    openSettingsFromMenu()
    fireEvent.change(screen.getByRole('slider', { name: '缩略图大小' }), {
      target: { value: '1' },
    })

    await waitFor(() =>
      expect(front.closest('[role="listitem"]')).toHaveStyle({
        width: '144px',
        height: '96px',
      }),
    )
    expect(viewer.requestImage).toHaveBeenCalledWith({
      entityId: 'image-1',
      representation: { kind: 'thumbnail', maxPixels: 144, scaleMilli: 1_000 },
    })
  })

  it('reuses the same thumbnail representation from a folder filmstrip in the content grid', async () => {
    const viewer = bridge()
    const sharedWorkspace = {
      ...contentWorkspace(),
      images: contentWorkspace().images.map((file) => ({
        ...file,
        imageMetadata: { width: 3, height: 2 },
      })),
    }
    vi.mocked(viewer.queryFolder).mockImplementation(async (entityId) =>
      entityId === null ? categoryWorkspace() : sharedWorkspace,
    )
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))

    await screen.findByRole('button', { name: '预览 front.jpg' })
    await waitFor(() => expect(matchingThumbnailCalls(viewer)).toHaveLength(1))
    fireEvent.click(screen.getByRole('button', { name: '打开 B01' }))
    expect(await screen.findByRole('option', { name: 'front.jpg' })).toBeVisible()
    await waitFor(() => expect(matchingThumbnailCalls(viewer)).toHaveLength(1))
  })

  it('starts a fresh thumbnail session after closing and reopening a project', async () => {
    const viewer = bridge()
    const sharedWorkspace = {
      ...contentWorkspace(),
      images: contentWorkspace().images.map((file) => ({
        ...file,
        imageMetadata: { width: 3, height: 2 },
      })),
    }
    vi.mocked(viewer.queryFolder).mockImplementation(async (entityId) =>
      entityId === null ? categoryWorkspace() : sharedWorkspace,
    )
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    await screen.findByRole('button', { name: '预览 front.jpg' })
    await waitFor(() => expect(matchingThumbnailCalls(viewer)).toHaveLength(1))

    closeProjectFromMenu()
    await screen.findByRole('button', { name: '选择项目文件夹' })
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    await screen.findByRole('button', { name: '预览 front.jpg' })

    await waitFor(() => expect(matchingThumbnailCalls(viewer)).toHaveLength(2))
  })

  it('applies global density to the normal content grid without replacing selection', async () => {
    const viewer = bridge()
    const baseContent = contentWorkspace()
    const content = {
      ...baseContent,
      images: [
        {
          ...defined(baseContent.images[0], 'Expected content image'),
          imageMetadata: { width: 3, height: 2 },
        },
      ],
    }
    vi.mocked(viewer.queryFolder).mockResolvedValue(content)
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))

    const file = await screen.findByRole('option', { name: 'front.jpg' })
    fireEvent.click(file)
    expect(
      defined(file.querySelector('.aspect-thumbnail'), 'Expected content aspect thumbnail'),
    ).toHaveStyle({
      width: '198px',
      height: '132px',
    })
    await waitFor(() =>
      expect(viewer.requestImage).toHaveBeenCalledWith({
        entityId: 'image-1',
        representation: { kind: 'thumbnail', maxPixels: 198, scaleMilli: 1_000 },
      }),
    )

    openSettingsFromMenu()
    fireEvent.change(screen.getByRole('slider', { name: '缩略图大小' }), {
      target: { value: '1' },
    })

    await waitFor(() =>
      expect(
        defined(file.querySelector('.aspect-thumbnail'), 'Expected resized content thumbnail'),
      ).toHaveStyle({
        width: '144px',
        height: '96px',
      }),
    )
    expect(file).toHaveAttribute('aria-selected', 'true')
    expect(screen.getByRole('listbox', { name: '图片文件' })).toHaveAttribute(
      'aria-activedescendant',
      'file-image-1',
    )
    expect(viewer.requestImage).toHaveBeenCalledWith({
      entityId: 'image-1',
      representation: { kind: 'thumbnail', maxPixels: 144, scaleMilli: 1_000 },
    })
  })

  it('reloads visible filmstrip rows after an external category refresh', async () => {
    const viewer = bridge()
    let receiveProjectChanged: Parameters<ViewerBridge['listenProjectChanged']>[0] | undefined
    vi.mocked(viewer.listenProjectChanged).mockImplementation(async (handler) => {
      receiveProjectChanged = handler
      return () => undefined
    })
    const refreshedImages = {
      ...compareContentWorkspace(),
      images: [
        {
          ...defined(compareContentWorkspace().images[0], 'Expected comparison workspace image'),
          entityId: 'image-updated',
          name: 'updated.jpg',
          relativePath: 'id/updated.jpg',
        },
      ],
    }
    vi.mocked(viewer.queryFolder)
      .mockResolvedValueOnce(categoryWorkspace())
      .mockResolvedValueOnce(compareContentWorkspace())
      .mockResolvedValueOnce(categoryWorkspace())
      .mockResolvedValueOnce(refreshedImages)
    render(<App bridge={viewer} />)
    await waitFor(() => expect(receiveProjectChanged).toBeDefined())
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    await screen.findByRole('button', { name: '预览 front.jpg' })

    act(() => {
      receiveProjectChanged?.({
        sessionId: 'session-1',
        generation: 1,
        reason: 'external_change',
        added: 0,
        removed: 0,
        modified: 1,
        moved: 0,
        markerPathsMoved: 0,
        failed: 0,
      })
    })

    expect(await screen.findByRole('button', { name: '预览 updated.jpg' })).toBeVisible()
    expect(viewer.queryFolder).toHaveBeenNthCalledWith(4, 'folder-b01', false)
    expect(viewer.queryFolder).toHaveBeenCalledTimes(4)
  })

  it('invalidates an open filmstrip preview when its category projection refreshes', async () => {
    const viewer = bridge()
    let receiveProjectChanged: Parameters<ViewerBridge['listenProjectChanged']>[0] | undefined
    vi.mocked(viewer.listenProjectChanged).mockImplementation(async (handler) => {
      receiveProjectChanged = handler
      return () => undefined
    })
    const refreshedImages = {
      ...compareContentWorkspace(),
      images: [
        {
          ...defined(compareContentWorkspace().images[0], 'Expected comparison workspace image'),
          entityId: 'image-updated',
          name: 'updated.jpg',
          relativePath: 'id/updated.jpg',
        },
      ],
    }
    vi.mocked(viewer.queryFolder)
      .mockResolvedValueOnce(categoryWorkspace())
      .mockResolvedValueOnce(compareContentWorkspace())
      .mockResolvedValueOnce(categoryWorkspace())
      .mockResolvedValueOnce(refreshedImages)
    render(<App bridge={viewer} />)
    await waitFor(() => expect(receiveProjectChanged).toBeDefined())
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    fireEvent.doubleClick(await screen.findByRole('button', { name: '预览 front.jpg' }))
    expect(screen.getByRole('dialog', { name: /^图片评审 / })).toHaveTextContent('1 / 2')

    act(() => {
      receiveProjectChanged?.({
        sessionId: 'session-1',
        generation: 1,
        reason: 'external_change',
        added: 0,
        removed: 1,
        modified: 0,
        moved: 0,
        markerPathsMoved: 0,
        failed: 0,
      })
    })

    await waitFor(() =>
      expect(screen.queryByRole('dialog', { name: /^图片评审 / })).not.toBeInTheDocument(),
    )
    const updated = await screen.findByRole('button', { name: '预览 updated.jpg' })
    fireEvent.keyDown(window, { key: 'ArrowRight' })
    expect(screen.queryByText('back.jpg')).not.toBeInTheDocument()

    fireEvent.doubleClick(updated)
    expect(screen.getByRole('dialog', { name: /^图片评审 / })).toHaveTextContent('1 / 1')
  })

  it('keeps read-only browsing and comparison available while disabling every write', async () => {
    const viewer = bridge('read_only')
    vi.mocked(viewer.queryFolder).mockResolvedValue(readOnlyContentWorkspace())
    vi.mocked(viewer.previewText).mockResolvedValue({
      entityId: 'text-1',
      format: 'plain_text',
      plainText: 'readonly product notes',
      markdownHtml: null,
      encoding: 'utf8',
      truncated: false,
    })
    render(<App bridge={viewer} />)

    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))

    expect(await screen.findByRole('status', { name: '只读模式' })).toBeVisible()
    expect(screen.getByRole('heading', { name: 'Catalog' })).toBeVisible()
    expect(viewer.openPermissionSettings).not.toHaveBeenCalled()

    fireEvent.click(screen.getByRole('button', { name: '权限设置' }))
    expect(viewer.openPermissionSettings).toHaveBeenCalledOnce()
    expect(screen.getByRole('searchbox', { name: '搜索项目' })).toBeEnabled()
    fireEvent.click(screen.getByRole('button', { name: '筛选' }))
    expect(screen.getByRole('combobox', { name: '排序方式' })).toBeEnabled()

    const front = await screen.findByRole('option', { name: 'front.jpg' })
    const back = screen.getByRole('option', { name: 'back.jpg' })
    fireEvent.doubleClick(front)
    expect(screen.getByRole('dialog', { name: /^图片预览 / })).toBeVisible()
    fireEvent.click(screen.getByRole('button', { name: '返回网格' }))
    fireEvent.click(front)
    fireEvent.click(back, { metaKey: true })

    openRadialMenu(back, 30)
    expect(screen.getByRole('menuitem', { name: '并排对比' })).toHaveAttribute(
      'aria-disabled',
      'false',
    )
    expect(screen.getByRole('menuitem', { name: '信息' })).toHaveAttribute('aria-disabled', 'false')
    expect(screen.getByRole('menuitem', { name: '标记' })).toHaveAttribute('aria-disabled', 'true')
    expect(screen.getByRole('menuitem', { name: '整理' })).toHaveAttribute('aria-disabled', 'true')
    expect(screen.getByRole('menuitem', { name: '移到废纸篓' })).toHaveAttribute(
      'aria-disabled',
      'true',
    )
    expect(screen.getByText('只读')).toBeVisible()
    fireEvent.click(screen.getByRole('button', { name: '关闭文件操作' }))
    expect(screen.getByRole('button', { name: '整理 front.jpg' })).toBeDisabled()
    fireEvent.pointerDown(screen.getByRole('button', { name: '整理 front.jpg' }), {
      pointerId: 31,
      button: 0,
      clientX: 10,
      clientY: 10,
    })
    fireEvent.pointerMove(screen.getByRole('button', { name: '整理 front.jpg' }), {
      pointerId: 31,
      clientX: 20,
      clientY: 20,
    })
    fireEvent.pointerUp(screen.getByRole('button', { name: '整理 front.jpg' }), {
      pointerId: 31,
      clientX: 20,
      clientY: 20,
    })
    expect(viewer.preflightFileCommand).not.toHaveBeenCalled()
    fireEvent.dragStart(
      defined(front.querySelector('.file-export-surface'), 'Expected front image export surface'),
      {
        dataTransfer: viewerDragTransfer(),
      },
    )
    await waitFor(() =>
      expect(viewer.beginFinderDrag).toHaveBeenCalledWith({
        sessionId: 'session-1',
        generation: 1,
        entityIds: ['image-1', 'image-2'],
      }),
    )

    openRadialMenu(back, 32)
    fireEvent.click(screen.getByRole('menuitem', { name: '信息' }))
    expect(screen.getByRole('complementary', { name: '文件信息' })).toBeVisible()
    fireEvent.click(screen.getByRole('button', { name: '关闭信息' }))

    openRadialMenu(back, 33)
    fireEvent.click(screen.getByRole('menuitem', { name: '并排对比' }))
    expect(screen.getByRole('region', { name: '图片对比' })).toBeVisible()
    expect(screen.getByRole('status', { name: '只读模式' })).toBeVisible()
    fireEvent.click(screen.getByRole('button', { name: '完成对比' }))
    fireEvent.click(screen.getByRole('button', { name: '其它文件 · 1' }))
    fireEvent.doubleClick(screen.getByRole('option', { name: 'notes.txt' }))
    expect(await screen.findByText('readonly product notes')).toBeVisible()
  })

  it('opens a generic unsupported-file preview without requesting text', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(genericOtherContentWorkspace())
    render(<App bridge={viewer} />)

    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    fireEvent.click(await screen.findByRole('button', { name: '其它文件 · 1' }))
    fireEvent.doubleClick(screen.getByRole('option', { name: 'license.other' }))

    expect(screen.getByRole('dialog', { name: 'license.other' })).toHaveTextContent('暂不支持预览')
    expect(viewer.previewText).not.toHaveBeenCalled()
  })

  it('routes video cards through the first-frame shell with video-only navigation', async () => {
    installVideoPreviewStageBounds()
    const viewer = bridge()
    const workspace: Extract<FolderWorkspace, { workspace: 'content' }> = videoContentWorkspace()
    const first = defined(workspace.videos[0], 'Expected first video fixture')
    workspace.videos.push({
      ...first,
      entityId: 'video-2',
      relativePath: 'id/trailer.mp4',
      name: 'trailer.mp4',
      videoMetadata: { ...first.videoMetadata, coverUrl: 'viewer-image://session/other-cover' },
    })
    workspace.otherFiles.push({
      entityId: 'notes-1',
      relativePath: 'id/notes.txt',
      name: 'notes.txt',
      kind: 'text',
      size: 20,
      modifiedNs: '3',
      marker: { reviewState: null, favorite: false },
      imageMetadata: null,
      imageUrl: null,
      videoMetadata: null,
    })
    vi.mocked(viewer.queryFolder).mockResolvedValue(workspace)
    vi.mocked(viewer.videoOpen).mockImplementation(async ({ entityId }) => ({
      generation: entityId === 'video-1' ? 11 : 12,
      sessionId: `playback-${entityId}`,
      media: {
        durationUs: 2_000_000,
        displayWidth: 1_920,
        displayHeight: 1_080,
        rotationDegrees: 0,
      },
    }))
    vi.mocked(viewer.videoClose).mockResolvedValue(undefined)
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))

    fireEvent.doubleClick(await screen.findByRole('option', { name: 'clip.mp4' }))

    const preview = await screen.findByRole('dialog', { name: '视频预览 clip.mp4' })
    expect(preview).toBeVisible()
    expect(preview.closest('.viewer-shell')).toHaveAttribute('data-video-preview-open', 'true')
    expect(screen.getByRole('status', { name: '正在加载视频' })).toBeVisible()
    await waitFor(() =>
      expect(viewer.videoOpen).toHaveBeenCalledWith({
        attemptId: expect.any(String),
        entityId: 'video-1',
      }),
    )
    expect(preview.querySelector('img[src^="viewer-image://session/"]')).toBeNull()
    const navigation = screen.getByRole('navigation', { name: '视频导航' })
    expect(navigation).toHaveTextContent('1 / 2')

    fireEvent.click(within(navigation).getByRole('button', { name: '下一个视频' }))

    expect(await screen.findByRole('dialog', { name: '视频预览 trailer.mp4' })).toBeVisible()
    await waitFor(() => expect(viewer.videoOpen).toHaveBeenCalledTimes(2))
    expect(viewer.videoClose).toHaveBeenCalledWith({ generation: 11 })
    expect(screen.getByRole('navigation', { name: '视频导航' })).toHaveTextContent('2 / 2')
  })

  it('routes two selected text files through radial preview while keeping double-click single-file', async () => {
    const viewer = bridge()
    let receiveProjectChanged: Parameters<ViewerBridge['listenProjectChanged']>[0] | undefined
    vi.mocked(viewer.listenProjectChanged).mockImplementation(async (handler) => {
      receiveProjectChanged = handler
      return () => undefined
    })
    const initial = splitTextContentWorkspace()
    const surviving = {
      ...initial,
      otherFiles: [defined(initial.otherFiles[0], 'Expected surviving left text preview fixture')],
    }
    const unrelatedRefresh = {
      ...surviving,
      otherFiles: [
        ...surviving.otherFiles,
        {
          entityId: 'other-added',
          relativePath: 'id/added.other',
          name: 'added.other',
          kind: 'other' as const,
          size: 10,
          modifiedNs: '5',
          marker: { reviewState: null, favorite: false },
          imageMetadata: null,
          imageUrl: null,
          videoMetadata: null,
        },
      ],
    }
    vi.mocked(viewer.queryFolder)
      .mockResolvedValueOnce(initial)
      .mockResolvedValueOnce(surviving)
      .mockResolvedValueOnce(unrelatedRefresh)
    vi.mocked(viewer.previewText).mockImplementation(async ({ entityId }) => ({
      entityId,
      format: 'plain_text',
      plainText: entityId === 'text-left' ? 'left body' : 'right body',
      markdownHtml: null,
      encoding: 'utf8',
      truncated: false,
    }))
    render(<App bridge={viewer} />)
    await waitFor(() => expect(receiveProjectChanged).toBeDefined())
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const left = await screen.findByRole('option', { name: 'left.txt' })
    const right = screen.getByRole('option', { name: 'right.md' })

    fireEvent.click(left)
    fireEvent.click(right, { metaKey: true })
    openRadialMenu(right, 230)
    fireEvent.click(screen.getByRole('menuitem', { name: '预览' }))

    const splitDialog = screen.getByRole('dialog', { name: 'left.txt、right.md' })
    expect(splitDialog).toHaveTextContent('left.txt')
    expect(splitDialog).toHaveTextContent('right.md')
    await waitFor(() => expect(viewer.previewText).toHaveBeenCalledWith({ entityId: 'text-left' }))
    expect(viewer.previewText).toHaveBeenCalledWith({ entityId: 'text-right' })

    fireEvent.click(within(splitDialog).getByRole('button', { name: '关闭预览' }))
    vi.mocked(viewer.previewText).mockClear()
    fireEvent.doubleClick(left)
    await waitFor(() => expect(viewer.previewText).toHaveBeenCalledWith({ entityId: 'text-left' }))
    expect(viewer.previewText).not.toHaveBeenCalledWith({ entityId: 'text-right' })
    fireEvent.click(screen.getByRole('button', { name: '关闭预览' }))

    vi.mocked(viewer.previewText).mockClear()
    fireEvent.doubleClick(right)
    await waitFor(() => expect(viewer.previewText).toHaveBeenCalledWith({ entityId: 'text-right' }))
    expect(viewer.previewText).not.toHaveBeenCalledWith({ entityId: 'text-left' })
    fireEvent.click(screen.getByRole('button', { name: '关闭预览' }))

    fireEvent.click(left)
    fireEvent.click(right, { metaKey: true })
    openRadialMenu(right, 231)
    fireEvent.click(screen.getByRole('menuitem', { name: '预览' }))
    expect(screen.getByRole('dialog', { name: 'left.txt、right.md' })).toBeVisible()

    act(() => {
      receiveProjectChanged?.({
        sessionId: 'session-1',
        generation: 1,
        reason: 'external_change',
        added: 0,
        removed: 1,
        modified: 0,
        moved: 0,
        markerPathsMoved: 0,
        failed: 0,
      })
    })

    const workspace = screen.getByRole('region', { name: '项目内容' })
    expect(
      await within(workspace).findByText('部分正在查看的文件已在项目外发生变化。'),
    ).toBeVisible()
    expect(screen.queryByRole('region', { name: '全局通知' })).not.toBeInTheDocument()
    const repairedDialog = screen.getByRole('dialog', { name: 'left.txt、right.md' })
    expect(repairedDialog).toBeVisible()
    expect(within(repairedDialog).getAllByText('文件已不可用')).toHaveLength(1)
    expect(within(repairedDialog).getByText('left body')).toBeVisible()

    vi.mocked(viewer.previewText).mockClear()
    act(() => {
      receiveProjectChanged?.({
        sessionId: 'session-1',
        generation: 1,
        reason: 'external_change',
        added: 1,
        removed: 0,
        modified: 0,
        moved: 0,
        markerPathsMoved: 0,
        failed: 0,
      })
    })

    expect(await screen.findByRole('option', { name: 'added.other' })).toBeVisible()
    expect(screen.getByText('部分正在查看的文件已在项目外发生变化。')).toBeVisible()
    const refreshedDialog = screen.getByRole('dialog', { name: 'left.txt、right.md' })
    expect(refreshedDialog).toBeVisible()
    expect(within(refreshedDialog).getAllByText('文件已不可用')).toHaveLength(1)
    expect(within(refreshedDialog).getByText('left body')).toBeVisible()
    expect(viewer.previewText).not.toHaveBeenCalledWith({ entityId: 'text-right' })
  })

  it('opens unsupported images in the request-free image preview', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(unsupportedImageContentWorkspace())
    render(<App bridge={viewer} />)

    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    fireEvent.doubleClick(await screen.findByRole('option', { name: 'poster.webp' }))

    const dialog = await screen.findByRole('dialog', { name: /^图片预览 / })
    expect(dialog).toBeVisible()
    expect(within(dialog).getByLabelText('poster.webp .WEBP 暂不支持预览')).toBeVisible()
    expect(screen.queryByRole('dialog', { name: 'poster.webp' })).not.toBeInTheDocument()
    expect(viewer.requestImage).not.toHaveBeenCalled()
    expect(viewer.previewText).not.toHaveBeenCalled()
  })

  it('reselects from a read-only project through normal close and forgets the old path', async () => {
    const viewer = bridge('read_only')
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    await screen.findByRole('heading', { name: 'Catalog' })

    const toolbar = screen.getByRole('toolbar', { name: 'Viewer 工具栏' })
    fireEvent.click(within(toolbar).getByRole('button', { name: '更多' }))
    fireEvent.click(within(toolbar).getByRole('button', { name: '重新选择目录' }))

    await waitFor(() => expect(viewer.closeProject).toHaveBeenCalledWith(undefined, 'project'))
    expect(await screen.findByRole('heading', { name: 'Viewer' })).toBeVisible()
    expect(screen.getByRole('alert')).toHaveTextContent('已关闭只读项目，请选择已授权的目录。')
    expect(viewer.chooseProject).toHaveBeenCalledOnce()
  })

  it('offers stay, wait and cancel-pending choices when close is blocked', async () => {
    const viewer = bridge()
    let receiveCloseBlocked: Parameters<ViewerBridge['listenCloseBlocked']>[0] | undefined
    vi.mocked(viewer.listenCloseBlocked).mockImplementation(async (handler) => {
      receiveCloseBlocked = handler
      return () => undefined
    })
    render(<App bridge={viewer} />)
    await waitFor(() => expect(receiveCloseBlocked).toBeDefined())
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    await screen.findByRole('heading', { name: 'Catalog' })

    act(() =>
      receiveCloseBlocked?.({
        sessionId: 'session-1',
        generation: 1,
        batchId: 'batch-1',
        target: 'project',
      }),
    )
    fireEvent.click(screen.getByRole('button', { name: '停留在当前项目' }))
    expect(screen.queryByRole('dialog', { name: '文件操作尚未完成' })).not.toBeInTheDocument()
    expect(screen.getByRole('heading', { name: 'Catalog' })).toBeVisible()
    expect(viewer.closeProject).not.toHaveBeenCalled()

    act(() =>
      receiveCloseBlocked?.({
        sessionId: 'session-1',
        generation: 1,
        batchId: 'batch-1',
        target: 'project',
      }),
    )
    fireEvent.click(screen.getByRole('button', { name: '等待完成后关闭' }))
    await waitFor(() => expect(viewer.closeProject).toHaveBeenCalledWith('wait', 'project'))
    expect(await screen.findByRole('heading', { name: 'Viewer' })).toBeVisible()

    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    await screen.findByRole('heading', { name: 'Catalog' })
    act(() =>
      receiveCloseBlocked?.({
        sessionId: 'session-1',
        generation: 1,
        batchId: 'batch-2',
        target: 'application',
      }),
    )
    fireEvent.click(screen.getByRole('button', { name: '取消待处理项目并关闭' }))
    await waitFor(() =>
      expect(viewer.closeProject).toHaveBeenLastCalledWith('cancel_pending', 'application'),
    )
    expect(await screen.findByRole('heading', { name: 'Viewer' })).toBeVisible()
  })

  it('removes the scan listener when unmounted', async () => {
    const unlisten = vi.fn()
    const viewer = bridge()
    vi.mocked(viewer.listenScan).mockResolvedValue(unlisten)
    const rendered = render(<App bridge={viewer} />)
    await waitFor(() => expect(viewer.listenScan).toHaveBeenCalledOnce())

    rendered.unmount()

    await waitFor(() => expect(unlisten).toHaveBeenCalledOnce())
  })

  it('reconciles a newer scan generation through a fresh snapshot', async () => {
    const viewer = bridge()
    let receiveScan: Parameters<ViewerBridge['listenScan']>[0] | undefined
    vi.mocked(viewer.listenScan).mockImplementation(async (handler) => {
      receiveScan = handler
      return () => undefined
    })
    vi.mocked(viewer.projectSnapshot).mockResolvedValueOnce(null).mockResolvedValue({
      projectId: 'project-1',
      sessionId: 'session-1',
      generation: 2,
      displayName: 'Catalog',
      access: 'read_write',
    })
    render(<App bridge={viewer} />)
    await waitFor(() => expect(receiveScan).toBeDefined())
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    await screen.findByRole('heading', { name: 'Catalog' })

    act(() => {
      receiveScan?.({
        type: 'folders',
        sessionId: 'session-1',
        generation: 2,
        taskId: 'task-2',
        nodes: [],
      })
    })

    await waitFor(() => expect(viewer.projectSnapshot).toHaveBeenCalledTimes(2))
  })

  it('returns to the empty surface when the native window closes the session', async () => {
    const viewer = bridge()
    let receiveProjectClosed: (() => void) | undefined
    vi.mocked(viewer.listenProjectClosed).mockImplementation(async (handler) => {
      receiveProjectClosed = handler
      return () => undefined
    })
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    render(<App bridge={viewer} />)
    await waitFor(() => expect(receiveProjectClosed).toBeDefined())
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const file = await screen.findByRole('option', { name: 'front.jpg' })
    openRadialMenu(file, 106)
    expect(screen.getByRole('menu', { name: '文件操作' })).toBeVisible()

    act(() => receiveProjectClosed?.())

    expect(await screen.findByRole('heading', { name: 'Viewer' })).toBeVisible()
    expect(screen.queryByRole('menu', { name: '文件操作' })).not.toBeInTheDocument()
    expect(viewer.closeProject).not.toHaveBeenCalled()
  })

  it('opens exactly one project path delivered by the native drop bridge', async () => {
    const viewer = bridge()
    let receiveProjectDrop: ((event: ProjectDropEvent) => void) | undefined
    vi.mocked(viewer.listenProjectDropEvents).mockImplementation(async (handler) => {
      receiveProjectDrop = handler
      return () => undefined
    })
    render(<App bridge={viewer} />)
    await waitFor(() => expect(receiveProjectDrop).toBeDefined())

    act(() => receiveProjectDrop?.({ type: 'drop', paths: ['/fixture/dropped-project'] }))

    await waitFor(() => expect(viewer.openProject).toHaveBeenCalledWith('/fixture/dropped-project'))
    expect(await screen.findByRole('heading', { name: 'Catalog' })).toBeVisible()
  })

  it('keeps a native single-file drop on the concise local invalid state', async () => {
    const viewer = bridge()
    let receiveProjectDrop: ((event: ProjectDropEvent) => void) | undefined
    vi.mocked(viewer.listenProjectDropEvents).mockImplementation(async (handler) => {
      receiveProjectDrop = handler
      return () => undefined
    })
    vi.mocked(viewer.openProject).mockRejectedValue({
      code: 'invalid_project_root',
      category: 'validation',
      userMessage: '请选择一个可读取的真实文件夹。',
      retryable: true,
      taskId: null,
      itemId: null,
    })
    render(<App bridge={viewer} />)
    await waitFor(() => expect(receiveProjectDrop).toBeDefined())

    act(() => receiveProjectDrop?.({ type: 'drop', paths: ['/fixture/not-a-directory.jpg'] }))

    expect(await screen.findByRole('alert')).toHaveTextContent('请选择一个文件夹')
    expect(screen.getByTestId('project-drop-zone')).toHaveAttribute('data-drop-state', 'invalid')
    expect(screen.queryByRole('heading', { name: '无法打开项目' })).not.toBeInTheDocument()
  })

  it('renders a safe fallback instead of raw thrown details', async () => {
    const viewer = bridge()
    vi.mocked(viewer.openProject).mockRejectedValue(
      new Error('/Users/private/project could not be opened'),
    )
    render(<App bridge={viewer} />)

    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))

    expect(await screen.findByRole('heading', { name: '无法打开项目' })).toBeVisible()
    expect(screen.getByText('操作未完成，请重试。')).toBeVisible()
    expect(screen.getByRole('button', { name: '重新选择' })).toBeVisible()
    expect(screen.queryByText(/Users\/private/)).not.toBeInTheDocument()
  })

  it('keeps grid selection and scroll mounted across image preview', async () => {
    installPreviewStageBounds()
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue({
      workspace: 'content',
      videos: [],
      images: Array.from({ length: 20 }, (_, index) => ({
        entityId: `image-${index}`,
        relativePath: `id-1/${index}.jpg`,
        name: `${index}.jpg`,
        kind: 'jpeg',
        size: 100,
        modifiedNs: String(index),
        marker: { reviewState: null, favorite: false },
        imageMetadata: null,
        imageUrl: null,
        videoMetadata: null,
      })),
      otherFiles: [],
    })
    vi.mocked(viewer.requestImage).mockImplementation(async ({ entityId }) => ({
      cacheKey: entityId,
      url: `viewer-image://localhost/session/${entityId}`,
      width: 800,
      height: 600,
      backend: 'quick_look',
    }))
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const grid = await screen.findByRole('listbox', { name: '图片文件' })
    const file = screen.getByRole('option', { name: '1.jpg' })
    fireEvent.click(file)
    grid.scrollTop = 200
    fireEvent.scroll(grid)
    openRadialMenu(file, 107)
    fireEvent.keyDown(grid, { key: ' ' })
    await waitFor(() => expect(document.querySelector('.image-preview-image')).not.toBeNull())
    fireEvent.load(document.querySelector('.image-preview-image') as HTMLImageElement)
    await screen.findByRole('img', { name: '1.jpg' })
    expect(screen.queryByRole('menu', { name: '文件操作' })).not.toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: '返回网格' }))

    expect(screen.getByRole('listbox', { name: '图片文件' })).toBe(grid)
    expect(grid.scrollTop).toBe(200)
    expect(screen.getByRole('option', { name: '1.jpg' })).toHaveAttribute('aria-selected', 'true')
  })

  it('replaces persistent selection toolbars with the right-click radial menu', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const file = await screen.findByRole('option', { name: 'front.jpg' })
    expect(screen.queryByLabelText('批量标记')).not.toBeInTheDocument()
    expect(screen.queryByLabelText('文件操作')).not.toBeInTheDocument()
    fireEvent.contextMenu(file, { button: 2, clientX: 420, clientY: 260 })
    expect(screen.getByRole('menu', { name: '文件操作' })).toBeVisible()
    fireEvent.click(screen.getByRole('menuitem', { name: '信息' }))
    expect(screen.getByRole('complementary', { name: '文件信息' })).toBeVisible()
  })

  it('uses the radial menu for both a secondary click and a held right button', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const file = await screen.findByRole('option', { name: 'front.jpg' })

    fireEvent.pointerDown(file, {
      pointerId: 401,
      button: 2,
      clientX: 420,
      clientY: 260,
    })
    fireEvent.pointerUp(window, {
      pointerId: 401,
      button: 2,
      clientX: 420,
      clientY: 260,
    })
    fireEvent.contextMenu(file, { button: 2, clientX: 420, clientY: 260 })
    const radialMenu = await screen.findByRole('menu', { name: '文件操作' })
    expect(radialMenu.closest('.radial-file-menu')).not.toBeNull()
    expect(document.querySelector('.file-context-menu')).toBeNull()
    fireEvent.keyDown(radialMenu, { key: 'Escape' })

    vi.useFakeTimers()
    fireEvent.pointerDown(file, {
      pointerId: 402,
      button: 2,
      clientX: 420,
      clientY: 260,
    })
    expect(document.querySelector('.radial-file-menu')).toBeNull()
    act(() => vi.advanceTimersByTime(180))
    expect(document.querySelector('.radial-file-menu')).not.toBeNull()
  })

  it('waits for release before an early context-menu event opens radial click mode', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const file = await screen.findByRole('option', { name: 'front.jpg' })

    fireEvent.pointerDown(file, {
      pointerId: 404,
      button: 2,
      clientX: 420,
      clientY: 260,
    })
    fireEvent.contextMenu(file, { button: 2, clientX: 420, clientY: 260 })

    expect(document.querySelector('.file-context-menu')).toBeNull()
    expect(document.querySelector('.radial-file-menu')).toBeNull()

    fireEvent.pointerUp(window, {
      pointerId: 404,
      button: 2,
      clientX: 420,
      clientY: 260,
    })

    const radialMenu = await screen.findByRole('menu', { name: '文件操作' })
    expect(radialMenu.closest('.radial-file-menu')).not.toBeNull()
    expect(document.querySelector('.file-context-menu')).toBeNull()
    expect(file).toHaveAttribute('aria-selected', 'true')
    fireEvent.keyDown(radialMenu, { key: 'Escape' })
    expect(screen.getByRole('listbox', { name: '图片文件' })).toHaveFocus()
  })

  it('lets dwell promote an early context-menu event to the pointer radial menu', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const file = await screen.findByRole('option', { name: 'front.jpg' })
    vi.useFakeTimers()

    fireEvent.pointerDown(file, {
      pointerId: 405,
      button: 2,
      clientX: 420,
      clientY: 260,
    })
    fireEvent.contextMenu(file, { button: 2, clientX: 420, clientY: 260 })
    expect(document.querySelector('.file-context-menu')).toBeNull()

    act(() => vi.advanceTimersByTime(180))

    expect(document.querySelector('.radial-file-menu')).not.toBeNull()
    expect(document.querySelector('.file-context-menu')).toBeNull()
  })

  it('lets meaningful movement promote an early context-menu event to the pointer radial menu', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const file = await screen.findByRole('option', { name: 'front.jpg' })

    fireEvent.pointerDown(file, {
      pointerId: 406,
      button: 2,
      clientX: 420,
      clientY: 260,
    })
    fireEvent.contextMenu(file, { button: 2, clientX: 420, clientY: 260 })
    expect(document.querySelector('.file-context-menu')).toBeNull()

    fireEvent.pointerMove(window, {
      pointerId: 406,
      buttons: 2,
      clientX: 428,
      clientY: 260,
    })

    expect(document.querySelector('.radial-file-menu')).not.toBeNull()
    expect(document.querySelector('.file-context-menu')).toBeNull()
  })

  it('keeps Control-click on the unified radial click-mode path', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const file = await screen.findByRole('option', { name: 'front.jpg' })

    fireEvent.pointerDown(file, {
      pointerId: 403,
      button: 0,
      ctrlKey: true,
      clientX: 420,
      clientY: 260,
    })
    fireEvent.pointerUp(window, {
      pointerId: 403,
      button: 0,
      ctrlKey: true,
      clientX: 420,
      clientY: 260,
    })
    fireEvent.contextMenu(file, {
      button: 0,
      ctrlKey: true,
      clientX: 420,
      clientY: 260,
    })

    expect(
      screen.getByRole('menu', { name: '文件操作' }).closest('.radial-file-menu'),
    ).not.toBeNull()
    expect(document.querySelector('.file-context-menu')).toBeNull()
  })

  it('starts a fresh held gesture when a second right-click replaces click fallback', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(compareContentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const front = await screen.findByRole('option', { name: 'front.jpg' })
    const back = screen.getByRole('option', { name: 'back.jpg' })

    openRadialMenu(front, 201)
    fireEvent.pointerUp(window, { pointerId: 201, clientX: 423, clientY: 263 })
    openRadialMenu(back, 202)
    fireEvent.pointerMove(window, { pointerId: 202, clientX: 420, clientY: 170 })
    fireEvent.pointerUp(window, { pointerId: 202, clientX: 420, clientY: 170 })

    expect(screen.getByRole('dialog', { name: /^图片评审 / })).toHaveTextContent('back.jpg')
  })

  it('restores the original content target after a click fallback is replaced', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(compareContentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const grid = await screen.findByRole('listbox', { name: '图片文件' })
    const front = screen.getByRole('option', { name: 'front.jpg' })
    const back = screen.getByRole('option', { name: 'back.jpg' })

    openRadialMenu(front, 211)
    fireEvent.pointerUp(window, { pointerId: 211, clientX: 420, clientY: 260 })
    expect(screen.getByRole('menuitem', { name: '预览' })).toHaveFocus()

    openRadialMenu(back, 212)
    expect(screen.getByRole('menuitem', { name: '预览' })).toHaveFocus()
    fireEvent.keyDown(screen.getByRole('menu', { name: '文件操作' }), { key: 'Escape' })

    expect(screen.queryByRole('menu', { name: '文件操作' })).not.toBeInTheDocument()
    expect(grid).toHaveFocus()
  })

  it('keeps mixed-favorite copy truthful while routing the frozen selection to toggle', async () => {
    const viewer = bridge()
    const workspace = compareContentWorkspace()
    workspace.images[0] = {
      ...defined(workspace.images[0], 'Expected first workspace image'),
      marker: { reviewState: null, favorite: true },
    }
    vi.mocked(viewer.queryFolder).mockResolvedValue(workspace)
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const front = await screen.findByRole('option', { name: 'front.jpg' })
    const back = screen.getByRole('option', { name: 'back.jpg' })
    fireEvent.click(front)
    fireEvent.click(back, { metaKey: true })

    openRadialMenu(back, 203)
    fireEvent.click(screen.getByRole('menuitem', { name: '标记' }))
    const favorite = screen.getByRole('menuitemcheckbox', { name: '切换收藏' })
    expect(favorite).toHaveAttribute('aria-checked', 'mixed')
    fireEvent.click(favorite)

    await waitFor(() =>
      expect(viewer.toggleFavorite).toHaveBeenCalledWith({
        sessionId: 'session-1',
        generation: 1,
        entityIds: ['image-1', 'image-2'],
      }),
    )
  })

  it('clears an earlier invalid keyboard compare status through the radial compare owner', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(compareContentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const front = await screen.findByRole('option', { name: 'front.jpg' })
    const back = screen.getByRole('option', { name: 'back.jpg' })
    fireEvent.click(front)
    fireEvent.keyDown(window, { key: 'c' })
    expect(screen.getByText('请选择 2–8 张图片进行对比。')).toBeVisible()

    fireEvent.click(back, { metaKey: true })
    openRadialMenu(back, 204)
    fireEvent.click(screen.getByRole('menuitem', { name: '并排对比' }))

    expect(await screen.findByRole('region', { name: '图片对比' })).toBeVisible()
    expect(screen.queryByText('请选择 2–8 张 JPG 或 PNG 图片进行对比。')).not.toBeInTheDocument()
  })

  it('closes the radial snapshot as soon as a deferred project close starts', async () => {
    const viewer = bridge()
    const closing = deferred<'closed'>()
    vi.mocked(viewer.closeProject).mockImplementation(() => closing.promise)
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const file = await screen.findByRole('option', { name: 'front.jpg' })
    openRadialMenu(file, 205)
    expect(screen.getByRole('menu', { name: '文件操作' })).toBeVisible()

    closeProjectFromMenu()

    expect(screen.queryByRole('menu', { name: '文件操作' })).not.toBeInTheDocument()
    expect(screen.getByRole('heading', { name: 'Catalog' })).toBeVisible()
    await act(async () => {
      closing.resolve('closed')
      await closing.promise
    })
  })

  it('invalidates a radial snapshot when the same session advances generation', async () => {
    const viewer = bridge()
    let receiveScan: Parameters<ViewerBridge['listenScan']>[0] | undefined
    vi.mocked(viewer.listenScan).mockImplementation(async (handler) => {
      receiveScan = handler
      return () => undefined
    })
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    vi.mocked(viewer.projectSnapshot).mockResolvedValueOnce(null).mockResolvedValue({
      projectId: 'project-1',
      sessionId: 'session-1',
      generation: 2,
      displayName: 'Catalog',
      access: 'read_write',
    })
    render(<App bridge={viewer} />)
    await waitFor(() => expect(receiveScan).toBeDefined())
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const file = await screen.findByRole('option', { name: 'front.jpg' })
    openRadialMenu(file, 206)
    expect(screen.getByRole('menu', { name: '文件操作' })).toBeVisible()

    act(() => {
      receiveScan?.({
        type: 'folders',
        sessionId: 'session-1',
        generation: 2,
        taskId: 'task-2',
        nodes: [],
      })
    })

    await waitFor(() => expect(viewer.projectSnapshot).toHaveBeenCalledTimes(2))
    await waitFor(() =>
      expect(screen.queryByRole('menu', { name: '文件操作' })).not.toBeInTheDocument(),
    )
  })

  it('routes keep, favorite, rename, copy, move, compare, and Trash through existing owners', async () => {
    const viewer = bridge()
    vi.mocked(viewer.folderTree).mockResolvedValue([
      {
        entityId: 'folder-b',
        parentEntityId: null,
        relativePath: 'selected',
        name: 'selected',
        marker: { reviewState: null, favorite: false },
      },
    ])
    vi.mocked(viewer.queryFolder).mockResolvedValue(compareContentWorkspace())
    vi.mocked(viewer.preflightFileCommand).mockImplementation(async (request) => ({
      rows: request.items.map((item) => ({
        entityId: item.entityId,
        relativePath: `selected/${item.entityId}.jpg`,
        state: 'ready' as const,
      })),
      executable: true,
    }))
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const front = await screen.findByRole('option', { name: 'front.jpg' })
    openRadialMenu(front, 91)
    fireEvent.click(screen.getByRole('menuitem', { name: '标记' }))
    fireEvent.click(screen.getByRole('menuitemcheckbox', { name: '保留' }))
    await waitFor(() =>
      expect(viewer.setReviewState).toHaveBeenCalledWith({
        sessionId: 'session-1',
        generation: 1,
        entityIds: ['image-1'],
        reviewState: 'keep',
      }),
    )

    openRadialMenu(front, 92)
    fireEvent.click(screen.getByRole('menuitem', { name: '标记' }))
    fireEvent.click(screen.getByRole('menuitemcheckbox', { name: '收藏' }))
    await waitFor(() =>
      expect(viewer.toggleFavorite).toHaveBeenCalledWith({
        sessionId: 'session-1',
        generation: 1,
        entityIds: ['image-1'],
      }),
    )

    openRadialMenu(front, 93)
    fireEvent.click(screen.getByRole('menuitem', { name: '整理' }))
    fireEvent.click(screen.getByRole('menuitem', { name: '重命名' }))
    expect(screen.getByRole('dialog', { name: '重命名文件' })).toBeVisible()
    expect(screen.queryByRole('menu', { name: '文件操作' })).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: '取消' }))

    for (const [mode, actionLabel, dialogName] of [
      ['copy', '复制到', '选择复制目标'],
      ['move', '移动到', '选择移动目标'],
    ] as const) {
      openRadialMenu(front, mode === 'copy' ? 94 : 95)
      fireEvent.click(screen.getByRole('menuitem', { name: '整理' }))
      fireEvent.click(screen.getByRole('menuitem', { name: actionLabel }))
      const dialog = screen.getByRole('dialog', { name: dialogName })
      fireEvent.click(within(dialog).getByRole('radio', { name: /selected/ }))
      fireEvent.click(within(dialog).getByRole('button', { name: '检查冲突' }))
      await waitFor(() =>
        expect(viewer.preflightFileCommand).toHaveBeenCalledWith({
          sessionId: 'session-1',
          generation: 1,
          kind: mode,
          items: [
            {
              entityId: 'image-1',
              action: { kind: mode, destinationFolderId: 'folder-b' },
            },
          ],
        }),
      )
      fireEvent.click(within(dialog).getByRole('button', { name: '取消' }))
    }

    const back = screen.getByRole('option', { name: 'back.jpg' })
    fireEvent.click(back, { metaKey: true })
    openRadialMenu(back, 96)
    expect(screen.getByRole('menuitem', { name: '并排对比' })).toHaveAttribute(
      'aria-disabled',
      'false',
    )
    fireEvent.click(screen.getByRole('menuitem', { name: '并排对比' }))
    expect(await screen.findByRole('region', { name: '图片对比' })).toBeVisible()
    expect(screen.queryByRole('menu', { name: '文件操作' })).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: '完成对比' }))

    openRadialMenu(front, 97)
    fireEvent.click(screen.getByRole('menuitem', { name: '移到废纸篓' }))
    expect(screen.getByRole('dialog', { name: '将文件移到废纸篓？' })).toBeVisible()
  })

  it('replaces only the right workspace with search and returns to folder context', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    vi.mocked(viewer.searchProject).mockResolvedValue({
      revision: 1,
      total: 1,
      progress: {
        imagesTotal: 1,
        imagesReady: 1,
        imagesFailed: 0,
        textTotal: 0,
        textReady: 0,
        textSkipped: 0,
        textFailed: 0,
        complete: false,
      },
      hits: [
        {
          entityId: 'image-1',
          relativePath: 'id-1/shoe.jpg',
          name: 'shoe.jpg',
          kind: 'jpeg',
          size: 10,
          modifiedNs: '1',
          marker: { reviewState: null, favorite: false },
          imageMetadata: null,
          matchedField: 'filename',
          score: 1,
          groupRelativePath: 'id-1',
          matchRanges: [{ start: 0, end: 4 }],
        },
      ],
    })
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const search = await screen.findByRole('searchbox', { name: '搜索项目' })
    const file = screen.getByRole('option', { name: 'front.jpg' })

    fireEvent.keyDown(window, { key: 'f', metaKey: true })
    expect(search).toHaveFocus()
    openRadialMenu(file, 108)
    fireEvent.change(search, { target: { value: 'shoe' } })
    await waitFor(() => expect(viewer.searchProject).toHaveBeenCalledOnce())
    expect(screen.queryByRole('menu', { name: '文件操作' })).not.toBeInTheDocument()
    expect(await screen.findByRole('option', { name: /shoe.jpg/ })).toBeVisible()
    expect(
      within(screen.getByRole('region', { name: '搜索结果区域' })).getByRole('status'),
    ).toHaveTextContent('结果仍在更新 · 图片 1/1 · 文本 0/0')
    expect(screen.getByRole('tree', { name: '项目文件夹' })).toBeVisible()

    fireEvent.click(screen.getByRole('button', { name: '返回文件夹内容' }))
    await waitFor(() => expect(screen.getByRole('option', { name: 'front.jpg' })).toBeVisible())
  })

  it('opens safe rename and Trash surfaces from keyboard without immediate deletion', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    vi.mocked(viewer.operationStatus).mockResolvedValue({
      sessionId: 'session-1',
      generation: 1,
      batchId: 'batch-1',
      lifecycle: 'completed',
      requested: 1,
      completed: 1,
      failed: 0,
      skipped: 0,
      cancelled: 0,
      activeEntityId: null,
    })
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const file = await screen.findByRole('option', { name: 'front.jpg' })
    fireEvent.click(file)

    fireEvent.keyDown(window, { key: 'Enter' })
    const renameDialog = screen.getByRole('dialog', { name: '重命名文件' })
    expect(renameDialog).toBeVisible()
    fireEvent.change(within(renameDialog).getByRole('textbox', { name: '新文件名' }), {
      target: { value: 'hero.jpg' },
    })
    fireEvent.click(within(renameDialog).getByRole('button', { name: '重命名' }))
    await waitFor(() =>
      expect(viewer.executeFileCommand).toHaveBeenCalledWith(
        expect.objectContaining({
          kind: 'rename',
          items: [
            {
              entityId: 'image-1',
              action: { kind: 'rename', proposedName: 'hero', editExtension: false },
            },
          ],
        }),
      ),
    )
    await waitFor(() => expect(viewer.operationResults).toHaveBeenCalledOnce())
    expect(await screen.findByRole('complementary', { name: '文件操作结果' })).toHaveClass(
      'viewer-inspector',
    )

    fireEvent.click(screen.getByRole('button', { name: '关闭文件操作结果' }))
    fireEvent.keyDown(window, { key: 'Delete' })
    expect(screen.getByRole('dialog', { name: '将文件移到废纸篓？' })).toBeVisible()
    expect(viewer.executeFileCommand).toHaveBeenCalledTimes(1)
    fireEvent.click(screen.getByRole('button', { name: '移到废纸篓' }))
    await waitFor(() => expect(viewer.executeFileCommand).toHaveBeenCalledTimes(2))
  })

  it('opens the selected file from a window-level native Space event', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const file = await screen.findByRole('option', { name: 'front.jpg' })
    fireEvent.click(file)

    fireEvent.keyDown(window, { key: 'Unidentified', code: 'Space' })

    expect(screen.getByRole('dialog', { name: /^图片评审 / })).toBeVisible()
  })

  it('routes Command-Z only outside editable and modal contexts', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const file = await screen.findByRole('option', { name: 'front.jpg' })
    fireEvent.click(file)

    fireEvent.keyDown(window, { key: 'z', metaKey: true })
    await waitFor(() => expect(viewer.undoLastOperation).toHaveBeenCalledOnce())
    fireEvent.keyDown(window, { key: 'z', metaKey: true, shiftKey: true })
    expect(viewer.undoLastOperation).toHaveBeenCalledOnce()
    fireEvent.keyDown(window, { key: 'z', metaKey: true, altKey: true })
    fireEvent.keyDown(window, { key: 'z', metaKey: true, ctrlKey: true })
    expect(viewer.undoLastOperation).toHaveBeenCalledOnce()

    const interactiveButton = screen.getByRole('button', { name: '整理 front.jpg' })
    interactiveButton.focus()
    fireEvent.keyDown(interactiveButton, { key: 'Enter' })
    expect(screen.queryByRole('dialog', { name: '重命名文件' })).not.toBeInTheDocument()

    const search = screen.getByRole('searchbox', { name: '搜索项目' })
    search.focus()
    fireEvent.keyDown(search, { key: 'z', metaKey: true })
    fireEvent.keyDown(search, { key: 'Delete' })
    expect(viewer.undoLastOperation).toHaveBeenCalledOnce()
    expect(screen.queryByRole('dialog', { name: '将文件移到废纸篓？' })).not.toBeInTheDocument()

    fireEvent.doubleClick(file)
    expect(screen.getByRole('dialog', { name: /^图片评审 / })).toBeVisible()
    fireEvent.keyDown(window, { key: 'Delete' })
    expect(screen.queryByRole('dialog', { name: '将文件移到废纸篓？' })).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: '返回网格' }))

    fireEvent.keyDown(window, { key: 'Delete' })
    fireEvent.keyDown(window, { key: 'z', metaKey: true })
    expect(viewer.undoLastOperation).toHaveBeenCalledOnce()
  })

  it('invalidates an open operation dialog when project closing begins', async () => {
    const viewer = bridge()
    const closing = deferred<'closed'>()
    vi.mocked(viewer.closeProject).mockImplementation(() => closing.promise)
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    fireEvent.click(await screen.findByRole('option', { name: 'front.jpg' }))
    fireEvent.keyDown(window, { key: 'Enter' })
    expect(screen.getByRole('dialog', { name: '重命名文件' })).toBeVisible()

    closeProjectFromMenu()
    expect(screen.queryByRole('dialog', { name: '重命名文件' })).not.toBeInTheDocument()
    expect(viewer.executeFileCommand).not.toHaveBeenCalled()

    await act(async () => {
      closing.resolve('closed')
      await closing.promise
    })
  })

  it('does not route Command-Z while a file operation is active', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    fireEvent.click(await screen.findByRole('option', { name: 'front.jpg' }))
    fireEvent.keyDown(window, { key: 'Enter' })
    const dialog = screen.getByRole('dialog', { name: '重命名文件' })
    fireEvent.change(within(dialog).getByRole('textbox', { name: '新文件名' }), {
      target: { value: 'hero.jpg' },
    })
    fireEvent.click(within(dialog).getByRole('button', { name: '重命名' }))
    await waitFor(() => expect(viewer.executeFileCommand).toHaveBeenCalledOnce())

    fireEvent.keyDown(window, { key: 'z', metaKey: true })
    expect(viewer.undoLastOperation).not.toHaveBeenCalled()
  })

  it('keeps writes blocked while busy without weakening single-selection preview', async () => {
    const viewer = bridge()
    const results = deferred<{ total: number; offset: number; items: [] }>()
    vi.mocked(viewer.queryFolder).mockResolvedValue(compareContentWorkspace())
    vi.mocked(viewer.operationResults).mockImplementation(() => results.promise)
    vi.mocked(viewer.operationStatus).mockResolvedValue({
      sessionId: 'session-1',
      generation: 1,
      batchId: 'batch-1',
      lifecycle: 'completed',
      requested: 1,
      completed: 1,
      failed: 0,
      skipped: 0,
      cancelled: 0,
      activeEntityId: null,
    })
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const file = await screen.findByRole('option', { name: 'front.jpg' })
    fireEvent.click(file)
    fireEvent.keyDown(window, { key: 'Enter' })
    const dialog = screen.getByRole('dialog', { name: '重命名文件' })
    fireEvent.change(within(dialog).getByRole('textbox', { name: '新文件名' }), {
      target: { value: 'hero.jpg' },
    })
    fireEvent.click(within(dialog).getByRole('button', { name: '重命名' }))
    await waitFor(() => expect(viewer.operationResults).toHaveBeenCalledOnce())

    openRadialMenu(file, 94)
    expect(screen.getByRole('menuitem', { name: '标记' })).toHaveAttribute('aria-disabled', 'true')
    expect(screen.getByRole('menuitem', { name: '整理' })).toHaveAttribute('aria-disabled', 'true')
    expect(screen.getByRole('menuitem', { name: '移到废纸篓' })).toHaveAttribute(
      'aria-disabled',
      'true',
    )
    expect(screen.getByRole('menuitem', { name: '预览' })).toHaveAttribute('aria-disabled', 'false')
    expect(screen.getByRole('menuitem', { name: '信息' })).toHaveAttribute('aria-disabled', 'false')
    fireEvent.click(screen.getByRole('menuitem', { name: '预览' }))
    expect(screen.getByRole('dialog', { name: /^图片评审 / })).toBeVisible()
    fireEvent.click(screen.getByRole('button', { name: '返回网格' }))

    const back = screen.getByRole('option', { name: 'back.jpg' })
    fireEvent.click(back, { metaKey: true })
    openRadialMenu(back, 95)
    const multiPreview = screen.getByRole('menuitem', { name: '预览' })
    expect(multiPreview).toHaveAttribute('aria-disabled', 'true')
    fireEvent.click(multiPreview)
    expect(screen.queryByRole('dialog', { name: /^图片评审 / })).not.toBeInTheDocument()

    await act(async () => {
      results.resolve({ total: 0, offset: 0, items: [] })
      await results.promise
    })
    await waitFor(() =>
      expect(screen.queryByRole('menu', { name: '文件操作' })).not.toBeInTheDocument(),
    )
  })

  it('routes an ordered frozen Option-copy pointer drop through the same commands', async () => {
    const viewer = bridge()
    vi.mocked(viewer.folderTree).mockResolvedValue([
      {
        entityId: 'folder-a',
        parentEntityId: null,
        relativePath: 'id',
        name: 'id',
        marker: { reviewState: null, favorite: false },
      },
      {
        entityId: 'folder-b',
        parentEntityId: null,
        relativePath: 'selected',
        name: 'selected',
        marker: { reviewState: null, favorite: false },
      },
    ])
    vi.mocked(viewer.queryFolder).mockResolvedValue(compareContentWorkspace())
    vi.mocked(viewer.preflightFileCommand).mockResolvedValue({
      rows: [
        { entityId: 'image-1', relativePath: 'selected/front.jpg', state: 'ready' },
        { entityId: 'image-2', relativePath: 'selected/back.jpg', state: 'ready' },
      ],
      executable: true,
    })
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const front = await screen.findByRole('option', { name: 'front.jpg' })
    fireEvent.click(front)
    fireEvent.click(screen.getByRole('option', { name: 'back.jpg' }), { metaKey: true })
    const handle = screen.getByRole('button', { name: '整理 front.jpg' })
    const destination = await screen.findByRole('treeitem', { name: 'selected' })
    organizationPointerMove(handle, destination, { pointerId: 32, altKey: true })
    expect(screen.getByText('复制 2 项')).toBeVisible()
    expect(destination).toHaveAttribute('data-drop-mode', 'copy')
    fireEvent.pointerUp(handle, {
      pointerId: 32,
      altKey: false,
      clientX: 20,
      clientY: 20,
    })
    expect(screen.queryByText('复制 2 项')).not.toBeInTheDocument()

    await waitFor(() =>
      expect(viewer.preflightFileCommand).toHaveBeenCalledWith({
        sessionId: 'session-1',
        generation: 1,
        kind: 'copy',
        items: [
          {
            entityId: 'image-1',
            action: { kind: 'copy', destinationFolderId: 'folder-b' },
          },
          {
            entityId: 'image-2',
            action: { kind: 'copy', destinationFolderId: 'folder-b' },
          },
        ],
      }),
    )
    await waitFor(() =>
      expect(viewer.executeFileCommand).toHaveBeenCalledWith(
        expect.objectContaining({
          kind: 'copy',
          items: [
            {
              entityId: 'image-1',
              action: { kind: 'copy', destinationFolderId: 'folder-b' },
            },
            {
              entityId: 'image-2',
              action: { kind: 'copy', destinationFolderId: 'folder-b' },
            },
          ],
        }),
      ),
    )
    expect(viewer.beginFinderDrag).not.toHaveBeenCalled()
  })

  it('starts copy-only native export during the file-body dragstart', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const file = await screen.findByRole('option', { name: 'front.jpg' })
    const exportSurface = defined(
      file.querySelector<HTMLElement>('.file-export-surface'),
      'Expected file export surface',
    )

    const event = createEvent.dragStart(exportSurface, { dataTransfer: viewerDragTransfer() })
    fireEvent(exportSurface, event)

    expect(event.defaultPrevented).toBe(true)
    await waitFor(() =>
      expect(viewer.beginFinderDrag).toHaveBeenCalledWith({
        sessionId: 'session-1',
        generation: 1,
        entityIds: ['image-1'],
      }),
    )
  })

  it('uses the organization handle for a frozen default move without Finder export', async () => {
    const viewer = bridge()
    vi.mocked(viewer.folderTree).mockResolvedValue([
      {
        entityId: 'folder-b',
        parentEntityId: null,
        relativePath: 'selected',
        name: 'selected',
        marker: { reviewState: null, favorite: false },
      },
    ])
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const handle = await screen.findByRole('button', { name: '整理 front.jpg' })
    const destination = await screen.findByRole('treeitem', { name: 'selected' })
    organizationPointerDrag(handle, destination, {
      pointerId: 33,
      altKey: false,
      releaseAltKey: true,
    })

    expect(viewer.beginFinderDrag).not.toHaveBeenCalled()
    await waitFor(() =>
      expect(viewer.preflightFileCommand).toHaveBeenCalledWith(
        expect.objectContaining({ kind: 'move' }),
      ),
    )
  })

  it('keeps video entity IDs through organization validation and move preflight', async () => {
    const viewer = bridge()
    vi.mocked(viewer.folderTree).mockResolvedValue([
      {
        entityId: 'folder-b',
        parentEntityId: null,
        relativePath: 'selected',
        name: 'selected',
        marker: { reviewState: null, favorite: false },
      },
    ])
    vi.mocked(viewer.queryFolder).mockResolvedValue(videoContentWorkspace())
    vi.mocked(viewer.preflightFileCommand).mockResolvedValue({
      rows: [{ entityId: 'video-1', relativePath: 'id/clip.mp4', state: 'ready' }],
      executable: true,
    })
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const video = await screen.findByRole('option', { name: 'clip.mp4' })
    fireEvent.click(video)
    const handle = screen.getByRole('button', { name: '整理 clip.mp4' })
    const destination = await screen.findByRole('treeitem', { name: 'selected' })

    organizationPointerDrag(handle, destination, {
      pointerId: 44,
      altKey: false,
      releaseAltKey: false,
    })

    await waitFor(() =>
      expect(viewer.preflightFileCommand).toHaveBeenCalledWith({
        sessionId: 'session-1',
        generation: 1,
        kind: 'move',
        items: [
          {
            entityId: 'video-1',
            action: { kind: 'move', destinationFolderId: 'folder-b' },
          },
        ],
      }),
    )
  })

  it('cancels an active pointer drag when the workspace identity changes', async () => {
    const viewer = bridge()
    vi.mocked(viewer.folderTree).mockResolvedValue([
      {
        entityId: 'folder-a',
        parentEntityId: null,
        relativePath: 'id',
        name: 'id',
        marker: { reviewState: null, favorite: false },
      },
      {
        entityId: 'folder-b',
        parentEntityId: null,
        relativePath: 'selected',
        name: 'selected',
        marker: { reviewState: null, favorite: false },
      },
    ])
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const handle = await screen.findByRole('button', { name: '整理 front.jpg' })
    const file = screen.getByRole('option', { name: 'front.jpg' })
    const destination = await screen.findByRole('treeitem', { name: 'selected' })
    organizationPointerMove(handle, destination, { pointerId: 34, altKey: false })
    expect(destination).toHaveAttribute('data-drop-mode', 'move')

    openRadialMenu(file, 109)
    fireEvent.click(screen.getByRole('treeitem', { name: 'id' }))
    await waitFor(() =>
      expect(screen.getByRole('treeitem', { name: 'id' })).toHaveAttribute('aria-selected', 'true'),
    )
    await waitFor(() =>
      expect(screen.queryByRole('menu', { name: '文件操作' })).not.toBeInTheDocument(),
    )
    fireEvent.pointerUp(handle, { pointerId: 34, clientX: 20, clientY: 20 })

    expect(viewer.preflightFileCommand).not.toHaveBeenCalled()
    expect(viewer.executeFileCommand).not.toHaveBeenCalled()
  })

  it('cancels an active pointer drag when the current workspace projection is replaced', async () => {
    const viewer = bridge()
    let receiveProjectChanged: Parameters<ViewerBridge['listenProjectChanged']>[0] | undefined
    vi.mocked(viewer.listenProjectChanged).mockImplementation(async (handler) => {
      receiveProjectChanged = handler
      return () => undefined
    })
    vi.mocked(viewer.folderTree).mockResolvedValue([
      {
        entityId: 'folder-b',
        parentEntityId: null,
        relativePath: 'selected',
        name: 'selected',
        marker: { reviewState: null, favorite: false },
      },
    ])
    const refreshedWorkspace = {
      ...contentWorkspace(),
      images: [
        {
          ...defined(contentWorkspace().images[0], 'Expected content workspace image'),
          modifiedNs: '2',
          marker: { reviewState: 'keep' as const, favorite: true },
        },
      ],
    }
    vi.mocked(viewer.queryFolder)
      .mockResolvedValueOnce(contentWorkspace())
      .mockResolvedValueOnce(refreshedWorkspace)
    render(<App bridge={viewer} />)
    await waitFor(() => expect(receiveProjectChanged).toBeDefined())
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const handle = await screen.findByRole('button', { name: '整理 front.jpg' })
    const destination = await screen.findByRole('treeitem', { name: 'selected' })
    organizationPointerMove(handle, destination, { pointerId: 38, altKey: false })
    expect(destination).toHaveAttribute('data-drop-mode', 'move')

    act(() => {
      receiveProjectChanged?.({
        sessionId: 'session-1',
        generation: 1,
        reason: 'external_change',
        added: 0,
        removed: 0,
        modified: 1,
        moved: 0,
        markerPathsMoved: 0,
        failed: 0,
      })
    })
    expect(await screen.findByText('保留 · 收藏')).toBeVisible()
    await waitFor(() => expect(destination).not.toHaveAttribute('data-drop-mode'))

    fireEvent.pointerUp(handle, { pointerId: 38, clientX: 20, clientY: 50 })

    expect(viewer.preflightFileCommand).not.toHaveBeenCalled()
    expect(viewer.executeFileCommand).not.toHaveBeenCalled()
  })

  it('closes the radial menu when external projection repair removes its file', async () => {
    const viewer = bridge()
    let receiveProjectChanged: Parameters<ViewerBridge['listenProjectChanged']>[0] | undefined
    vi.mocked(viewer.listenProjectChanged).mockImplementation(async (handler) => {
      receiveProjectChanged = handler
      return () => undefined
    })
    const initial = compareContentWorkspace()
    const surviving = {
      ...initial,
      images: [defined(initial.images[1], 'Expected surviving comparison image')],
    }
    vi.mocked(viewer.queryFolder).mockResolvedValueOnce(initial).mockResolvedValueOnce(surviving)
    render(<App bridge={viewer} />)
    await waitFor(() => expect(receiveProjectChanged).toBeDefined())
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const front = await screen.findByRole('option', { name: 'front.jpg' })
    openRadialMenu(front, 110)
    expect(screen.getByRole('menu', { name: '文件操作' })).toBeVisible()

    act(() => {
      receiveProjectChanged?.({
        sessionId: 'session-1',
        generation: 1,
        reason: 'external_change',
        added: 0,
        removed: 1,
        modified: 0,
        moved: 0,
        markerPathsMoved: 0,
        failed: 0,
      })
    })

    expect(await screen.findByText('部分正在查看的文件已在项目外发生变化。')).toBeVisible()
    await waitFor(() =>
      expect(screen.queryByRole('menu', { name: '文件操作' })).not.toBeInTheDocument(),
    )
  })

  it('rejects a same-folder move target while allowing Option-copy', async () => {
    const viewer = bridge()
    vi.mocked(viewer.folderTree).mockResolvedValue([
      {
        entityId: 'folder-a',
        parentEntityId: null,
        relativePath: 'id',
        name: 'id',
        marker: { reviewState: null, favorite: false },
      },
    ])
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const target = await screen.findByRole('treeitem', { name: 'id' })
    const handle = await screen.findByRole('button', { name: '整理 front.jpg' })
    organizationPointerMove(handle, target, { pointerId: 35, altKey: false })
    expect(target).toHaveAttribute('data-drop-invalid', 'true')
    fireEvent.pointerUp(handle, { pointerId: 35, clientX: 20, clientY: 20 })
    expect(viewer.preflightFileCommand).not.toHaveBeenCalled()

    organizationPointerMove(handle, target, { pointerId: 36, altKey: true })
    expect(target).toHaveAttribute('data-drop-mode', 'copy')
    fireEvent.pointerCancel(handle, { pointerId: 36 })
  })

  it('shows a safe retry message when native Finder drag cannot start', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    vi.mocked(viewer.beginFinderDrag).mockRejectedValue({ code: 'finder_drag_selection_stale' })
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const file = await screen.findByRole('option', { name: 'front.jpg' })
    fireEvent.dragStart(
      defined(file.querySelector('.file-export-surface'), 'Expected file export surface'),
      {
        dataTransfer: viewerDragTransfer(),
      },
    )

    const notices = await screen.findByRole('region', { name: '全局通知' })
    expect(within(notices).getByRole('alert')).toHaveTextContent(
      '部分文件已发生变化，请刷新后重试。',
    )
    expect(getComputedStyle(notices).top).toBe('68px')
    expect(screen.queryByText(/Users\/private/)).not.toBeInTheDocument()
  })

  it('keeps global Finder-drag feedback below the read-only recovery strip', async () => {
    const viewer = bridge('read_only')
    vi.mocked(viewer.queryFolder).mockResolvedValue(readOnlyContentWorkspace())
    vi.mocked(viewer.beginFinderDrag).mockRejectedValue({ code: 'finder_drag_selection_stale' })
    render(<App bridge={viewer} />)

    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const file = await screen.findByRole('option', { name: 'front.jpg' })
    fireEvent.dragStart(
      defined(file.querySelector('.file-export-surface'), 'Expected file export surface'),
      { dataTransfer: viewerDragTransfer() },
    )

    const notices = await screen.findByRole('region', { name: '全局通知' })
    expect(notices).toHaveClass('global-notice-stack--below-read-only')
    expect(getComputedStyle(notices).top).toBe('106px')
    fireEvent.click(screen.getByRole('button', { name: '权限设置' }))
    expect(viewer.openPermissionSettings).toHaveBeenCalledOnce()
  })

  it('shows a generic retry message without exposing a native Finder error path', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    vi.mocked(viewer.beginFinderDrag).mockRejectedValue(
      new Error('/Users/private/project/front.jpg could not be dragged'),
    )
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const file = await screen.findByRole('option', { name: 'front.jpg' })
    fireEvent.dragStart(
      defined(file.querySelector('.file-export-surface'), 'Expected file export surface'),
      {
        dataTransfer: viewerDragTransfer(),
      },
    )

    const notices = await screen.findByRole('region', { name: '全局通知' })
    expect(within(notices).getByRole('alert')).toHaveTextContent('无法拖到 Finder，请重新拖动。')
    expect(screen.queryByText(/Users\/private/)).not.toBeInTheDocument()
  })

  it('opens compare with C and restores the mounted grid selection after closing', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(compareContentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const front = await screen.findByRole('option', { name: 'front.jpg' })
    const back = screen.getByRole('option', { name: 'back.jpg' })
    const organizeHandle = screen.getByRole('button', { name: '整理 front.jpg' })
    fireEvent.click(front)
    fireEvent.click(back, { metaKey: true })

    fireEvent.keyDown(window, { key: 'c' })
    const compare = await screen.findByRole('region', { name: '图片对比' })
    expect(organizeHandle).toBeDisabled()
    expect(compare).toHaveFocus()
    fireEvent.click(screen.getByRole('button', { name: '完成对比' }))

    expect(front).toHaveAttribute('aria-selected', 'true')
    expect(back).toHaveAttribute('aria-selected', 'true')
  })

  it('forwards compare pane cancellation signals through the production bridge boundary', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(compareContentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const front = await screen.findByRole('option', { name: 'front.jpg' })
    const back = screen.getByRole('option', { name: 'back.jpg' })
    fireEvent.click(front)
    fireEvent.click(back, { metaKey: true })

    fireEvent.keyDown(window, { key: 'c' })
    await screen.findByRole('region', { name: '图片对比' })

    await waitFor(() => {
      expect(viewer.requestImage).toHaveBeenCalledWith(
        expect.objectContaining({
          representation: expect.objectContaining({ kind: 'fit_preview' }),
        }),
        expect.any(AbortSignal),
      )
    })
  })

  it('opens comparison with C after selecting 8 images', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(compareContentWorkspaceWithCount(8))
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const grid = await screen.findByRole('listbox', { name: '图片文件' })

    fireEvent.keyDown(grid, { key: 'a', metaKey: true })
    fireEvent.keyDown(window, { key: 'c' })

    expect(await screen.findByRole('region', { name: '图片对比' })).toBeVisible()
    fireEvent.click(screen.getByRole('button', { name: '完成对比' }))
    const mountedOptions = screen.getAllByRole('option')
    expect(mountedOptions.every((item) => item.ariaSelected === 'true')).toBe(true)
    openRadialMenu(defined(mountedOptions[0], 'Expected a mounted selected image'), 222)
    expect(screen.getByText('8 个文件')).toBeVisible()
  })

  it('uses one fit row for four portrait files in a wide comparison workspace', async () => {
    const resize = installCompareResizeObserver()
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(comparePortraitWorkspace(4))
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const grid = await screen.findByRole('listbox', { name: '图片文件' })

    fireEvent.keyDown(grid, { key: 'a', metaKey: true })
    fireEvent.keyDown(window, { key: 'c' })

    const compare = await screen.findByRole('region', { name: '图片对比' })
    act(() => resize.flush())
    expect(compare).toHaveAttribute('data-layout', 'fit-row')
  })

  it('keeps compare disabled for a 9-image selection without truncating selection', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(compareContentWorkspaceWithCount(9))
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const grid = await screen.findByRole('listbox', { name: '图片文件' })

    fireEvent.keyDown(grid, { key: 'a', metaKey: true })
    const image = screen.getByRole('option', { name: 'image-1.jpg' })
    openRadialMenu(image, 221)
    const compare = screen.getByRole('menuitem', { name: '并排对比' })
    expect(compare).toHaveAttribute('aria-disabled', 'true')
    expect(compare).toHaveAttribute('title', '最多同时对比 8 张图片')
    fireEvent.click(compare)

    expect(screen.queryByRole('region', { name: '图片对比' })).not.toBeInTheDocument()
    expect(screen.getByRole('status', { name: '选择摘要' })).toHaveTextContent('已选择 9 项')
  })

  it('applies an inline pane marker without replacing the underlying grid selection', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(compareContentWorkspace())
    vi.mocked(viewer.setReviewState).mockResolvedValue({ changes: [] })
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const front = await screen.findByRole('option', { name: 'front.jpg' })
    const back = screen.getByRole('option', { name: 'back.jpg' })
    fireEvent.click(front)
    fireEvent.click(back, { metaKey: true })
    fireEvent.keyDown(window, { key: 'c' })

    fireEvent.click(await screen.findByRole('button', { name: 'front.jpg 标记为保留' }))
    await waitFor(() =>
      expect(viewer.setReviewState).toHaveBeenCalledWith({
        sessionId: 'session-1',
        generation: 1,
        entityIds: ['image-1'],
        reviewState: 'keep',
      }),
    )
    fireEvent.click(screen.getByRole('button', { name: '完成对比' }))
    expect(front).toHaveAttribute('aria-selected', 'true')
    expect(back).toHaveAttribute('aria-selected', 'true')
  })

  it('falls back to the surviving image preview when compare reaches one pane', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(compareContentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const front = await screen.findByRole('option', { name: 'front.jpg' })
    const back = screen.getByRole('option', { name: 'back.jpg' })
    fireEvent.click(front)
    fireEvent.click(back, { metaKey: true })
    fireEvent.keyDown(window, { key: 'c' })

    fireEvent.click(await screen.findByRole('button', { name: '移除 front.jpg' }))
    expect(await screen.findByRole('dialog', { name: /^图片评审 / })).toHaveTextContent('back.jpg')
    expect(screen.queryByRole('region', { name: '图片对比' })).not.toBeInTheDocument()
  })

  it('suppresses the C compare shortcut while an editable control owns the event', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(compareContentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const front = await screen.findByRole('option', { name: 'front.jpg' })
    const back = screen.getByRole('option', { name: 'back.jpg' })
    fireEvent.click(front)
    fireEvent.click(back, { metaKey: true })

    fireEvent.keyDown(screen.getByRole('searchbox', { name: '搜索项目' }), { key: 'c' })
    expect(screen.queryByRole('region', { name: '图片对比' })).not.toBeInTheDocument()
  })

  it('does not open a stale grid selection from search results and closes compare on search', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(compareContentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const front = await screen.findByRole('option', { name: 'front.jpg' })
    const back = screen.getByRole('option', { name: 'back.jpg' })
    fireEvent.click(front)
    fireEvent.click(back, { metaKey: true })
    fireEvent.keyDown(window, { key: 'c' })
    expect(await screen.findByRole('region', { name: '图片对比' })).toHaveFocus()

    const search = screen.getByRole('searchbox', { name: '搜索项目' })
    fireEvent.change(search, { target: { value: 'shoe' } })
    await waitFor(() =>
      expect(screen.queryByRole('region', { name: '图片对比' })).not.toBeInTheDocument(),
    )
    expect(screen.queryByRole('menu', { name: '文件操作' })).not.toBeInTheDocument()
    fireEvent.keyDown(window, { key: 'c' })
    expect(screen.queryByRole('region', { name: '图片对比' })).not.toBeInTheDocument()
  })

  it('opens the equivalent preselected conflict dialog when a dropped item is not ready', async () => {
    const viewer = bridge()
    vi.mocked(viewer.folderTree).mockResolvedValue([
      {
        entityId: 'folder-b',
        parentEntityId: null,
        relativePath: 'selected',
        name: 'selected',
        marker: { reviewState: null, favorite: false },
      },
    ])
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    vi.mocked(viewer.preflightFileCommand).mockResolvedValue({
      rows: [
        {
          entityId: 'image-1',
          relativePath: 'selected/front.jpg',
          state: 'conflict',
          code: 'destination_occupied',
        },
      ],
      executable: true,
    })
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const handle = await screen.findByRole('button', { name: '整理 front.jpg' })
    const target = await screen.findByRole('treeitem', { name: 'selected' })
    organizationPointerDrag(handle, target, {
      pointerId: 37,
      altKey: false,
      releaseAltKey: false,
    })

    const dialog = await screen.findByRole('dialog', { name: '选择移动目标' })
    expect(within(dialog).getByRole('radio')).toBeChecked()
    expect(within(dialog).getByText('selected/front.jpg')).toBeVisible()
    expect(within(dialog).getByRole('combobox')).toHaveValue('')
    expect(viewer.executeFileCommand).not.toHaveBeenCalled()
  })
})

function organizationPointerMove(
  handle: HTMLElement,
  target: HTMLElement,
  { pointerId, altKey }: { pointerId: number; altKey: boolean },
) {
  let capturedPointerId: number | null = null
  Object.defineProperties(handle, {
    setPointerCapture: {
      configurable: true,
      value: vi.fn((nextPointerId: number) => {
        capturedPointerId = nextPointerId
      }),
    },
    hasPointerCapture: {
      configurable: true,
      value: vi.fn((nextPointerId: number) => capturedPointerId === nextPointerId),
    },
    releasePointerCapture: {
      configurable: true,
      value: vi.fn((nextPointerId: number) => {
        if (capturedPointerId === nextPointerId) capturedPointerId = null
      }),
    },
  })
  const surface = defined(
    target.closest<HTMLElement>('[data-organization-drop-surface]'),
    'Expected organization drop surface',
  )
  vi.spyOn(surface, 'getBoundingClientRect').mockReturnValue({
    left: 0,
    top: 0,
    right: 260,
    bottom: 100,
    width: 260,
    height: 100,
    x: 0,
    y: 0,
    toJSON: () => undefined,
  })
  Object.defineProperty(document, 'elementFromPoint', {
    configurable: true,
    value: vi.fn(() => target),
  })
  fireEvent.pointerDown(handle, {
    pointerId,
    button: 0,
    altKey,
    clientX: 10,
    clientY: 50,
  })
  fireEvent.pointerMove(handle, {
    pointerId,
    altKey: !altKey,
    clientX: 20,
    clientY: 50,
  })
}

function organizationPointerDrag(
  handle: HTMLElement,
  target: HTMLElement,
  {
    pointerId,
    altKey,
    releaseAltKey,
  }: { pointerId: number; altKey: boolean; releaseAltKey: boolean },
) {
  organizationPointerMove(handle, target, { pointerId, altKey })
  fireEvent.pointerUp(handle, {
    pointerId,
    altKey: releaseAltKey,
    clientX: 20,
    clientY: 20,
  })
}

function viewerDragTransfer() {
  const data = new Map<string, string>()
  return {
    effectAllowed: 'uninitialized',
    dropEffect: 'none',
    get types() {
      return [...data.keys()]
    },
    setData(type: string, value: string) {
      data.set(type, value)
    },
    getData(type: string) {
      return data.get(type) ?? ''
    },
  }
}

function contentWorkspace() {
  return {
    workspace: 'content' as const,
    videos: [],
    images: [
      {
        entityId: 'image-1',
        relativePath: 'id/front.jpg',
        name: 'front.jpg',
        kind: 'jpeg' as const,
        size: 100,
        modifiedNs: '1',
        marker: { reviewState: null, favorite: false },
        imageMetadata: null,
        imageUrl: null,
        videoMetadata: null,
      },
    ],
    otherFiles: [],
  }
}

function twoImageContentWorkspace() {
  const first = contentWorkspace()
  return {
    ...first,
    images: [
      ...first.images,
      {
        ...defined(first.images[0], 'Expected front image fixture'),
        entityId: 'image-2',
        relativePath: 'id/back.jpg',
        name: 'back.jpg',
        modifiedNs: '2',
      },
    ],
  }
}

function activeImageReviewSnapshot(): ReviewSessionSnapshot {
  return {
    phase: 'active',
    resume: null,
    reviewStreamId: 'stream-1',
    reviewRoundId: 'round-1',
    revision: 1,
    members: [
      {
        assetVersionId: 'asset-1',
        entityId: 'image-1',
        relativePath: 'id/front.jpg',
        displayName: 'front.jpg',
        kind: 'image',
        feedbackItems: 0,
      },
    ],
    feedback: [],
    restorableFeedbackId: null,
    unreviewable: [],
    conflicts: [],
    counts: { total: 1, feedbackItems: 0, revise: 0, unreviewable: 0, pass: 0 },
    error: null,
  }
}

function legacySnapshotWithFeedback(count: number): ReviewSessionSnapshot {
  const snapshot = activeImageReviewSnapshot()
  snapshot.feedback = Array.from({ length: count }, (_, index) => ({
    feedbackId: `legacy-feedback-${index}`,
    text: `旧意见 ${index + 1}`,
    createdAtMs: index + 1,
    targetCount: 1,
    targetEntityIds: ['image-1'],
    targets: [
      {
        assetVersionId: 'legacy-asset-1',
        entityId: 'image-1',
        anchor: { kind: 'asset' },
      },
    ],
  }))
  snapshot.counts.feedbackItems = count
  return snapshot
}

function preparedFrontAsset() {
  return {
    asset: {
      id: 'asset-1',
      sourceEntityId: 'image-1',
      relativePath: 'id/front.jpg',
      evidence: { sizeBytes: 100, modifiedNs: '1', blake3: '12'.repeat(32) },
      media: { kind: 'image' as const, width: 640, height: 480 },
      producerAssetId: null,
      parentAssetVersionId: null,
    },
    preview: {
      assetVersionId: 'asset-1',
      role: 'base' as const,
      url: 'viewer-review-image://localhost/asset-1',
      width: 640,
      height: 480,
      sourceWidth: 640,
      sourceHeight: 480,
    },
  }
}

function continuousWorkspaceWithFeedback(count: number, snapshotId: string): ReviewWorkspaceView {
  const view = workspace(snapshotId)
  if (view.current === null) throw new Error('Expected current continuous review fixture')
  const prepared = preparedFrontAsset()
  view.current.state.assets = [prepared.asset]
  view.current.state.feedback = Array.from({ length: count }, (_, index) => ({
    id: `continuous-feedback-${index}`,
    textRevisionId: `continuous-text-${index}`,
    text: `持续意见 ${index + 1}`,
    createdAtMs: index + 1,
    historyRef: null,
    targets: [
      {
        id: `continuous-target-${index}`,
        revisionId: `continuous-revision-${index}`,
        assetVersionId: prepared.asset.id,
        anchor: { kind: 'asset' },
        availability: { kind: 'ready' },
      },
    ],
  }))
  view.projection.actionable = view.current.state.feedback.flatMap((item) =>
    item.targets.map((target) => target.id),
  )
  return view
}

function workspaceWithHistorySelectors(selectors: ReviewHistorySelector[]): ReviewWorkspaceView {
  return Object.assign(workspace('snapshot-1'), { historySelectors: structuredClone(selectors) })
}

function emptyHistory(selector: ReviewHistorySelector): ReviewHistoryView {
  return {
    selector: structuredClone(selector),
    entries: [],
    legacy: null,
    limitations: ['background_only'],
    restoreActions: [],
  }
}

function videoContentWorkspace() {
  return {
    workspace: 'content' as const,
    images: [],
    videos: [
      {
        entityId: 'video-1',
        relativePath: 'id/clip.mp4',
        name: 'clip.mp4',
        kind: 'video' as const,
        size: 200,
        modifiedNs: '2',
        marker: { reviewState: null, favorite: false },
        imageMetadata: null,
        imageUrl: null,
        videoMetadata: {
          durationUs: 2_000_000,
          displayWidth: 1_920,
          displayHeight: 1_080,
          rotationDegrees: 0,
          frameRateMillihertz: 30_000,
          videoCodec: 'h264',
          audioCodec: 'aac',
          probeStatus: 'ready' as const,
          failureKind: null,
          coverUrl: null,
        },
      },
    ],
    otherFiles: [],
  }
}

function matchingThumbnailCalls(viewer: ViewerBridge) {
  return vi
    .mocked(viewer.requestImage)
    .mock.calls.filter(
      ([request]) =>
        request.entityId === 'image-1' &&
        request.representation.kind === 'thumbnail' &&
        request.representation.maxPixels === 198 &&
        request.representation.scaleMilli === 1_000,
    )
}

function mixedContentWorkspace({
  entityId = 'text-root',
  relativePath = 'root/root-notes.txt',
  name = 'root-notes.txt',
}: {
  entityId?: string
  relativePath?: string
  name?: string
} = {}) {
  return {
    ...contentWorkspace(),
    otherFiles: [
      {
        entityId,
        relativePath,
        name,
        kind: 'text' as const,
        size: 20,
        modifiedNs: '2',
        marker: { reviewState: null, favorite: false },
        imageMetadata: null,
        imageUrl: null,
        videoMetadata: null,
      },
    ],
  }
}

function genericOtherContentWorkspace() {
  return {
    ...contentWorkspace(),
    otherFiles: [
      {
        entityId: 'other-license',
        relativePath: 'id/license.other',
        name: 'license.other',
        kind: 'other' as const,
        size: 20,
        modifiedNs: '3',
        marker: { reviewState: null, favorite: false },
        imageMetadata: null,
        imageUrl: null,
        videoMetadata: null,
      },
    ],
  }
}

function splitTextContentWorkspace() {
  return {
    workspace: 'content' as const,
    images: [],
    videos: [],
    otherFiles: [
      {
        entityId: 'text-left',
        relativePath: 'id/left.txt',
        name: 'left.txt',
        kind: 'text' as const,
        size: 20,
        modifiedNs: '3',
        marker: { reviewState: null, favorite: false },
        imageMetadata: null,
        imageUrl: null,
        videoMetadata: null,
      },
      {
        entityId: 'text-right',
        relativePath: 'id/right.md',
        name: 'right.md',
        kind: 'markdown' as const,
        size: 20,
        modifiedNs: '4',
        marker: { reviewState: null, favorite: false },
        imageMetadata: null,
        imageUrl: null,
        videoMetadata: null,
      },
    ],
  }
}

function unsupportedImageContentWorkspace() {
  const source = contentWorkspace().images[0]
  if (source === undefined) throw new Error('Expected supported image fixture')
  return {
    workspace: 'content' as const,
    videos: [],
    images: [
      {
        ...source,
        name: 'poster.webp',
        relativePath: 'id/poster.webp',
        kind: 'unsupported_image' as const,
      },
    ],
    otherFiles: [],
  }
}

function categoryWorkspace() {
  return {
    workspace: 'category' as const,
    folders: [
      {
        entityId: 'folder-b01',
        relativePath: '角色/B01',
        name: 'B01',
        marker: { reviewState: null, favorite: false },
        imageCount: 2,
        videoCount: 0,
        otherFileCount: 0,
        reviewProgress: {
          total: 2,
          keep: 0,
          pending: 0,
          reject: 0,
          unmarked: 2,
          favorite: 0,
        },
        representativeImages: [],
      },
    ],
  }
}

function compareContentWorkspace() {
  const first = defined(contentWorkspace().images[0], 'Expected first content workspace image')
  return {
    workspace: 'content' as const,
    videos: [],
    images: [
      first,
      {
        ...first,
        entityId: 'image-2',
        relativePath: 'id/back.jpg',
        name: 'back.jpg',
        modifiedNs: '2',
      },
    ],
    otherFiles: [],
  }
}

function compareContentWorkspaceWithCount(count: number) {
  const source = defined(contentWorkspace().images[0], 'Expected source image')
  return {
    workspace: 'content' as const,
    videos: [],
    images: Array.from({ length: count }, (_, index) => ({
      ...source,
      entityId: `image-${index + 1}`,
      relativePath: `id/image-${index + 1}.jpg`,
      name: `image-${index + 1}.jpg`,
      modifiedNs: String(index + 1),
    })),
    otherFiles: [],
  }
}

function comparePortraitWorkspace(count: number) {
  return {
    ...compareContentWorkspaceWithCount(count),
    images: compareContentWorkspaceWithCount(count).images.map((file) => ({
      ...file,
      imageMetadata: { width: 600, height: 800 },
    })),
  }
}

function installCompareResizeObserver() {
  const frames = new Map<number, FrameRequestCallback>()
  let nextFrameId = 1
  class Observer {
    private readonly callback: ResizeObserverCallback

    constructor(callback: ResizeObserverCallback) {
      this.callback = callback
    }

    observe(node: Element) {
      const bounds = node.classList.contains('compare-layout-region')
        ? { width: 1_700, height: 900 }
        : { width: 600, height: 800 }
      this.callback(
        [{ target: node, contentRect: bounds } as ResizeObserverEntry],
        this as unknown as ResizeObserver,
      )
    }

    disconnect() {}
  }
  vi.stubGlobal('ResizeObserver', Observer)
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
    const id = nextFrameId
    nextFrameId += 1
    frames.set(id, callback)
    return id
  })
  vi.stubGlobal('cancelAnimationFrame', (id: number) => {
    frames.delete(id)
  })
  return {
    flush() {
      while (frames.size > 0) {
        const callbacks = [...frames.values()]
        frames.clear()
        for (const callback of callbacks) callback(0)
      }
    },
  }
}

function installPreviewStageBounds() {
  const nativeGetBoundingClientRect = HTMLElement.prototype.getBoundingClientRect
  const spy = vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (
    this: HTMLElement,
  ) {
    if (this.classList.contains('image-preview-stage')) {
      return {
        x: 0,
        y: 0,
        left: 0,
        top: 0,
        right: 640,
        bottom: 480,
        width: 640,
        height: 480,
        toJSON: () => undefined,
      }
    }
    return nativeGetBoundingClientRect.call(this)
  })
  restorePreviewStageBounds = () => spy.mockRestore()
}

function installVideoPreviewStageBounds() {
  const nativeGetBoundingClientRect = HTMLElement.prototype.getBoundingClientRect
  const spy = vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (
    this: HTMLElement,
  ) {
    if (this.classList.contains('video-preview-stage')) {
      return {
        x: 100,
        y: 50,
        left: 100,
        top: 50,
        right: 900,
        bottom: 650,
        width: 800,
        height: 600,
        toJSON: () => undefined,
      }
    }
    return nativeGetBoundingClientRect.call(this)
  })
  restorePreviewStageBounds = () => spy.mockRestore()
}

function readOnlyContentWorkspace() {
  return {
    ...compareContentWorkspace(),
    otherFiles: [
      {
        entityId: 'text-1',
        relativePath: 'id/notes.txt',
        name: 'notes.txt',
        kind: 'text' as const,
        size: 20,
        modifiedNs: '3',
        marker: { reviewState: null, favorite: false },
        imageMetadata: null,
        imageUrl: null,
        videoMetadata: null,
      },
    ],
  }
}
