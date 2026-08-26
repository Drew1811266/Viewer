import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, expectTypeOf, it } from 'vitest'
import type { BrowserFile } from '../../api/types'
import { tauriViewerBridge } from '../../api/viewer'
import type { OpenPreviewIntent, WorkspaceIntent } from './intents'
import { createWorkspacePorts } from './ports'

const source = (path: string) => readFileSync(resolve(process.cwd(), path), 'utf8')
const appSource = source('src/App.tsx')
const organizationSource = source('src/app/workspace/useOrganizationCoordinator.ts')
const viewingSource = source('src/app/workspace/useViewingCoordinator.ts')
const feedbackSource = source('src/app/workspace/useFeedbackCoordinator.ts')
const reviewCoordinatorSource = source('src/app/review/useReviewSessionCoordinator.ts')
const reviewModelSource = source('src/app/review/reviewModel.ts')
const workspaceViewSource = source('src/app/workspace/WorkspaceProjectView.tsx')
const reviewLayerSource = source('src/components/review/ReviewWorkspaceLayer.tsx')
const reviewInspectorSource = source('src/components/review/ReviewInspector.tsx')
const emptyProjectSource = source('src/components/EmptyProject.tsx')
const settingsSource = source('src/components/SettingsDialog.tsx')
const videoPreviewSource = source('src/components/VideoPreview.tsx')

function exhaust(intent: WorkspaceIntent): string {
  switch (intent.kind) {
    case 'open-preview':
      return intent.file.entityId
    case 'enter-compare':
      return String(intent.files.length)
    case 'start-rename':
      return String(intent.files.length)
    case 'show-operation-results':
      return intent.batchId
    case 'open-settings':
      return intent.kind
    case 'open-info':
      return intent.kind
    case 'close-project':
      return intent.kind
  }
}

describe('workspace contracts', () => {
  it('keeps preview intents as the exact open-preview member of the closed intent union', () => {
    expectTypeOf<OpenPreviewIntent>().toEqualTypeOf<
      Extract<WorkspaceIntent, { kind: 'open-preview' }>
    >()
    expectTypeOf<OpenPreviewIntent['file']>().toEqualTypeOf<BrowserFile>()
    expect(exhaust({ kind: 'open-settings' })).toBe('open-settings')
  })

  it('creates explicit narrow ports without forwarding the full bridge object', () => {
    const ports = createWorkspacePorts(tauriViewerBridge)

    expect(Object.keys(ports.preview).sort()).toEqual([
      'openExternalLink',
      'previewText',
      'queryFolder',
      'requestImage',
      'videoRequestCover',
    ])
    expect('executeFileCommand' in ports.preview).toBe(false)
    expect(Object.keys(ports.playback).sort()).toEqual([
      'listenVideo',
      'videoCancelOpen',
      'videoClose',
      'videoOpen',
      'videoPause',
      'videoPlay',
      'videoRequestCover',
      'videoRequestThumbnail',
      'videoSeek',
      'videoSetFullscreen',
      'videoSetMuted',
      'videoSetRate',
      'videoSetVolume',
      'videoStep',
    ])
    expect(Object.keys(ports.settings).sort()).toEqual(['videoCacheClear', 'videoCacheStats'])
    expect(Object.keys(ports.emptyProject).sort()).toEqual([
      'chooseProject',
      'listenProjectDropEvents',
      'openProject',
    ])
    expect('closeProject' in ports.emptyProject).toBe(false)
    expect(Object.keys(ports.shell)).toEqual(['revealProjectInFileManager'])
    expect(Object.keys(ports.review).sort()).toEqual([
      'listenReviewProgress',
      'reviewAbandon',
      'reviewAddFeedback',
      'reviewCancelTask',
      'reviewComplete',
      'reviewCompletionSummary',
      'reviewDeleteFeedback',
      'reviewPreviewStart',
      'reviewReplaceFeedbackAnchor',
      'reviewRestoreDeletedFeedback',
      'reviewResume',
      'reviewStart',
      'reviewStartWithFeedback',
      'reviewStatus',
      'reviewUpdateFeedback',
      'reviewUpdateFeedbackText',
    ])
  })

  it('keeps ViewerWorkspace as the typed composition boundary', () => {
    expect(appSource).not.toMatch(
      /useState<TaskFeedback|useOperationDialogs|usePreviewSession|useOrganizationPointerDrag/,
    )
    expect(appSource).toMatch(/WorkspaceIntentSink/)
    expect(appSource).toMatch(/useWorkspaceShellCoordinator/)
    expect(appSource).toMatch(/useFeedbackCoordinator/)
    expect(appSource).toMatch(/useOrganizationCoordinator/)
    expect(appSource).toMatch(/useViewingCoordinator/)
    expect(appSource).toMatch(/useReviewSessionCoordinator/)
    expect(appSource).toMatch(/ReviewWorkspaceLayer/)
    expect(workspaceViewSource).toMatch(/reviewToolbarAction/)
    expect(workspaceViewSource).toMatch(/reviewLayer/)
    expect(workspaceViewSource).not.toMatch(/ReviewWorkspaceLayer|ReviewInspector/)
    expect(organizationSource).not.toMatch(/useViewingCoordinator|useWorkspaceShellCoordinator/)
    expect(viewingSource).not.toMatch(/useOrganizationCoordinator|useWorkspaceShellCoordinator/)
    expect(feedbackSource).not.toMatch(/ViewerBridge|useViewerController/)
  })

  it('keeps composition, organization, and viewing orchestration structurally focused', () => {
    expect(appSource).toMatch(/WorkspaceProjectView/)
    expect(appSource).not.toMatch(/ContentBrowser|WorkspaceOperationDialogs/)
    expect(organizationSource).toMatch(/useOrganizationOperations/)
    expect(organizationSource).toMatch(/useOrganizationDrag/)
    expect(organizationSource).toMatch(/useOrganizationShortcuts/)
    expect(organizationSource).not.toMatch(
      /useOperationDialogs|useOrganizationPointerDrag|useReviewShortcuts/,
    )
    expect(viewingSource).toMatch(/usePreviewCoordinator/)
    expect(viewingSource).toMatch(/useCompareCoordinator/)
    expect(viewingSource).not.toMatch(/usePreviewSession/)
  })

  it('keeps child bridge capabilities restricted to their declared ports', () => {
    expect(emptyProjectSource).toMatch(/EmptyProjectPort/)
    expect(emptyProjectSource).not.toMatch(/ViewerBridge/)
    expect(settingsSource).toMatch(/SettingsPort/)
    expect(settingsSource).not.toMatch(/ViewerBridge/)
    expect(videoPreviewSource).toMatch(/VideoPreviewBridge/)
    expect(videoPreviewSource).not.toMatch(/ViewerBridge/)
    expect(reviewCoordinatorSource).not.toMatch(
      /@tauri-apps|ViewerBridge|useViewerController|ViewerState/,
    )
    expect(reviewModelSource).not.toMatch(
      /@tauri-apps|ViewerBridge|useViewerController|ViewerState/,
    )
    expect(reviewLayerSource).not.toMatch(
      /@tauri-apps|ViewerBridge|useViewerController|ViewerState/,
    )
    expect(reviewInspectorSource).not.toMatch(
      /@tauri-apps|ViewerBridge|useViewerController|ViewerState/,
    )
  })
})
