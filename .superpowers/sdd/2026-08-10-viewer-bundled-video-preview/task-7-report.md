# Task 7 report — generation-safe video services

Worktree: `/Users/abc/Project/Viewer/.worktrees/video-preview`
Branch: `codex/video-preview`
Base: `a2c305a55611a3b1789dadb2050acd379d3b83b2`
Required commit: `feat: add generation-safe video services`

## Scope and decisions

Task 7 adds only the platform-neutral playback/session layer. The application
owns one serialized lane for opens, commands, engine events, navigation close,
and final close. Every externally reusable command/event carries a monotonic
generation; stale commands fail before an engine call and stale events are
discarded before reveal or any state mutation. An open closes the active
generation first and resets position, duration, volume, mute, and rate.

The exact `VideoEngine` capability port contains no mpv, AppKit, Win32, Tauri,
or UI type. `PlaybackRate` is a closed enum with only the six approved values.
The test-support fake implements the whole port, records ordered typed calls,
returns generation-scoped events, and supports deterministic one-shot engine
failure injection for future Windows adapter contract tests.

The timeline request coordinator is separate from Task 6 extraction/cache
work. It keeps only the newest pending hover request for the active
`(VideoSessionId, generation)` and publishes only when session, generation,
request ID, and bucket all match. Navigation and close invalidate pending
results.

No real adapter, playback runtime, Tauri bridge, UI, or Task 8+ code was added.

## Files

- Modified `crates/viewer-application/src/lib.rs`.
- Modified `crates/viewer-application/src/ports.rs`.
- Created `crates/viewer-application/src/video.rs`.
- Created `crates/viewer-application/src/video_thumbnail.rs`.
- Created `crates/viewer-application/tests/video_preview_service.rs`.
- Created `crates/viewer-application/tests/video_thumbnail_service.rs`.
- Modified `crates/viewer-test-support/src/lib.rs`.
- Created `crates/viewer-test-support/src/video_engine.rs`.

## RED / GREEN evidence

1. Initial session state machine:
   - RED: `cargo test -p viewer-application --test video_preview_service`
     exited 101 because `VideoEngine`, engine types, playback state, and
     `VideoPreviewService` did not exist.
   - GREEN: the same command passed 2/2 for first reveal ordering/exactly once
     and stale first-frame rejection after navigation.
2. Lifecycle, frame-step, and navigation:
   - RED: the same preview command exited 101 because `close`, end/time events,
     generation-scoped commands, frame stepping, and video-only neighbors did
     not exist.
   - GREEN: the suite passed 8/8 after close-idempotence, final-frame end,
     preference reset, Pause-before-Step, stale-command, and video-only
     navigation behavior was implemented.
3. Seek completion and normalized engine failure:
   - RED: the preview suite exited 101 because `SeekCompleted` and `Failed`
     engine events did not exist.
   - GREEN: the suite passed 10/10. Seek targets clamp to duration and return
     to the pre-seek playing/paused state; a normalized failure blocks a late
     first-frame reveal.
   - Self-review RED: the focused late-progress regression failed because a
     same-generation `TimeChanged` arriving after `Ended` moved snapshot time
     from 2,000,000 back to 1,250,000 microseconds.
   - Self-review GREEN: terminal `Ended`/`Failed` states now discard progress;
     the exact regression passes 1/1 and the preview suite contains 11 tests.
4. Timeline request generations:
   - RED: `cargo test -p viewer-application --test video_thumbnail_service`
     exited 101 because request/result/service types did not exist.
   - GREEN: the same command passed 4/4 for newest-only pending replacement,
     all four publication dimensions, navigation invalidation, and close.
5. Reusable fake adapter:
   - RED: `cargo test -p viewer-test-support video_engine` exited 101 because
     `FakeVideoEngine`, its call model, and `VideoEvent` did not exist.
   - GREEN: the same command passed 2/2 for the full port call contract,
     generation-scoped events, and one-shot failures.

## Focused verification before review

- `cargo test -p viewer-application --test video_preview_service --test
  video_thumbnail_service`: 14/14 preview and 4/4 thumbnail tests pass after
  the self-review and review corrections.
- `cargo test -p viewer-test-support video_engine`: 2/2 pass.
- Brief command `cargo test -p viewer-application video_ && cargo test -p
  viewer-test-support`: exit 0; the name filter selects the three pre-existing
  scheduler video tests plus `navigation_is_video_only`, and test-support
  passes 6/6. The two new integration binaries are therefore also run directly
  above instead of relying on the name filter.
- Strict Clippy RED first found derivable/collapsible implementations and the
  test-only manual no-op waker (then its needless borrow). Each was corrected
  mechanically. Final `cargo clippy -p viewer-application -p
  viewer-test-support --all-targets -- -D warnings`: exit 0.
- `git diff --check`: exit 0.

## Root causes and safety boundaries

- The pre-Task-7 application had no playback port or serialized ownership
  boundary, so a callback could not be tied to the active video. The service
  now checks generation and active state inside the same async lane used for
  engine side effects.
- Playback-to-frame-step must pause before requesting a logical frame. The
  state transition and ordered fake calls prove `Pause` precedes `Step`.
- Hover request identity cannot be inferred from only a timestamp. Publication
  requires the complete session/generation/request/bucket tuple so an old
  result cannot occupy a current placeholder.
- `close` always enters `Closing`, awaits the engine close, and then clears the
  active session even when the engine close returns an error. Duplicate close
  and all late events leave `Idle`.

## Review and final verification

The single fresh reviewer reported 0 Critical, 2 Important, and 0 Minor:

1. `Ready` admitted seek/frame-step before the first-frame gate, allowing the
   completion event to leave the session paused while the surface remained
   permanently hidden.
2. Replaying a generation-scoped `Close` after its first successful close
   returned `NoActiveSession` instead of succeeding idempotently.

Review-fix RED:

- `cargo test -p viewer-application --test video_preview_service
  before_first_frame -- --nocapture` failed 2/2: both seek and step returned
  `Ok(())` instead of `InvalidState`.
- `cargo test -p viewer-application --test video_preview_service
  generation_scoped_close_is_idempotent_but_stale_close_is_rejected --
  --exact --nocapture` failed because the duplicate close returned
  `NoActiveSession`.

Review-fix GREEN:

- The same pre-first-frame filter passed 2/2 after seek/step were restricted
  to post-reveal states; each regression then delivered `FirstFrameReady` and
  proved the normal RevealSurface→Play sequence remained reachable.
- The exact close regression passed 1/1 after same-generation idle close was
  treated as success without a second engine call. A close from an older
  generation still returns `StaleGeneration` after a new open.

The same reviewer re-read the complete fixed staged diff and reported 0
Critical, 0 Important, and 0 Minor. Both findings have exact regression
coverage and no new issue was found; verdict: Ready to merge.

The one and only final `pnpm verify` run exited 0. Evidence includes:

- repository policy 29/29 and clean-verifier tests 11/11;
- UI check, 92 test files / 851 passed + 1 skipped, and production build;
- locked workspace formatting, strict Clippy, and all workspace Rust tests,
  including Task 7 preview 14/14 and thumbnail 4/4;
- Tauri security boundaries 8/8, offline locked cargo-deny bans/licenses/
  sources, and the reviewed npm-license policy.

The duplicate dependency listings printed by cargo-deny are the repository's
existing allowed warnings; the policy command concluded `bans ok, licenses
ok, sources ok`.

## Concerns

- Task 7 intentionally does not authorize canonical-path/entity validation;
  the future bridge/composition boundary must supply only already validated
  `VideoSource` values.
- Thumbnail request orchestration here does not duplicate Task 6 bucket
  quantization or extraction. Its responsibility is newest-request identity
  and publication safety only.
- No real macOS or Windows adapter is in scope; the fake preserves the neutral
  contract that both implementations can exercise.
