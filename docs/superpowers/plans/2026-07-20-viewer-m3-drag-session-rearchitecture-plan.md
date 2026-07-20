# Viewer M3 Drag Session Rearchitecture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the failed WebView-edge drag handoff with deterministic, separately owned internal-organization and native Finder-export sessions while preserving entity-only IPC and Finder-folder project import.

**Architecture:** File card/row bodies start a copy-only AppKit drag immediately at DOM `dragstart`; explicit organization handles retain entity-only HTML drag to the folder tree. The macOS adapter starts from the owning window's content view with a synthesized `LeftMouseDragged` event, so asynchronous entity revalidation no longer depends on a live `NSApplication.currentEvent`.

**Tech Stack:** Rust 1.97, Tauri 2.11, React 19, TypeScript 6, Vitest, objc2/objc2-app-kit 0.6/0.3, AppKit `NSDraggingSession`, Apple Silicon macOS 13+.

## Global Constraints

- Viewer remains Apache-2.0 and this change adds no new build dependency.
- Frontend and `DataTransfer` receive entity IDs/Viewer MIME intent only; source absolute paths never cross IPC.
- File card/row body means Finder export; explicit organization handle means Viewer-internal move or Option-copy.
- Internal drag samples Option at drag start and freezes that mode for the complete session.
- Finder export is copy-only and remains available in read-only projects; internal organization is disabled there.
- Tauri's native incoming file-drop handler remains enabled for Finder-folder project import.
- External drag must not depend on DOM `drag`, `dragover`, `dragleave`, edge coordinates or depth counters.
- Native file identity, project containment, alias/symlink and session/generation checks remain in Rust.
- Existing uncommitted diagnostic/edge-handoff experiments are superseded; remove them through scoped edits and do not use destructive Git reset/checkout.
- Every task completes red-green-refactor, focused verification, a review gate and a focused commit before the next task.

---

## File map

- `ui/src/components/ContentBrowser.tsx`: owns selection freezing and the two explicit frontend drag initiators.
- `ui/src/components/FolderTree.tsx`: consumes the internal drag's already-frozen mode and routes a valid drop.
- `ui/src/App.tsx`: owns active internal-drag state and calls the ID-only Finder bridge immediately.
- `crates/viewer-application/src/finder_drag.rs`: retains portable validation and no longer exposes an expired-event error.
- `crates/viewer-platform-macos/src/files/drag.rs`: owns content-view lookup, synthetic AppKit event and copy-only session.
- `src-tauri/src/commands/finder_drag.rs`: composes asynchronous validation with main-thread native publication without diagnostics.
- `src-tauri/src/error.rs`: maps stable drag errors after removal of `MissingMouseDrag`.
- `scripts/check-tauri-security.sh`: freezes the required native incoming-drop channel.
- `scripts/repository-policy.test.mjs`: locks the native source architecture and prevents edge-handoff regression.
- `ACKNOWLEDGEMENTS.md`: records `drag-rs` as a consulted architecture reference, not a dependency.

### Task 1: Rebuild frontend drag ownership end to end

**Files:**
- Modify: `ui/src/components/ContentBrowser.tsx`
- Modify: `ui/src/components/ContentBrowser.test.tsx`
- Modify: `ui/src/components/FolderTree.tsx`
- Modify: `ui/src/components/FolderTree.test.tsx`
- Modify: `ui/src/styles/app.css`
- Modify: `ui/src/App.tsx`
- Modify: `ui/src/App.test.tsx`

**Interfaces:**
- Produces: `onFinderDragStart(entityIds: string[])`, `onOrganizationDragStart(entityIds, mode)`, `onOrganizationDragEnd()`, `organizationDragDisabled`, `internalDragMode`, atomic `InternalDragState`, immediate `beginFinderDrag` invocation and safe stale/generic messages.
- Preserves: ordered current-folder selection, `VIEWER_SELECTION_MIME`, folder validation callback and `onDropFiles(entityIds, destinationId, mode)`.

- [ ] **Step 1: Replace the old component tests with failing owner-specific tests**

In `ContentBrowser.test.tsx`, replace the generic card-drag test with tests that create the event explicitly and assert ownership:

```tsx
it('starts Finder export immediately from the file body without an HTML payload', () => {
  const exportFiles = vi.fn()
  const setData = vi.fn()
  render(<ContentBrowser workspace={workspace(4)} onFinderDragStart={exportFiles} />)
  fireEvent.click(screen.getByRole('option', { name: '1.jpg' }))
  fireEvent.click(screen.getByRole('option', { name: '3.jpg' }), { metaKey: true })

  const event = createEvent.dragStart(screen.getByRole('option', { name: '3.jpg' }), {
    dataTransfer: { setData, effectAllowed: 'none' },
  })
  fireEvent(screen.getByRole('option', { name: '3.jpg' }), event)

  expect(event.defaultPrevented).toBe(true)
  expect(exportFiles).toHaveBeenCalledWith(['image-1', 'image-3'])
  expect(setData).not.toHaveBeenCalled()
})

it('starts entity-only internal drag from the organization handle and freezes Option-copy', () => {
  const organize = vi.fn()
  const exportFiles = vi.fn()
  const setData = vi.fn()
  render(
    <ContentBrowser
      workspace={workspace(2)}
      onFinderDragStart={exportFiles}
      onOrganizationDragStart={organize}
    />,
  )

  fireEvent.dragStart(screen.getByRole('button', { name: '整理 1.jpg' }), {
    altKey: true,
    dataTransfer: { setData, effectAllowed: 'none' },
  })

  expect(organize).toHaveBeenCalledWith(['image-1'], 'copy')
  expect(exportFiles).not.toHaveBeenCalled()
  expect(setData).toHaveBeenCalledWith('application/x-viewer-selection', 'viewer-selection')
  expect(setData).not.toHaveBeenCalledWith(expect.anything(), expect.stringContaining('image-'))
  expect(setData).not.toHaveBeenCalledWith(expect.anything(), expect.stringContaining('/'))
})

it('keeps Finder export available but disables the organization handle in read-only mode', () => {
  const exportFiles = vi.fn()
  render(
    <ContentBrowser
      workspace={workspace(1)}
      organizationDragDisabled
      onFinderDragStart={exportFiles}
    />,
  )

  expect(screen.getByRole('button', { name: '整理 1.jpg' })).toBeDisabled()
  fireEvent.dragStart(screen.getByRole('option', { name: '1.jpg' }))
  expect(exportFiles).toHaveBeenCalledWith(['image-1'])
})
```

Import `createEvent`. Preserve the stale-selection test, but perform its drag on `整理 2.jpg` and expect `onOrganizationDragStart(['image-2'], 'move')`.

In `FolderTree.test.tsx`, pass `internalDragMode="copy"` and deliberately dispatch `altKey: false`; expect copy highlighting and a copy callback. Add the inverse with `internalDragMode="move"` and `altKey: true`; expect move. These tests prove that mode is not recomputed over the destination.

- [ ] **Step 2: Run focused tests and verify RED**

Run:

```bash
pnpm --dir ui test -- src/components/ContentBrowser.test.tsx src/components/FolderTree.test.tsx
```

Expected: FAIL because `onFinderDragStart`, `onOrganizationDragStart`, `organizationDragDisabled`, `internalDragMode` and the per-file `整理 文件名` handles do not exist; the old body drag still writes the Viewer MIME payload.

- [ ] **Step 3: Implement explicit initiators in ContentBrowser**

Replace the old drag props with:

```tsx
type InternalDragMode = 'move' | 'copy'

interface ContentBrowserProps {
  workspace: ContentWorkspace
  viewportHeight?: number
  requestThumbnail?: (
    file: BrowserFile,
    maxPixels: number,
    scaleMilli: number,
  ) => Promise<string>
  onPreview?: (file: BrowserFile) => void
  onSelectionChange?: (files: BrowserFile[]) => void
  onThumbnailTaskChange?: (task: TaskFeedback | null) => void
  organizationDragDisabled?: boolean
  onFinderDragStart?: (entityIds: string[]) => void
  onOrganizationDragStart?: (entityIds: string[], mode: InternalDragMode) => void
  onOrganizationDragEnd?: () => void
}
```

Extract one selection-freezing helper and two owners:

```tsx
function freezeDragSelection(file: BrowserFile): string[] {
  const dragSelection = selected.has(file.entityId) ? selected : new Set([file.entityId])
  if (!selected.has(file.entityId)) {
    anchorId.current = file.entityId
    commitSelection(dragSelection)
  }
  return allFiles
    .filter((candidate) => dragSelection.has(candidate.entityId))
    .map((candidate) => candidate.entityId)
}

function startFinderDrag(file: BrowserFile, event: DragEvent<HTMLElement>) {
  const entityIds = freezeDragSelection(file)
  event.preventDefault()
  event.stopPropagation()
  if (entityIds.length > 0) onFinderDragStart?.(entityIds)
}

function startOrganizationDrag(file: BrowserFile, event: DragEvent<HTMLElement>) {
  event.stopPropagation()
  if (organizationDragDisabled) {
    event.preventDefault()
    return
  }
  const entityIds = freezeDragSelection(file)
  if (entityIds.length === 0) {
    event.preventDefault()
    return
  }
  event.dataTransfer.effectAllowed = 'copyMove'
  event.dataTransfer.setData(VIEWER_SELECTION_MIME, 'viewer-selection')
  onOrganizationDragStart?.(entityIds, event.altKey ? 'copy' : 'move')
}
```

Keep the image/text root `draggable` and route its `onDragStart` to `startFinderDrag`. Add this child to both roots; change the text root from a `<button>` to a `<div role="option" tabIndex={-1}>` so the nested button is valid HTML:

```tsx
<button
  type="button"
  className="organization-drag-handle"
  aria-label={`整理 ${file.name}`}
  title="拖到左侧文件夹，按住 Option 复制"
  disabled={organizationDragDisabled}
  draggable={!organizationDragDisabled}
  onClick={(event) => event.stopPropagation()}
  onDragStart={(event) => startOrganizationDrag(file, event)}
  onDragEnd={() => onOrganizationDragEnd?.()}
>
  ⋮⋮
</button>
```

Remove `onDragSelectionMove` and the body's old `onDragSelectionEnd`; only the organization handle ends the internal session. Update CSS selectors from `.text-file-list > button` to `.text-file-row`, position the image handle at the card's top-right, and provide a visible focus ring and disabled state without obscuring the thumbnail.

- [ ] **Step 4: Freeze mode in FolderTree**

Add `internalDragMode?: 'move' | 'copy' | null`, default it to `null`, remove `dragMode(event)`, and require the frozen value:

```tsx
function acceptsViewerDrag(event: DragEvent<HTMLElement>): boolean {
  return (
    internalDragMode !== null &&
    Array.from(event.dataTransfer.types).includes('application/x-viewer-selection')
  )
}

const mode = internalDragMode
```

Use that same `mode` in `dragOver` and `drop`; never read `event.altKey`. Clear `dropTarget` if the frozen mode becomes `null` or the dragged selection becomes empty.

- [ ] **Step 5: Run focused component tests and verify GREEN before App wiring**

Run:

```bash
pnpm --dir ui test -- src/components/ContentBrowser.test.tsx src/components/FolderTree.test.tsx
```

Expected: all focused component tests PASS. Do not commit yet; App must consume the new contract in the same task.

- [ ] **Step 6: Replace edge-handoff tests with failing orchestration tests**

Delete tests named for leaving the window, inside-edge handoff, source `drag`, and root `dragleave`. Add:

```tsx
it('starts copy-only native export during the file-body dragstart', async () => {
  const viewer = bridge()
  vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
  render(<App bridge={viewer} />)
  fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
  const file = await screen.findByRole('option', { name: 'front.jpg' })

  const event = createEvent.dragStart(file, { dataTransfer: viewerDragTransfer() })
  fireEvent(file, event)

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
  const transfer = viewerDragTransfer()

  fireEvent.dragStart(await screen.findByRole('button', { name: '整理 front.jpg' }), {
    dataTransfer: transfer,
  })
  fireEvent.dragOver(await screen.findByRole('treeitem', { name: 'selected' }), {
    altKey: true,
    dataTransfer: transfer,
  })
  fireEvent.drop(screen.getByRole('treeitem', { name: 'selected' }), {
    altKey: true,
    dataTransfer: transfer,
  })

  expect(viewer.beginFinderDrag).not.toHaveBeenCalled()
  await waitFor(() =>
    expect(viewer.preflightFileCommand).toHaveBeenCalledWith(
      expect.objectContaining({ kind: 'move' }),
    ),
  )
})
```

Add a parallel Option-at-handle-start test that releases Option over the folder but still expects `kind: 'copy'`. Update the safe-error test so the error occurs immediately after body `dragstart`. Add a stale error object `{ code: 'finder_drag_selection_stale' }` and expect `部分文件已发生变化，请刷新后重试。`. Extend the read-only test: the organization handle is disabled, but body drag invokes `beginFinderDrag`.

- [ ] **Step 7: Run App tests and verify RED**

Run:

```bash
pnpm --dir ui test -- src/App.test.tsx
```

Expected: FAIL because App still uses edge/depth refs and the superseded ContentBrowser callbacks; it does not pass a frozen mode to FolderTree.

- [ ] **Step 8: Implement atomic internal state and immediate export**

Remove `FINDER_DRAG_HANDOFF_MARGIN_PX`, `draggedEntityIdsRef`, `finderExportStartedRef`, `viewerDragDepthRef`, all root drag capture handlers, `exportDraggedSelection(clientX, clientY, leftViewer)`, and every edge/source/root-exit branch.

Use one state value:

```tsx
type InternalDragState = {
  entityIds: string[]
  mode: 'move' | 'copy'
}

const [internalDrag, setInternalDrag] = useState<InternalDragState | null>(null)
const draggedEntityIds = internalDrag?.entityIds ?? []
```

Add a direct export callback:

```tsx
const exportToFinder = useCallback(
  (entityIds: string[]) => {
    const project = state.project
    setFinderDragMessage(null)
    if (project === null || state.status !== 'active' || entityIds.length === 0) return
    void bridge
      .beginFinderDrag({
        sessionId: project.sessionId,
        generation: project.generation,
        entityIds: [...entityIds],
      })
      .catch((error: unknown) => {
        const code =
          typeof error === 'object' && error !== null && 'code' in error
            ? String(error.code)
            : null
        setFinderDragMessage(
          code === 'finder_drag_selection_stale' || code === 'stale_project_session'
            ? '部分文件已发生变化，请刷新后重试。'
            : '无法拖到 Finder，请重新拖动。',
        )
      })
  },
  [bridge, state.project, state.status],
)
```

Wire the explicit component contracts:

```tsx
<FolderTree
  folders={state.folders}
  selectedId={state.selectedFolderId}
  onSelect={selectFolderTarget}
  draggedEntityIds={draggedEntityIds}
  internalDragMode={internalDrag?.mode ?? null}
  readOnly={state.project.access !== 'read_write' || operationBusy}
  isDropTargetValid={isDropTargetValid}
  onDropFiles={(entityIds, destinationId, mode) =>
    void dropFiles(entityIds, destinationId, mode)
  }
/>

<ContentBrowser
  workspace={state.workspace}
  requestThumbnail={requestContentThumbnail}
  onThumbnailTaskChange={setThumbnailTask}
  onPreview={openPreview}
  onSelectionChange={selectFiles}
  organizationDragDisabled={state.project.access !== 'read_write' || operationBusy}
  onFinderDragStart={exportToFinder}
  onOrganizationDragStart={(entityIds, mode) => {
    setFinderDragMessage(null)
    setInternalDrag({ entityIds, mode })
  }}
  onOrganizationDragEnd={() => setInternalDrag(null)}
/>
```

`dropFiles` clears `internalDrag` before preflight. Session reset also clears it. `isDropTargetValid` reads only `draggedEntityIds` derived from the atomic state.

- [ ] **Step 9: Run the entire UI suite and production build**

Run:

```bash
pnpm --dir ui test
pnpm --dir ui build
```

Expected: all UI tests PASS; TypeScript and Vite production build PASS with no edge-handoff symbols remaining:

```bash
rg -n "FINDER_DRAG_HANDOFF|viewerDragDepth|onDragSelectionMove|onDragOverCapture|onDragLeaveCapture" ui/src
```

Expected `rg` exit status: 1 with no matches.

- [ ] **Step 10: Review and commit Task 1**

Review that the body call occurs exactly once at `dragstart`, the organization handle cannot invoke Finder export, read-only export remains enabled, and safe UI messages expose no path. Then run:

```bash
git add ui/src/App.tsx ui/src/App.test.tsx ui/src/components/ContentBrowser.tsx ui/src/components/ContentBrowser.test.tsx ui/src/components/FolderTree.tsx ui/src/components/FolderTree.test.tsx ui/src/styles/app.css
git commit -m "refactor: split internal and Finder drag ownership"
```

### Task 2: Make AppKit publication independent of the live WebView event

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/viewer-application/src/finder_drag.rs`
- Modify: `crates/viewer-platform-macos/src/files/drag.rs`
- Modify: `src-tauri/src/commands/finder_drag.rs`
- Modify: `src-tauri/src/error.rs`
- Modify: `tests/m3_drag_export.rs`
- Modify: `scripts/check-tauri-security.sh`
- Modify: `scripts/repository-policy.test.mjs`
- Modify: `ACKNOWLEDGEMENTS.md`

**Interfaces:**
- Consumes: existing `PreparedFinderDrag`, entity-only DTO and Tauri `with_webview` main-thread callback.
- Produces: `MacFinderDragPort` using window `contentView`, optional timestamp only, a synthesized `LeftMouseDragged` event, and the existing copy-only file URL session.

- [ ] **Step 1: Write failing native and policy tests**

In the native adapter test module add:

```rust
#[test]
fn a_missing_current_event_uses_a_valid_synthetic_timestamp() {
    assert_eq!(drag_event_timestamp(None), 0.0);
}
```

In `tests/m3_drag_export.rs`, replace any expired-event mapping assertion with:

```rust
#[test]
fn changed_selection_maps_to_a_safe_refreshable_error() {
    let error = CommandError::from(FinderDragError::EntityNotFound);
    assert_eq!(error.code, "finder_drag_selection_stale");
    assert_eq!(error.user_message, "部分所选文件已不可用，请刷新项目后重试。");
    assert!(error.retryable);
}
```

Append a repository policy test that reads `crates/viewer-platform-macos/src/files/drag.rs` and asserts it contains `mouseLocationOutsideOfEventStream`, `mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure`, and `contentView()`, while rejecting `filter(|event| event.r#type() == NSEventType::LeftMouseDragged)`.

In `check-tauri-security.sh`, read the main window configuration and fail only when `dragDropEnabled === false`, because that would disable the native Finder-folder import channel required by `onDragDropEvent`.

- [ ] **Step 2: Run focused tests and verify RED**

Run:

```bash
cargo test -p viewer-platform-macos files::drag::tests::a_missing_current_event_uses_a_valid_synthetic_timestamp
cargo test --locked -p viewer-desktop --test m3_drag_export
node --test scripts/repository-policy.test.mjs
./scripts/check-tauri-security.sh
```

Expected: the native test fails to compile because `drag_event_timestamp` does not exist; the policy test fails because the adapter still filters `currentEvent` and starts on the WKWebView; existing non-new security checks remain green.

- [ ] **Step 3: Remove the expired-event application contract**

Delete `FinderDragError::MissingMouseDrag` and its `finder_drag_event_expired` mapping. Do not replace it with another timing error. `NativeUnavailable` remains the adapter error for missing window/content view or event construction failure. Preserve every file identity and containment check.

- [ ] **Step 4: Synthesize the AppKit event from the owning window**

Add `NSGraphicsContext` and `NSWindow` to the workspace `objc2-app-kit` feature list. In `drag.rs`, import `NSEvent` and `NSEventModifierFlags`, retain the source object as today, remove all diagnostic `eprintln!`, and use:

```rust
fn drag_event_timestamp(event: Option<&NSEvent>) -> f64 {
    event.map(NSEvent::timestamp).unwrap_or(0.0)
}

let application = NSApplication::sharedApplication(mtm);
let window = self.view.window().ok_or(FinderDragError::NativeUnavailable)?;
let content_view = window
    .contentView()
    .ok_or(FinderDragError::NativeUnavailable)?;
let mouse = window.mouseLocationOutsideOfEventStream();
let current_event = application.currentEvent();
let event = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
    NSEventType::LeftMouseDragged,
    mouse,
    NSEventModifierFlags::empty(),
    drag_event_timestamp(current_event.as_deref()),
    window.windowNumber(),
    None,
    0,
    1,
    1.0,
)
.ok_or(FinderDragError::NativeUnavailable)?;
let origin = content_view.convertPoint_fromView(mouse, None);
```

Build the existing `NSDraggingItem` values only after `selection.revalidate_file(index)` succeeds. Start with:

```rust
content_view.beginDraggingSessionWithItems_event_source(
    &items,
    &event,
    objc2::runtime::ProtocolObject::from_ref(&*source),
);
```

Keep `NSDraggingSource::operation_mask` returning only `NSDragOperation::Copy`. In `src-tauri/src/commands/finder_drag.rs`, retain asynchronous preparation and the `run_if_project_current` guard, but remove every diagnostic `eprintln!`; the synthesized event is what makes this async boundary safe.

- [ ] **Step 5: Record the consulted open-source architecture pattern**

Add this bullet under `ACKNOWLEDGEMENTS.md` research references:

```markdown
- [CrabNebula drag-rs](https://github.com/crabnebula-dev/drag-rs) — reference for the public AppKit pattern of starting a native file drag from the window content view with a synthesized mouse-drag event. Viewer does not depend on or copy the crate; its adapter keeps Viewer-specific entity validation, identity checks, error boundaries and pasteboard construction.
```

Do not add `drag` or `tauri-plugin-drag` to Cargo manifests or notices.

- [ ] **Step 6: Run focused native, security and policy verification**

Run:

```bash
cargo fmt --all
cargo test --locked -p viewer-platform-macos files::drag::tests
cargo test --locked -p viewer-desktop --test m3_drag_export
cargo clippy --locked -p viewer-platform-macos -p viewer-desktop --all-targets -- -D warnings
node --test scripts/repository-policy.test.mjs
./scripts/check-tauri-security.sh
```

Expected: all PASS; source contains no diagnostic drag logs and no live-event filter:

```bash
rg -n "viewer_finder_drag_|MissingMouseDrag|finder_drag_event_expired|filter\(\|event\| event\.r#type" crates src-tauri tests
```

Expected `rg` exit status: 1 with no matches.

- [ ] **Step 7: Review and commit Task 2**

Review main-thread confinement, source lifetime, copy-only operation, no frontend paths, optional current-event timestamp, and unchanged Tauri incoming-drop configuration. Then run:

```bash
git add Cargo.toml crates/viewer-application/src/finder_drag.rs crates/viewer-platform-macos/src/files/drag.rs src-tauri/src/commands/finder_drag.rs src-tauri/src/error.rs tests/m3_drag_export.rs scripts/check-tauri-security.sh scripts/repository-policy.test.mjs ACKNOWLEDGEMENTS.md
git commit -m "fix: start Finder drag from a synthetic AppKit session"
```

### Task 3: Complete automated, packaged and physical M3 acceptance

**Files:**
- Modify: `docs/superpowers/plans/2026-07-16-viewer-m3-organization-comparison-plan.md`
- Modify: `docs/reviews/2026-07-16-m3-organization-comparison-review.md`

**Interfaces:**
- Consumes: Tasks 1–2 and the existing `gate:m3`.
- Produces: reproducible Finder hash evidence, one final M3 review decision, and a merge-ready branch.

- [ ] **Step 1: Run the exact automated M3 gate**

Run:

```bash
pnpm gate:m3
```

Expected final line: `M3 organization and comparison gate passed`. Preserve the command output in the M3 review notes, including elapsed time and commit SHA.

- [ ] **Step 2: Build the packaged Apple Silicon application**

Run:

```bash
pnpm build:macos
```

Expected artifacts:

```text
target/aarch64-apple-darwin/release/bundle/macos/Viewer.app
target/aarch64-apple-darwin/release/bundle/dmg/Viewer_0.1.0_aarch64.dmg
```

- [ ] **Step 3: Prepare one disposable physical acceptance fixture**

Create a temporary project with JPG, PNG, Markdown and TXT files plus empty single/multi/text/import destinations. Record SHA-256 hashes before launch. Use the packaged app, not `tauri dev`, and keep terminal logs visible for one evidence-rich run.

The physical checklist is exact:

1. Drag one image card body to Finder; the destination copy exists and its SHA-256 equals the source.
2. Multi-select two images and drag either selected card body; both copies exist and match.
3. Drag Markdown and TXT row bodies; both copies exist and match.
4. Cancel one Finder drag outside a valid target; no project or destination file changes.
5. Drag an image's `整理` handle to a different Viewer folder; default operation is move through the existing preflight/execute path.
6. Hold Option when the handle drag starts, release it before drop, and confirm the operation remains copy.
7. Close the project, then drag the fixture project folder from Finder into Viewer; it opens through the native incoming channel.
8. Quit and relaunch the packaged app, then repeat step 1 successfully.

No test passes from visual drag feedback alone; destination existence and source/destination hashes are mandatory for export.

- [ ] **Step 4: Update milestone evidence only after every physical check passes**

In the original M3 plan, append a dated correction below Task 12 explaining that the edge-handoff implementation failed physical acceptance and was superseded by the 2026-07-20 two-owner design/plan. Mark Task 17 drag acceptance complete only with the fixture path, source/destination hashes, packaged app path and relaunch result.

Add a `Drag architecture correction and physical evidence — 2026-07-20` section to the M3 review. Record the Task 1 frontend commit and Task 2 native commit from their agent reports, plus the literal output of `git rev-parse HEAD` for the gated implementation SHA. Record the packaged build result, each of the eight physical checks, every source/destination SHA-256 pair, and the final Critical/Important finding counts. Do not write or commit the section until every recorded value has been directly observed.

- [ ] **Step 5: Run final review and commit evidence**

Review the complete branch diff against the amended design, with special attention to event ownership, async/main-thread boundaries, path privacy, read-only behavior and incoming folder drop. Re-run focused commands if review changes code, then run `pnpm gate:m3` again.

Commit only truthful evidence:

```bash
git add docs/superpowers/plans/2026-07-16-viewer-m3-organization-comparison-plan.md docs/reviews/2026-07-16-m3-organization-comparison-review.md
git commit -m "docs: record corrected M3 drag acceptance"
```

- [ ] **Step 6: Merge M3 locally only after the final gate and review pass**

Confirm the feature worktree is clean, switch to `/Users/abc/Project/Viewer`, fast-forward `main` to `codex/m3-organization-comparison`, and run `pnpm gate:m3` again on `main`. Do not merge with unresolved Critical/Important findings or incomplete physical evidence. Record the merged SHA in the roadmap/progress record before beginning M4.

---

## Plan self-review

- Spec coverage: amended design sections 2.4, 3, 8.1, 8.2, 13 and 14 map to Tasks 1–3.
- Security coverage: entity-only IPC, path confinement, identity revalidation, no capability/dependency expansion and incoming-drop preservation are tested.
- Regression coverage: image/text, single/multi, read-only, stale selection, internal move/Option-copy, cancellation, relaunch and folder import are explicit.
- Architecture cleanup: every edge/source/leave heuristic and live-event precondition is explicitly removed and guarded against recurrence.
- Scope: Windows implementation remains deferred; the adapter boundary and copy-only semantics remain compatible with later platform work.
