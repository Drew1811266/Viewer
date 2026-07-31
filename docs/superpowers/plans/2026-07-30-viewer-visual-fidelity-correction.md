# Viewer Visual Fidelity Correction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restore the approved radial-menu character, thumbnail-only selection, minimal toolbar popovers, compact task feedback, and verified preview chrome in the current Viewer development build.

**Architecture:** Keep Viewer controller and file-operation behavior unchanged. Correct the interaction boundary in `ContentBrowser`, render one radial command surface in `App`, express selection and menu chrome through focused component classes, and use the existing task and preview components with tighter presentation contracts. Native same-state screenshot comparison is the final gate.

**Tech Stack:** React 19, TypeScript 6, Vitest, Testing Library, CSS design tokens, Tauri 2, Node.js development launcher.

## Global Constraints

- `2026-07-30-viewer-visual-fidelity-correction-design.md` overrides conflicting radial fallback and selection language in prior specs.
- Preserve file-operation semantics, safety dialogs, shortcuts, selection rules, preview behavior, comparison limits, and project access rules.
- Keep `筛选`, `视图`, and `更多` as the only persistent controls after search.
- Use existing project thumbnails and Viewer symbols; add no decorative raster assets.
- Use `pnpm start:viewer` for native review and require one current-worktree development executable.
- Every production behavior change starts with a focused failing test.
- P0, P1, and P2 visual mismatches block handoff.

---

### Task 1: Unify every file-action invocation on the radial menu

**Files:**
- Modify: `ui/src/components/ContentBrowser.tsx`
- Modify: `ui/src/components/ContentBrowser.test.tsx`
- Modify: `ui/src/App.tsx`
- Modify: `ui/src/App.test.tsx`
- Modify: `ui/src/components/RadialFileMenu.tsx`
- Modify: `ui/src/components/RadialFileMenu.test.tsx`
- Delete: `ui/src/components/FileContextMenu.tsx`
- Delete: `ui/src/components/FileContextMenu.test.tsx`
- Modify: `ui/src/styles/app.css`
- Modify: `ui/src/styles/app.test.ts`

**Interfaces:**
- Consumes: existing `RadialMenuRequest`, `onRadialMenuRequest`, radial action model, pointer gesture thresholds, selection snapshot, and return-focus target.
- Produces: one `RadialFileMenu` render path for both `pointerId: number` gesture mode and `pointerId: null` click/keyboard mode.
- Produces: `openRadialMenuFromKeyboard(file, target)` behavior through the existing content-browser keyboard boundary.

- [ ] **Step 1: Write failing secondary-click and application render tests**

Replace the compact-menu assertion in `ContentBrowser.test.tsx` with:

```tsx
it('routes a normal secondary click to radial click mode', () => {
  vi.useFakeTimers()
  const request = vi.fn()
  render(<ContentBrowser workspace={workspace(3)} onRadialMenuRequest={request} />)
  const option = screen.getAllByRole('option')[0]!

  fireEvent.pointerDown(option, {
    button: 2,
    pointerId: 41,
    clientX: 240,
    clientY: 180,
  })
  fireEvent.contextMenu(option, { button: 2, clientX: 240, clientY: 180 })
  fireEvent.pointerUp(window, {
    button: 2,
    pointerId: 41,
    clientX: 240,
    clientY: 180,
  })

  expect(request).toHaveBeenCalledWith(
    expect.objectContaining({ origin: { x: 240, y: 180 }, pointerId: null }),
  )
  vi.useRealTimers()
})
```

Replace the App fallback test with:

```tsx
it('renders the radial menu for click mode and never renders a conventional file menu', async () => {
  render(<App />)
  await openProject()
  fireEvent.contextMenu(screen.getAllByRole('option')[0]!, {
    button: 2,
    clientX: 300,
    clientY: 220,
  })
  expect(document.querySelector('.radial-file-menu')).not.toBeNull()
  expect(document.querySelector('.file-context-menu')).toBeNull()
})
```

- [ ] **Step 2: Run focused tests and verify the red state**

Run:

```bash
pnpm --dir ui exec vitest run \
  src/components/ContentBrowser.test.tsx \
  src/App.test.tsx -t "secondary click|radial menu|conventional"
```

Expected: FAIL because a released secondary gesture requests `pointerId: null`,
which `App` currently maps to `FileContextMenu`.

- [ ] **Step 3: Remove compact activation and render one radial path**

In `ContentBrowser`, remove `activateCompact`. A context-menu event recorded
for a pending pointer session must release the session as:

```ts
requestRadialMenu(
  file,
  pendingGesture.contextMenu.eventTarget,
  pendingGesture.contextMenu.origin,
  null,
)
```

Keep dwell and movement promotion with the real pointer id. In `App`, remove
the `FileContextMenu` import and replace both render branches with:

```tsx
{activeRadialMenu !== null && (
  <RadialFileMenu
    key={activeRadialMenu.requestId}
    origin={activeRadialMenu.origin}
    pointerId={activeRadialMenu.pointerId}
    selectionCount={activeRadialMenu.files.length}
    readOnly={state.project.access === 'read_only'}
    returnFocusTarget={activeRadialMenu.returnFocusTarget}
    model={radialModel}
    onAction={runRadialAction}
    onClose={finishRadialSession}
  />
)}
```

Delete the conventional menu component, tests, and CSS.

- [ ] **Step 4: Add the radial scrim and preserve outside close**

Render a sibling `.radial-menu-scrim` before the radial root:

```tsx
<div className="radial-menu-layer">
  <button
    type="button"
    className="radial-menu-scrim"
    aria-label="关闭文件操作"
    tabIndex={-1}
    onClick={requestClose}
  />
  <div ref={rootRef} className="radial-file-menu">...</div>
</div>
```

The layer fills the viewport, the scrim uses `var(--viewer-backdrop-subtle)`,
and the radial root remains positioned by its fitted origin. Keep Escape,
center cancel, click-mode outside close, keyboard focus, and gesture listeners.

- [ ] **Step 5: Add keyboard Context Menu and Shift+F10 tests**

Add a content-browser test that selects one option, fires `Shift+F10`, and
asserts a request with `pointerId: null`, the active file, and an origin inside
the option's mocked bounds. Add the ContextMenu-key variant using
`key: 'ContextMenu'`.

- [ ] **Step 6: Implement keyboard invocation**

In the content browser keyboard handler, when the active file exists and the
key is `ContextMenu` or `Shift+F10`, prevent default and request a click-mode
radial session at the active file element's bounding-box center. Use
`document.getElementById(\`file-${activeId}\`)` as the focus target.

- [ ] **Step 7: Run focused tests and commit**

Run:

```bash
pnpm --dir ui exec vitest run \
  src/components/ContentBrowser.test.tsx \
  src/components/RadialFileMenu.test.tsx \
  src/components/radialMenuGeometry.test.ts \
  src/App.test.tsx -t "secondary click|right button|radial|Context Menu|Shift"
```

Expected: PASS.

Commit:

```bash
git add ui/src/App.tsx ui/src/App.test.tsx \
  ui/src/components/ContentBrowser.tsx ui/src/components/ContentBrowser.test.tsx \
  ui/src/components/RadialFileMenu.tsx ui/src/components/RadialFileMenu.test.tsx \
  ui/src/components/FileContextMenu.tsx ui/src/components/FileContextMenu.test.tsx \
  ui/src/styles/app.css ui/src/styles/app.test.ts
git commit -m "fix: make radial menu the single file action surface"
```

---

### Task 2: Separate thumbnail selection from keyboard focus

**Files:**
- Modify: `ui/src/components/contentBrowser/ImageCell.tsx`
- Modify: `ui/src/components/ContentBrowser.test.tsx`
- Modify: `ui/src/styles/app.css`
- Modify: `ui/src/styles/app.test.ts`

**Interfaces:**
- Consumes: `selected`, `active`, `AspectThumbnail`, organization drag state.
- Produces: `.image-cell-thumbnail-frame` as the sole selected visual boundary.
- Produces: `.image-cell[data-active="true"]` as the keyboard-focus contract.

- [ ] **Step 1: Write failing selection structure and style tests**

Add to `ContentBrowser.test.tsx`:

```tsx
it('keeps selection inside the thumbnail and separate from active focus', () => {
  render(<ContentBrowser workspace={workspace(3)} />)
  const option = screen.getAllByRole('option')[0]!
  fireEvent.click(option)
  expect(option).toHaveAttribute('aria-selected', 'true')
  expect(option.querySelector('.image-cell-thumbnail-frame')).not.toBeNull()
})
```

Add to `app.test.ts` assertions that:

```ts
expect(selectedCard?.declarations.border).toBeUndefined()
expect(selectedCard?.declarations['box-shadow']).toBeUndefined()
expect(selectionOverlay?.declarations).toMatchObject({
  border: '2px solid var(--viewer-accent)',
  inset: '6px',
  'border-radius': '8px',
  'pointer-events': 'none',
  position: 'absolute',
})
```

- [ ] **Step 2: Run focused tests and verify the red state**

Run:

```bash
pnpm --dir ui exec vitest run \
  src/components/ContentBrowser.test.tsx \
  src/styles/app.test.ts -t "selection|thumbnail|active focus"
```

Expected: FAIL because the current selected rule applies a border and shadow to
the complete `.image-cell`.

- [ ] **Step 3: Add the thumbnail frame and move the selected overlay**

Wrap `AspectThumbnail` and its unsupported state in:

```tsx
<div className="image-cell-thumbnail-frame">
  <AspectThumbnail ... />
</div>
```

Use `.image-cell[aria-selected="true"] .image-cell-thumbnail-frame::after` for
the inset overlay. Remove selected border and shadow from `.image-cell`.
Use the shared focus outline only for `[data-active="true"]`.

- [ ] **Step 4: Make the organization handle contextual**

Hide the handle at rest and reveal it for `.image-cell:hover`,
`.image-cell[data-active="true"]`, `:focus-within`, or active organization
drag. Preserve its accessible button and pointer target.

- [ ] **Step 5: Correct the selection summary copy and spacing**

Change the supporting line to:

```tsx
<span>右键打开圆盘菜单 · Esc 取消选择</span>
```

Retain the selected count, role, and accessible label. Ensure bottom padding
reserves the capsule height without covering content.

- [ ] **Step 6: Run focused tests and commit**

Run:

```bash
pnpm --dir ui exec vitest run \
  src/components/ContentBrowser.test.tsx \
  src/components/contentBrowser/OtherFilePanel.test.tsx \
  src/styles/app.test.ts
```

Expected: PASS.

Commit:

```bash
git add ui/src/components/contentBrowser/ImageCell.tsx \
  ui/src/components/ContentBrowser.test.tsx ui/src/styles/app.css \
  ui/src/styles/app.test.ts
git commit -m "fix: keep image selection inside the thumbnail"
```

---

### Task 3: Replace toolbar button stacks with menu rows

**Files:**
- Modify: `ui/src/components/WorkspaceViewMenu.tsx`
- Modify: `ui/src/components/WorkspaceViewMenu.test.tsx`
- Modify: `ui/src/components/WorkspaceMoreMenu.tsx`
- Modify: `ui/src/components/WorkspaceMoreMenu.test.tsx`
- Modify: `ui/src/styles/app.css`
- Modify: `ui/src/styles/app.test.ts`

**Interfaces:**
- Consumes: current view contexts, project access actions, popover coordination,
  viewport width hook, Escape focus restoration.
- Produces: `.workspace-menu-item`, `.workspace-menu-separator`, and optional
  `data-tone="destructive"` menu-row styling.

- [ ] **Step 1: Write failing semantic menu-row tests**

Update both component tests to assert that opened command buttons have
`.workspace-menu-item`. In `WorkspaceMoreMenu.test.tsx`, assert:

```tsx
expect(document.querySelector('.workspace-menu-separator')).not.toBeNull()
expect(screen.getByRole('button', { name: '关闭项目' })).toHaveAttribute(
  'data-tone',
  'destructive',
)
```

Add CSS assertions that `.workspace-menu-item` has `border: 0`,
`justify-content: flex-start`, and `min-height: 32px`.

- [ ] **Step 2: Run focused tests and verify the red state**

Run:

```bash
pnpm --dir ui exec vitest run \
  src/components/WorkspaceViewMenu.test.tsx \
  src/components/WorkspaceMoreMenu.test.tsx \
  src/styles/app.test.ts -t "menu|popover|separator"
```

Expected: FAIL because commands inherit bordered toolbar-button styling and
the More separator is an unclassed `hr`.

- [ ] **Step 3: Apply menu-row classes and semantic separators**

Add `className="workspace-menu-item"` to every view and more command. Replace
`<hr />` with:

```tsx
<hr className="workspace-menu-separator" aria-hidden="true" />
```

Add `data-tone="destructive"` to Close Project. Style rows as borderless,
left-aligned, full-width commands with soft hover/focus and pressed-state
treatment. Keep the trigger buttons and their popover coordination unchanged.

- [ ] **Step 4: Run focused tests and commit**

Run:

```bash
pnpm --dir ui exec vitest run \
  src/components/WorkspaceViewMenu.test.tsx \
  src/components/WorkspaceMoreMenu.test.tsx \
  src/components/SearchToolbar.test.tsx \
  src/styles/app.test.ts
```

Expected: PASS.

Commit:

```bash
git add ui/src/components/WorkspaceViewMenu.tsx \
  ui/src/components/WorkspaceViewMenu.test.tsx \
  ui/src/components/WorkspaceMoreMenu.tsx \
  ui/src/components/WorkspaceMoreMenu.test.tsx \
  ui/src/styles/app.css ui/src/styles/app.test.ts
git commit -m "fix: simplify Viewer toolbar popover commands"
```

---

### Task 4: Collapse task feedback into one compact surface

**Files:**
- Modify: `ui/src/components/TaskBar.tsx`
- Modify: `ui/src/components/TaskBar.test.tsx`
- Modify: `ui/src/styles/app.css`
- Modify: `ui/src/styles/app.test.ts`

**Interfaces:**
- Consumes: `TaskFeedback[]`, existing cancellation, dismissal, results, live
  announcement, and clean-success timers.
- Produces: one `.task-surface`, one `.task-summary-toggle`, and expanded
  `.task-list` rows.

- [ ] **Step 1: Write failing collapsed-surface tests**

Add:

```tsx
it('collapses multiple tasks into one summary surface', () => {
  render(<TaskBar tasks={[runningScan, runningThumbnails]} onCancel={vi.fn()} />)
  const bar = screen.getByRole('complementary', { name: '后台任务' })
  expect(bar.querySelectorAll('.task-surface')).toHaveLength(1)
  expect(screen.getByRole('button', { name: '展开后台任务' })).toBeVisible()
  expect(screen.queryByRole('button', { name: '关闭任务' })).not.toBeInTheDocument()
})
```

Add a clean-success assertion that the task is visible briefly but no Close
Task button is rendered.

- [ ] **Step 2: Run focused tests and verify the red state**

Run:

```bash
pnpm --dir ui exec vitest run src/components/TaskBar.test.tsx
```

Expected: FAIL because every task currently renders as an independent
`.task-row` with its own action chrome.

- [ ] **Step 3: Implement one collapsed surface**

Render one outer task surface. The collapsed summary uses the most relevant
task (failed, running, result-bearing, then recent complete), aggregate task
count, and aggregate progress. Expansion reveals the existing per-task details
and action callbacks. Do not show dismissal for a clean success; its timer
remains the only dismissal path.

- [ ] **Step 4: Run focused tests and commit**

Run:

```bash
pnpm --dir ui exec vitest run \
  src/components/TaskBar.test.tsx \
  src/App.test.tsx -t "task|completed|results"
```

Expected: PASS.

Commit:

```bash
git add ui/src/components/TaskBar.tsx ui/src/components/TaskBar.test.tsx \
  ui/src/styles/app.css ui/src/styles/app.test.ts
git commit -m "fix: consolidate Viewer task feedback"
```

---

### Task 5: Tighten preview and compare chrome

**Files:**
- Modify: `ui/src/components/ImagePreview.tsx`
- Modify: `ui/src/components/ImagePreview.test.tsx`
- Modify: `ui/src/components/TextPreview.tsx`
- Modify: `ui/src/components/TextPreview.test.tsx`
- Modify: `ui/src/components/UnsupportedFilePreview.tsx`
- Modify: `ui/src/components/UnsupportedFilePreview.test.tsx`
- Modify: `ui/src/components/CompareWorkspace.tsx`
- Modify: `ui/src/components/CompareWorkspace.test.tsx`
- Modify: `ui/src/styles/app.css`
- Modify: `ui/src/styles/app.test.ts`

**Interfaces:**
- Consumes: existing preview identity, transform actions, navigation, compare
  layout, and close callbacks.
- Produces: shared `.preview-segmented-controls` and `.preview-complete-action`
  visual contracts while retaining accessible names.

- [ ] **Step 1: Write failing toolbar grouping tests**

In preview and compare tests, assert that the transform toolbar has
`.preview-segmented-controls` and the close action has
`.preview-complete-action` with visible `完成`. Keep the current role and
accessible-name assertions.

- [ ] **Step 2: Run focused tests and verify the red state**

Run:

```bash
pnpm --dir ui exec vitest run \
  src/components/ImagePreview.test.tsx \
  src/components/TextPreview.test.tsx \
  src/components/UnsupportedFilePreview.test.tsx \
  src/components/CompareWorkspace.test.tsx
```

Expected: FAIL because the common grouping classes are absent or inconsistent.

- [ ] **Step 3: Apply shared toolbar classes**

Group related transform controls into one segmented surface, retain the
existing Rotate action, and make the close action visibly `完成` in image,
text, unsupported, and compare modes. Do not change zoom math, navigation,
image requests, compare layout, or Escape behavior.

- [ ] **Step 4: Run focused tests and commit**

Run:

```bash
pnpm --dir ui exec vitest run \
  src/components/ImagePreview.test.tsx \
  src/components/TextPreview.test.tsx \
  src/components/UnsupportedFilePreview.test.tsx \
  src/components/CompareWorkspace.test.tsx \
  src/styles/app.test.ts
```

Expected: PASS.

Commit:

```bash
git add ui/src/components/ImagePreview.tsx ui/src/components/ImagePreview.test.tsx \
  ui/src/components/TextPreview.tsx ui/src/components/TextPreview.test.tsx \
  ui/src/components/UnsupportedFilePreview.tsx \
  ui/src/components/UnsupportedFilePreview.test.tsx \
  ui/src/components/CompareWorkspace.tsx \
  ui/src/components/CompareWorkspace.test.tsx \
  ui/src/styles/app.css ui/src/styles/app.test.ts
git commit -m "fix: unify Viewer preview and compare chrome"
```

---

### Task 6: Verify the launcher and complete native visual QA

**Files:**
- Modify only if a failing launcher test proves a gap:
  `scripts/viewer-dev-launcher.mjs`
- Modify only if required by that gap:
  `scripts/viewer-dev-launcher.test.mjs`
- Create: `design-qa.md`
- Create ignored evidence under:
  `target/visual-qa/corrective-final/`

**Interfaces:**
- Consumes: `pnpm start:viewer`, current-worktree executable path, approved
  1440x900 and 1024x720 reference images.
- Produces: exactly one current development Viewer process and a
  `design-qa.md` whose last result is `final result: passed`.

- [ ] **Step 1: Run the launcher unit tests**

Run:

```bash
node --test scripts/viewer-dev-launcher.test.mjs
```

Expected: PASS, including packaged `Viewer.app` process recognition and exact
development executable waiting.

- [ ] **Step 2: Run all automated gates**

Run:

```bash
pnpm --dir ui check
pnpm --dir ui test
pnpm --dir ui build
node --test scripts/viewer-dev-launcher.test.mjs
cargo fmt --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
```

Expected: every command exits 0 with no warnings attributable to this change.

- [ ] **Step 3: Restart the only review build**

Run:

```bash
pnpm start:viewer
```

Then inspect:

```bash
ps -axo pid=,command= | rg 'viewer-desktop|tauri dev|vite'
```

Expected: one Viewer executable at this worktree's
`target/debug/viewer-desktop`, with no
`target/debug/bundle/macos/Viewer.app/Contents/MacOS/viewer-desktop` process.

- [ ] **Step 4: Capture the full acceptance matrix**

Use the native Viewer window to capture all states listed in the corrective
design at 1440x900 and 1024x720. Save exact screenshots under
`target/visual-qa/corrective-final/` with names such as:

```text
1440-content-single-selection.png
1440-radial-click.png
1440-view-menu.png
1024-preview.png
```

- [ ] **Step 5: Run blocking design QA**

Open each reference and same-state current capture in one comparison input.
Record P0–P3 findings in `design-qa.md`. Fix every P0, P1, and P2, recapture,
and repeat until the report ends with:

```text
final result: passed
```

- [ ] **Step 6: Commit verification evidence**

Commit tracked QA documentation only:

```bash
git add design-qa.md
git commit -m "test: pass Viewer corrective visual QA"
```

