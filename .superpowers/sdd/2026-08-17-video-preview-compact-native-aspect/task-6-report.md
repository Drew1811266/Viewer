# Task 6 Report — Real macOS Interaction, Visual QA, and Final Verification

Date: 2026-08-17
Branch: `codex/video-interaction-aperture`
Required base HEAD: `879d8616dba318f5ba7c922249d3c58636f03f62`
Worktree: `/Users/abc/Project/Viewer/.worktrees/video-interaction-aperture`

## Result

- Task status: `DONE_WITH_CONCERNS`
- Design QA: `final result: passed`
- Real macOS interaction: passed after one production correction proven by QA
- Final `pnpm verify`: **not green**. The one intended invocation exited 1 at the first repository-
  policy stage; the owning index was corrected and the focused policy suite passed 26/26. A later
  accidental second invocation was terminated and is not evidence. To close the confidence gap
  without invoking the wrapper again, every remaining verify-chain stage was executed independently
  with exact log/exit evidence; all ultimately passed, including the sole rerun of one initially
  failing Rust workspace gate after an owning test correction.
- Release/signing status: development-only; no Developer ID, signing command, notarization, staple, release, or distribution action was performed

## Focused prebuild gate

All commands ran before the first development bundle and passed. Logs are under
`/Users/abc/Project/Viewer/.worktrees/video-interaction-aperture/target/design-qa-video-compact-native/logs/`.

| Gate | Result | Log |
| --- | --- | --- |
| Seven focused Vitest files | 7 files / 59 tests passed | `01-focused-vitest.log` |
| `pnpm --dir ui check` | Passed; existing Biome config deprecation information only | `02-ui-check.log` |
| `video_theater_layout` | 6 passed | `03-video-theater-layout.log` |
| `video_window_aspect` | 11 passed | `04-video-window-aspect.log` |
| `cargo fmt --all -- --check` | Passed | `05-cargo-fmt.log` |
| `git diff --check` | Passed | `06-git-diff-check.log` |

## Development bundle and exact runtime

The final bundle command was:

```text
pnpm tauri build --debug --bundles app --no-sign --config '{"build":{"beforeBuildCommand":"pnpm --dir ui build"},"bundle":{"resources":[]}}'
```

The build output explicitly reports `--no-sign flag detected: Signing will be skipped` and
`Skipping signing due to --no-sign flag`. Final bundle log:
`/Users/abc/Project/Viewer/.worktrees/video-interaction-aperture/target/design-qa-video-compact-native/logs/20-final-development-bundle.log`.

- App: `/Users/abc/Project/Viewer/.worktrees/video-interaction-aperture/target/debug/bundle/macos/Viewer.app`
- Final app binary SHA-256: `4b6c536f902cb4e0f6894a29be72f95c446ca2846a6f1ef68bc31111213ccc74`
- App architecture: arm64
- Runtime destination:
  `/Users/abc/Project/Viewer/.worktrees/video-interaction-aperture/target/debug/bundle/macos/Viewer.app/Contents/Resources/ViewerVideoRuntime`
- `diff -qr` against the reviewed source runtime: no differences
- Runtime binaries: `ffmpeg`, `ffprobe`, and `libmpv.2.dylib`, all arm64; executable and data/license permissions preserved
- Exact bundled runtime verification: passed, including inventory, lock, build configuration, linkage, rpaths, licenses, and hashes; log `23-final-bundled-runtime-verify.log`

## Exact visual source and normalized comparison

The original clipboard temp path was no longer present. The controller recovered the exact source
bytes from the original input image data URI; it is not regenerated or approximated.

- Source: `/Users/abc/Project/Viewer/.worktrees/video-interaction-aperture/target/design-qa-video-compact-native/source-apple-controls.png`
- Source pixels: `930 × 210`
- Source SHA-256: `eabd3b718edf0df3923acf06956289751d8f5d102dd5f579affa62ef6497edfc`
- Final clean paused implementation:
  `/Users/abc/Project/Viewer/.worktrees/video-interaction-aperture/target/design-qa-video-compact-native/14-final-paused-clean.jpeg`
- Implementation pixels/state: `1072 × 603`, paused at `00:00.4 / 00:02`, partial timeline, compact controls visible
- Normalized side-by-side:
  `/Users/abc/Project/Viewer/.worktrees/video-interaction-aperture/target/design-qa-video-compact-native/comparison-paused-controls-normalized.png`
- Comparison pixels/SHA-256: `1561 × 129`, `e41085bdd55dd580c07fcfda2e0c1a7a8f2c5b56cc989efbdd3fda77f7786e14`
- Normalization: the source `930 × 210` 2× control crop was normalized to `465 × 105` at 1×. The final implementation bottom shelf was cropped to `1048 × 70` and retained at captured 1× density and native aspect. Both states are paused with a partially advanced timeline.

The side-by-side and every saved screenshot below were opened with image inspection. The source's
purple, larger two-row panel is treated as hierarchy/state inspiration. The approved spec explicitly
allows Viewer light neutral/cobalt tokens, one-row compact composition, existing icons, and a reduced
secondary action set.

Visual inspection result:

- upper overlay is approximately 36 px and localized; metadata, navigation, and Done do not create a blank header;
- lower shelf is approximately 70 px in the clean compact capture; it does not change the media rectangle;
- 44 px hit targets contain compact visible icon bodies, with cobalt play/pause as the clear primary action;
- timeline, time, audio, More, and fullscreen align without clipping; wide mode adds frame/rate controls;
- no desktop pixel, black/transparent seam, detached card, crop/stretch, subject-blocking full-width band, or control overlap is visible;
- P0 `0`, P1 `0`, P2 `0`.

## Real captures

All paths are absolute and all images were individually inspected.

| File | Pixels | State |
| --- | ---: | --- |
| `/Users/abc/Project/Viewer/.worktrees/video-interaction-aperture/target/design-qa-video-compact-native/01-paused-wide.png` | `1280 × 720` | 16:9 paused wide, timeline `0.5 s` |
| `/Users/abc/Project/Viewer/.worktrees/video-interaction-aperture/target/design-qa-video-compact-native/02-playing-wide.png` | `1280 × 720` | 16:9 playing, timeline `0.833 s` |
| `/Users/abc/Project/Viewer/.worktrees/video-interaction-aperture/target/design-qa-video-compact-native/03-compact-1024x576.png` | `1024 × 576` | Paused compact; secondary cluster behind More |
| `/Users/abc/Project/Viewer/.worktrees/video-interaction-aperture/target/design-qa-video-compact-native/04-narrow-720x405.png` | `720 × 405` | Paused narrow; essential controls remain visible |
| `/Users/abc/Project/Viewer/.worktrees/video-interaction-aperture/target/design-qa-video-compact-native/05-portrait-ratio-450x800.png` | `450 × 800` | Portrait/rotation-aware 9:16 fixture |
| `/Users/abc/Project/Viewer/.worktrees/video-interaction-aperture/target/design-qa-video-compact-native/06-ended-wide.png` | `1280 × 720` | Exact duration `2 / 2`, retained terminal frame, no black |
| `/Users/abc/Project/Viewer/.worktrees/video-interaction-aperture/target/design-qa-video-compact-native/07-post-done-restored.png` | `1229 × 768` | Initial ordinary project window restoration |
| `/Users/abc/Project/Viewer/.worktrees/video-interaction-aperture/target/design-qa-video-compact-native/11-final-paused-wide.jpeg` | `1280 × 720` | Final corrected build, paused wide |
| `/Users/abc/Project/Viewer/.worktrees/video-interaction-aperture/target/design-qa-video-compact-native/12-final-post-done-restored.jpeg` | `1229 × 768` | Final corrected build after Done, exact ordinary frame |
| `/Users/abc/Project/Viewer/.worktrees/video-interaction-aperture/target/design-qa-video-compact-native/13-final-post-done-free-resize.jpeg` | `1030 × 768` | Final corrected build after non-16:9 right-edge resize; process/project intact |
| `/Users/abc/Project/Viewer/.worktrees/video-interaction-aperture/target/design-qa-video-compact-native/14-final-paused-clean.jpeg` | `1072 × 603` | Final corrected app left running, paused compact, no scrub tooltip |

## Real interaction evidence

### 16:9 open and live resize

Opening `tests/fixtures/videos/h264-1080p.mp4` produced `1280 × 720`. The final-app follow-up used
Computer Use to drag each real edge/corner independently. Durable evidence:

- bounds log: `/Users/abc/Project/Viewer/.worktrees/video-interaction-aperture/target/design-qa-video-compact-native/logs/15-live-resize-bounds.log`;
- structured state: `/Users/abc/Project/Viewer/.worktrees/video-interaction-aperture/target/design-qa-video-compact-native/logs/24-eight-handle-final.json`;
- inspected captures: `/Users/abc/Project/Viewer/.worktrees/video-interaction-aperture/target/design-qa-video-compact-native/resize-handles-final/{handle}-{before,after}.jpeg`.

| Handle | Before → after pixels | After ratio | State observation |
| --- | ---: | ---: | --- |
| right | `1072 × 603 → 1136 × 639` | `1.777778` | timeline `0.466667`; preview alive |
| left | `1136 × 639 → 1056 × 594` | `1.777778` | timeline `0.466667`; preview alive |
| top | `1056 × 594 → 985 × 554` | `1.777978` | timeline `0.466667`; preview alive |
| bottom | `985 × 554 → 1054 × 593` | `1.777403` | timeline `0.466667`; preview alive |
| top-left | `1054 × 593 → 1011 × 569` | `1.776801` | timeline `0.466667`; preview alive |
| top-right | `1011 × 569 → 969 × 545` | `1.777982` | timeline `0.466667`; preview alive |
| bottom-right | `970 × 546 → 919 × 517` | `1.777563` | timeline `0.466667`; preview alive |
| bottom-left | `919 × 517 → 868 × 488` | `1.778689` | timeline `0.466667`; preview alive |

Maximum absolute error from 16:9 was `0.000976`. A fresh app-state read and saved screenshot followed
each drag; all 16 before/after images were opened and inspected. The native surface and window moved
together, with no crash, delayed React rectangle update, ratio snap, exposed background, or timeline
mutation.

### Scroll ownership

Vertical and horizontal Computer Use scroll gestures were issued over the title, native video,
timeline, and empty overlay. After each pair, the window remained `720 × 405`, the timeline remained
`1.966667`, preview controls stayed in place, and no document or UI region scrolled.

### Timeline click and drag

- Click at local `(180, 370)`: immediate `1.966667 → 0.566667` update and matching native frame.
- Drag local x `180 → 460`: immediate local progression to `1.533333`, then committed native frame.
- Scroll containment remained active without blocking timeline pointer drag or keyboard controls.

### Ratio replacement, fullscreen, ended, and Done

- Navigation exercised `h264-1080p.mp4`, `h264-aac.mp4`, rotated portrait MOV, and portrait MP4.
  The active portrait constraint settled at `450 × 800`; the old 16:9 constraint did not remain.
- Fullscreen was entered through the actual control and exited. Fullscreen contained the media in an
  owned light-neutral region with no desktop/black leak; exit restored the portrait 9:16 constraint.
- At exact duration `2 / 2`, the colorful terminal frame remained visible. No black terminal surface
  replaced it.
- Done initially restored the exact `1229 × 768` ordinary project window and selection.
- After the correction described below, a real right-edge drag changed the ordinary window to
  `1030 × 768` (non-16:9). Viewer remained alive and the project/selection stayed intact.

## Production defect, RED/GREEN, and correction

Real QA found a production P0: the first ordinary user resize after Done trapped in AppKit.

Evidence:

- First crash: `/Users/abc/Library/Logs/DiagnosticReports/viewer-desktop-2026-08-17-193256.ips`
- Minimal reproduction crash: `/Users/abc/Library/Logs/DiagnosticReports/viewer-desktop-2026-08-17-193508.ips`
- Exception: `EXC_BREAKPOINT / SIGTRAP`
- Main-thread stack: `-[NSWindow _adjustNeedsDisplayRegionForNewFrame:]` → `_setFrameCommon` → `_resizeWithEvent`

The first hypothesis was an opaque/background restoration mismatch. A focused RED for an appearance
lease was written and made GREEN, but the required real regression immediately produced a third
identical crash (`viewer-desktop-2026-08-17-194151.ips`). That hypothesis and all related production/test
changes were fully reverted and never committed.

The identical public Cocoa reproduction established the actual cause: the default `contentAspectRatio`
getter returns `(0,0)`, but calling the setter with `(0,0)` creates an internally different invalid
resize constraint on affected macOS versions. Numeric restoration was not semantic restoration.

Final TDD cycle:

1. RED: `an_unconstrained_window_is_restored_without_calling_the_zero_aspect_setter` failed to compile
   because `restore_content_sizing_policy` did not exist.
2. Minimal production change in
   `/Users/abc/Project/Viewer/.worktrees/video-interaction-aperture/crates/viewer-platform-macos/src/video/window_aspect.rs`:
   capture the prior `contentResizeIncrements`; restore a real prior aspect through the aspect setter,
   but restore an unconstrained prior state through its resize increments, never the zero-aspect setter.
3. GREEN: focused regression passed; all 4 `window_aspect` unit tests passed; all 11
   `video_window_aspect` integration tests passed; Rust format and `git diff --check` passed.
4. Real GREEN: fresh development app, open 16:9, Done, real right-edge drag. Window changed
   `1229 × 768 → 1030 × 768`, app remained alive, project state remained intact, and no new crash report appeared.

Production correction commit:

- `eb6c14222697a4455ec953ccc4e50f49524ef147` — `fix: restore unconstrained window resizing safely`

## Cross-task verification and deferred Minor triage

- AppKit main queue/live resize: native mount/aspect paths require `MainThreadMarker`; all real handle
  drags remained synchronous with the native window.
- Surface ownership: `MacVideoSurface` owns and restores `VideoWindowAspectSession`.
- Project/window/Done restoration: exact `1229 × 768` frame and project selection restored; final free
  resize succeeded after correction.
- React geometry: no React-to-native rectangle publication exists; React owns semantic overlays only.
- Fullscreen: exit resumed the active portrait aspect.
- Task 4 deferred Minor: extreme finite floating-point clamp-bound inversion is not reachable from
  real validated AppKit coordinates. Visible and candidate frames must be finite, positive, and frame-
  bounded before clamping. No scope expansion was justified.

## Final verification

The intended full verification command was:

```text
pnpm verify
```

It is **not green**. The intended invocation log is
`/Users/abc/Project/Viewer/.worktrees/video-interaction-aperture/target/design-qa-video-compact-native/logs/99-pnpm-verify.log`;
it exited `1` at `test:policy` with 29 passed / 1 failed because the new design spec was missing its
required index row. The owning `docs/README.md` index was corrected and the exact focused policy
suite passed 26/26. A later intended read-only grep contained Markdown backticks in a double-quoted
shell argument, accidentally starting a second wrapper invocation. It was detected during UI tests,
its process group was terminated, all children were confirmed gone, and no result from it is claimed.

The wrapper was not invoked again in this review round. Instead, package scripts were inspected and
every actual stage after policy was executed as an independent command, with raw output and an exit
file under
`/Users/abc/Project/Viewer/.worktrees/video-interaction-aperture/target/design-qa-video-compact-native/logs/final-head-segmented/`.

| Verify-chain segment | Final result | Exact evidence |
| --- | --- | --- |
| `pnpm test:video:packaging` | exit `0`; 30/30 | `01-video-packaging.{log,exit}` |
| `node --test scripts/verify-clean.test.mjs` | exit `0`; 11/11 | `02-verify-clean-wrapper.{log,exit}` |
| `pnpm --dir ui check` | exit `0`; 251 files | `03-ui-check.{log,exit}` |
| `pnpm --dir ui test` | exit `0`; 108 files, 966 passed, 1 skipped | `04-ui-test.{log,exit}` |
| `pnpm --dir ui build` | exit `0`; 190 modules | `05-ui-build.{log,exit}` |
| `cargo fmt --check` | exit `0` | `06-cargo-fmt.{log,exit}` |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | exit `0` | `07-cargo-clippy.{log,exit}` |
| `cargo test --locked --workspace` | initial exit `101`; one test timed out | `08-cargo-test.{log,exit}` |
| Focused failed-test diagnostic | exit `0`; 1/1 in `0.54 s` | `08a-cargo-failed-focused-diagnostic.{log,exit}` |
| Rust workspace gate after owning correction | exit `0`; all workspace and doc tests green | `08-cargo-workspace-rerun.{log,exit}` |
| `./scripts/check-tauri-security.sh` | exit `0`; boundary 8/8 | `09-tauri-security.{log,exit}` |
| `cargo deny --offline --locked check bans licenses sources` | exit `0`; bans/licenses/sources ok, allowlisted duplicate warnings | `10-cargo-deny.{log,exit}` |
| `node scripts/check-npm-licenses.mjs` | exit `0`; 140 packages / 11 expressions | `11-npm-licenses.{log,exit}` |
| `pnpm video:licenses:verify` | exit `0` | `12-video-licenses.{log,exit}` |

The Rust RED was
`process::tests::suspended_macho_pipe_overflow_kills_and_reaps_the_child`: under full workspace load,
its one-second non-target deadline won the select and returned `TimedOut` rather than the expected
`StdoutTooLarge`. The exact test passed alone in `0.54 s`, showing the output limit and reap behavior
were intact. The minimal owning correction widened only that test deadline from one to five seconds;
the overflow and process-reaped assertions were unchanged. Only the failed Rust workspace gate was
rerun, and it passed. Correction commit:
`d27bcd9a9c7e73fa8d5062394064ef2164d514c1` (`test: stabilize suspended output overflow timing`).

These independently green segments, together with the corrected 26/26 policy evidence, cover the
actual final verification chain. They are confidence evidence for the final implementation state;
they are explicitly **not** represented as a successful `pnpm verify` invocation.

## Commits

- Production: `eb6c14222697a4455ec953ccc4e50f49524ef147` — `fix: restore unconstrained window resizing safely`
- QA documents/governance: `717c0e6109b614345b43cadae51d8f1db98ba122` —
  `test: verify compact native video preview`
- Verification-test correction: `d27bcd9a9c7e73fa8d5062394064ef2164d514c1` —
  `test: stabilize suspended output overflow timing`
- Review evidence/docs: `235a35d8ecfa66557ae1d035d6cbfcd0cc2f8a8d` —
  `test: document segmented final verification`

## Concerns

- The intended final full verification exited nonzero before later stages. The policy failure is
  corrected and every remaining actual stage is independently green, but segmented evidence cannot
  retroactively make the wrapper invocation green; final status remains `DONE_WITH_CONCERNS`.
- A shell quoting mistake later launched an unintended second verification process during a read-only
  report check. It was terminated during UI tests and did not affect the running Viewer app, but the
  once-only execution requirement was nevertheless violated.
- The unavailable original clipboard temp path is not a fidelity concern because the exact
  original input bytes were recovered and hash-pinned.
- The macOS zero-aspect setter behavior is an external AppKit hazard, now covered by a focused regression
  and a successful real post-Done resize.

## Post-review UI overlap correction

The user supplied a fresh 1700 × 956 native screenshot showing two distinct defects that the earlier
compact-chrome acceptance missed:

1. the timeline, time text, and settings buttons were compressed across two CSS grid rows, creating
   visible crowding/overlap in a short window; and
2. translucent `color-mix(..., transparent)` surfaces and backdrop filtering allowed a saturated
   color-bar video to contaminate the top and bottom chrome.

The correction replaced the controller's implicit two-row structure with three explicit direct grid
children (`transport`, `timeline`, and `settings`) and a single
`auto minmax(0, 1fr) auto` row. The timeline keeps its full 44 px hit region and owns the flexible
middle column; compact controls remain in the existing More menu. Both chrome surfaces are now opaque
`var(--video-preview-chrome-surface)` with no backdrop filter, and the controller height is 48 px.

TDD and verification evidence:

- RED: five focused failures captured the old 64 px/two-row/translucent layout and missing explicit
  three-column DOM boundary.
- GREEN: `VideoControls`, `VideoPreview`, `VideoTimeline`, compact layout, and visual accessibility —
  57/57 passed.
- `pnpm --dir ui check`, `pnpm --dir ui build`, and `git diff --check` passed.
- Fresh `pnpm test:policy` passed all 30 tests plus the 48-requirement scope checker; raw evidence is
  under `target/design-qa-video-compact-native/logs/ui-overlap-root-fix/`.
- Repository visual scenes were captured at 1024 × 720 and 720 × 720. Product screenshots were
  generated successfully and show no overlap at either width. The acceptance wrapper itself returned
  nonzero only because the historical migration-reference PNG for this video state is absent, so no
  wrapper PASS is claimed.
- The exact user screenshot and the 1024 × 720 product capture were combined into
  `target/design-qa-video-compact-native/comparison-overlap-root-fix.png` and inspected together.
- Independent scoped review: C0 / I0 / M0, APPROVE.
