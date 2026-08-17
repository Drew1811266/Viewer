# Compact Native Video Preview Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the current thick video-preview bands with compact Apple-inspired edge overlays, eliminate trackpad-driven preview movement, and constrain only the active video-preview window to the current media aspect ratio.

**Architecture:** React owns fixed semantic chrome and a preview-scoped wheel/overscroll boundary. AppKit owns the full native theater viewport, rotation-aware contain-fit layout, and a scoped `NSWindow` aspect session that restores the exact pre-preview frame and resize policy on teardown. The existing generation-safe runtime remains the lifecycle authority; no browser geometry IPC is reintroduced.

**Tech Stack:** React 19, TypeScript, Vitest/Testing Library, CSS design tokens, Tauri 2, Rust 1.85, objc2/AppKit, existing Viewer icon registry, libmpv native surface.

## Global Constraints

- Work only in `/Users/abc/Project/Viewer/.worktrees/video-interaction-aperture` on `codex/video-interaction-aperture`.
- Preserve all unrelated dirty-worktree changes; never reset or rewrite user-owned work.
- The selected control reference is `/var/folders/hh/jj77kbxs0db1_j1kgh7hbd2c0000gn/T/codex-clipboard-f6d217de-ce8a-48bc-bb60-d4c5b5f38e75.png`.
- Reuse Viewer tokens, typography, `ViewerIconButton`, and the existing icon registry. Do not add handcrafted SVG/CSS art or a Figma dependency.
- Upper overlay visible height is at most 40 px. Lower controller visible height is at most 72 px.
- Visible button bodies are 28-32 px except the emphasized play/pause body; every interactive target remains at least 44 x 44 CSS px.
- Video preview is the only aspect-constrained mode. Closing, failure, cancellation, project close, and window close restore ordinary Viewer resizing.
- AppKit performs live-resize layout synchronously. React must not publish native surface rectangles.
- Development-only scope: do not add signing, notarization, distribution, or release gates.
- Every behavior change follows RED -> GREEN -> refactor. Do not write production code before observing the intended failing test.
- Do not run repeated full verification during development. Use focused suites, then one fresh final `pnpm verify` after visual/native QA passes.

---

### Task 1: Freeze The Preview Interaction Island

**Files:**
- Create: `ui/src/components/videoPreview/useVideoPreviewScrollLock.ts`
- Create: `ui/src/components/videoPreview/useVideoPreviewScrollLock.test.tsx`
- Modify: `ui/src/components/VideoPreview.tsx`
- Modify: `ui/src/styles/videoPreview.css`
- Modify: `ui/src/styles/videoPreviewLayout.test.ts`
- Modify: `ui/src/styles/visualAccessibility.test.ts`

**Interfaces:**
- Consumes: `React.RefObject<HTMLElement | null>` for the mounted preview dialog.
- Produces: `useVideoPreviewScrollLock(root: RefObject<HTMLElement | null>): void`.
- Guarantees: wheel gestures inside preview are cancelled, preview scroll offsets remain zero, and timeline pointer events are unaffected.

- [ ] **Step 1: Write the failing hook behavior test**

```tsx
it('cancels horizontal and vertical wheel gestures without consuming pointer input', () => {
  const { getByTestId } = render(<ScrollLockHarness />)
  const root = getByTestId('scroll-lock-root')
  const wheel = new WheelEvent('wheel', {
    bubbles: true,
    cancelable: true,
    deltaX: 48,
    deltaY: 64,
  })

  expect(root.dispatchEvent(wheel)).toBe(false)
  expect(wheel.defaultPrevented).toBe(true)

  const pointer = new PointerEvent('pointerdown', { bubbles: true, cancelable: true })
  expect(root.dispatchEvent(pointer)).toBe(true)
})
```

- [ ] **Step 2: Run the hook test and verify RED**

Run:

```bash
pnpm --dir ui exec vitest run src/components/videoPreview/useVideoPreviewScrollLock.test.tsx
```

Expected: FAIL because `useVideoPreviewScrollLock` does not exist.

- [ ] **Step 3: Implement the preview-scoped passive-false wheel boundary**

```ts
import { type RefObject, useEffect } from 'react'

export function useVideoPreviewScrollLock(root: RefObject<HTMLElement | null>): void {
  useEffect(() => {
    const node = root.current
    if (node === null) return
    const preventScroll = (event: WheelEvent) => event.preventDefault()
    node.addEventListener('wheel', preventScroll, { passive: false })
    return () => node.removeEventListener('wheel', preventScroll)
  }, [root])
}
```

Call the hook once from `VideoPreview` with the existing `dialog` ref. Do not add global listeners.

- [ ] **Step 4: Add failing CSS contract assertions**

```ts
expect(rule('.video-preview')).toMatchObject({
  inset: '0',
  overflow: 'hidden',
  'overscroll-behavior': 'none',
  position: 'fixed',
})
expect(rule(":root:has(.viewer-shell[data-video-preview-open='true'])")).toMatchObject({
  overflow: 'hidden',
  'overscroll-behavior': 'none',
})
expect(rule("body:has(.viewer-shell[data-video-preview-open='true'])")).toMatchObject({
  overflow: 'hidden',
  'overscroll-behavior': 'none',
})
```

- [ ] **Step 5: Run the style test and verify RED**

Run:

```bash
pnpm --dir ui exec vitest run src/styles/videoPreviewLayout.test.ts
```

Expected: FAIL on missing `position: fixed` and `overscroll-behavior: none` declarations.

- [ ] **Step 6: Implement the CSS containment boundary**

Set `.video-preview` to fixed/inset-zero and add `overflow: hidden; overscroll-behavior: none` to the active-preview `:root`, `body`, and `#root` selectors. Keep pointer interaction enabled; do not use `touch-action: none` on the timeline or control tree.

- [ ] **Step 7: Run focused UI verification**

Run:

```bash
pnpm --dir ui exec vitest run \
  src/components/videoPreview/useVideoPreviewScrollLock.test.tsx \
  src/components/VideoPreview.test.tsx \
  src/styles/videoPreviewLayout.test.ts \
  src/styles/visualAccessibility.test.ts
```

Expected: all tests PASS.

- [ ] **Step 8: Commit the interaction boundary**

```bash
git add ui/src/components/VideoPreview.tsx \
  ui/src/components/videoPreview/useVideoPreviewScrollLock.ts \
  ui/src/components/videoPreview/useVideoPreviewScrollLock.test.tsx \
  ui/src/styles/videoPreview.css \
  ui/src/styles/videoPreviewLayout.test.ts \
  ui/src/styles/visualAccessibility.test.ts
git commit -m "fix: lock video preview scroll gestures"
```

---

### Task 2: Replace Thick Bands With Compact Apple-Inspired Overlays

**Files:**
- Modify: `ui/src/components/VideoPreview.tsx`
- Modify: `ui/src/components/VideoPreview.test.tsx`
- Modify: `ui/src/components/videoPreview/VideoControls.tsx`
- Modify: `ui/src/components/videoPreview/VideoControls.test.tsx`
- Modify: `ui/src/components/videoPreview/VideoControlsMoreMenu.tsx`
- Modify: `ui/src/styles/videoPreview.css`
- Modify: `ui/src/styles/videoPreviewLayout.test.ts`
- Modify: `ui/src/styles/visualAccessibility.test.ts`
- Modify only if an icon is missing: the existing Viewer icon registry and its registry test

**Interfaces:**
- Consumes: existing `VideoControlViewState`, `VideoControlCommands`, `ViewerToolbar`, `ViewerIconButton`, and Viewer tokens.
- Produces: compact upper overlay, lower controller, and complete idle/hover/pressed/focus/active/disabled state styling.
- Does not change: bridge command names, playback semantics, timeline request semantics, or accessibility labels.

- [ ] **Step 1: Rewrite the layout contract as a failing test**

Replace the old 50/88 reservation expectations with:

```ts
expect(rule('.video-preview')).toMatchObject({
  '--video-preview-top-overlay-height': '36px',
  '--video-preview-bottom-controller-height': '64px',
})
expect(rule('.video-preview-topbar')).toMatchObject({
  height: 'var(--video-preview-top-overlay-height)',
  position: 'absolute',
})
expect(rule('.video-preview-bottom-chrome')).toMatchObject({
  bottom: '10px',
  height: 'var(--video-preview-bottom-controller-height)',
  position: 'absolute',
})
expect(rule('.video-controls .viewer-button')).toMatchObject({
  height: '44px',
  width: '44px',
})
expect(rule('.video-controls .viewer-button .viewer-icon')).toMatchObject({
  height: '28px',
  width: '28px',
})
```

Also assert `.video-preview-ended-poster { inset: 0 }` so ended media does not reserve the old bands.

- [ ] **Step 2: Run the layout test and verify RED**

Run:

```bash
pnpm --dir ui exec vitest run src/styles/videoPreviewLayout.test.ts
```

Expected: FAIL on the old 50 px/88 px tokens and full-width opaque bands.

- [ ] **Step 3: Add failing component assertions for hierarchy and states**

```tsx
expect(screen.getByRole('toolbar', { name: '视频预览工具' })).toHaveClass(
  'video-preview-topbar',
)
expect(screen.getByRole('group', { name: '视频播放控制' })).toHaveAttribute(
  'data-layout',
  'wide',
)
expect(screen.getByRole('button', { name: '播放' })).toHaveAttribute('aria-pressed', 'false')
```

If `ViewerIconButton` already exposes pressed state through `active`, reuse it. Do not introduce a second button primitive.

- [ ] **Step 4: Run component tests and verify RED only for the new state contract**

Run:

```bash
pnpm --dir ui exec vitest run \
  src/components/VideoPreview.test.tsx \
  src/components/videoPreview/VideoControls.test.tsx
```

Expected: FAIL on the missing pressed-state or compact overlay contract, not on test setup.

- [ ] **Step 5: Implement the compact overlays**

Use these visible dimensions:

```css
.video-preview {
  --video-preview-top-overlay-height: 36px;
  --video-preview-bottom-controller-height: 64px;
}

.video-preview-topbar {
  height: var(--video-preview-top-overlay-height);
  inset: 10px 12px auto;
}

.video-preview-bottom-chrome {
  bottom: 10px;
  height: var(--video-preview-bottom-controller-height);
  inset-inline: 12px;
}
```

Use existing Viewer neutral surfaces with restrained translucency/backdrop support, a 1 px neutral boundary, and the existing cobalt accent. Keep chrome edge-anchored and visually continuous; do not recreate the purple source or add a detached oversized card.

Keep the 44 px button box but size its visible icon/body to 28-32 px through the existing icon element and inner surface. Do not shrink the accessible target.

- [ ] **Step 6: Implement responsive disclosure without duplicate controls**

Keep the existing `useVideoResponsiveLayout` boundary. On compact layouts, frame stepping, volume, and rate remain in the existing More menu. On wide layouts, they remain inline. Add no duplicate hidden instances.

- [ ] **Step 7: Verify control and accessibility suites**

Run:

```bash
pnpm --dir ui exec vitest run \
  src/components/VideoPreview.test.tsx \
  src/components/videoPreview/VideoControls.test.tsx \
  src/components/videoPreview/VideoControlsMoreMenu.test.tsx \
  src/components/videoPreview/VideoTimeline.test.tsx \
  src/styles/videoPreviewLayout.test.ts \
  src/styles/visualAccessibility.test.ts
pnpm --dir ui check
```

Expected: all focused tests and UI check PASS.

- [ ] **Step 8: Commit the compact visual system**

```bash
git add ui/src/components/VideoPreview.tsx \
  ui/src/components/VideoPreview.test.tsx \
  ui/src/components/videoPreview/VideoControls.tsx \
  ui/src/components/videoPreview/VideoControls.test.tsx \
  ui/src/components/videoPreview/VideoControlsMoreMenu.tsx \
  ui/src/styles/videoPreview.css \
  ui/src/styles/videoPreviewLayout.test.ts \
  ui/src/styles/visualAccessibility.test.ts
git commit -m "feat: compact video preview controls"
```

---

### Task 3: Give The Native Video Surface The Full Content Bounds

**Files:**
- Modify: `crates/viewer-platform-macos/src/video/theater.rs`
- Modify: `crates/viewer-platform-macos/src/video/mod.rs`
- Modify: `crates/viewer-platform-macos/src/video/surface.rs`
- Modify: `crates/viewer-platform-macos/tests/video_theater_layout.rs`

**Interfaces:**
- Consumes: `TheaterBounds`, `VideoDisplayGeometry`, `contain_fit_frame`.
- Produces: `theater_viewport_frame(bounds) -> Option<TheaterFrame>` where valid bounds map to the complete theater.
- Removes: fixed native reservations for the old 50 px top and 88 px bottom bands.

- [ ] **Step 1: Write the failing full-viewport tests**

```rust
#[test]
fn overlay_chrome_does_not_reserve_native_video_space() {
    assert_eq!(
        theater_viewport_frame(TheaterBounds {
            width: 1024.0,
            height: 720.0,
            scale_factor: 2.0,
        }),
        Some(TheaterFrame {
            x: 0.0,
            y: 0.0,
            width: 1024.0,
            height: 720.0,
        })
    );
}
```

Add a second test proving 16:9 media contain-fits inside the complete bounds rather than the old reserved viewport.

- [ ] **Step 2: Run the native layout test and verify RED**

Run:

```bash
cargo test -p viewer-platform-macos --test video_theater_layout -- --nocapture
```

Expected: FAIL because the current viewport starts at y=88 and subtracts 138 px.

- [ ] **Step 3: Remove band reservations from AppKit layout**

Implement:

```rust
pub fn theater_viewport_frame(bounds: TheaterBounds) -> Option<TheaterFrame> {
    if !valid_positive(bounds.width)
        || !valid_positive(bounds.height)
        || !valid_positive(bounds.scale_factor)
    {
        return None;
    }
    Some(TheaterFrame {
        x: 0.0,
        y: 0.0,
        width: align_to_backing_pixel(bounds.width, bounds.scale_factor),
        height: align_to_backing_pixel(bounds.height, bounds.scale_factor),
    })
}
```

Delete `THEATER_TOP_COMMAND_BAR_HEIGHT` and `THEATER_BOTTOM_INSPECTOR_HEIGHT` exports after all callers/tests are migrated.

- [ ] **Step 4: Verify layout and surface suites**

Run:

```bash
cargo test -p viewer-platform-macos --test video_theater_layout -- --nocapture
cargo test -p viewer-platform-macos video::surface::tests --lib -- --nocapture
cargo fmt --all -- --check
```

Expected: PASS.

- [ ] **Step 5: Commit the full native viewport**

```bash
git add crates/viewer-platform-macos/src/video/theater.rs \
  crates/viewer-platform-macos/src/video/mod.rs \
  crates/viewer-platform-macos/src/video/surface.rs \
  crates/viewer-platform-macos/tests/video_theater_layout.rs
git commit -m "refactor: overlay chrome on native video"
```

---

### Task 4: Add A Preview-Scoped Native Window Aspect Session

**Files:**
- Create: `crates/viewer-platform-macos/src/video/window_aspect.rs`
- Create: `crates/viewer-platform-macos/tests/video_window_aspect.rs`
- Modify: `crates/viewer-platform-macos/src/video/mod.rs`
- Modify: `crates/viewer-platform-macos/src/video/surface.rs`
- Modify: `crates/viewer-platform-macos/Cargo.toml` only if an already-workspace-pinned objc2 AppKit feature is required

**Interfaces:**
- Consumes: `VideoDisplayGeometry`, current `NSWindow`, current content frame, and visible screen frame.
- Produces:

```rust
pub struct VideoWindowAspectSession { /* retained native window and prior state */ }

impl VideoWindowAspectSession {
    pub fn install(
        window: &NSWindow,
        media: VideoDisplayGeometry,
    ) -> Result<Self, WindowAspectError>;
    pub fn restore(&mut self) -> Result<(), WindowAspectError>;
}

pub fn display_aspect(media: VideoDisplayGeometry) -> Option<f64>;
pub fn best_fit_content_size(
    current: AspectSize,
    visible: AspectSize,
    aspect: f64,
) -> Option<AspectSize>;
```

- Guarantees: rotation-aware ratio, bounded initial size, synchronous AppKit aspect constraint, idempotent restoration, and no global policy changes.

- [ ] **Step 1: Write failing pure geometry tests**

```rust
#[test]
fn rotation_changes_the_installed_display_aspect() {
    assert_eq!(display_aspect(VideoDisplayGeometry {
        width: 1920,
        height: 1080,
        rotation_degrees: 90,
    }), Some(1080.0 / 1920.0));
}

#[test]
fn best_fit_stays_inside_the_visible_screen() {
    let size = best_fit_content_size(
        AspectSize::new(1600.0, 900.0),
        AspectSize::new(1200.0, 800.0),
        16.0 / 9.0,
    ).expect("valid fit");
    assert!(size.width <= 1200.0);
    assert!(size.height <= 800.0);
    assert!((size.width / size.height - 16.0 / 9.0).abs() < 0.000_001);
}
```

Add invalid zero/non-finite and unsupported-rotation cases.

- [ ] **Step 2: Run geometry tests and verify RED**

Run:

```bash
cargo test -p viewer-platform-macos --test video_window_aspect -- --nocapture
```

Expected: FAIL because `window_aspect` and the exported functions do not exist.

- [ ] **Step 3: Implement pure ratio and best-fit geometry**

Keep calculations independent from AppKit objects so tests run deterministically. Normalize rotation with `rem_euclid(360)`, accept only 0/90/180/270, and reject invalid dimensions.

- [ ] **Step 4: Write failing lifecycle tests around an injected window policy seam**

Use a small internal test seam rather than mocking `NSWindow`:

```rust
#[test]
fn restore_is_idempotent_and_restores_the_prior_policy_once() {
    let calls = RefCell::new(Vec::new());
    let mut lease = WindowAspectLease::new_for_test(
        || calls.borrow_mut().push("install"),
        || calls.borrow_mut().push("restore"),
    ).expect("install");
    lease.restore().expect("first restore");
    lease.restore().expect("second restore");
    assert_eq!(&*calls.borrow(), &["install", "restore"]);
}
```

- [ ] **Step 5: Implement the AppKit session**

On the main thread:

1. capture the previous content aspect ratio and window frame;
2. compute a best-fit content size inside the current screen visible frame;
3. call AppKit's content-aspect API with the rotated media dimensions;
4. resize once using the calculated content size;
5. retain enough native ownership to restore safely after surface teardown.

`restore` must clear/restore the prior aspect policy and restore the pre-preview frame exactly once. `Drop` is a fail-safe only; normal teardown calls `restore` explicitly so errors remain observable.

- [ ] **Step 6: Bind the session to `MacVideoSurface` ownership**

Add an `aspect_session: Option<VideoWindowAspectSession>` field. Install it during `mount_theater` before attaching the video child. On `unmount`, restore before removing the theater. If mounting later fails, the partially created session must restore through normal ownership cleanup.

- [ ] **Step 7: Run focused macOS tests and strict Clippy**

Run:

```bash
cargo test -p viewer-platform-macos --test video_window_aspect -- --nocapture
cargo test -p viewer-platform-macos video:: --lib -- --nocapture
cargo clippy -p viewer-platform-macos --all-targets -- -D warnings
cargo fmt --all -- --check
```

Expected: PASS without `allow`/`expect` lint suppressions.

- [ ] **Step 8: Commit the native aspect session**

```bash
git add crates/viewer-platform-macos/src/video/window_aspect.rs \
  crates/viewer-platform-macos/tests/video_window_aspect.rs \
  crates/viewer-platform-macos/src/video/mod.rs \
  crates/viewer-platform-macos/src/video/surface.rs \
  crates/viewer-platform-macos/Cargo.toml
git commit -m "feat: constrain video preview aspect"
```

---

### Task 5: Prove Generation-Safe Install, Replacement, Fullscreen, And Restoration

**Files:**
- Modify: `crates/viewer-platform-macos/src/video/adapter.rs`
- Modify: `src-tauri/src/commands/video.rs`
- Modify: `src-tauri/src/video_runtime.rs` only if a lifecycle signal is required after tests prove surface ownership is insufficient
- Modify: `tests/video_runtime_lifecycle.rs`
- Modify: `crates/viewer-platform-macos/tests/video_window_aspect.rs`

**Interfaces:**
- Consumes: `MacVideoSurface`-owned `VideoWindowAspectSession` and existing generation-gated `video_open`, `video_close`, and fullscreen commands.
- Produces: exact restoration for normal close, replacement, stale close, failed prepare, cancellation, project close, and window close.
- Rule: prefer surface ownership. Do not add a new JS command or geometry DTO unless a failing lifecycle test proves it necessary.

- [ ] **Step 1: Write failing lifecycle regressions**

Add tests that express these observable sequences:

```rust
#[tokio::test]
async fn replacing_video_restores_old_aspect_before_installing_the_new_generation() {
    // expected ordered calls: install(16:9), restore(16:9), install(9:16)
}

#[tokio::test]
async fn stale_close_cannot_restore_the_active_generation_aspect() {
    // generation 7 close after generation 8 open leaves generation 8 installed
}

#[tokio::test]
async fn failed_prepare_restores_the_pre_preview_window_policy() {
    // prepare failure leaves no installed aspect lease
}
```

Use existing fake engine/port patterns; assert ordered behavior rather than implementation fields.

- [ ] **Step 2: Run focused lifecycle tests and verify RED**

Run:

```bash
cargo test -p viewer-desktop --test video_runtime_lifecycle aspect -- --nocapture
cargo test -p viewer-platform-macos --test video_window_aspect -- --nocapture
```

Expected: FAIL on missing ordered install/restore behavior.

- [ ] **Step 3: Implement the smallest lifecycle integration**

Keep all AppKit calls on the existing main-queue boundary. `prepare_surface` closes the prior owned surface/session before installing the replacement. `close_on_main` restores only the currently owned surface. A stale generation never reaches current surface teardown.

Fullscreen uses the existing native window transition. Verify AppKit ignores the content-aspect constraint while fullscreen and resumes it on exit; add explicit suspend/resume calls only if real focused testing demonstrates that AppKit does not preserve this behavior.

- [ ] **Step 4: Run desktop, platform, and application regressions**

Run:

```bash
cargo test -p viewer-desktop --test video_runtime_lifecycle -- --nocapture
cargo test -p viewer-desktop video_ --lib -- --nocapture
cargo test -p viewer-platform-macos video:: --lib -- --nocapture
cargo test -p viewer-application --test video_preview_service -- --nocapture
cargo clippy -p viewer-desktop -p viewer-platform-macos -p viewer-application \
  --all-targets -- -D warnings
```

Expected: PASS.

- [ ] **Step 5: Commit lifecycle integration**

```bash
git add crates/viewer-platform-macos/src/video/adapter.rs \
  crates/viewer-platform-macos/tests/video_window_aspect.rs \
  src-tauri/src/commands/video.rs \
  src-tauri/src/video_runtime.rs \
  tests/video_runtime_lifecycle.rs
git commit -m "fix: restore preview window policy on close"
```

---

### Task 6: Verify The Real macOS Experience And Finish Design QA

**Files:**
- Modify: `design-qa.md`
- Modify: `docs/reviews/2026-08-16-viewer-native-video-theater-visual-contract.md`
- Create screenshots under: `target/design-qa-video-compact-native/`
- Modify production/test files only for failures proven by this task, with a fresh RED before each fix

**Interfaces:**
- Consumes: real development Viewer app, bundled reviewed runtime, supplied Apple control reference, current project fixtures.
- Produces: same-state reference/implementation comparison and `design-qa.md` with `final result: passed` or `final result: blocked`.

- [ ] **Step 1: Run the focused prebuild gate**

```bash
pnpm --dir ui exec vitest run \
  src/components/VideoPreview.test.tsx \
  src/components/videoPreview/VideoControls.test.tsx \
  src/components/videoPreview/VideoControlsMoreMenu.test.tsx \
  src/components/videoPreview/VideoTimeline.test.tsx \
  src/components/videoPreview/useVideoPreviewScrollLock.test.tsx \
  src/styles/videoPreviewLayout.test.ts \
  src/styles/visualAccessibility.test.ts
pnpm --dir ui check
cargo test -p viewer-platform-macos --test video_theater_layout -- --nocapture
cargo test -p viewer-platform-macos --test video_window_aspect -- --nocapture
cargo fmt --all -- --check
git diff --check
```

Expected: PASS.

- [ ] **Step 2: Build one fresh development app**

Use the repository's existing Tauri development bundle command. After the build, copy the verified runtime to:

```text
target/debug/bundle/macos/Viewer.app/Contents/Resources/ViewerVideoRuntime
```

Run `scripts/video/verify-runtime.sh` with `VIEWER_VIDEO_STAGE_DIR` pointing at that exact resource before launch. Do not invoke Developer ID, notarization, stapling, or release scripts.

- [ ] **Step 3: Run real interaction checks with Computer Use**

At minimum verify:

1. open 16:9 video and record initial window/content dimensions;
2. live-resize from each edge/corner and confirm the media ratio remains fixed without React lag;
3. perform vertical and horizontal two-finger scroll gestures over title, video, timeline, and empty overlay; confirm no UI/document movement;
4. drag and click timeline; confirm immediate local response and committed frame update;
5. switch to a different video ratio and confirm the window constraint changes once;
6. enter/exit fullscreen and confirm aspect resumes;
7. reach ended state and confirm retained poster/final frame, no black;
8. press Done and confirm the ordinary Viewer window can resize freely.

Capture paused wide, playing wide, compact, narrow, ended, and post-Done states.

- [ ] **Step 4: Build a same-state visual comparison**

Place the supplied reference and the paused implementation capture side-by-side at normalized density. Inspect upper overlay height, lower controller height, button visual size, hit spacing, icon contrast, timeline alignment, subject obstruction, and all visible transparent/black gaps.

- [ ] **Step 5: Update `design-qa.md` and close P0-P2 findings**

Record exact source and implementation paths, viewport sizes, interaction state, differences, and fixes. If any source capture, real app capture, trackpad check, or aspect check is unavailable, write `final result: blocked`. Do not claim completion from unit tests alone.

- [ ] **Step 6: Run one fresh final verification**

Only after design QA passes:

```bash
pnpm verify
```

Expected: exit 0 across policy, UI, build, Rust, security, and license gates. Existing informational deprecation or allowed duplicate notices may remain; new warnings may not.

- [ ] **Step 7: Commit QA evidence and final corrections**

```bash
git add design-qa.md \
  docs/reviews/2026-08-16-viewer-native-video-theater-visual-contract.md
git commit -m "test: verify compact native video preview"
```

Do not add ignored `target/` screenshots to source control. If real QA proves a production defect,
return to the owning task, add a focused failing regression, and commit that correction with the
owning task's production/test files before committing these QA documents.

---

## Plan Self-Review

- Spec coverage: compact dimensions, 44 px hit targets, Apple-inspired states, fixed overlay ownership, scroll containment, full native viewport, rotation-aware aspect, fullscreen behavior, generation replacement, restoration, ended state, real trackpad QA, and visual QA are each assigned to a task.
- Scope: one coherent feature. UI overlay, native viewport, and native window policy are coupled by the single requirement that chrome no longer changes the media aspect.
- Type consistency: `VideoDisplayGeometry` remains the authoritative media geometry; `VideoWindowAspectSession` is owned by `MacVideoSurface`; React receives no new geometry API.
- Placeholder scan: no unfinished markers or deferred implementation gaps remain. Conditional production changes are explicitly gated by failing evidence rather than left unspecified.
- Safety: the plan preserves the dirty worktree, limits changes to video preview, and excludes release signing/notarization.
