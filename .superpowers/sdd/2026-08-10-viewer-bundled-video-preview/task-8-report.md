# Task 8 report — typed native video bridge

Worktree: `/Users/abc/Project/Viewer/.worktrees/video-preview`
Branch: `codex/video-preview`
Base: `776e10192fa950c1992a0ddc0bf71c8b2bb31a57`
Required commit: `feat: bridge native video playback`

## Scope and decisions

Task 8 connects the Task 7 generation-safe playback service to the proven
Task 3 macOS video surface and Task 2 safe libmpv wrapper. It adds a managed
native runtime, fourteen explicit typed Tauri commands, one tagged
`viewer://video-event` channel, canonical entity authorization, verified
thumbnail-cache commands, and a single close path shared by preview, project,
window, and application shutdown.

The bridge never accepts a frontend filesystem path. `video_open` accepts an
entity ID and resolves it only against the active indexed session. Resolution
requires the indexed kind to be Video, the current path to remain a regular
non-symlink file with the indexed identity, and its canonical path to remain
inside the canonical project root. The resulting canonical path is kept on the
native side.

All command DTOs deny unknown fields. Every reusable playback mutation is
generation-scoped, frame step and playback rate are closed enums, and the
fullscreen generation check occurs before any Viewer window mutation.
`Progress` alone is coalesced to at most one event per 100 ms; its newest
pending value is flushed before `Ended`, `Failed`, or `Closed`. Engine events
enter one serialized consumer so terminal ordering cannot race progress.

Task 9 UI, video browsing, navigation controls, and frontend state are not in
scope and were not added.

## Native adapter and lifecycle

`MacOsLibmpvAdapter` owns the mpv client, render session, surface, active
generation, event sink, and diagnostics behind a serialized main-dispatch
boundary. Surface preparation uses the bundled runtime layout and the proven
AppKit/OpenGL surface. First-frame reveal is still generation-gated, and EOF is
read through the typed safe wrapper's exact `eof-reached` accessor.

Native teardown is ordered as follows:

1. hide the video surface;
2. pause playback and mute audio;
3. drop the render context, unregistering render callbacks;
4. drop the mpv client;
5. clear the OpenGL context;
6. unmount the native surface;
7. revoke only the active session's generation-tagged video artifacts.

Project close removes the active session from command reach and invalidates its
coordinator before awaiting the same idempotent video close. It does not revoke
the general image-artifact registry or close the project session until video
teardown completes. A native close failure remains a committed terminal close:
the runtime resets its session and revokes video thumbnail artifacts rather
than leaving a half-active generation.

Opening a different video first closes the active native generation, then
prepares and opens the replacement surface. Window close request, app exit,
final exit, project close, and explicit preview close all converge on the same
runtime close operation.

## Files

- Modified `Cargo.lock`.
- Modified `crates/viewer-infrastructure/src/image_cache.rs`.
- Modified `crates/viewer-platform-macos/Cargo.toml`.
- Created `crates/viewer-platform-macos/src/video/adapter.rs`.
- Modified `crates/viewer-platform-macos/src/video/diagnostics.rs`.
- Modified `crates/viewer-platform-macos/src/video/mod.rs`.
- Modified `crates/viewer-platform-macos/src/video/render_loop.rs`.
- Modified `crates/viewer-video-mpv/src/client.rs`.
- Modified `crates/viewer-video-mpv/tests/client_contract.rs`.
- Modified `src-tauri/Cargo.toml`.
- Modified `src-tauri/src/commands/mod.rs`.
- Created `src-tauri/src/commands/video.rs`.
- Modified `src-tauri/src/dto/mod.rs`.
- Created `src-tauri/src/dto/video.rs`.
- Modified `src-tauri/src/error.rs`.
- Modified `src-tauri/src/lib.rs`.
- Modified `src-tauri/src/state/mod.rs`.
- Modified `src-tauri/src/state/preview.rs`.
- Modified `src-tauri/src/state/session.rs`.
- Created `src-tauri/src/video_events.rs`.
- Created `src-tauri/src/video_runtime.rs`.
- Created `tests/video_runtime_lifecycle.rs`.
- Created `tests/video_security_boundaries.rs`.

## RED / GREEN evidence

1. Security and project-close ordering:
   - RED: `cargo test -p viewer-desktop --test video_security_boundaries
     --test video_runtime_lifecycle` exited 101 because the canonical video
     resolver and video lifecycle hook did not exist.
   - GREEN: the first focused slice passed 3/3 after active-session entity
     authorization and close-before-artifact/session ordering were added.
2. Tagged events and progress coalescing:
   - RED: the lifecycle test binary exited 101 because the event DTO and
     coalescer did not exist.
   - GREEN: the focused lifecycle slice passed 3/3 with the camelCase tagged
     contract and terminal flush ordering.
3. Typed runtime and generation safety:
   - RED: the `typed_` lifecycle filter exited 101 because `VideoRuntime` and
     the typed command boundary did not exist.
   - GREEN: the filter passed 2/2 after stale commands were rejected before
     engine mutation and strict DTO parsing was added.
4. Fullscreen and cache boundaries:
   - Deliberately removing the fullscreen generation helper produced a compile
     RED; restoring the generation-first window gate passed its exact test
     1/1.
   - Verified cache stats/clear passed through the managed `VideoCache`, not an
     arbitrary path supplied by the frontend.
5. Latest-only timeline thumbnails:
   - RED: the focused test exited 101 because the thumbnail bridge port and
     request types did not exist.
   - GREEN: it passed 1/1 after replacement cancelled old work and publication
     required the active generation/request.
6. Safe EOF access:
   - RED: `cargo test -p viewer-video-mpv --test client_contract` exited 101
     because the typed `eof_reached` accessor did not exist.
   - GREEN: the exact wrapper contract passed 1/1 after adding only that fixed
     property accessor.
7. Close-error cleanup:
   - Self-review identified that an engine close error could skip runtime
     reset and thumbnail revocation.
   - GREEN: the exact lifecycle regression now proves cleanup occurs while the
     normalized close error is still returned.

## Focused verification before review

- `cargo test -p viewer-video-mpv --test client_contract`: 1/1 pass.
- `cargo test -p viewer-platform-macos video`: 4/4 pass.
- `cargo test -p viewer-desktop --test video_security_boundaries --test
  video_runtime_lifecycle`: security 2/2 and lifecycle 15/15 pass after the
  review corrections.
- The earlier Task 8 regression boundary `cargo test -p viewer-desktop
  --tests` exited 0: desktop unit tests and every desktop integration binary
  passed. Later EOF/cleanup changes are covered by the fresher focused commands
  above.
- `cargo clippy -p viewer-video-mpv -p viewer-platform-macos -p
  viewer-infrastructure -p viewer-desktop --all-targets -- -D warnings`:
  exit 0.
- `cargo fmt --all -- --check`: exit 0.
- `git diff --check`: exit 0.

## Review corrections

The single fresh reviewer froze its initial result at 0 Critical, 3 Important,
and 0 Minor:

1. `video_open` did not hold a project/session lease or a complete runtime
   transition guard, so project close or another open could interleave between
   authorization, surface preparation, engine open, and event publication.
2. Fullscreen held only a one-shot generation check; close/navigation could
   change generation between window mutation, confirmed state, and event
   publication.
3. The Task 6 thumbnail service's playback watch sender was retained but never
   updated, so extraction did not yield while native playback was active.

Review-fix RED / GREEN evidence:

- Open serialization RED: `cargo test -p viewer-desktop --test
  video_runtime_lifecycle video_open_ -- --nocapture` exited 101 because neither
  `video_open_project_lease` nor `replace_authorized` existed. GREEN: the same
  command passed 2/2. A project read lease now covers entity resolution through
  publication, project close takes the write side before teardown, and one
  runtime transition mutex linearizes close/prepare/open/publish across
  concurrent replacements.
- Fullscreen RED: the exact deterministic concurrency regression exited 101
  because `set_fullscreen_guarded` did not exist. GREEN: the fullscreen filter
  passed 2/2. The generation gate, window mutation, confirmed-state read, and
  event publication now share the runtime transition guard; close cannot
  change generation midway through that sequence.
- Playback activity RED: the exact state-transition regression exited 101
  because `PlaybackActivityPort` did not exist. GREEN: it passed 1/1. The
  native thumbnail bridge now drives its Task 6 watch sender from generation-
  scoped Task 7 state: first-frame/play mark active, and pause/end/fail/close
  mark inactive. Stale events cannot update the watch.

Fix-round-1 review closed the open/fullscreen findings but retained one
Important subcase in playback activity: an engine failure during a command
changed the Task 7 snapshot to `Failed`, while the bridge's early `?` skipped
the activity update. The exact focused regression first exited 101 because the
bridge exposed no playback-state observation. It then passed 1/1 after command
execution was refactored to snapshot and publish activity on both `Ok` and
`Err`, before returning the original error. The same regression proves a stale
generation command cannot mutate the watch.

After these fixes, the complete Task 8 focused set passes: mpv 1/1, macOS video
4/4, lifecycle 15/15, and security 2/2. Fresh strict Clippy, format check, and
diff check all exit 0.

The same single reviewer re-read the scoped fix diff and closed every original
finding. Final open counts are 0 Critical, 0 Important, and 0 Minor; verdict:
APPROVE / Task 8 ready.

## Final verification

The one and only final `pnpm verify` run exited 0. Evidence includes:

- repository policy 29/29 and clean-verifier tests 11/11;
- UI check, 92 test files / 851 passed + 1 skipped, and production build;
- locked workspace formatting, strict Clippy, and all workspace Rust tests,
  including Task 8 lifecycle 15/15 and security 2/2;
- Tauri security boundaries 8/8;
- offline locked cargo-deny `bans ok, licenses ok, sources ok`;
- npm license policy: 140 packages across 11 reviewed expressions.

Cargo-deny's duplicate dependency listings are the repository's existing
allowed warnings; the policy command completed successfully.

## Safety boundaries and non-goals

- The renderer cannot be opened with an unindexed path or a path from a stale
  session, and a symlink replacement that escapes the project is rejected.
- The webview receives only DTO metadata and opaque `viewer-image://` artifact
  URLs; canonical media/cache paths remain native.
- No arbitrary mpv property getter or command-string API is exposed.
- Only generation-tagged video thumbnail artifacts are revoked by preview
  close; ordinary image preview artifacts remain owned by the existing image
  lifecycle.
- Task 8 does not add frontend controls, video browse participation, keyboard
  behavior, or Windows native playback.
