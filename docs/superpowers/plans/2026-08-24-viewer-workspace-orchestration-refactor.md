# Viewer Workspace Orchestration Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> Status: Active

**Goal:** Reduce `ViewerWorkspace` to a composition-and-routing root by moving shell, feedback, organization, and viewing responsibilities into focused React coordinators without changing any product behavior.

**Architecture:** Keep `App` as the injected `ViewerBridge` entry and `ViewerState` as the only global projection of backend project state. Coordinators are ordinary TypeScript hooks/modules with narrow ports and typed root intents; they do not introduce a Context, store, singleton, or event bus.

**Tech Stack:** React 19, TypeScript 6 strict mode, Vitest 4, Testing Library, Biome, Tauri bridge types, existing Rust backend unchanged.

**Spec:** `docs/superpowers/specs/2026-08-24-viewer-architecture-governance-and-workspace-orchestration-design.md`

**Prerequisite:** Complete and merge `docs/superpowers/plans/2026-08-24-viewer-architecture-boundary-governance.md` first. `pnpm architecture:boundaries` must pass before this plan starts.

## Global Constraints

- This is architecture-only work: add no product feature and change no user-visible behavior.
- Freeze Tauri commands, events, DTOs, capabilities, CSP, SQLite schemas, `.viewer` schemas, shortcuts, focus restoration, accessibility semantics, copy, error messages, feedback timing, and file-operation semantics.
- Keep `ViewerState` as the only global frontend projection of project, scan, search, and file-operation state.
- Keep media event state local to the Viewing Coordinator; do not copy it into `ViewerState` or another global store.
- Do not add Redux, Zustand, an event bus, a new global Context, a service singleton, or direct coordinator-to-coordinator calls.
- Production React views must not import Tauri packages. `App` remains the full bridge injection/composition entry; coordinators and components receive typed narrow ports.
- Every async result remains scoped by the existing project session, generation, and request revision. Cancellation saves resources; scope checks preserve correctness.
- File operations remain backend-authoritative and must not use optimistic frontend mutations.
- Move one responsibility at a time, keep existing integration tests, and delete duplicate root logic immediately after each coordinator owns it.
- Do not use line count as a completion criterion. Completion is based on ownership, dependency, test, and lifecycle boundaries.
- Every task ends in an independently testable commit and leaves the working tree clean.
- If a task requires a frozen-contract change, a second writable copy of project state, direct coordinator coupling, a new global state mechanism, or a user-visible behavior change, stop and return to architecture review.

---

## File Structure

### Shared workspace contracts

- `ui/src/app/workspace/intents.ts` — closed union of root-routed workspace intents.
- `ui/src/app/workspace/ports.ts` — explicit adapters from `ViewerBridge` to preview, playback, settings, and empty-project ports.
- `ui/src/app/workspace/contracts.test.ts` — type/shape tests for intents and narrow ports.

### Workspace Shell Coordinator

- `ui/src/app/workspace/useWorkspaceShellCoordinator.ts` — sidebar, responsive state, toolbar popovers, settings/info visibility, content view commands, panel preferences, and recovery acknowledgement.
- `ui/src/app/workspace/useWorkspaceShellCoordinator.test.tsx` — session reset, focus, viewport, popover, and command tests.

### Feedback Coordinator

- `ui/src/app/workspace/feedbackModel.ts` — pure scan/operation task and global-notice projection.
- `ui/src/app/workspace/feedbackModel.test.ts` — exact task status, ordering, dismissal, and copy tests.
- `ui/src/app/workspace/useFeedbackCoordinator.ts` — thumbnail/text feedback and dismissed-task session state.
- `ui/src/app/workspace/useFeedbackCoordinator.test.tsx` — session reset and completed/running visibility tests.

### Organization Coordinator

- `ui/src/app/workspace/organizationModel.ts` — selection capabilities, same-folder drop validation, radial model inputs, and operation labels.
- `ui/src/app/workspace/organizationModel.test.ts` — pure organization rules.
- `ui/src/app/workspace/useOrganizationCoordinator.ts` — selection, dialogs, Finder/internal drag, radial actions, review/organization shortcuts, result dialog ownership.
- `ui/src/app/workspace/useOrganizationCoordinator.test.tsx` — command, intent, reset, stale identity, and error-copy tests.

### Viewing Coordinator

- `ui/src/app/workspace/viewingModel.ts` — preview file resolution, video neighbors, compare projection, and repair derivation.
- `ui/src/app/workspace/viewingModel.test.ts` — pure preview/compare/repair rules.
- `ui/src/app/workspace/useViewingCoordinator.ts` — preview session, narrow request functions, comparison, cancellation/repair, and media-session projection.
- `ui/src/app/workspace/useViewingCoordinator.test.tsx` — project-session invalidation, navigation, repair, compare fallback, and request-scope tests.

### Composition and compatibility

- Modify: `ui/src/App.tsx` — invoke coordinators, route typed intents, and render existing components.
- Modify: `ui/src/App.test.tsx` — retain full behavior coverage and add root routing assertions only where an existing behavior lacks coverage.
- Modify: `ui/src/state/useViewerController.ts` — expose Finder drag through the existing controller facade.
- Modify: `ui/src/state/viewerControllerContract.test.tsx` — freeze the expanded internal facade during migration.
- Retain: `ui/src/app/useAppShellState.ts`, `usePreviewSession.ts`, `useOperationDialogs.ts`, `useRadialMenuSession.ts`, and existing focused hooks as coordinator internals.

---

### Task 1: Define typed intents and narrow bridge ports

**Files:**

- Create: `ui/src/app/workspace/intents.ts`
- Create: `ui/src/app/workspace/ports.ts`
- Create: `ui/src/app/workspace/contracts.test.ts`
- Modify: `ui/src/components/videoPreview/useVideoBridge.ts:19-40`
- Read from: `ui/src/api/viewer.ts:68-121`
- Read from: `ui/src/components/videoPreview/useVideoBridge.ts:19-40`

**Interfaces:**

- Consumes: `ViewerBridge`, `BrowserFile`.
- Produces:

```ts
export type WorkspaceIntent =
  | { kind: 'open-preview'; file: BrowserFile; files: readonly BrowserFile[] | null; folderOverviewIdentity: string | null }
  | { kind: 'enter-compare'; files: readonly BrowserFile[] }
  | { kind: 'start-rename'; files: readonly BrowserFile[] }
  | { kind: 'show-operation-results'; batchId: string }
  | { kind: 'open-settings' }
  | { kind: 'open-info' }
  | { kind: 'close-project' }

export type OpenPreviewIntent = Extract<WorkspaceIntent, { kind: 'open-preview' }>
export type WorkspaceIntentSink = (intent: WorkspaceIntent) => void

export type PreviewDataPort = Pick<ViewerBridge,
  'queryFolder' | 'requestImage' | 'previewText' | 'openExternalLink' | 'videoRequestCover'
>
export type VideoPlaybackPort = Pick<ViewerBridge,
  | 'listenVideo'
  | 'videoClose'
  | 'videoCancelOpen'
  | 'videoOpen'
  | 'videoPause'
  | 'videoPlay'
  | 'videoRequestCover'
  | 'videoRequestThumbnail'
  | 'videoSeek'
  | 'videoSetFullscreen'
  | 'videoSetMuted'
  | 'videoSetRate'
  | 'videoSetVolume'
  | 'videoStep'
>
export type SettingsPort = Pick<ViewerBridge, 'videoCacheStats' | 'videoCacheClear'>
export type EmptyProjectPort = Pick<ViewerBridge,
  'chooseProject' | 'openProject' | 'listenProjectDropEvents'
>
export type WorkspaceShellPort = Pick<ViewerBridge, 'revealProjectInFileManager'>

export interface WorkspacePorts {
  preview: PreviewDataPort
  playback: VideoPlaybackPort
  settings: SettingsPort
  emptyProject: EmptyProjectPort
  shell: WorkspaceShellPort
}

export function createWorkspacePorts(bridge: ViewerBridge): WorkspacePorts
```

- [ ] **Step 1: Write failing type and runtime-shape tests**

Create a type test proving exhaustive intent discrimination and a runtime test proving ports expose no unrelated method:

```ts
it('creates explicit narrow ports without forwarding the full bridge object', () => {
  const ports = createWorkspacePorts(fullBridge)
  expect(Object.keys(ports.preview).sort()).toEqual([
    'openExternalLink',
    'previewText',
    'queryFolder',
    'requestImage',
    'videoRequestCover',
  ])
  expect('executeFileCommand' in ports.preview).toBe(false)
  expect(Object.keys(ports.emptyProject).sort()).toEqual([
    'chooseProject',
    'listenProjectDropEvents',
    'openProject',
  ])
  expect('closeProject' in ports.emptyProject).toBe(false)
})

function exhaust(intent: WorkspaceIntent): string {
  switch (intent.kind) {
    case 'open-preview': return intent.file.entityId
    case 'enter-compare': return String(intent.files.length)
    case 'start-rename': return String(intent.files.length)
    case 'show-operation-results': return intent.batchId
    case 'open-settings': return intent.kind
    case 'open-info': return intent.kind
    case 'close-project': return intent.kind
  }
}
```

Use the existing `ViewerBridge` fake pattern from `ui/src/App.test.tsx`; do not create a second production bridge implementation.

- [ ] **Step 2: Run the focused test and verify failure**

```bash
pnpm --dir ui exec vitest run src/app/workspace/contracts.test.ts
```

Expected: FAIL because the workspace contracts do not exist.

- [ ] **Step 3: Implement explicit method-copy adapters**

Do not return `bridge` under a narrower TypeScript annotation. Construct each object explicitly so runtime consumers cannot reach unrelated methods:

```ts
export function createWorkspacePorts(bridge: ViewerBridge): WorkspacePorts {
  return {
    preview: {
      queryFolder: bridge.queryFolder,
      requestImage: bridge.requestImage,
      previewText: bridge.previewText,
      openExternalLink: bridge.openExternalLink,
      videoRequestCover: bridge.videoRequestCover,
    },
    playback: {
      listenVideo: bridge.listenVideo,
      videoClose: bridge.videoClose,
      videoCancelOpen: bridge.videoCancelOpen,
      videoOpen: bridge.videoOpen,
      videoPause: bridge.videoPause,
      videoPlay: bridge.videoPlay,
      videoRequestCover: bridge.videoRequestCover,
      videoRequestThumbnail: bridge.videoRequestThumbnail,
      videoSeek: bridge.videoSeek,
      videoSetFullscreen: bridge.videoSetFullscreen,
      videoSetMuted: bridge.videoSetMuted,
      videoSetRate: bridge.videoSetRate,
      videoSetVolume: bridge.videoSetVolume,
      videoStep: bridge.videoStep,
    },
    settings: {
      videoCacheStats: bridge.videoCacheStats,
      videoCacheClear: bridge.videoCacheClear,
    },
    emptyProject: {
      chooseProject: bridge.chooseProject,
      openProject: bridge.openProject,
      listenProjectDropEvents: bridge.listenProjectDropEvents,
    },
    shell: {
      revealProjectInFileManager: bridge.revealProjectInFileManager,
    },
  }
}
```

`EmptyProjectPort` contains exactly the three methods used by the component, including its existing `openProject` fallback. Do not pass `ViewerBridge` wholesale.
In `useVideoBridge.ts`, use a type-only import and preserve its public compatibility name without reversing the dependency:

```ts
import type { VideoPlaybackPort } from '../../app/workspace/ports'

export type VideoPreviewBridge = VideoPlaybackPort
```

- [ ] **Step 4: Run contracts, typecheck, and the architecture boundary**

```bash
pnpm --dir ui exec vitest run src/app/workspace/contracts.test.ts
pnpm --dir ui exec vitest run src/components/videoPreview/useVideoBridge.test.tsx
pnpm --dir ui typecheck
pnpm architecture:boundaries
```

Expected: PASS with no production UI Tauri import violation.

- [ ] **Step 5: Commit the shared contracts**

```bash
git add ui/src/app/workspace/intents.ts ui/src/app/workspace/ports.ts ui/src/app/workspace/contracts.test.ts ui/src/components/videoPreview/useVideoBridge.ts
git commit -m "refactor: define workspace coordinator contracts"
```

---

### Task 2: Extract the Workspace Shell Coordinator

**Files:**

- Create: `ui/src/app/workspace/useWorkspaceShellCoordinator.ts`
- Create: `ui/src/app/workspace/useWorkspaceShellCoordinator.test.tsx`
- Modify: `ui/src/App.tsx:143-217,393-427,998-1020,1021-1120,1260-1308`
- Reuse: `ui/src/app/useAppShellState.ts`
- Reuse: `ui/src/app/useOtherFilePanelPreference.ts`
- Reuse: `ui/src/app/useVideoPanelPreference.ts`
- Reuse: `ui/src/app/useToolbarPopover.ts`

**Interfaces:**

- Consumes:

```ts
export interface WorkspaceShellOptions {
  projectSessionId: string
  videoProjectSessionId: string | null
  port: WorkspaceShellPort
}
```

- Produces:

```ts
export interface WorkspaceShellCoordinator {
  sidebarCollapsed: boolean
  sidebarWidth: number
  narrowViewport: boolean
  effectiveSidebarCollapsed: boolean
  toggleSidebar(): void
  startSidebarResize: AppShellState['startSidebarResize']
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
```

Export the resize callback directly from `useAppShellState` or expose a public `onSidebarKeyDown`; remove the WeakMap internal accessor only if its existing tests and consumers are migrated in the same commit.

- [ ] **Step 1: Write failing shell lifecycle tests**

Cover the existing synchronous values and effect timing:

```ts
it('resets shell overlays and panel preferences by the existing session rules', () => {
  const hook = renderHook(
    ({ sessionId }) => useWorkspaceShellCoordinator({
      projectSessionId: sessionId,
      videoProjectSessionId: sessionId,
      port: { revealProjectInFileManager: vi.fn().mockResolvedValue(undefined) },
    }),
    { initialProps: { sessionId: 'session-1' } },
  )
  act(() => {
    hook.result.current.openInfo()
    hook.result.current.openSettings()
  })
  hook.rerender({ sessionId: 'session-2' })
  expect(hook.result.current.infoOpen).toBe(false)
  expect(hook.result.current.settingsOpen).toBe(false)
})
```

Also test the 760px narrow threshold, 52px effective rail behavior, ArrowLeft/ArrowRight resize, mutually exclusive toolbar popovers, and monotonic `requestId`.

- [ ] **Step 2: Run the shell test and verify failure**

```bash
pnpm --dir ui exec vitest run src/app/workspace/useWorkspaceShellCoordinator.test.tsx
```

Expected: FAIL because the coordinator does not exist.

- [ ] **Step 3: Compose existing hooks without changing their semantics**

Move these exact ownership blocks from `ViewerWorkspace`:

- `useAppShellState`, narrow viewport listener, and effective collapse calculation;
- other/video panel preferences and toolbar popover;
- `infoOpen`, `settingsOpen`, content select-all request/command, request counter;
- recovery acknowledgement;
- workspace reveal invocation and its exact existing fallback message;
- project-session resets for shell-owned state.

Keep the current `window.innerWidth <= 760`, sidebar bounds `200..420`, step `16`, and effect-based session reset. Do not introduce persistence outside the current hooks.
Preserve the current reveal-error lifetime: clear `workspaceActionError` immediately before a new reveal request, but do not add it to the project-session reset effect. Likewise, keep recovery acknowledgement keyed by session comparison rather than force-resetting the stored ID.

- [ ] **Step 4: Rewire `ViewerWorkspace` to the shell output**

Replace individual shell hooks/states with one call:

```ts
const ports = useMemo(() => createWorkspacePorts(bridge), [bridge])
const shell = useWorkspaceShellCoordinator({
  projectSessionId,
  videoProjectSessionId: state.project?.sessionId ?? null,
  port: ports.shell,
})
```

Preserve existing component props and text exactly. This intermediate call may still receive values owned by the root; later tasks replace them with coordinator outputs.

- [ ] **Step 5: Run shell and App regression tests**

```bash
pnpm --dir ui exec vitest run src/app/workspace/useWorkspaceShellCoordinator.test.tsx src/app/appSessionCoordinators.test.tsx src/App.test.tsx
pnpm --dir ui check
```

Expected: all existing shell, settings, focus, sidebar, and viewport tests PASS without snapshot/copy changes.

- [ ] **Step 6: Commit the shell extraction**

```bash
git add ui/src/App.tsx ui/src/app/useAppShellState.ts ui/src/app/appSessionCoordinators.test.tsx ui/src/app/workspace/useWorkspaceShellCoordinator.ts ui/src/app/workspace/useWorkspaceShellCoordinator.test.tsx
git commit -m "refactor: extract workspace shell coordinator"
```

---

### Task 3: Extract feedback projections and session state

**Files:**

- Create: `ui/src/app/workspace/feedbackModel.ts`
- Create: `ui/src/app/workspace/feedbackModel.test.ts`
- Create: `ui/src/app/workspace/useFeedbackCoordinator.ts`
- Create: `ui/src/app/workspace/useFeedbackCoordinator.test.tsx`
- Modify: `ui/src/App.tsx:177-179,203-217,251-319,782-788,973-997,1228-1259`

**Interfaces:**

- Consumes:

```ts
export interface FeedbackCoordinatorOptions {
  projectSessionId: string
  scan: ViewerState['scan']
  operation: ViewerState['operation']
  projectError: string | null
  finderDragMessage: string | null
  workspaceActionError: string | null
}
```

- Produces:

```ts
export function scanTaskFromState(scan: ViewerState['scan']): TaskFeedback | null
export function operationTaskFromState(operation: ViewerState['operation']): TaskFeedback | null
export function visibleTaskFeedback(
  candidates: readonly (TaskFeedback | null)[],
  dismissed: ReadonlySet<string>,
): TaskFeedback[]
export function buildGlobalNotices(input: {
  projectError: string | null
  finderDragMessage: string | null
  workspaceActionError: string | null
}): GlobalNotice[]

export interface FeedbackCoordinator {
  visibleTasks: TaskFeedback[]
  globalNotices: GlobalNotice[]
  setThumbnailTask(task: TaskFeedback | null): void
  setTextTask(task: TaskFeedback | null): void
  dismissTask(taskId: string): void
}
```

The coordinator does not consume `ViewerBridge` or `ViewerController` and cannot start, retry, or cancel a task.

- [ ] **Step 1: Write failing pure projection tests**

Copy the exact current mapping expectations from `App.tsx` into table-driven tests, including:

```ts
expect(scanTaskFromState(cancelledScan)).toMatchObject({
  label: '扫描项目',
  status: 'cancelled',
  cancellable: false,
})
expect(operationTaskFromState(allCancelledOperation)).toMatchObject({
  label: '移动文件',
  status: 'cancelled',
})
expect(visibleTaskFeedback([running, complete], new Set())).toEqual([running])
```

Assert the exact existing Chinese notice titles and messages for Finder drag, workspace reveal, and project errors. Keep `settingsError` local to `SettingsDialog`; do not add a new global settings notice.

- [ ] **Step 2: Run model tests and verify failure**

```bash
pnpm --dir ui exec vitest run src/app/workspace/feedbackModel.test.ts
```

Expected: FAIL because the pure projection module does not exist.

- [ ] **Step 3: Move projections byte-for-byte before simplifying**

Move `scanTask`, `operationTask`, `visibleTasks`, `operationLabel`, and `globalNotices` construction from `App.tsx` into pure functions. Preserve condition order so running tasks, completed tasks with results, and failure precedence remain identical.

- [ ] **Step 4: Write and implement coordinator session tests**

Test that thumbnail/text tasks and dismissals reset when `projectSessionId` changes, while a running task reappears even when previously dismissed. Implement only `useState`, `useEffect`, and the pure functions; do not call controller commands.

- [ ] **Step 5: Rewire TaskBar and notices**

Use coordinator setters for `ContentBrowser.onThumbnailTaskChange` and `TextPreview.onTaskChange`. Keep `TaskBar.onCancel` routed by the root to existing controller commands. Keep the existing root-owned results handler during this task; Organization takes ownership in Task 4 and typed intent routing begins in Task 5.

- [ ] **Step 6: Run feedback and App tests**

```bash
pnpm --dir ui exec vitest run src/app/workspace/feedbackModel.test.ts src/app/workspace/useFeedbackCoordinator.test.tsx src/App.test.tsx
pnpm --dir ui check
```

Expected: all taskbar, error-copy, Finder-drag, local settings-error, and operation-result tests PASS.

- [ ] **Step 7: Commit the feedback extraction**

```bash
git add ui/src/App.tsx ui/src/app/workspace/feedbackModel.ts ui/src/app/workspace/feedbackModel.test.ts ui/src/app/workspace/useFeedbackCoordinator.ts ui/src/app/workspace/useFeedbackCoordinator.test.tsx
git commit -m "refactor: extract workspace feedback coordinator"
```

---

### Task 4: Move selection, dialogs, and Finder export behind Organization

**Files:**

- Create: `ui/src/app/workspace/organizationModel.ts`
- Create: `ui/src/app/workspace/organizationModel.test.ts`
- Create: `ui/src/app/workspace/useOrganizationCoordinator.ts`
- Create: `ui/src/app/workspace/useOrganizationCoordinator.test.tsx`
- Modify: `ui/src/state/useViewerController.ts:16-142`
- Modify: `ui/src/state/viewerControllerContract.test.tsx:4-43`
- Modify: `ui/src/App.tsx:170-190,325-347,429-560,782-788,1309-1458`
- Reuse: `ui/src/app/useOperationDialogs.ts`

**Interfaces:**

- Add to `ViewerController`:

```ts
beginFinderDrag(entityIds: readonly string[]): Promise<void>
```

- Organization consumes:

```ts
export type OrganizationCommands = Pick<ViewerController,
  | 'setSelectedEntityIds'
  | 'setReviewState'
  | 'toggleFavorite'
  | 'previewRename'
  | 'preflightFileCommand'
  | 'executeFileCommand'
  | 'cancelOperation'
  | 'loadOperationResults'
  | 'undoLastOperation'
  | 'consumeContextRepair'
  | 'beginFinderDrag'
>

export interface OrganizationOptions {
  state: ViewerState
  commands: OrganizationCommands
}

export type OrganizationFileCommandKind = 'rename' | 'copy' | 'move' | 'trash'
```

- Initial Organization output:

```ts
export interface OrganizationCoordinator {
  selectedFiles: BrowserFile[]
  selectFiles(files: BrowserFile[]): void
  operationDialog: OperationDialog | null
  operationSubmitting: boolean
  operationBusy: boolean
  canMutateSelection: boolean
  finderDragMessage: string | null
  clearFinderDragMessage(): void
  exportToFinder(entityIds: string[]): void
  openRenameDialog(files?: readonly BrowserFile[]): void
  openTrashDialog(files?: readonly BrowserFile[]): void
  submitFileCommand(kind: OrganizationFileCommandKind, items: FileCommandItem[], conflicts?: ConflictResolution[]): Promise<boolean>
  resultsBatchId: string | null
  showResults(batchId: string): void
  closeResults(): void
}
```

- [ ] **Step 1: Write the controller facade test first**

Add `'beginFinderDrag'` to `ExpectedCommands` and a hook test that verifies the existing request shape:

```ts
await result.current.beginFinderDrag(['image-1', 'video-1'])
expect(viewer.beginFinderDrag).toHaveBeenCalledWith({
  sessionId: 'session-1',
  generation: 1,
  entityIds: ['image-1', 'video-1'],
})
```

Also assert that empty selection or a non-active/no-project state makes no bridge call.

- [ ] **Step 2: Run the controller tests and verify failure**

```bash
pnpm --dir ui exec vitest run src/state/viewerControllerContract.test.tsx src/state/useViewerController.test.tsx
```

Expected: FAIL because `beginFinderDrag` is absent.

- [ ] **Step 3: Add the thin controller command**

Implement with the current render’s project identity and preserve error propagation:

```ts
const beginFinderDrag = useCallback(async (entityIds: readonly string[]) => {
  const project = state.project
  if (project === null || state.status !== 'active' || entityIds.length === 0) return
  await bridge.beginFinderDrag({
    sessionId: project.sessionId,
    generation: project.generation,
    entityIds: [...entityIds],
  })
}, [bridge, state.project, state.status])
```

Return it from `useViewerController`; do not translate its error because Organization must preserve the current Finder-specific copy.

- [ ] **Step 4: Write organization model and hook tests**

Cover exact existing rules:

- `operationBusy` is true for submitting, non-active project, finishing, pending, queued, or running operation;
- mutation requires non-empty selection, read-write access, and not busy;
- project-session change clears selection, Finder message, dialog, result batch, and submitting state;
- Finder error code `finder_drag_selection_stale` or `stale_project_session` maps to `部分文件已发生变化，请刷新后重试。`; other errors map to `无法拖到 Finder，请重新拖动。`;
- completed operation results set the result batch exactly once;
- `submitFileCommand` always clears submitting in `finally` and closes the dialog only when the controller reports `true`.

- [ ] **Step 5: Move the root blocks without semantic edits**

Move selection state, `useOperationDialogs`, Finder export, rename/trash openers, `submitFileCommand`, operation result ownership, and dialog props into the new coordinator. Keep dialog components rendered by `ViewerWorkspace`; pass coordinator state/handlers as props.

Construct Organization before Feedback in this intermediate commit and replace the former root Finder-message argument with `organization.finderDragMessage`; keep `shell.workspaceActionError` and the existing project error input unchanged. This preserves the Task 3 notice projection while transferring Finder-message ownership exactly once.

During this task, the root calls `organization.showResults` and `organization.openRenameDialog` directly. Task 5 introduces the typed intent sink when cross-domain radial and keyboard actions move into Organization.

- [ ] **Step 6: Run targeted and App organization tests**

```bash
pnpm --dir ui exec vitest run src/app/workspace/organizationModel.test.ts src/app/workspace/useOrganizationCoordinator.test.tsx src/state/useViewerController.test.tsx src/state/viewerControllerContract.test.tsx src/App.test.tsx
pnpm --dir ui check
```

Expected: all selection, rename, copy, move, Trash, undo, result, Finder export, read-only, and busy-state tests PASS.

- [ ] **Step 7: Commit the organization core**

```bash
git add ui/src/App.tsx ui/src/app/workspace/organizationModel.ts ui/src/app/workspace/organizationModel.test.ts ui/src/app/workspace/useOrganizationCoordinator.ts ui/src/app/workspace/useOrganizationCoordinator.test.tsx ui/src/state/useViewerController.ts ui/src/state/useViewerController.test.tsx ui/src/state/viewerControllerContract.test.tsx
git commit -m "refactor: move file organization behind coordinator"
```

---

### Task 5: Move internal drag, radial actions, and shortcuts into Organization

**Files:**

- Modify: `ui/src/app/workspace/organizationModel.ts`
- Modify: `ui/src/app/workspace/organizationModel.test.ts`
- Modify: `ui/src/app/workspace/useOrganizationCoordinator.ts`
- Modify: `ui/src/app/workspace/useOrganizationCoordinator.test.tsx`
- Modify: `ui/src/App.tsx:144-152,348-392,489-579,691-754,834-917,1185-1227`
- Modify: `ui/src/state/comparePolicy.ts`
- Modify: `ui/src/state/comparePolicy.test.ts`
- Reuse: `ui/src/app/useRadialMenuSession.ts`
- Reuse: `ui/src/state/useOrganizationPointerDrag.ts`
- Reuse: `ui/src/state/useReviewShortcuts.tsx`

**Interfaces:**

Extend `OrganizationOptions` with:

```ts
emitIntent: WorkspaceIntentSink
compareOpen: boolean
activePreviewOpen: boolean
infoOpen: boolean
```

Extend `OrganizationCoordinator` with:

```ts
radialMenu: ReturnType<typeof useRadialMenuSession>['radialMenu']
activeRadialMenu: ReturnType<typeof useRadialMenuSession>['activeRadialMenu']
radialModel: ReturnType<typeof buildRadialMenuModel>
beginRadialSession: ReturnType<typeof useRadialMenuSession>['beginRadialSession']
finishRadialSession: ReturnType<typeof useRadialMenuSession>['finishRadialSession']
runRadialAction(action: RadialLeafAction): void
organizationDragView: OrganizationDragView | null
organizationDropTarget: OrganizationDropTarget | null
handleOrganizationPointerInput(input: OrganizationPointerInput): void
```

Add a shared command-availability policy used by Organization now and Viewing in Task 7:

```ts
export type CompareEntryAvailability = 'available' | 'busy' | 'folder-context-required'

export function compareEntryAvailability(input: {
  workspace: ViewerState['workspace']
  searchResultsOpen: boolean
  operationBusy: boolean
}): CompareEntryAvailability
```

Do not export implementation-only identity strings.

- [ ] **Step 1: Add failing intent-routing tests**

Using a fake `emitIntent`, assert:

```ts
expect(intents).toContainEqual({
  kind: 'open-preview',
  file: selectedImage,
  files: null,
  folderOverviewIdentity: null,
})
expect(intents).toContainEqual({ kind: 'enter-compare', files: selectedImages })
expect(intents).toContainEqual({ kind: 'start-rename', files: selectedFiles })
expect(intents).toContainEqual({ kind: 'open-info' })
```

Also assert marker/favorite actions call controller commands directly because they remain inside the Organization domain.

- [ ] **Step 2: Run the hook test and verify failure**

```bash
pnpm --dir ui exec vitest run src/app/workspace/useOrganizationCoordinator.test.tsx
```

Expected: FAIL because radial, drag, and shortcut ownership is not yet present.

- [ ] **Step 3: Move pure organization rules**

Move `parentRelativePath`, same-folder drop validation, radial model input construction, and organization workspace identity into `organizationModel.ts`. Preserve the exact current file set (`images`, `videos`, `otherFiles`) and copy-vs-move behavior.

- [ ] **Step 4: Move the interaction hooks and window listeners**

Move:

- `useRadialMenuContextToken` and `useRadialMenuSession`;
- `useOrganizationPointerDrag`, reset key, drop preflight, and workspace-change cancellation;
- `handleOrganizationShortcut` with Space, C, Enter, Delete/Backspace, and Command-Z ownership;
- the existing Command-I listener, which emits `open-info` after applying the same editable/modal ownership rules;
- `useReviewShortcuts` ownership and disabled conditions.

Cross-domain actions emit typed intents. Do not call Viewing or Shell directly. Use `compareEntryAvailability(...) === 'available'` for the radial compare-enabled flag; keep validation and user-facing compare error copy in Viewing.

Create the root-local `WorkspaceIntentSink` ref described in Task 8 with handlers for `open-preview`, `enter-compare`, `start-rename`, and `open-info`; later tasks add the remaining cases to the same exhaustive switch. Do not introduce intent state or an asynchronous queue.

Use this stable sink shape; during Task 5 the preview/compare cases delegate to the still-root-owned handlers, and Tasks 6-7 retarget them to Viewing:

```ts
const intentTargetRef = useRef<WorkspaceIntentSink>(() => undefined)
const emitIntent = useCallback<WorkspaceIntentSink>((intent) => {
  intentTargetRef.current(intent)
}, [])

intentTargetRef.current = (intent) => {
  switch (intent.kind) {
    case 'open-preview':
      openWorkspacePreviewIntent(intent)
      return
    case 'enter-compare':
      openComparison([...intent.files])
      return
    case 'start-rename':
      organization.openRenameDialog(intent.files)
      return
    case 'show-operation-results':
      organization.showResults(intent.batchId)
      return
    case 'open-settings':
      shell.openSettings()
      return
    case 'open-info':
      shell.openInfo()
      return
    case 'close-project':
      void closeProject()
  }
}
```

Implement the temporary adapter with existing root functions and delete it when Viewing exposes `openIntentPreview` in Task 6:

```ts
const openWorkspacePreviewIntent = useCallback((intent: OpenPreviewIntent) => {
  if (isVideoFile(intent.file)) {
    openVideoPreview(intent.file.entityId)
    return
  }
  setPreviewRepair(EMPTY_PREVIEW_REPAIR)
  openPreviewSession({
    file: intent.file,
    files: intent.files === null ? null : [...intent.files],
    folderOverviewIdentity: intent.folderOverviewIdentity,
  })
  setPreviewEntityId(intent.file.entityId)
}, [openPreviewSession, openVideoPreview, setPreviewEntityId])
```

- [ ] **Step 5: Preserve stale-session and focus behavior**

Keep radial project identity as session plus generation, include context repair in the radial invalidation token, call `finishRadialSession` before emitting an action, and retain focus restoration through the existing radial hook. Keep pointer-drag cancellation on both identity and workspace projection changes.

- [ ] **Step 6: Run the full organization regression slice**

```bash
pnpm --dir ui exec vitest run src/app/workspace/organizationModel.test.ts src/app/workspace/useOrganizationCoordinator.test.tsx src/app/appSessionCoordinators.test.tsx src/state/useOrganizationPointerDrag.test.tsx src/App.test.tsx
pnpm --dir ui check
```

Expected: all radial click/dwell/control-click, Finder/internal drag, same-folder validation, review shortcut, modal ownership, focus restoration, and stale generation tests PASS.

- [ ] **Step 7: Commit the interaction extraction**

```bash
git add ui/src/App.tsx ui/src/app/workspace/organizationModel.ts ui/src/app/workspace/organizationModel.test.ts ui/src/app/workspace/useOrganizationCoordinator.ts ui/src/app/workspace/useOrganizationCoordinator.test.tsx
git commit -m "refactor: centralize organization interactions"
```

---

### Task 6: Extract preview sessions, requests, and repair into Viewing

**Files:**

- Create: `ui/src/app/workspace/viewingModel.ts`
- Create: `ui/src/app/workspace/viewingModel.test.ts`
- Create: `ui/src/app/workspace/useViewingCoordinator.ts`
- Create: `ui/src/app/workspace/useViewingCoordinator.test.tsx`
- Modify: `ui/src/App.tsx:78-83,160-169,190,218-250,579-659,789-833,931-963,1121-1184,1277-1333`
- Reuse: `ui/src/app/usePreviewSession.ts`
- Reuse: `ui/src/app/projectThumbnailCache.ts`

**Interfaces:**

- Consumes:

```ts
export type ViewingCommands = Pick<ViewerController,
  'setPreviewEntityId' | 'setCompareEntityIds'
>

export interface ViewingOptions {
  state: ViewerState
  projectSessionId: string
  port: PreviewDataPort
  playbackPort: VideoPlaybackPort
  commands: ViewingCommands
}
```

- Produces initially:

```ts
export interface ViewingCoordinator {
  activePreview: PreviewSession | null
  activePreviewFile: BrowserFile | null
  activePreviewFiles: BrowserFile[]
  activeTextPreviewFiles: TextPreviewFiles | null
  folderOverviewIdentity: string
  unavailablePreviewEntityIds: ReadonlySet<string>
  dimensions: Record<string, { width: number; height: number } | undefined>
  requestThumbnail: ThumbnailLoader
  requestFolderImages(entityId: string): Promise<BrowserFile[]>
  requestPreviewImage(file: BrowserFile, representation: ImageRepresentationRequest, signal?: AbortSignal): ReturnType<PreviewDataPort['requestImage']>
  requestTextPreview(file: BrowserFile, encoding?: TextEncoding): ReturnType<PreviewDataPort['previewText']>
  requestVideoCover(entityId: string): Promise<string>
  openExternalLink: PreviewDataPort['openExternalLink']
  openPreview(file: BrowserFile): void
  openFilmstripPreview(file: BrowserFile, files: BrowserFile[]): void
  openIntentPreview(intent: OpenPreviewIntent): void
  navigatePreview(file: BrowserFile): void
  closePreview(): void
  recordDimensions(entityId: string, width: number, height: number): void
  playbackPort: VideoPlaybackPort
}
```

- [ ] **Step 1: Write failing pure viewing-model tests**

Cover:

- video neighbors are restricted to videos and respect current search hits;
- unsupported files resolve without image/text requests;
- split text preview retains exactly two ordered files;
- removed preview entities become unavailable without replacing remaining session order;
- active file resolution falls back to the session file and marks removed IDs unavailable, matching the current repair behavior.

- [ ] **Step 2: Write failing hook lifecycle tests**

Use deferred promises and session rerenders:

```ts
it('invalidates preview state and cache ownership when the project session changes', () => {
  const hook = renderHook(({ sessionId }) => useViewingCoordinator(options(sessionId)), {
    initialProps: { sessionId: 'session-1' },
  })
  act(() => hook.result.current.openPreview(image1))
  hook.rerender({ sessionId: 'session-2' })
  expect(hook.result.current.activePreview).toBeNull()
  expect(hook.result.current.unavailablePreviewEntityIds.size).toBe(0)
})
```

Also test stale navigation cannot reopen a closed preview, folder-overview identity invalidation, context repair accumulation, and request cancellation forwarding to `requestImage`.

- [ ] **Step 3: Run the new tests and verify failure**

```bash
pnpm --dir ui exec vitest run src/app/workspace/viewingModel.test.ts src/app/workspace/useViewingCoordinator.test.tsx
```

Expected: FAIL because the Viewing modules do not exist.

- [ ] **Step 4: Move the preview and request blocks**

Compose `usePreviewSession` and `createProjectThumbnailCache` inside Viewing. Move the current folder-overview projection sequence/identity, request wrappers, video-neighbor calculation, preview open/navigate/close handlers, preview repair effect, unavailable ID selector, active preview/text file derivation, and folder-overview invalidation without changing condition order or copy.

Use only `PreviewDataPort` and `VideoPlaybackPort`; do not accept `ViewerBridge`.

Implement `openIntentPreview` as the final typed adapter:

```ts
const openIntentPreview = useCallback((intent: OpenPreviewIntent) => {
  if (isVideoFile(intent.file)) {
    openVideoPreview(intent.file.entityId)
    return
  }
  setPreviewRepair(EMPTY_PREVIEW_REPAIR)
  openPreviewSession({
    file: intent.file,
    files: intent.files === null ? null : [...intent.files],
    folderOverviewIdentity: intent.folderOverviewIdentity,
  })
  commands.setPreviewEntityId(intent.file.entityId)
}, [commands, openPreviewSession, openVideoPreview])
```

Reuse `OpenPreviewIntent` from `intents.ts`; do not introduce a second payload type.

- [ ] **Step 5: Rewire preview components through Viewing**

Pass:

- `viewing.requestPreviewImage` to image/compare views;
- `viewing.requestTextPreview` and `viewing.openExternalLink` to text preview;
- `viewing.playbackPort` to `VideoPreview`;
- Viewing-owned open/close/navigation/repair values to existing components.

Preserve all existing keys, `pointerClientPoint`, magnifier preferences, task callbacks, and component mount behavior.

- [ ] **Step 6: Run preview and App regression tests**

```bash
pnpm --dir ui exec vitest run src/app/workspace/viewingModel.test.ts src/app/workspace/useViewingCoordinator.test.tsx src/app/appSessionCoordinators.test.tsx src/components/VideoPreview.test.tsx src/components/videoPreview/useVideoBridge.test.tsx src/App.test.tsx
pnpm --dir ui check
```

Expected: all image, text, video, filmstrip, unsupported-file, repair, cancellation, and project-switch tests PASS.

- [ ] **Step 7: Commit the preview extraction**

```bash
git add ui/src/App.tsx ui/src/app/workspace/viewingModel.ts ui/src/app/workspace/viewingModel.test.ts ui/src/app/workspace/useViewingCoordinator.ts ui/src/app/workspace/useViewingCoordinator.test.tsx
git commit -m "refactor: extract workspace viewing coordinator"
```

---

### Task 7: Move comparison into Viewing

**Files:**

- Modify: `ui/src/app/workspace/viewingModel.ts`
- Modify: `ui/src/app/workspace/viewingModel.test.ts`
- Modify: `ui/src/app/workspace/useViewingCoordinator.ts`
- Modify: `ui/src/app/workspace/useViewingCoordinator.test.tsx`
- Modify: `ui/src/App.tsx:348-358,660-690,755-781,943-965,1140-1184`
- Reuse: `ui/src/state/comparePolicy.ts`

**Interfaces:**

Extend `ViewingCoordinator`:

```ts
compareFiles: BrowserFile[]
compareOpen: boolean
compareStatus: string | null
enterCompare(files: readonly BrowserFile[], operationBusy: boolean): void
changeComparedEntities(entityIds: string[]): void
setCompareStatus(message: string | null): void
```

- [ ] **Step 1: Add failing comparison ownership tests**

Cover the current exact behavior:

- invalid count/type produces `compareValidationMessage` without changing IDs;
- busy state produces `请等待当前文件操作完成后再开始对比。`;
- wrong workspace produces `请先返回文件夹内容，再选择图片进行对比。`;
- successful compare closes preview, clears preview repair/entity, then sets ordered IDs;
- falling to one live image closes compare and opens that survivor as preview;
- entering search closes compare;
- external removal repairs compare IDs without reordering survivors.

- [ ] **Step 2: Run tests and verify failure**

```bash
pnpm --dir ui exec vitest run src/app/workspace/viewingModel.test.ts src/app/workspace/useViewingCoordinator.test.tsx
```

Expected: FAIL because comparison is still root-owned.

- [ ] **Step 3: Move comparison state and effects unchanged**

Move `compareFiles`, `compareOpen`, `compareStatus`, `openComparison`, `changeComparedEntities`, and the two compare-repair effects into Viewing. Reuse `compareEntryAvailability`, `validateCompareCandidates`, and `compareValidationMessage`; do not duplicate their rules. `enterCompare` receives the current Organization `operationBusy` value at intent-routing time, avoiding a coordinator construction cycle.

- [ ] **Step 4: Route comparison through typed intents**

The root handles `{ kind: 'enter-compare', files }` by calling `viewing.enterCompare(files, organization.operationBusy)`. Organization emits the intent from keyboard and radial actions. `CompareWorkspace` continues to call `viewing.changeComparedEntities` and `viewing.setCompareStatus`.

- [ ] **Step 5: Run comparison and App tests**

```bash
pnpm --dir ui exec vitest run src/app/workspace/viewingModel.test.ts src/app/workspace/useViewingCoordinator.test.tsx src/state/comparePolicy.test.ts src/components/CompareWorkspace.test.tsx src/App.test.tsx
pnpm --dir ui check
```

Expected: all 2-to-8 item, invalid selection, cancellation, search transition, marker, layout, survivor-preview, and mounted-grid tests PASS.

- [ ] **Step 6: Commit comparison ownership**

```bash
git add ui/src/App.tsx ui/src/app/workspace/viewingModel.ts ui/src/app/workspace/viewingModel.test.ts ui/src/app/workspace/useViewingCoordinator.ts ui/src/app/workspace/useViewingCoordinator.test.tsx
git commit -m "refactor: move comparison into viewing coordinator"
```

---

### Task 8: Make `ViewerWorkspace` a typed composition root

**Files:**

- Modify: `ui/src/App.tsx`
- Modify: `ui/src/App.test.tsx`
- Modify: `ui/src/components/EmptyProject.tsx`
- Modify: `ui/src/components/EmptyProject.test.tsx`
- Modify: `ui/src/components/SettingsDialog.tsx`
- Modify: `ui/src/components/SettingsDialog.test.tsx`
- Modify: `ui/src/components/VideoPreview.tsx`
- Modify: `ui/src/app/workspace/contracts.test.ts`
- Modify: `ui/src/state/viewerControllerContract.test.tsx`

**Interfaces:**

- Consumes: all four coordinator outputs and `WorkspaceIntent`.
- Produces: one root-local typed router; no coordinator imports another coordinator.

- [ ] **Step 1: Add a failing root ownership test and retain behavior coverage**

First extend `contracts.test.ts` with source-level ownership assertions that fail while duplicate root logic remains:

```ts
import { readFileSync } from 'node:fs'

const appSource = readFileSync(new URL('../../App.tsx', import.meta.url), 'utf8')
expect(appSource).not.toMatch(/useState<TaskFeedback|useOperationDialogs|usePreviewSession|useOrganizationPointerDrag/)
expect(appSource).toMatch(/WorkspaceIntentSink/)
expect(appSource).toMatch(/useWorkspaceShellCoordinator/)
expect(appSource).toMatch(/useFeedbackCoordinator/)
expect(appSource).toMatch(/useOrganizationCoordinator/)
expect(appSource).toMatch(/useViewingCoordinator/)
```

Retain the existing App tests that use user events to verify:

1. Space opens the selected preview;
2. C enters compare for a valid selection;
3. Enter opens rename;
4. TaskBar results opens the existing results dialog;
5. More opens settings;
6. project close still calls the same bridge command.

Behavior tests assert visible UI and bridge/controller effects only; the source test asserts only the approved ownership/import boundary.

- [ ] **Step 2: Run the ownership and root integration slice**

```bash
pnpm --dir ui exec vitest run src/app/workspace/contracts.test.ts src/App.test.tsx
```

Expected: the new ownership assertion FAILS until duplicate root hooks are removed; all pre-existing behavior assertions remain passing.

- [ ] **Step 3: Retarget the typed intent sink to final coordinator owners**

Reuse the exhaustive root-local ref introduced in Task 5, remove `openWorkspacePreviewIntent`, and ensure every case delegates to its final coordinator owner without adding async intent state:

```ts
const intentTargetRef = useRef<WorkspaceIntentSink>(() => undefined)
const emitIntent = useCallback<WorkspaceIntentSink>((intent) => {
  intentTargetRef.current(intent)
}, [])

const shell = useWorkspaceShellCoordinator({
  projectSessionId,
  videoProjectSessionId: state.project?.sessionId ?? null,
  port: ports.shell,
})
const viewing = useViewingCoordinator({
  state,
  projectSessionId,
  port: ports.preview,
  playbackPort: ports.playback,
  commands: { setPreviewEntityId, setCompareEntityIds },
})
const organization = useOrganizationCoordinator({
  state,
  commands: {
    setSelectedEntityIds,
    setReviewState,
    toggleFavorite,
    previewRename,
    preflightFileCommand,
    executeFileCommand,
    cancelOperation,
    loadOperationResults,
    undoLastOperation,
    consumeContextRepair,
    beginFinderDrag,
  },
  emitIntent,
  compareOpen: viewing.compareOpen,
  activePreviewOpen: viewing.activePreview !== null,
  infoOpen: shell.infoOpen,
})
const feedback = useFeedbackCoordinator({
  projectSessionId,
  scan: state.scan,
  operation: state.operation,
  projectError: state.errorMessage,
  finderDragMessage: organization.finderDragMessage,
  workspaceActionError: shell.workspaceActionError,
})

intentTargetRef.current = (intent) => {
  switch (intent.kind) {
    case 'open-preview':
      viewing.openIntentPreview(intent)
      return
    case 'enter-compare':
      viewing.enterCompare(intent.files, organization.operationBusy)
      return
    case 'start-rename':
      organization.openRenameDialog(intent.files)
      return
    case 'show-operation-results':
      organization.showResults(intent.batchId)
      return
    case 'open-settings':
      shell.openSettings()
      return
    case 'open-info':
      shell.openInfo()
      return
    case 'close-project':
      void closeProject()
  }
}
```

`openIntentPreview` is a thin Viewing method that distinguishes normal, filmstrip, video, and split-text intent payloads. The ref is local to one `ViewerWorkspace` instance and is not exported.

- [ ] **Step 4: Finish passing narrow ports to child views**

Reuse the ports created when Shell was extracted:

```ts
const ports = useMemo(() => createWorkspacePorts(bridge), [bridge])
```

Change `EmptyProject`, `SettingsDialog`, and `VideoPreview` props to their existing narrow port types. Remove imports of `ViewerBridge` from components where only a `Pick` is required. Keep `ViewerSettingsProvider` and `useViewerController` on the full injected bridge at the composition boundary.

Replace only the three bridge expressions: `EmptyProject` receives `ports.emptyProject`, `SettingsDialog` receives `ports.settings`, and `VideoPreview` receives `viewing.playbackPort`. Leave every other existing prop explicit and unchanged; do not create spread objects solely for this refactor.

- [ ] **Step 5: Delete duplicate root ownership**

Remove all root-local states/effects/handlers now owned by coordinators. `ViewerWorkspace` may retain only:

- `useViewerSettings` and `useViewerController` composition;
- project/session identity construction needed to call coordinators;
- `createWorkspacePorts`;
- the root-local typed intent router;
- pure render-time composition and existing component trees.

Do not create pass-through aliases solely to preserve old local names; render directly from grouped coordinator outputs where readable.

- [ ] **Step 6: Add static ownership assertions**

Extend `contracts.test.ts` or the architecture boundary test to assert:

```ts
expect(appSource).not.toMatch(/useState<TaskFeedback|useOperationDialogs|usePreviewSession|useOrganizationPointerDrag/)
expect(organizationSource).not.toMatch(/useViewingCoordinator|useWorkspaceShellCoordinator/)
expect(viewingSource).not.toMatch(/useOrganizationCoordinator|useWorkspaceShellCoordinator/)
expect(feedbackSource).not.toMatch(/ViewerBridge|useViewerController/)
```

These assertions enforce ownership/import direction, not file length or function length.

- [ ] **Step 7: Run the complete UI and boundary verification**

```bash
pnpm --dir ui test
pnpm --dir ui check
pnpm --dir ui build
pnpm architecture:boundaries
```

Expected: all UI tests pass with the same one intentionally skipped test; typecheck, Biome, build, and blocking architecture boundary pass.

- [ ] **Step 8: Commit the composition-root result**

```bash
git add ui/src/App.tsx ui/src/App.test.tsx ui/src/components/EmptyProject.tsx ui/src/components/EmptyProject.test.tsx ui/src/components/SettingsDialog.tsx ui/src/components/SettingsDialog.test.tsx ui/src/components/VideoPreview.tsx ui/src/app/workspace ui/src/state/viewerControllerContract.test.tsx
git commit -m "refactor: reduce ViewerWorkspace to typed orchestration"
```

---

### Task 9: Run behavior, coverage, trend, and clean verification

**Files:**

- Modify only after reviewed measurement: `docs/quality/architecture-trends-baseline.json`
- Modify only after reviewed measurement: `docs/quality/architecture-trend-classifications.json`
- Do not modify: frozen product/IPC/schema/capability documents.

**Interfaces:**

- Consumes: the completed coordinator architecture.
- Produces: an independently mergeable architecture refactor with evidence that visible behavior and external contracts remain frozen.

- [ ] **Step 1: Run the complete frontend behavior matrix**

```bash
pnpm --dir ui test
pnpm --dir ui check
pnpm --dir ui build
```

Expected: all existing App, preview, compare, organization, settings, accessibility, and controller tests pass; no new skip/ignore is present.

- [ ] **Step 2: Run blocking architecture and external-contract gates**

```bash
pnpm architecture:boundaries
pnpm test:policy
```

Expected: dependency, cycle, direct-Tauri-import, bridge contract, capability, DTO/security, and repository policy gates pass.

- [ ] **Step 3: Run both coverage gates**

```bash
pnpm coverage:ui
pnpm coverage:rust
```

Expected: UI coverage remains at or above the checked-in baseline; unchanged Rust code remains at or above its baseline. Do not lower either baseline.

- [ ] **Step 4: Review the non-blocking architecture trend improvement**

```bash
pnpm architecture:trends
node scripts/architecture-trends.mjs --report
```

Expected: no unclassified new coordinator outlier. If the report confirms removal of the `ui/src/App.tsx` file/function outliers, use `apply_patch` to remove those exact baseline and classification keys so future root regrowth is visible. Do not add a coordinator exception merely to avoid splitting a newly mixed responsibility.

- [ ] **Step 5: Run canonical clean verification**

```bash
pnpm verify:clean
```

Expected: exit `0`; all UI, Rust, build, security, license, and cleanliness gates pass.

- [ ] **Step 6: Inspect frozen contracts and repository state**

```bash
git diff HEAD~1 -- ui/src/api/types.ts ui/src/api/viewer.ts src-tauri/capabilities src-tauri/src/dto
git status --short
```

Expected: no frozen-contract diff and a clean worktree. If the task series spans more than one commit, compare from the governance-plan completion commit instead of `HEAD~1`.

- [ ] **Step 7: Commit reviewed trend evidence only if it changed**

If the baseline/classification files changed solely to record confirmed removal of old App outliers:

```bash
git add docs/quality/architecture-trends-baseline.json docs/quality/architecture-trend-classifications.json
git commit -m "test: record workspace architecture improvement"
```

If no tracked evidence changed, do not create an empty commit. Final state must be clean, fully verified, and contain no product or frozen-contract change.
