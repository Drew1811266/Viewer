# Video Interaction and Surface Aperture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove libmpv work from AppKit's interaction hot paths, make live resize track naturally, and eliminate every transparent pixel outside the fitted native video surface.

**Architecture:** A dedicated serial media worker owns seek commands and playback-property snapshots while AppKit owns only NSView/OpenGL presentation. Resize applies newest geometry immediately and redraws a paused retained frame once after settling. The transparent WKWebView substrate is covered by four opaque matte regions calculated from the same integer `VideoSurfaceRect` sent to native code.

**Tech Stack:** Rust 2024, libmpv client API, AppKit/NSOpenGLView through objc2, dispatch2, React 19, TypeScript, Vitest, Tauri v2.

## Global Constraints

- Keep the bundled libmpv/OpenGL architecture; do not introduce AVPlayer or a new media dependency.
- No synchronous libmpv command or property read may run in AppKit's seek/render hot paths.
- Seek, frame, geometry, and close work remains generation/request/sequence guarded.
- Playing resize uses natural frame callbacks; paused resize performs one trailing retained-frame redraw.
- The WKWebView substrate may be transparent, but every visible pixel outside the exact native aperture must be covered by the theater matte.
- Invalid geometry fails closed to a fully opaque stage veil.
- Use RED -> GREEN TDD for every behavior change and preserve current first-frame, final-frame, lifecycle, and accessibility contracts.

---

### Task 1: Cloneable libmpv command handle

**Files:**
- Modify: `crates/viewer-video-mpv/src/client.rs`
- Modify: `crates/viewer-video-mpv/src/lib.rs`
- Modify: `crates/viewer-video-mpv/tests/client_contract.rs`

**Interfaces:**
- Produces: `MpvCommandClient: Clone + Send + Sync`
- Produces: `MpvPlaybackSnapshot { time_us: Option<u64>, eof_reached: bool, picture_type: Option<String> }`
- Produces: `MpvClient::command_client(&self) -> MpvCommandClient`
- Produces: `MpvCommandClient::{seek_absolute_us, playback_snapshot}`

- [ ] **Step 1: Write the failing cross-thread command test**

Extend the injected client contract with a fake API that records the calling thread and blocks `seek` until released. Exercise the real public command handle:

```rust
let mut client = unsafe { MpvClient::from_api(fake_api()) }.unwrap();
client.initialize_for_rendering().unwrap();
let commands = client.command_client();
let caller = std::thread::current().id();
let worker = std::thread::spawn(move || {
    commands.seek_absolute_us(2_500_000, SeekMode::CommitExact).unwrap();
});
wait_until_seek_enters();
assert_ne!(recorded_seek_thread(), caller);
release_seek();
worker.join().unwrap();
```

Also assert one `playback_snapshot()` performs the three typed reads and normalizes unavailable values without destroying the original `MpvClient`.

- [ ] **Step 2: Run the focused test and verify RED**

Run: `cargo test -p viewer-video-mpv --test client_contract command_client -- --nocapture`

Expected: compile failure because `MpvCommandClient`, `MpvPlaybackSnapshot`, and `command_client` do not exist.

- [ ] **Step 3: Implement the minimal shared command handle**

Add a cloneable wrapper around `Arc<ClientInner>` and move the shared command/property helpers onto `ClientInner`-backed methods. Keep initialization and render API ownership on `MpvClient`:

```rust
#[derive(Clone)]
pub struct MpvCommandClient {
    inner: Arc<ClientInner>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MpvPlaybackSnapshot {
    pub time_us: Option<u64>,
    pub eof_reached: bool,
    pub picture_type: Option<String>,
}

impl MpvClient {
    pub fn command_client(&self) -> MpvCommandClient {
        MpvCommandClient { inner: Arc::clone(&self.inner) }
    }
}
```

Do not expose raw handles or generic string commands.

- [ ] **Step 4: Run the client contract suite and strict Clippy**

Run: `cargo test -p viewer-video-mpv --test client_contract && cargo clippy -p viewer-video-mpv --all-targets -- -D warnings`

Expected: all client contracts pass with no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/viewer-video-mpv/src/client.rs crates/viewer-video-mpv/src/lib.rs crates/viewer-video-mpv/tests/client_contract.rs
git commit -m "feat: add thread-safe media command handle"
```

---

### Task 2: Serial media worker and frame-gated seek state machine

**Files:**
- Create: `crates/viewer-platform-macos/src/video/media_worker.rs`
- Modify: `crates/viewer-platform-macos/src/video/mod.rs`
- Modify: `crates/viewer-platform-macos/src/video/interaction.rs`

**Interfaces:**
- Consumes: `MpvCommandClient`, `MpvPlaybackSnapshot`, `SeekRequest`, `SeekIntent`
- Produces: `MediaCommandWorker::start(commands, event_sink) -> Result<Self, VideoEngineError>`
- Produces: `activate(generation)`, `publish_seek(generation, request)`, `frame_rendered(generation, frame_serial)`, `shutdown_and_join()`
- Produces: `MediaWorkerEvent::{FrameSnapshot, SeekCompleted, Ended, Failed}`

- [ ] **Step 1: Write failing worker behavior tests**

Use a stateful fake backend at the libmpv boundary, not a mocked worker. Record actual worker-thread calls and emitted events. Cover these independent breaks:

```rust
worker.activate(7).unwrap();
worker.publish_seek(7, preview(10, 4_000_000)).unwrap();
worker.publish_seek(7, commit(11, 4_000_000)).unwrap();
assert_eq!(backend.calls(), ["preview:4000000"]);
worker.frame_rendered(7, 41).unwrap();
assert_eq!(backend.calls(), ["preview:4000000", "snapshot", "commit:4000000"]);
```

Add tests proving: a new preview supersedes an unacknowledged old preview/commit; keyboard commit without a preview executes immediately; stale generation input is rejected; shutdown joins a backend blocked in no operation and no event appears after shutdown.

- [ ] **Step 2: Run the worker tests and verify RED**

Run: `cargo test -p viewer-platform-macos video::media_worker::tests --lib -- --nocapture`

Expected: compile failure because the module and types do not exist.

- [ ] **Step 3: Implement the minimal worker**

Use one named `std::thread`, one bounded/coalesced state object, and a `Condvar`. The worker owns the backend and never dispatches libmpv calls to AppKit. Its seek state is:

```rust
struct SeekPipeline {
    latest_request_id: u64,
    pending_preview: Option<SeekRequest>,
    pending_commit: Option<SeekRequest>,
    awaiting_preview_frame: Option<PreviewFrameGate>,
}

struct PreviewFrameGate {
    generation: u64,
    target_time_us: u64,
    frame_serial_before_seek: u64,
}
```

After each rendered-frame notification, read one playback snapshot on the worker, emit progress/EOF, and release only a matching exact commit whose preview gate observed a strictly newer frame serial.

- [ ] **Step 4: Run worker and interaction tests**

Run: `cargo test -p viewer-platform-macos video::media_worker::tests video::interaction::tests --lib -- --nocapture`

Expected: all worker/state-machine tests pass; no sleep-based test is permitted.

- [ ] **Step 5: Commit**

```bash
git add crates/viewer-platform-macos/src/video/media_worker.rs crates/viewer-platform-macos/src/video/mod.rs crates/viewer-platform-macos/src/video/interaction.rs
git commit -m "feat: serialize video commands off the main thread"
```

---

### Task 3: Integrate worker snapshots with the native render adapter

**Files:**
- Modify: `crates/viewer-platform-macos/src/video/render_loop.rs`
- Modify: `crates/viewer-platform-macos/src/video/adapter.rs`
- Modify: `crates/viewer-platform-macos/src/video/diagnostics.rs`

**Interfaces:**
- Consumes: Task 2 `MediaCommandWorker` and `MediaWorkerEvent`
- Produces: `RenderedFrame { serial: u64, media_loaded: bool }`
- Produces: `MacVideoRenderSession::confirm_first_decoded_frame(serial, picture_type)`
- Produces: diagnostics counters for worker seek issue, frame acknowledgement, and stale rejection

- [ ] **Step 1: Write failing render/adapter contract tests**

Add deterministic tests proving the render path does not call synchronous client properties and that confirmation is generation/frame guarded:

```rust
let outcome = draw_after_media_load_with_frame_update();
assert_eq!(outcome, RenderedFrame { serial: 1, media_loaded: true });
assert!(!session.first_decoded_frame_ready());
session.confirm_first_decoded_frame(1, Some("I".into())).unwrap();
assert!(session.first_decoded_frame_ready());
```

Add adapter tests where a blocking fake seek is running: a queued AppKit closure must still execute before the seek is released. Add a close test proving `shutdown_and_join` completes before the render context/client/surface teardown record.

- [ ] **Step 2: Run focused tests and verify RED**

Run: `cargo test -p viewer-platform-macos video::adapter::tests video::render_loop::tests --lib -- --nocapture`

Expected: compile/behavior failures because draw still reads `time-pos`, `eof-reached`, and picture type synchronously and seek still drains on main.

- [ ] **Step 3: Wire the worker into `MacOsLibmpvAdapter`**

Replace `drain_seek_on_main` with worker publication. `draw_on_main` performs only render/present, increments a frame serial, and notifies the worker. Worker events update first-frame readiness on main and emit `TimeChanged`, `SeekCompleted`, `Ended`, or normalized failure. Remove these calls from `draw_on_main`:

```rust
session.playback_time_us()
session.eof_reached()
client.current_video_picture_type()
```

On close, invalidate generation, stop/join the worker, then drop render context, client, and native surface. Preserve the last rendered frame on `Ended`.

- [ ] **Step 4: Run macOS video tests and strict Clippy**

Run: `cargo test -p viewer-platform-macos video:: --lib -- --nocapture && cargo clippy -p viewer-platform-macos -p viewer-video-mpv --all-targets -- -D warnings`

Expected: all focused tests pass and no main-thread synchronous property/seek path remains.

- [ ] **Step 5: Commit**

```bash
git add crates/viewer-platform-macos/src/video/render_loop.rs crates/viewer-platform-macos/src/video/adapter.rs crates/viewer-platform-macos/src/video/diagnostics.rs
git commit -m "fix: keep media commands off AppKit"
```

---

### Task 4: Natural live resize and one paused trailing redraw

**Files:**
- Modify: `crates/viewer-platform-macos/src/video/interaction.rs`
- Modify: `crates/viewer-platform-macos/src/video/adapter.rs`
- Modify: `crates/viewer-platform-macos/src/video/render_loop.rs`
- Modify: `crates/viewer-platform-macos/src/video/surface.rs`

**Interfaces:**
- Produces: `LatestGeometryMailbox::mark_applied(generation, sequence, playback_active)`
- Produces: `LatestGeometryMailbox::take_settled_redraw(generation, sequence)`
- Produces: one `GEOMETRY_SETTLE_INTERVAL` trailing timer; no periodic redraw loop

- [ ] **Step 1: Replace the old redraw test with failing behavior tests**

Name the regressions directly:

```rust
for sequence in 1..=120 {
    mailbox.publish(5, sequence, geometry(sequence)).unwrap();
    mailbox.mark_applied(5, sequence, false);
}
assert_eq!(mailbox.take_settled_redraw(5, 1), None);
assert_eq!(mailbox.take_settled_redraw(5, 120), Some(120));
```

Add a playing case that produces no forced redraw, a paused case that produces exactly one, and a generation-change case that rejects the delayed timer. Add a surface geometry harness that records one logical context update per frame change.

- [ ] **Step 2: Run focused tests and verify RED**

Run: `cargo test -p viewer-platform-macos resize_ --lib -- --nocapture`

Expected: failures because the current mailbox schedules a 33 ms redraw loop and geometry performs duplicate update work.

- [ ] **Step 3: Implement the minimal trailing-redraw path**

Delete `GEOMETRY_REDRAW_INTERVAL`, `finish_redraw`, and the repeat scheduling loop. Apply every newest NSView frame immediately. When paused, schedule one settle timer carrying `(generation, latest_sequence)`; a newer sequence makes that timer a no-op and schedules the next trailing timer. When playing, rely on libmpv update callbacks.

Make one layer own OpenGL geometry notification: `MacVideoSurface::update_geometry` sets the view frame; `MacVideoRenderSession::update_geometry` must not perform a second explicit context update if AppKit already updated it through the view.

- [ ] **Step 4: Run resize, render, lifecycle, and strict Clippy tests**

Run: `cargo test -p viewer-platform-macos video:: --lib -- --nocapture && cargo test -p viewer-desktop video_ --lib -- --nocapture && cargo clippy -p viewer-platform-macos -p viewer-desktop --all-targets -- -D warnings`

Expected: all tests pass; resize bursts have no periodic forced render.

- [ ] **Step 5: Commit**

```bash
git add crates/viewer-platform-macos/src/video/interaction.rs crates/viewer-platform-macos/src/video/adapter.rs crates/viewer-platform-macos/src/video/render_loop.rs crates/viewer-platform-macos/src/video/surface.rs
git commit -m "fix: redraw paused video after resize settles"
```

---

### Task 5: Exact opaque matte and transparent video aperture

**Files:**
- Modify: `ui/src/components/videoPreview/videoGeometry.ts`
- Modify: `ui/src/components/videoPreview/videoGeometry.test.ts`
- Modify: `ui/src/components/videoPreview/useVideoBridge.ts`
- Modify: `ui/src/components/videoPreview/useVideoBridge.test.tsx`
- Modify: `ui/src/components/VideoPreview.tsx`
- Modify: `ui/src/components/VideoPreview.test.tsx`
- Modify: `ui/src/styles/videoPreview.css`
- Modify: `ui/src/styles/videoPreviewLayout.test.ts`
- Modify: `ui/src/styles/visualAccessibility.test.ts`

**Interfaces:**
- Produces: `VideoSurfaceLayout { surfaceRect: VideoSurfaceRect, aperture: VideoAperture }`
- Produces: `fitVideoSurfaceLayout(stage, media) -> VideoSurfaceLayout`
- Produces: four `.video-preview-matte--{top,right,bottom,left}` cover elements

- [ ] **Step 1: Write failing geometry and component tests**

For fractional stage geometry, hand-derive and assert that native and CSS use the same global integer bounds:

```ts
expect(fitVideoSurfaceLayout(stage(0.4, 0.4, 800.2, 600.2), media())).toEqual({
  surfaceRect: { x: 0, y: 75, width: 800, height: 450 },
  aperture: { left: -0.4, top: 74.6, width: 800, height: 450 },
})
```

Render `VideoPreview` and assert four real matte regions exist, their union covers every point outside the aperture, the reveal veil exists before first frame, and invalid geometry shows a fully opaque veil. Assert there is no border-width matte and no visible stage edge depends on rounded CSS border painting.

- [ ] **Step 2: Run UI tests and verify RED**

Run: `pnpm --dir ui exec vitest run src/components/videoPreview/videoGeometry.test.ts src/components/videoPreview/useVideoBridge.test.tsx src/components/VideoPreview.test.tsx src/styles/videoPreviewLayout.test.ts src/styles/visualAccessibility.test.ts`

Expected: failures because layout currently returns independently rounded insets and CSS uses one border pseudo-element.

- [ ] **Step 3: Implement one shared fitted layout**

Replace `fitVideoMatteInsets` with `fitVideoSurfaceLayout`. Publish `surfaceRect` to `videoSetSurfaceRect` and use the returned local aperture coordinates for CSS variables. Render four matte elements:

```tsx
<div className="video-preview-matte video-preview-matte--top" aria-hidden="true" />
<div className="video-preview-matte video-preview-matte--right" aria-hidden="true" />
<div className="video-preview-matte video-preview-matte--bottom" aria-hidden="true" />
<div className="video-preview-matte video-preview-matte--left" aria-hidden="true" />
```

The substrate remains transparent only so the native sibling is visible. The four regions use `top/left/width/height` from the aperture variables and overlap outward by one CSS pixel, never inward, so rounding cannot expose the desktop. Keep toolbar/navigation/controls above z-index 4 and use a full-stage veil until first-frame readiness.

- [ ] **Step 4: Run focused UI tests, type/style checks, and build**

Run: `pnpm --dir ui exec vitest run src/components/videoPreview/videoGeometry.test.ts src/components/videoPreview/useVideoBridge.test.tsx src/components/VideoPreview.test.tsx src/styles/videoPreviewLayout.test.ts src/styles/visualAccessibility.test.ts && pnpm --dir ui check && pnpm --dir ui build`

Expected: focused tests, TypeScript/Biome, and production build pass.

- [ ] **Step 5: Commit**

```bash
git add ui/src/components/videoPreview/videoGeometry.ts ui/src/components/videoPreview/videoGeometry.test.ts ui/src/components/videoPreview/useVideoBridge.ts ui/src/components/videoPreview/useVideoBridge.test.tsx ui/src/components/VideoPreview.tsx ui/src/components/VideoPreview.test.tsx ui/src/styles/videoPreview.css ui/src/styles/videoPreviewLayout.test.ts ui/src/styles/visualAccessibility.test.ts
git commit -m "fix: confine video transparency to the native aperture"
```

---

### Task 6: Interaction acceptance, regression gate, and handoff

**Files:**
- Modify if counters changed: `scripts/video/render-feasibility.mjs`
- Modify if assertions changed: `scripts/video/render-feasibility-assertions.test.mjs`
- Create: `docs/reviews/2026-08-14-video-interaction-latency.md`

**Interfaces:**
- Consumes: Tasks 1-5 complete behavior
- Produces: reproducible latency/resize/aperture evidence and final review record

- [ ] **Step 1: Add a failing acceptance assertion only for missing measured evidence**

Extend the existing executable runner contract to require fields derived from a real interaction run:

```js
assert.equal(result.interaction.mainThreadSeekWaitSamples, 0)
assert.equal(result.interaction.periodicResizeRedraws, 0)
assert.equal(result.interaction.desktopLeakPixels, 0)
assert.ok(result.interaction.seekP95Ms < 100)
```

The test must execute the result validator with controlled JSON; do not grep runner source.

- [ ] **Step 2: Run validator tests and verify RED**

Run: `node --test scripts/video/render-feasibility-assertions.test.mjs`

Expected: failure because the interaction fields are not yet part of the accepted evidence schema.

- [ ] **Step 3: Record a fresh native interaction run**

Build and launch one development app, then use the existing native acceptance helper to:

- click/drag six distant timeline targets;
- resize continuously for three seconds while playing and while paused;
- inspect all four aperture edges before, during, and after resize;
- play to EOF and verify the final frame remains visible;
- capture a process sample during the interaction.

Write measured results to the existing target evidence location and validate them without relaxing thresholds.

Run: `scripts/video/run-render-feasibility.sh`

Expected: the runner builds one development app, exercises the native surface, writes `target/video-render-feasibility/matrix-result.json`, and exits zero with the new interaction fields passing.

- [ ] **Step 4: Run the full verification gate**

Run: `cargo fmt --all -- --check && git diff --check && pnpm verify`

Expected: formatting, UI, Rust workspace, security boundaries, dependency policy, and license gates all pass.

- [ ] **Step 5: Request one fresh code review and fix all Critical/Important findings**

Review exact base-to-HEAD diff for thread ownership, close/join order, generation/request races, AppKit-only access, aperture coverage, and regression-test quality. Re-run focused gates after fixes and the full gate only if production code changes after the first full run.

- [ ] **Step 6: Commit acceptance evidence**

```bash
git add scripts/video/render-feasibility.mjs scripts/video/render-feasibility-assertions.test.mjs docs/reviews/2026-08-14-video-interaction-latency.md
git commit -m "test: verify responsive native video interaction"
```

---

## Self-review

- Spec coverage: worker ownership, frame-gated exact commit, off-main snapshots, natural resize, paused trailing redraw, exact aperture, stale cancellation, lifecycle cleanup, native acceptance, and final-frame preservation each map to Tasks 1-6.
- Placeholder scan: no TBD/TODO/similar-to references or unspecified error handling remain.
- Type consistency: `MpvCommandClient` and `MpvPlaybackSnapshot` are introduced in Task 1 and consumed by Task 2; `MediaCommandWorker` events are introduced in Task 2 and consumed in Task 3; `VideoSurfaceLayout` is introduced and consumed entirely in Task 5.
- Scope: one integrated player interaction project; no AVPlayer migration, codec/build work, signing, or unrelated control redesign.
