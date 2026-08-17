# Viewer Native Video Theater Refactor Implementation Plan

> **For Codex:** Use `superpowers:executing-plans` to execute this plan task by task. Use
> `superpowers:test-driven-development` for every behavior change and
> `superpowers:verification-before-completion` before any completion claim.

**Goal:** Replace the browser-predicted transparent video aperture with one native opaque theater,
move high-frequency resize ownership fully into AppKit, simplify the React overlay, and make timeline
commit seeking immediate and generation-safe.

**Architecture:** AppKit mounts an opaque theater below the transparent WKWebView and lays out the
native OpenGL video child from media geometry and native bounds. React renders only semantic overlay
chrome and never publishes video geometry. Timeline click emits one exact seek; drag previews are
latest-wins and release invalidates them before issuing the exact commit without waiting.

**Tech stack:** Rust, objc2/AppKit, Tauri 2, libmpv render API, React 19, TypeScript, Vitest, Cargo
tests, existing native acceptance controller.

**Design source:**
`docs/superpowers/specs/2026-08-16-viewer-native-video-theater-refactor-design.md`

## Scope Lock

- Keep libmpv, VideoToolbox, OpenGL render API, bundled runtime, thumbnails, and existing player
  commands other than the obsolete browser geometry command.
- Do not preserve the CSS aperture contract behind compatibility styles.
- Do not add a second resize coordinator or another frontend debounce.
- Do not claim success without real native resize and pixel-leak acceptance.
- The feature-only `video_feasibility` route may retain explicit fixture geometry where needed for
  acceptance, but production UI must have no DOM-to-native geometry path.

## Task 1: Freeze A Repository-Local Visual Target

**Files:**

- Reference screenshot
  `/var/folders/hh/jj77kbxs0db1_j1kgh7hbd2c0000gn/T/codex-clipboard-e1de389c-ae60-4985-ab25-9b95bf1aa5b3.png`
- Create: `docs/reviews/assets/viewer-native-video-theater-target.png`
- Create: `docs/reviews/2026-08-16-viewer-native-video-theater-visual-contract.md`
- Modify: `docs/superpowers/specs/2026-08-16-viewer-native-video-theater-refactor-design.md`

**Step 1: Select and preserve the local visual target**

Generate three independent, screenshot-grounded directions and stop for user selection. Copy only
the selected result into the repository as the immutable visual reference. Do not create a second
application runtime or duplicate production behavior.

**Step 2: Add responsive target frames**

Add wide 1440×900, compact 1024×720, and narrow 720×720 frames. Each frame shows the native theater,
compact top bar, navigation, timeline, transport, secondary controls, and safe-area relationships.

**Step 3: Annotate structure and behavior**

Document native opaque theater, AppKit contain-fit, overlay-only WebView, integrated bottom controls,
click seek, drag preview, no desktop leakage, 44 px targets, and responsive disclosure.

**Step 4: Verify the production implementation at the target viewports**

Use the selected wide target plus the responsive contract as the source truth. Capture the real
Tauri implementation at all three viewports and correct clipping, spacing, icon contrast, target
size, and narrow layout. Record the accepted measurements and comparison evidence in the
visual-contract document. No Figma account or remote editor is involved.

## Task 2: Add Native Theater Layout Behavior

**Files:**

- Create: `crates/viewer-platform-macos/src/video/theater.rs`
- Modify: `crates/viewer-platform-macos/src/video/mod.rs`
- Modify: `crates/viewer-platform-macos/src/video/surface.rs`
- Create or modify: `crates/viewer-platform-macos/tests/video_theater_layout.rs`

**Step 1: Write failing contain-fit tests**

Cover:

- 16:9 media in wide and tall theater bounds;
- portrait media;
- 90 and 270 degree rotation;
- fractional point bounds and 2× backing scale;
- invalid/zero media geometry;
- unchanged bounds producing no new layout.

Run:

```bash
cargo test -p viewer-platform-macos --test video_theater_layout -- --nocapture
```

Expected: RED because native theater layout types do not exist.

**Step 2: Implement pure native layout types**

Add `VideoDisplayGeometry`, `TheaterBounds`, `TheaterFrame`, and a pure `contain_fit_frame` function.
Keep DOM coordinates out of this module.

**Step 3: Make layout tests green**

Run the focused test and confirm all cases pass.

**Step 4: Add failing hierarchy/background tests**

Test the behavioral ordering helpers:

- opaque window/theater configuration before attachment;
- WKWebView transparency before theater/video attachment;
- theater remains present when video child is hidden/unmounted;
- observer cancellation precedes render-context teardown.

Expected: RED against the current transparent-window sibling model.

**Step 5: Refactor `MacVideoSurface` around the theater**

- Create an opaque native theater view below WKWebView.
- Mount `NSOpenGLView` as the fitted child of the theater.
- Remove transparent NSWindow configuration.
- Retain typed WKWebView transparency failure.
- Store current media geometry.
- Observe native bounds/frame changes and apply fit on AppKit main.
- Unregister observations before unmount/drop.

Do not add a browser callback.

**Step 6: Verify focused native surface behavior**

```bash
cargo test -p viewer-platform-macos video:: --lib -- --nocapture
cargo test -p viewer-platform-macos --test video_theater_layout -- --nocapture
cargo clippy -p viewer-platform-macos --all-targets -- -D warnings
```

## Task 3: Pass Media Geometry Into Native Surface Mount

**Files:**

- Modify: `crates/viewer-application/src/video.rs`
- Modify: `crates/viewer-application/src/ports.rs`
- Modify: `crates/viewer-application/src/lib.rs`
- Modify: `crates/viewer-platform-macos/src/video/adapter.rs`
- Modify: `crates/viewer-platform-macos/src/video/render_loop.rs`
- Modify: `crates/viewer-test-support/src/video_engine.rs`
- Modify: `src-tauri/src/commands/video.rs`
- Modify: `src-tauri/src/video_runtime.rs`
- Modify: corresponding application/desktop/platform tests

**Step 1: Write a failing engine-open contract test**

Prove that surface preparation receives authoritative display width, height, and rotation and does
not require a browser `SurfaceRect`.

Run:

```bash
cargo test -p viewer-application --test video_preview_service native_theater -- --nocapture
```

Expected: RED because open/preparation still requires/publishes `SurfaceRect`.

**Step 2: Reshape the engine contract**

Replace browser rectangle ownership with authoritative media display geometry in the open/surface
preparation boundary. Keep generation and source identity unchanged.

**Step 3: Wire the macOS adapter**

Mount the theater using the WebView/native content bounds plus media geometry. Ensure render context
creation remains before `loadfile`, first-frame reveal remains generation guarded, and final-frame
retention remains unchanged.

**Step 4: Update fakes and tests**

Remove `PublishSurfaceRect` calls from production service expectations. Add fake calls for theater
preparation only if behavior verification needs them.

**Step 5: Run focused contract suites**

```bash
cargo test -p viewer-application --test video_preview_service -- --nocapture
cargo test -p viewer-test-support video_engine -- --nocapture
cargo test -p viewer-platform-macos video:: --lib -- --nocapture
cargo test -p viewer-desktop video_ --lib -- --nocapture
```

## Task 4: Delete The Production Browser Geometry Command

**Files:**

- Modify: `src-tauri/src/dto/video.rs`
- Modify: `src-tauri/src/commands/video.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/video_runtime.rs`
- Modify: `ui/src/api/types.ts`
- Modify: `ui/src/api/viewer.ts`
- Modify: `ui/src/api/viewer.test.ts`
- Modify: `ui/src/acceptance/acceptanceBridge.ts`
- Modify: `ui/src/acceptance/acceptanceBridge.test.ts`
- Modify: desktop security/lifecycle tests that enumerate commands

**Step 1: Write failing API inventory tests**

Assert that the production bridge exposes no `videoSetSurfaceRect` method and the Tauri invoke
registry contains no `video_set_surface_rect` command.

Run:

```bash
pnpm --dir ui exec vitest run src/api/viewer.test.ts src/acceptance/acceptanceBridge.test.ts
cargo test -p viewer-desktop video_security -- --nocapture
```

Expected: RED because the obsolete API is still public and registered.

**Step 2: Remove the DTO, command, bridge method, and runtime publication**

Delete production geometry publication end to end. Do not leave a no-op method in the public bridge.

**Step 3: Preserve feature-only acceptance geometry explicitly**

If the feasibility route still needs explicit fixture framing, keep its private DTO and platform
call feature-gated. It must not be reachable from normal Viewer commands.

**Step 4: Re-run API/security tests**

Confirm exact bridge and invoke inventories pass.

## Task 5: Remove DOM Geometry And Aperture Ownership

**Files:**

- Modify: `ui/src/components/videoPreview/useVideoBridge.ts`
- Modify: `ui/src/components/videoPreview/useVideoBridge.test.tsx`
- Delete or narrow: `ui/src/components/videoPreview/videoGeometry.ts`
- Delete or narrow: `ui/src/components/videoPreview/videoGeometry.test.ts`
- Modify: `ui/src/components/VideoPreview.tsx`
- Modify: `ui/src/components/VideoPreview.test.tsx`
- Modify: `ui/src/styles/videoPreview.css`
- Replace: `ui/src/styles/videoPreviewLayout.test.ts`
- Modify: `ui/src/styles/visualAccessibility.test.ts`

**Step 1: Write failing component/bridge behavior tests**

Prove:

- `VideoPreview` renders no `.video-preview-matte` elements;
- no aperture CSS custom properties are set;
- `useVideoBridge` opens and controls a session without `ResizeObserver`;
- window resize causes zero video geometry bridge calls;
- `html`, `body`, and `#root` do not become player-owned transparent surfaces.

Run:

```bash
pnpm --dir ui exec vitest run \
  src/components/VideoPreview.test.tsx \
  src/components/videoPreview/useVideoBridge.test.tsx
```

Expected: RED against the four-matte/measured-stage implementation.

**Step 2: Simplify `useVideoBridge`**

Remove:

- `useMeasuredVideoStage`;
- `surfaceLayout` state/result;
- geometry keys/sequences/effects;
- DOM-stage dependency from lifecycle startup;
- `videoSetSurfaceRect` bridge ownership.

Open becomes entity/attempt based. Authoritative media remains event/session state for UI labels,
not native layout.

**Step 3: Simplify `VideoPreview`**

Remove aperture style calculation and four matte nodes. Keep semantic loading/failure/chrome states.

**Step 4: Rewrite CSS around overlay-only ownership**

- Keep the preview/dialog clipped to the content view.
- Remove global/root player transparency selectors and matte geometry.
- Make chrome surfaces intentional and localized.
- Ensure any transparent overlay pixel reveals native theater, not desktop.
- Preserve reduced-motion and forced-colors behavior.

**Step 5: Replace source-text layout tests**

Delete CSS parser assertions that merely search declarations. Use rendered behavior/accessibility
tests. Reserve screenshot/native tests for actual visual evidence.

**Step 6: Verify focused UI**

```bash
pnpm --dir ui exec vitest run \
  src/components/VideoPreview.test.tsx \
  src/components/videoPreview/useVideoBridge.test.tsx \
  src/styles/visualAccessibility.test.ts
pnpm --dir ui check
pnpm --dir ui build
```

## Task 6: Recompose The Player Chrome

**Files:**

- Modify: `ui/src/components/VideoPreview.tsx`
- Modify: `ui/src/components/videoPreview/VideoControls.tsx`
- Modify: `ui/src/components/videoPreview/VideoControlsMoreMenu.tsx`
- Modify: `ui/src/components/videoPreview/useVideoResponsiveLayout.ts`
- Modify: `ui/src/styles/videoPreview.css`
- Modify: corresponding component tests

**Step 1: Write failing wide/compact/narrow behavior tests**

Test one accessible instance per control and the approved priority rules at 1440, 1024, and 720
CSS-pixel widths. Test navigation, Done, retry, More, focus restoration, and idle visibility.

Expected: RED where current layout leaves oversized detached regions or duplicates responsive
controls.

**Step 2: Implement the compact top bar**

Keep filename/duration left, Retry/Done right, 44 px targets, one-line truncation, and no large empty
header region.

**Step 3: Integrate navigation and controls**

Place navigation in the same bottom safe region as the control dock. Use one dock, two rows, a
bounded max width, and responsive disclosure. Do not cover the timeline or create transparent gaps.

**Step 4: Apply interaction states**

Verify hover, pressed, disabled, focus-visible, playing-idle, paused, seeking, adjusting, failed,
ended, fullscreen, reduced-motion, and forced-colors states.

**Step 5: Run component and accessibility tests**

```bash
pnpm --dir ui exec vitest run \
  src/components/VideoPreview.test.tsx \
  src/components/videoPreview/VideoControls.test.tsx \
  src/components/videoPreview/VideoControlsMoreMenu.test.tsx \
  src/components/videoPreview/useVideoResponsiveLayout.test.tsx \
  src/styles/visualAccessibility.test.ts
```

## Task 7: Change Timeline Click And Drag Semantics

**Files:**

- Modify: `ui/src/components/videoPreview/VideoTimeline.tsx`
- Modify: `ui/src/components/videoPreview/VideoTimeline.test.tsx`
- Modify: `ui/src/components/videoPreview/VideoControls.tsx`
- Modify: `ui/src/components/videoPreview/useVideoBridge.ts`

**Step 1: Write failing pointer behavior tests**

Prove:

- click emits zero preview seeks and one exact commit;
- movement below the drag threshold remains a click;
- drag emits rAF-coalesced latest preview requests;
- pointer release calls commit immediately without awaiting a preview promise;
- pointer cancel emits no commit;
- generation change cancels preview/drag state;
- thumbnail stale results do not replace the accepted current thumbnail.

Run:

```bash
pnpm --dir ui exec vitest run src/components/videoPreview/VideoTimeline.test.tsx
```

Expected: RED because pointer down currently schedules preview and pointer up awaits it.

**Step 2: Implement one interaction state machine**

Use one interaction ref/state record containing pointer ID, epoch, origin, latest target, dragging,
and pending preview. Promote to drag only after the threshold. Schedule at most one visual/preview
update per animation frame.

**Step 3: Commit immediately**

On release:

1. cancel pending preview animation frame;
2. increment/invalidate preview epoch;
3. update local committed time;
4. call exact commit immediately;
5. release pointer capture and seeking state.

Do not chain commit from the preview promise.

**Step 4: Run timeline and bridge tests**

```bash
pnpm --dir ui exec vitest run \
  src/components/videoPreview/VideoTimeline.test.tsx \
  src/components/videoPreview/useVideoBridge.test.tsx \
  src/components/videoPreview/VideoControls.test.tsx
```

## Task 8: Make Native Seek Commit Supersede Preview

**Files:**

- Modify: `crates/viewer-platform-macos/src/video/media_worker.rs`
- Modify: `crates/viewer-platform-macos/src/video/adapter.rs`
- Modify: `crates/viewer-platform-macos/src/video/diagnostics.rs`
- Modify: application/runtime seek tests if event semantics change

**Step 1: Write failing media-worker tests**

Replace the old `exact_commit_waits_for_a_new_frame_from_the_preview_seek` expectation with:

- commit invalidates pending/awaiting preview immediately;
- commit becomes runnable without preview frame acknowledgement;
- late preview frame cannot publish after commit;
- rapid previews keep at most one pending replacement;
- generation close cancels preview and commit work;
- direct click commit executes once.

Run:

```bash
cargo test -p viewer-platform-macos media_worker::tests -- --nocapture
```

Expected: RED because `next_work` currently blocks commit while `awaiting_preview_frame` exists.

**Step 2: Simplify worker state**

On `SeekIntent::Commit`:

- clear pending preview and preview frame gate;
- advance the publication epoch/request owner;
- make the commit the highest-priority next work;
- keep exact-seek completion bound to generation/request/frame serial.

Do not run libmpv commands on AppKit main.

**Step 3: Update diagnostics**

Record preview invalidation, commit issue, matching exact frame publication, and stale preview
rejection. Keep path-safe production errors.

**Step 4: Run platform/runtime tests**

```bash
cargo test -p viewer-platform-macos media_worker::tests -- --nocapture
cargo test -p viewer-platform-macos video:: --lib -- --nocapture
cargo test -p viewer-desktop video_ --lib -- --nocapture
cargo clippy -p viewer-platform-macos -p viewer-desktop --all-targets -- -D warnings
```

## Task 9: Add Real Resize, Leakage, Seek, And Ended Acceptance

**Files:**

- Modify: `scripts/video/render-feasibility.mjs` or add a focused production-player acceptance runner
- Modify: `scripts/video/render-feasibility-assertions.test.mjs`
- Modify: `src-tauri/src/video_feasibility.rs` only for test diagnostics, not production ownership
- Modify: `ui/src/acceptance/scenes/videoFeasibilityScene.tsx` only if the existing scene can exercise
  the production hierarchy faithfully
- Add target-only evidence schema/consumer tests as needed

**Step 1: Write failing executable acceptance assertions**

Require evidence for:

- no desktop-color pixel inside the content area across continuous resize frames;
- native theater bounds and video fit update on the same AppKit generation;
- playing, paused, and ended resize retain a visible frame/matte;
- click seek exact-frame latency;
- drag preview responsiveness and exact release convergence;
- wide/compact/narrow chrome non-overlap.

Tests must consume actual screenshots/structured native evidence, not read runner source strings.

**Step 2: Instrument only observable boundaries**

Add test/debug diagnostics for theater bound changes, fitted child frames, retained redraws, seek
issue, and matching frame publication. Keep absolute paths out of IPC/events.

**Step 3: Build once and run the focused native matrix**

Use the reviewed bundled runtime. Reuse the built app for retries only when the failure is a test
harness issue; rebuild after production changes.

Capture at the same viewport as the supplied screenshot and compare side by side.

**Step 4: Treat native failures honestly**

Any desktop leakage, black flash, stale frame, overlap, or latency-budget failure blocks completion.
Do not replace native evidence with a DOM screenshot.

## Task 10: Remove Dead Architecture And Update Documentation

**Files:**

- Delete unused geometry/aperture code and tests found by `rg`
- Modify: `docs/TECHNICAL_FOUNDATIONS.md`
- Modify: `docs/PRODUCT_SPEC.md`
- Modify: `docs/reviews/viewer-native-smoke-matrix.md`
- Modify: `docs/README.md` if the index needs the new spec/plan

**Step 1: Prove no production aperture/geometry ownership remains**

```bash
rg -n "video-preview-matte|video-preview-aperture|videoSetSurfaceRect|video_set_surface_rect|useMeasuredVideoStage" \
  ui/src src-tauri/src crates/viewer-application crates/viewer-platform-macos
```

Expected: no production matches. Feature-only acceptance matches must be explicitly justified.

**Step 2: Update architecture documents**

Document native theater ownership, overlay-only WebView, native live resize, and immediate commit
seek. Mark the superseded aperture documents accordingly instead of leaving contradictory current
claims.

**Step 3: Run documentation/policy gates**

Use the repository's exact policy and scope commands from `package.json`, then run `git diff --check`.

## Task 11: Focused Regression And Strict Static Gates

**Step 1: Run focused UI suites**

```bash
pnpm --dir ui exec vitest run \
  src/components/VideoPreview.test.tsx \
  src/components/videoPreview/VideoControls.test.tsx \
  src/components/videoPreview/VideoTimeline.test.tsx \
  src/components/videoPreview/useVideoBridge.test.tsx \
  src/components/videoPreview/useVideoControlsVisibility.test.tsx \
  src/components/videoPreview/useVideoResponsiveLayout.test.tsx \
  src/components/videoPreview/useVideoShortcuts.test.tsx \
  src/styles/visualAccessibility.test.ts
pnpm --dir ui check
pnpm --dir ui build
```

**Step 2: Run focused Rust suites**

```bash
cargo test -p viewer-video-mpv
cargo test -p viewer-application --test video_preview_service
cargo test -p viewer-platform-macos video:: --lib
cargo test -p viewer-platform-macos --test video_theater_layout
cargo test -p viewer-desktop video_ --lib
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
git diff --check
```

Fix every warning or failure at its source. Do not add lint suppression for refactor-created code.

## Task 12: Review, Native Acceptance, And Final Gate

**Step 1: Freeze the diff and request one fresh review**

Ask the reviewer to focus on:

- no remaining dual ownership of visible geometry;
- AppKit observer/main-thread lifecycle safety;
- opaque composition and desktop-leak prevention;
- immediate commit/stale preview rejection;
- teardown ordering;
- responsive/accessibility regression;
- tests that prove behavior rather than source-token presence.

Fix all Critical, Important, and actionable Minor findings, then return to the same reviewer.

**Step 2: Run the real native acceptance matrix**

Run the Task 9 acceptance against the reviewed frozen diff. Preserve same-viewport reference and
post-refactor captures.

**Step 3: Run one fresh full gate**

```bash
pnpm verify
```

If the gate fails, fix the exact issue, run the smallest focused proof, obtain review if the fix is
material, and run the single necessary full retry. Record both outcomes honestly.

**Step 4: Final audit**

```bash
git status --short
git diff --check
```

Completion requires:

- reviewer C0/I0 and no unresolved material Minor;
- native resize/leakage/seek/ended acceptance green;
- full gate green;
- no unrelated worktree changes;
- updated spec, architecture docs, and evidence report.

Do not commit or merge unless the user requests it.

## Task 13: Apply The Approved Extreme-Compact Light Chrome

**Design decision:** The user selected and approved the extreme compact option. The production
contract is exactly `50px` top chrome and `88px` bottom chrome, rendered with the existing Viewer
light surfaces. Figma is not part of this workflow.

**Files:**

- Modify: `crates/viewer-platform-macos/src/video/theater.rs`
- Modify: `crates/viewer-platform-macos/src/video/surface.rs`
- Modify: `crates/viewer-platform-macos/tests/video_theater_layout.rs`
- Modify: `ui/src/styles/tokens.css`
- Modify: `ui/src/styles/videoPreview.css`
- Modify: `ui/src/styles/videoPreviewLayout.test.ts`
- Modify: `ui/src/styles/visualAccessibility.test.ts`
- Modify if behavior coverage requires it: `ui/src/components/videoPreview/VideoControls.test.tsx`

### 13.1 Lock native and browser chrome dimensions

- [ ] Change the native layout test first so it requires
  `THEATER_TOP_COMMAND_BAR_HEIGHT == 50.0`,
  `THEATER_BOTTOM_INSPECTOR_HEIGHT == 88.0`, and a `662px` video viewport inside a
  `1200x800` window.
- [ ] Change the CSS contract test first so it requires `50px` and `88px` custom-property values.
- [ ] Run the two focused tests and capture the expected RED against the current `86/156` values.
- [ ] Change only the native constants and CSS custom properties.
- [ ] Re-run the focused tests and confirm GREEN before changing visual styling.

Focused commands:

```bash
cargo test -p viewer-platform-macos --test video_theater_layout -- --nocapture
pnpm --dir ui exec vitest run src/styles/videoPreviewLayout.test.ts
```

### 13.2 Replace dark player chrome with Viewer light tokens

- [ ] Extend the CSS contract test so top bar and bottom controls require
  `var(--viewer-surface)`, `var(--viewer-border)`, `var(--viewer-text)`, and
  `var(--viewer-text-secondary)` instead of the dark video palette.
- [ ] Require neutral controls with one accent-filled play/pause action and no icon inversion.
- [ ] Require the non-video stage and ended-poster background to use the Viewer light neutral
  surface; permit darkness only inside the decoded contain-fit video child.
- [ ] Add or adjust a native surface behavior test that verifies the AppKit window/theater color is
  the approved light neutral before the video child attaches.
- [ ] Run the focused UI/native tests and capture RED.
- [ ] Remap or remove the dark-only video tokens and update `videoPreview.css` and AppKit theater
  color without adding hard-coded component colors.
- [ ] Re-run focused tests and confirm GREEN.

### 13.3 Compress spacing without weakening interaction targets

- [ ] Add CSS behavior assertions for one 50px top row, two compact bottom rows, 44px minimum
  action targets, a 44px primary play/pause target, and reduced gaps/padding that fit inside 88px.
- [ ] Preserve filename truncation, centered navigation, Done, timeline time, play/pause, mute,
  fullscreen, and focus-visible states.
- [ ] Preserve the existing responsive disclosure contract: compact layouts move frame stepping,
  volume slider, and playback rate into More; chrome height never grows.
- [ ] Run `VideoControls` and responsive-layout tests before and after the CSS implementation.

Focused commands:

```bash
pnpm --dir ui exec vitest run \
  src/components/videoPreview/VideoControls.test.tsx \
  src/components/videoPreview/useVideoResponsiveLayout.test.tsx \
  src/styles/videoPreviewLayout.test.ts \
  src/styles/visualAccessibility.test.ts
pnpm --dir ui check
pnpm --dir ui build
```

### 13.4 Verify the real application, not only CSS source

- [ ] Stop any existing Viewer process before launching the new build; maintain exactly one app
  instance throughout acceptance.
- [ ] Build the Tauri application once with the reviewed bundled runtime and launch it.
- [ ] At wide and compact window sizes, capture the same real video and verify: 50px top chrome,
  88px bottom chrome, no transparent desktop leakage, no overlap, no clipping, and light styling
  consistent with the rest of Viewer.
- [ ] Exercise play/pause, timeline click, navigation, live resize, ended poster, Retry, and Done.
- [ ] Compare the native screenshots with the supplied failure screenshot and record any remaining
  mismatch rather than declaring success from unit tests.
- [ ] Run the full focused Rust/UI gates, then one fresh `pnpm verify`.
- [ ] Run `git diff --check` and report the exact dirty scope; do not commit or merge unless the user
  requests it.
