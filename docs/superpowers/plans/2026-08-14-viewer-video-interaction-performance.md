# Viewer Video Interaction Performance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make live video resize and timeline scrubbing responsive by replacing historical command queues with generation-safe latest-value native coordinators while retaining libmpv.

**Architecture:** React publishes one fitted rectangle per animation frame and separates scrub previews from the final exact seek. Tauri bypasses the lifecycle transition lane for high-frequency publication, while the macOS adapter owns bounded geometry and seek mailboxes that schedule at most one main-queue drain and reject stale generations, sequences, and request IDs. libmpv uses keyframe seeks during dragging, exact seek on release, and redraws the retained decoded frame after geometry changes.

**Tech Stack:** React 19, TypeScript, Vitest, Tauri 2, Rust 2024, Tokio, AppKit/OpenGL, libmpv render API.

## Global Constraints

- Retain libmpv, `vo=libmpv`, `hwdec=auto-safe`, the native contain-fit surface, and the first-frame gate.
- Do not add a dependency or decoded-frame IPC path.
- Resize and preview seek traffic must be bounded latest-value work, never FIFO replay.
- A drag uses keyframe previews and emits exactly one exact commit on release.
- Generation invalidation wins over geometry, seek, transport, and completion work.
- Transient geometry and superseded preview errors do not close the player.
- Precise commit failures use the existing normalized playback failure state.
- No IPC event, diagnostic, or error may expose an absolute source path.
- Write each production behavior only after its focused test has failed for the expected reason.

---

## File Structure

- `crates/viewer-video-mpv/src/client.rs` owns explicit keyframe-preview and exact-commit libmpv commands.
- `crates/viewer-application/src/video.rs` owns platform-neutral request identities and commit state transitions.
- `crates/viewer-application/src/ports.rs` exposes synchronous high-frequency publication separately from async lifecycle commands.
- `crates/viewer-test-support/src/video_engine.rs` records the complete real engine contract for application/runtime tests.
- `src-tauri/src/dto/video.rs` and `src-tauri/src/commands/video.rs` own strict IPC payloads.
- `src-tauri/src/video_runtime.rs` validates active generation and routes preview/geometry publication outside the transition mutex.
- `crates/viewer-platform-macos/src/video/interaction.rs` owns pure bounded mailbox state for deterministic tests.
- `crates/viewer-platform-macos/src/video/adapter.rs` schedules main-queue drains and publishes request-bound completion events.
- `crates/viewer-platform-macos/src/video/render_loop.rs` applies geometry and redraws the retained decoded picture.
- `ui/src/api/types.ts` and `ui/src/api/viewer.ts` expose typed seek intent/request ID and geometry sequence.
- `ui/src/components/videoPreview/VideoTimeline.tsx` owns gesture-level preview/commit behavior.
- `ui/src/components/videoPreview/useVideoBridge.ts` owns generation-local request counters and native publication.
- `tests/video_runtime_lifecycle.rs` proves lane independence and stale-event rejection across the desktop boundary.

---

### Task 1: Give libmpv explicit fast-preview and exact-commit seek modes

**Files:**
- Modify: `crates/viewer-video-mpv/src/client.rs`
- Modify: `crates/viewer-video-mpv/src/lib.rs`
- Test: `crates/viewer-video-mpv/tests/client_contract.rs`

**Interfaces:**
- Consumes: initialized `MpvClient` and bounded microsecond target.
- Produces: `SeekMode::{PreviewKeyframe, CommitExact}` and `MpvClient::seek_absolute_us(time_us, mode)`.

- [ ] **Step 1: Write the failing command-contract test**

Add a test that invokes the real `MpvClient` against the existing stateful fake API and hand-checks the emitted command vectors:

```rust
#[test]
fn preview_seek_uses_keyframes_and_commit_seek_is_exact() {
    let _guard = fake_guard().lock().unwrap();
    reset_fake();
    let mut client = unsafe { MpvClient::from_api(fake_api()) }.unwrap();
    client.initialize_for_rendering().unwrap();

    client
        .seek_absolute_us(1_250_000, SeekMode::PreviewKeyframe)
        .unwrap();
    client
        .seek_absolute_us(2_500_000, SeekMode::CommitExact)
        .unwrap();

    assert!(calls().lock().unwrap().contains(&Call::Command(vec![
        "seek".into(),
        "1.250000".into(),
        "absolute+keyframes".into(),
    ])));
    assert!(calls().lock().unwrap().contains(&Call::Command(vec![
        "seek".into(),
        "2.500000".into(),
        "absolute+exact".into(),
    ])));
}
```

- [ ] **Step 2: Run the focused test and verify RED**

Run:

```bash
cargo test -p viewer-video-mpv --test client_contract preview_seek_uses_keyframes_and_commit_seek_is_exact -- --nocapture
```

Expected: compile failure because `SeekMode` and the two-argument seek method do not exist.

- [ ] **Step 3: Implement the minimal typed seek mode**

In `client.rs`:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeekMode {
    PreviewKeyframe,
    CommitExact,
}

impl SeekMode {
    const fn command_flag(self) -> &'static str {
        match self {
            Self::PreviewKeyframe => "absolute+keyframes",
            Self::CommitExact => "absolute+exact",
        }
    }
}

pub fn seek_absolute_us(&self, time_us: u64, mode: SeekMode) -> Result<(), MpvError> {
    let seconds = format!("{:.6}", time_us as f64 / 1_000_000.0);
    self.command_from_strings(&["seek", &seconds, mode.command_flag()])
}
```

Re-export `SeekMode` from `lib.rs`. Update the existing exact-seek call sites to pass `CommitExact` so the workspace compiles without changing behavior yet.

- [ ] **Step 4: Run the package tests and verify GREEN**

```bash
cargo test -p viewer-video-mpv --test client_contract -- --nocapture
cargo test -p viewer-video-mpv --lib
```

Expected: all runnable tests pass; staged runtime tests retain their existing ignored status.

- [ ] **Step 5: Commit Task 1**

```bash
git add crates/viewer-video-mpv/src/client.rs crates/viewer-video-mpv/src/lib.rs crates/viewer-video-mpv/tests/client_contract.rs crates/viewer-platform-macos/src/video/render_loop.rs
git commit -m "feat: distinguish preview and exact video seeks"
```

---

### Task 2: Add generation- and request-bound seek contracts across application and IPC

**Files:**
- Modify: `crates/viewer-application/src/video.rs`
- Modify: `crates/viewer-application/src/ports.rs`
- Modify: `crates/viewer-application/src/lib.rs`
- Modify: `crates/viewer-test-support/src/video_engine.rs`
- Modify: `crates/viewer-application/tests/video_preview_service.rs`
- Modify: `src-tauri/src/dto/video.rs`
- Modify: `src-tauri/src/commands/video.rs`
- Modify: `src-tauri/src/video_runtime.rs`
- Modify: `tests/video_runtime_lifecycle.rs`

**Interfaces:**
- Consumes: Task 1 `SeekMode` mapping in the platform adapter.
- Produces: `SeekIntent`, `SeekRequest`, request-bound `EngineEvent::SeekCompleted`, synchronous `VideoEngine::publish_seek`, strict `VideoSeekDto { generation, request_id, time_us, intent }`.

- [ ] **Step 1: Write failing application tests for preview/commit ownership**

Add literal request fixtures:

```rust
const PREVIEW: SeekRequest = SeekRequest {
    request_id: 10,
    time_us: 2_000_000,
    intent: SeekIntent::Preview,
};
const COMMIT: SeekRequest = SeekRequest {
    request_id: 11,
    time_us: 2_250_000,
    intent: SeekIntent::Commit,
};

#[tokio::test]
async fn preview_does_not_enter_seeking_and_only_matching_commit_completes() {
    let (service, engine) = harness();
    let generation = service.open(source(1)).await.unwrap();
    start_playing(&service, generation).await;
    service.execute(VideoCommand {
        generation,
        kind: VideoCommandKind::Pause,
    }).await.unwrap();

    service.preview_seek(generation, PREVIEW).unwrap();
    assert_eq!(service.snapshot().state, VideoPlaybackState::Paused);

    service.execute(VideoCommand::seek(generation, COMMIT)).await.unwrap();
    assert_eq!(service.snapshot().state, VideoPlaybackState::Seeking);

    service.handle_engine_event(
        generation,
        EngineEvent::SeekCompleted { request_id: 10, time_us: 2_000_000 },
    ).await
    .unwrap();
    assert_eq!(service.snapshot().state, VideoPlaybackState::Seeking);

    service.handle_engine_event(
        generation,
        EngineEvent::SeekCompleted { request_id: 11, time_us: 2_250_000 },
    ).await
    .unwrap();
    assert_eq!(service.snapshot().state, VideoPlaybackState::Paused);
    assert_eq!(engine.calls().last(), Some(&FakeVideoEngineCall::PublishSeek(generation, COMMIT)));
}
```

Add a desktop lifecycle test that starts a blocked transport command, publishes a preview seek and geometry update, and asserts both publication calls return before the transport blocker is released.

- [ ] **Step 2: Run focused application/desktop tests and verify RED**

```bash
cargo test -p viewer-application --test video_preview_service preview_does_not_enter_seeking_and_only_matching_commit_completes -- --nocapture
cargo test -p viewer-desktop --test video_runtime_lifecycle high_frequency_publication_bypasses_the_transition_lane -- --nocapture
```

Expected: compile failures for the missing request types and synchronous publication methods.

- [ ] **Step 3: Implement the platform-neutral request contract**

In `viewer-application/src/video.rs`:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeekIntent {
    Preview,
    Commit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeekRequest {
    pub request_id: u64,
    pub time_us: u64,
    pub intent: SeekIntent,
}
```

Change `VideoCommandKind::Seek` to carry `SeekRequest`. Track
`active_commit_request_id: Option<u64>` in service state. `preview_seek` validates generation/state, clamps the time, forces `intent=Preview`, and calls `engine.publish_seek` without taking the application lane. Commit execution records the request ID, enters `Seeking`, and publishes the request. `SeekCompleted` changes state only when its request ID equals `active_commit_request_id`.

In `ports.rs` replace the async seek method with:

```rust
fn publish_seek(&self, generation: u64, request: SeekRequest)
    -> Result<(), VideoEngineError>;

fn publish_surface_rect(
    &self,
    generation: u64,
    sequence: u64,
    rect: SurfaceRect,
) -> Result<(), VideoEngineError>;
```

Update `FakeVideoEngine` to record both calls synchronously and update every explicit backend/mock implementation so no default method can ignore request identity.

- [ ] **Step 4: Implement strict IPC and runtime routing**

Use a snake-case enum on the Rust wire and camel-case field names:

```rust
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VideoSeekIntentDto {
    Preview,
    Commit,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VideoSeekDto {
    pub generation: u64,
    pub request_id: u64,
    pub time_us: u64,
    pub intent: VideoSeekIntentDto,
}
```

`VideoRuntime::seek` routes preview through `preview.preview_seek` without taking `transition`; commit takes `transition` and calls `preview.execute`. `VideoRuntime::set_surface_rect` becomes synchronous publication after `ensure_generation` and does not call `execute`. Tauri handlers return immediately after publication.

- [ ] **Step 5: Run focused suites and verify GREEN**

```bash
cargo test -p viewer-application --test video_preview_service -- --nocapture
cargo test -p viewer-test-support video_engine -- --nocapture
cargo test -p viewer-desktop --test video_runtime_lifecycle -- --nocapture
cargo test -p viewer-desktop --test video_security_boundaries -- --nocapture
```

Expected: matching commit transitions pass, stale completions remain inert, and high-frequency publication is not blocked by the runtime transition lane.

- [ ] **Step 6: Commit Task 2**

```bash
git add crates/viewer-application crates/viewer-test-support src-tauri/src/dto/video.rs src-tauri/src/commands/video.rs src-tauri/src/video_runtime.rs tests/video_runtime_lifecycle.rs
git commit -m "feat: bind video seeks to interaction requests"
```

---

### Task 3: Make the React timeline emit latest previews and one exact commit

**Files:**
- Modify: `ui/src/api/types.ts`
- Modify: `ui/src/api/viewer.ts`
- Modify: `ui/src/api/viewer.test.ts`
- Modify: `ui/src/components/videoPreview/VideoTimeline.tsx`
- Modify: `ui/src/components/videoPreview/VideoTimeline.test.tsx`
- Modify: `ui/src/components/videoPreview/VideoControls.tsx`
- Modify: `ui/src/components/videoPreview/useVideoBridge.ts`
- Modify: `ui/src/components/videoPreview/useVideoBridge.test.tsx`

**Interfaces:**
- Consumes: Task 2 wire request `{ generation, requestId, timeUs, intent }`.
- Produces: `onPreviewSeek(timeUs)` during drag, `onCommitSeek(timeUs)` once on release/keyboard, and generation-local monotonically increasing request IDs.

- [ ] **Step 1: Replace the existing timeline test with a failing two-phase behavior test**

```tsx
it('previews only the latest drag position per frame and commits exactly once on release', () => {
  let frame: FrameRequestCallback | null = null
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
    frame = callback
    return 19
  })
  const onPreviewSeek = vi.fn().mockResolvedValue(undefined)
  const onCommitSeek = vi.fn().mockResolvedValue(undefined)
  render(
    <TimelineHarness
      onPreviewSeek={onPreviewSeek}
      onCommitSeek={onCommitSeek}
      thumbnail={null}
    />,
  )

  const slider = screen.getByRole('slider', { name: '视频时间轴' })
  fireEvent.pointerDown(slider, { pointerId: 7, clientX: 20 })
  fireEvent.pointerMove(slider, { pointerId: 7, clientX: 40 })
  fireEvent.pointerMove(slider, { pointerId: 7, clientX: 60 })
  act(() => frame?.(0))

  expect(onPreviewSeek.mock.calls).toEqual([[3_000_000]])
  expect(onCommitSeek).not.toHaveBeenCalled()

  fireEvent.pointerUp(slider, { pointerId: 7, clientX: 60 })
  expect(onCommitSeek.mock.calls).toEqual([[3_000_000]])
  expect(onPreviewSeek).toHaveBeenCalledTimes(1)
})
```

Add keyboard coverage proving Arrow/Home/End invokes commit and never preview. Add generation rerender coverage proving the request counter and pending rAF are invalidated.

- [ ] **Step 2: Run the focused UI tests and verify RED**

```bash
pnpm --dir ui exec vitest run src/components/videoPreview/VideoTimeline.test.tsx src/components/videoPreview/useVideoBridge.test.tsx src/api/viewer.test.ts --reporter=verbose
```

Expected: compile/test failures because the component and bridge expose only `onSeek` and the request lacks intent/request ID.

- [ ] **Step 3: Implement gesture separation and typed bridge publication**

Change timeline props to:

```ts
onPreviewSeek(timeUs: number): Promise<void>
onCommitSeek(timeUs: number): Promise<void>
```

The rAF callback invokes only `onPreviewSeek`. Pointer release cancels the pending preview frame, updates local time, and calls `onCommitSeek` once. Keyboard changes call only commit.

Change the API request to:

```ts
export interface VideoSeekRequest extends VideoGenerationRequest {
  requestId: number
  timeUs: number
  intent: 'preview' | 'commit'
}

export interface VideoSurfaceRectRequest extends VideoGenerationRequest, VideoSurfaceRect {
  sequence: number
}
```

In `useVideoBridge`, keep `seekRequestId` and `surfaceSequence` refs. Reset both when generation changes. Publish preview and commit with increasing IDs; do not place preview calls on `commandQueue`. Keep local displayed time controlled by `VideoTimeline`, not native progress from stale requests.

- [ ] **Step 4: Run focused UI tests and verify GREEN**

```bash
pnpm --dir ui exec vitest run src/components/videoPreview/VideoTimeline.test.tsx src/components/videoPreview/useVideoBridge.test.tsx src/components/videoPreview/VideoControls.test.tsx src/api/viewer.test.ts --reporter=verbose
pnpm --dir ui exec tsc -b
```

Expected: one preview per animation frame, one commit per gesture, strict bridge payloads, and no stale state across generation changes.

- [ ] **Step 5: Commit Task 3**

```bash
git add ui/src/api ui/src/components/videoPreview
git commit -m "feat: make timeline scrubbing responsive"
```

---

### Task 4: Implement bounded native seek publication and request-bound completion

**Files:**
- Create: `crates/viewer-platform-macos/src/video/interaction.rs`
- Modify: `crates/viewer-platform-macos/src/video/mod.rs`
- Modify: `crates/viewer-platform-macos/src/video/adapter.rs`
- Modify: `crates/viewer-platform-macos/src/video/render_loop.rs`
- Test: `crates/viewer-platform-macos/src/video/interaction.rs`
- Test: `crates/viewer-platform-macos/src/video/adapter.rs`

**Interfaces:**
- Consumes: Task 1 `SeekMode`; Task 2 `SeekRequest` and synchronous `VideoEngine::publish_seek`.
- Produces: `LatestSeekMailbox`, one main-queue drain, preview replacement, commit priority, and request-bound exact completion.

- [ ] **Step 1: Write failing mailbox behavior tests**

```rust
#[test]
fn one_hundred_twenty_previews_keep_only_the_latest_request() {
    let mut mailbox = LatestSeekMailbox::default();
    for request_id in 1..=120 {
        mailbox.publish(7, SeekRequest {
            request_id,
            time_us: request_id * 10_000,
            intent: SeekIntent::Preview,
        });
    }

    assert_eq!(mailbox.take_for_drain().unwrap().request_id, 120);
    assert!(mailbox.take_for_drain().is_none());
}

#[test]
fn commit_replaces_preview_and_stale_generation_is_rejected() {
    let mut mailbox = LatestSeekMailbox::default();
    mailbox.activate(9);
    assert!(mailbox.publish(8, preview(1)).is_err());
    mailbox.publish(9, preview(2)).unwrap();
    mailbox.publish(9, commit(3)).unwrap();
    assert_eq!(mailbox.take_for_drain(), Some(commit(3)));
}
```

Add an adapter completion test proving a completion for request 10 cannot complete active request 11.

- [ ] **Step 2: Run focused Rust tests and verify RED**

```bash
cargo test -p viewer-platform-macos interaction::tests --lib -- --nocapture
cargo test -p viewer-platform-macos video::adapter::tests --lib -- --nocapture
```

Expected: compile failure because the mailbox and request-bound pending completion do not exist.

- [ ] **Step 3: Implement the pure latest-seek mailbox**

Use one active generation, one pending request, and one `drain_scheduled` flag. `publish` replaces pending work only when `(generation, request_id)` is current; `invalidate` clears pending work. `take_for_drain` returns one newest request and leaves scheduling ownership with the adapter until it calls `finish_drain`.

The implementation must satisfy this state shape:

```rust
#[derive(Default)]
pub(super) struct LatestSeekMailbox {
    active_generation: Option<u64>,
    latest_request_id: u64,
    pending: Option<SeekRequest>,
    drain_scheduled: bool,
}
```

- [ ] **Step 4: Wire the mailbox into the macOS adapter**

`publish_seek` stores the latest request and schedules `DispatchQueue::main().exec_async` only when the mailbox grants scheduling ownership. The main drain:

1. takes the newest request;
2. validates the adapter's active generation again;
3. maps preview to `SeekMode::PreviewKeyframe` and commit to `SeekMode::CommitExact`;
4. calls `MacVideoRenderSession::seek`;
5. stores `PendingCompletion::Seek { request_id, target_time_us }` only for commits;
6. schedules one further drain only if a newer request arrived.

`draw_on_main` publishes commit completion only when the pending request is still current and playback time is within the literal `COMMIT_COMPLETION_TOLERANCE_US = 50_000` of its target. Superseded preview frames publish normal progress but cannot complete a commit.

- [ ] **Step 5: Run platform/application regression tests and verify GREEN**

```bash
cargo test -p viewer-platform-macos video:: --lib -- --nocapture
cargo test -p viewer-application --test video_preview_service -- --nocapture
cargo test -p viewer-desktop --test video_runtime_lifecycle -- --nocapture
```

Expected: bounded mailbox, exact request ownership, step completion, EOF, and existing first-frame behavior all pass.

- [ ] **Step 6: Commit Task 4**

```bash
git add crates/viewer-platform-macos crates/viewer-application src-tauri/src/video_runtime.rs tests/video_runtime_lifecycle.rs
git commit -m "feat: coalesce native video seek requests"
```

---

### Task 5: Implement bounded geometry publication and retained-frame redraw

**Files:**
- Modify: `crates/viewer-platform-macos/src/video/interaction.rs`
- Modify: `crates/viewer-platform-macos/src/video/adapter.rs`
- Modify: `crates/viewer-platform-macos/src/video/render_loop.rs`
- Modify: `crates/viewer-video-mpv/src/render.rs`
- Modify: `src-tauri/src/video_runtime.rs`
- Modify: `ui/src/components/videoPreview/useVideoBridge.ts`
- Test: `crates/viewer-platform-macos/src/video/interaction.rs`
- Test: `crates/viewer-video-mpv/src/render.rs`
- Test: `ui/src/components/videoPreview/useVideoBridge.test.tsx`

**Interfaces:**
- Consumes: Task 2 `publish_surface_rect(generation, sequence, rect)` and Task 3 geometry sequence.
- Produces: `LatestGeometryMailbox`, one main-queue geometry drain, and unconditional retained-frame redraw after valid geometry application.

- [ ] **Step 1: Write failing bounded-geometry and redraw tests**

```rust
#[test]
fn resize_burst_keeps_one_pending_rect_and_one_drain_owner() {
    let mut mailbox = LatestGeometryMailbox::default();
    mailbox.activate(5);
    let mut schedules = 0;
    for sequence in 1..=120 {
        if mailbox.publish(5, geometry(sequence)).unwrap() {
            schedules += 1;
        }
    }

    assert_eq!(schedules, 1);
    assert_eq!(mailbox.take_for_drain().unwrap().sequence, 120);
}
```

Extend the existing fake render API test so a geometry redraw calls `render` even when `update()` returns no new-frame flag, while it does not mark a second first frame or increment decoded-frame completion.

Add a hook test that resolves an older `videoSetSurfaceRect` promise after a newer measurement and verifies only the newest sequence is accepted by the bridge/native fake.

- [ ] **Step 2: Run focused tests and verify RED**

```bash
cargo test -p viewer-platform-macos interaction::tests::resize_burst_keeps_one_pending_rect_and_one_drain_owner --lib -- --nocapture
cargo test -p viewer-video-mpv render::tests --lib -- --nocapture
pnpm --dir ui exec vitest run src/components/videoPreview/useVideoBridge.test.tsx -t "publishes only the newest surface geometry" --reporter=verbose
```

Expected: missing mailbox/redraw API and old geometry command behavior fail.

- [ ] **Step 3: Implement the geometry mailbox and main-queue drain**

Use this bounded state:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct GeometryUpdate {
    pub generation: u64,
    pub sequence: u64,
    pub rect: SurfaceRect,
}

#[derive(Default)]
pub(super) struct LatestGeometryMailbox {
    active_generation: Option<u64>,
    latest_sequence: u64,
    pending: Option<GeometryUpdate>,
    drain_scheduled: bool,
}
```

The adapter publication method validates generation synchronously, overwrites pending geometry, and schedules one main-queue drain. The drain applies the newest rectangle, then yields and schedules once more only if a newer rectangle appeared. `close_on_main` invalidates both interaction mailboxes before dropping the session.

- [ ] **Step 4: Redraw the current decoded picture after geometry application**

Extract a render helper in `MacVideoRenderSession` that can draw the current libmpv frame without requiring `MpvRenderContext::update()` to return `MPV_RENDER_UPDATE_FRAME`. `update_geometry_and_redraw` must:

```rust
surface.update_geometry(rect)?;
open_gl_context.update(main_thread_marker);
if first_frame_ready {
    render_current_frame_without_completion()?;
}
```

The redraw flushes the OpenGL buffer and reports the swap, but it does not emit first-frame, progress, seek completion, or ended events and does not count as a newly decoded frame.

- [ ] **Step 5: Run focused geometry/UI suites and verify GREEN**

```bash
cargo test -p viewer-video-mpv render::tests --lib -- --nocapture
cargo test -p viewer-platform-macos video:: --lib -- --nocapture
cargo test -p viewer-platform-macos --test video_surface_geometry -- --nocapture
pnpm --dir ui exec vitest run src/components/videoPreview/useVideoBridge.test.tsx src/components/videoPreview/videoGeometry.test.ts --reporter=verbose
```

Expected: bounded scheduling, stale generation rejection, retained paused/ended redraw, and current geometry tests all pass.

- [ ] **Step 6: Commit Task 5**

```bash
git add crates/viewer-platform-macos crates/viewer-video-mpv/src/render.rs src-tauri/src/video_runtime.rs ui/src/components/videoPreview
git commit -m "feat: coalesce native video surface updates"
```

---

### Task 6: Add interaction diagnostics, native acceptance, and final verification

**Files:**
- Modify: `crates/viewer-platform-macos/src/video/diagnostics.rs`
- Modify: `crates/viewer-platform-macos/src/video/adapter.rs`
- Modify: `src-tauri/src/video_feasibility.rs`
- Modify: `ui/src/acceptance/scenes/videoFeasibilityScene.tsx`
- Modify: `ui/src/acceptance/scenes/videoFeasibilityScene.test.tsx`
- Modify: `scripts/video/render-feasibility.mjs`
- Modify: `scripts/video/render-feasibility-assertions.test.mjs`
- Modify: `docs/reviews/2026-08-10-video-render-feasibility.md`

**Interfaces:**
- Consumes: Task 4/5 adapter counters and request identity.
- Produces: path-safe interaction metrics and fresh native evidence for resize and scrub behavior.

- [ ] **Step 1: Write failing structured diagnostics tests**

Add atomic counters/timestamps with no source path:

```rust
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct VideoInteractionDiagnostics {
    pub geometry_published: u64,
    pub geometry_applied: u64,
    pub geometry_replaced: u64,
    pub preview_published: u64,
    pub preview_issued: u64,
    pub commit_issued: u64,
    pub stale_completions_rejected: u64,
    pub latest_geometry_sequence: u64,
    pub latest_seek_request_id: u64,
}
```

Write a Rust test that publishes 120 geometry values and 120 previews, drains them, and hand-checks `published=120`, `applied<120`, `replaced>0`, `commit_issued=1`, and latest identities `120`/`121`.

Write a Node assertion fixture that rejects evidence when geometry/seek backlog is nonzero or more than one exact commit is recorded for a gesture.

- [ ] **Step 2: Run diagnostics tests and verify RED**

```bash
cargo test -p viewer-platform-macos video::diagnostics::tests::interaction_metrics_report_bounded_work --lib -- --nocapture
node --test scripts/video/render-feasibility-assertions.test.mjs
```

Expected: missing diagnostic fields and acceptance schema assertions.

- [ ] **Step 3: Implement path-safe counters and the focused acceptance scene**

Increment counters at publication, replacement, AppKit application, libmpv issue, and stale completion rejection. Expose them only through the existing `video-feasibility` feature route.

Extend the native acceptance scene with two deterministic actions:

- resize the active stage through 30 alternating rectangles while playing, pause and resize again, seek to end and resize the retained ending frame;
- perform one drag gesture with at least 30 preview positions and one release target.

The runner records generation, latest sequence/request ID, publication/application counts, commit count, final fitted rectangle, and final exact playback time. It fails if work remains pending, geometry trails the final rectangle, a stale request completes, or exact commits differ from one.

- [ ] **Step 4: Run the focused native acceptance once**

Preflight the reviewed runtime and fixture, then run the repository's existing development feasibility command once with network disabled. Do not rebuild the media runtime.

```bash
VIEWER_VIDEO_RUNTIME_DIR="$PWD/target/task6-review-resources/ViewerVideoRuntime" \
  scripts/video/generate-test-fixtures.sh --verify
scripts/video/run-clean-machine-smoke.sh --mode development \
  target/debug/bundle/macos/Viewer.app
```

Expected: playing/paused/ended resize rows pass; scrub preview is bounded; one exact commit reaches the target; no residual Viewer or helper process remains.

- [ ] **Step 5: Run the complete verification gate**

```bash
pnpm --dir ui check
pnpm --dir ui build
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
pnpm verify
git diff --check
```

Expected: UI type/style/build, strict Clippy, workspace tests, security boundaries, dependency/license policy, and repository verification all exit 0.

- [ ] **Step 6: Update evidence and commit Task 6**

Document the exact native run ID, fixture hash, resize counts, preview/commit counts, final rectangle/time, and verification outputs. Do not promote unmeasured 4K or AV1 performance.

```bash
git add crates/viewer-platform-macos/src/video/diagnostics.rs crates/viewer-platform-macos/src/video/adapter.rs src-tauri/src/video_feasibility.rs ui/src/acceptance/scenes scripts/video docs/reviews/2026-08-10-video-render-feasibility.md
git commit -m "test: verify responsive video interactions"
```

---

## Plan Self-review

- Spec coverage: Tasks 1–4 cover two-phase request-bound seek; Task 5 covers bounded geometry and retained-frame redraw; Task 6 covers structured measurements, native interaction evidence, and full verification.
- Scope: no player replacement, codec work, visual redesign, streaming, or dependency addition.
- Type consistency: `SeekIntent`/`VideoSeekIntentDto`/TypeScript intent use `Preview|Commit`, `preview|commit`, and request IDs/sequences remain `u64`/safe JavaScript integers generated from zero per generation.
- Mutation checks: tests fail for wrong seek flag, FIFO replacement, missing generation/request guard, multiple commit calls, geometry redraw gated only by new-frame update, lifecycle-lane blocking, and stale completion publication.
- Placeholder scan: the plan contains no deferred implementation decisions; unmeasured 4K/AV1 performance is explicitly outside the acceptance claim.
