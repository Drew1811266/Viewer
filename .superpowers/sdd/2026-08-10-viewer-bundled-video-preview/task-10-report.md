# Task 10 report — first-frame-gated video preview shell

Worktree: `/Users/abc/Project/Viewer/.worktrees/video-preview`
Branch: `codex/video-preview`
Base: `390c3deffda8601adb5f84cb72e6986395168525`
Required commit: `feat: gate video preview on first frame`

## Scope and decisions

Task 10 adds only the dedicated video preview shell described by the frozen
brief: generation-keyed state, bridge lifecycle ownership, rotation-aware
native geometry, a neutral first-frame gate, a recoverable error shell, and
video-only routing/navigation. It deliberately does not add transport
controls, playback shortcuts, timeline/scrubbing interaction, frame stepping,
volume, speed, or later-task playback UI.

The native surface is an AppKit sibling below the WKWebView, so frontend
transparency alone cannot make the decoded frame visible unless the production
WKWebView also permits transparent under-page content. Task 3 proved the API in
its feasibility path, but production `MacVideoSurface::mount` had not inherited
that contract. The controller authorized the minimal production follow-up in
`viewer-platform-macos`: main-thread mount now configures the WKWebView's
under-page background as clear before attaching the below-webview surface.
Unsupported configuration returns typed
`SurfaceError::WebviewTransparencyUnavailable` and attaches no native surface.
No Tauri command routing or window-background behavior was duplicated.

## Delivered behavior

- Added a reducer that filters every native event by active generation, reveals
  a surface only once after that generation's `firstFrameReady`, and closes the
  visibility gate again for failure/closing without allowing late events to
  reopen it.
- Subscribed before `videoOpen`, buffered events until the returned generation
  was known, and detached/closed exactly once on file change, retry, Done,
  project close, or unmount. An open rejection detaches immediately without
  attempting to close a generation that was never created.
- Measured the video slot with `ResizeObserver`, waited for both valid stage and
  media dimensions, fitted into window coordinates, swapped axes for quarter
  turns, and updated rects without reopening media.
- Rendered an immediate shell with a neutral indeterminate `正在加载视频` gate,
  no percentage, no minimum delay, and no preview cover image. Failure keeps
  navigation and Done available and exposes Retry.
- Routed videos to `VideoPreview` while preserving image, text, and unsupported
  routes. Workspace navigation follows workspace video order; active search
  navigation follows only video hits in search-result order.
- Made the web content/background transparent only after the active first-frame
  gate opens, while retaining the toolbar and floating video-only navigation
  above the native surface.

## RED / GREEN evidence

1. Reducer and geometry:
   - RED: the exact two-file Vitest invocation exited 1 because the new modules
     did not exist.
   - GREEN: generation filtering, one-way reveal, progress/failure reduction,
     reset behavior, centered landscape fit, quarter-turn fit, and invalid
     geometry checks passed. The final reducer file has 6 passing tests.
2. Bridge lifecycle:
   - RED: the initial hook test could not import `useVideoBridge`; a later reset
     regression observed generation 9 persisting across retry/file reset.
   - GREEN: subscribe-before-open buffering, stale-event filtering, exactly-once
     close/detach, resize-without-reopen, and retryable open failure passed.
3. Preview shell and first-frame transparency gate:
   - RED: the initial component module was missing. The later production-hole
     regression failed because the dialog did not publish its surface-visible
     gate (`expected data-surface-visible=true`, received null).
   - GREEN: the shell's 3 tests pass with loading removal, dialog/stage reveal,
     no cover image, persistent error/navigation, retry, Done, and bounded
     neighbor navigation.
4. Video routing and navigation policy:
   - RED: the policy tests failed because `videoPreviewNeighbors` was absent;
     the App regression found no video preview dialog after card activation.
   - GREEN: workspace/search order policy passed, and the App regression opens
     two video generations, omits the cover in preview, closes generation 11 on
     navigation, and reports 1/2 then 2/2.
5. Production native transparency contract:
   - RED: the focused Rust test exited 101 because the ordered transparency
     mount helper and typed error did not exist.
   - GREEN: 2/2 focused tests prove transparency precedes attachment and typed
     configuration failure prevents attachment. A compile failure for the
     missing `NSObjectProtocol` trait import was diagnosed from the compiler
     output, fixed directly, and the focused test then passed.
6. Self-review lifecycle leak:
   - RED: `useVideoBridge.test.tsx` failed 1/4 because a rejected `videoOpen`
     left the listener attached (`expected once`, received zero calls).
   - GREEN: the same file passed 4/4 after rejection detached the listener and
     discarded generation-less buffered events.

## Focused verification before review

- Exact pre-review Task 10 UI set: 6 files / 110 tests passed.
- Native transparency contract: 2 passed, 0 failed, 40 filtered out.
- `pnpm --dir ui check`: exit 0; 236 files checked, with only the repository's
  existing Biome configuration deprecation information.
- `pnpm --dir ui build`: exit 0; 176 modules transformed and production output
  built successfully.
- `cargo fmt --check`: exit 0.
- `cargo check -p viewer-platform-macos --all-targets`: exit 0.
- `git diff --check`: exit 0.

Process note: one early command used `pnpm --dir ui test --` with intended name
filters, but this Vitest script ignored those filters and ran the full UI suite
(100 files, 888 passed, 1 skipped). This was reported immediately and was not
repeated during implementation. The required post-review `pnpm verify` remains
reserved for a single final invocation after the review gate reaches zero open
Critical/Important findings.

## Safety boundaries and non-goals

- No filesystem media path crosses the frontend boundary; open uses only the
  opaque entity ID and a fitted surface rectangle.
- Native attachment still occurs only on the AppKit main thread, below the
  retained WKWebView, and starts hidden under the native first-frame gate.
- The cover URL remains browse-only and is never rendered by `VideoPreview`.
- No later-task controls, timeline, scrub/seek, frame-step, playback speed,
  volume, or general shortcut ownership was implemented.

## Review

The single fresh reviewer reported 0 Critical, 2 Important, and 0 Minor:

1. A failed probe with null indexed dimensions never entered a lifecycle or an
   error state, leaving the loading shell permanent; pending metadata also
   needed an explicit path into lifecycle startup when it later became ready.
2. A rejected `videoSetSurfaceRect` changed frontend state to failed but did
   not detach or close the native generation, and later same-generation native
   state could overwrite the failure shell.

Review-fix RED / GREEN evidence:

- Failed/pending metadata RED: the hook run failed 1/6 because failed probe
  metadata stayed `0:hidden` instead of `0:hidden:failed:damaged`. GREEN: 6/6
  passed after failed probes mapped to an explicit typed shell error and pending
  metadata waited until a ready, valid geometry update. App refreshes the
  session's video DTO from the current workspace by entity ID so that update
  reaches the mounted preview.
- Surface terminality RED: the reducer/hook run failed 3/14: late playing state
  replaced failed state, and geometry rejection neither closed nor detached.
  GREEN: 14/14 passed after failed/closing states became terminal and the
  active lifecycle owned detach, buffer discard, exactly-once close, and error
  dispatch for surface failure. Tests cover rejection both before and after
  first-frame reveal plus a later native state event.
- Post-fix exact Task 10 UI set: 6 files / 114 tests passed.
- Post-fix `pnpm --dir ui check`: exit 0; 236 files checked with only the
  existing Biome configuration deprecation information.
- Post-fix `pnpm --dir ui build`: exit 0; 176 modules transformed.

The same reviewer closed the surface-lifecycle finding and retained one
Important metadata edge case: `normalize_ffprobe` legitimately returns Ready
when both source dimensions are absent, so Ready/null metadata still remained
behind the loading gate. The exact hook regression first failed 1/9, observing
`0:hidden` instead of `0:hidden:failed:video_geometry_unavailable`, then passed
9/9 after invalid Ready dimensions became a typed terminal, non-retryable
geometry error. The refreshed exact Task 10 set passes 6 files / 115 tests;
UI check and build also pass again.

The same reviewer re-read this final scoped fix and returned no findings. Final
open counts are 0 Critical, 0 Important, and 0 Minor; verdict: APPROVE. No
second reviewer was used. The one permitted final `pnpm verify` is now
authorized.

## Final verification

The first authorized final `pnpm verify` exited 1 at the first repository-policy
stage. Of 29 policy tests, 28 passed; the sole failure was the frozen direct
dependency inventory rejecting the newly declared `objc2-web-kit`. Because the
failure occurred immediately, UI, Rust, build, and security stages did not run.

The production transparency contract does not require a new dependency: Task
3's feasibility implementation already uses `objc2::msg_send!` for the same
public WKWebView selector. The scoped fix removed every Task 10 Cargo manifest
and lockfile change, retained the runtime `respondsToSelector` guard and typed
failure, and invoked `setUnderPageBackgroundColor:` through the existing
`objc2` dependency. Post-fix evidence:

- exact repository policy: 29/29 passed;
- scope coverage: 47 requirements mapped exactly once, including 13 frozen M3
  requirements;
- native surface transparency contract: 2/2 passed;
- `cargo check -p viewer-platform-macos --all-targets`: exit 0;
- `cargo fmt --check` and `git diff --check`: exit 0.

The controller explicitly authorized one necessary full verification retry
after this scoped fix reaches zero open Critical/Important findings with the
same reviewer.

The same reviewer approved the dependency-free selector call with final open
counts of 0 Critical, 0 Important, and 0 Minor. The authorized `pnpm verify`
retry then exited 0. Evidence includes:

- repository policy 29/29, scope coverage 47/47, and clean-verifier tests
  11/11;
- UI check, 100 test files / 894 passed + 1 skipped, and production build with
  176 transformed modules;
- locked workspace formatting, strict Clippy, and all workspace Rust tests,
  including the production transparency contract in the 42-test macOS
  platform suite;
- Tauri security boundaries 8/8;
- offline locked cargo-deny `bans ok, licenses ok, sources ok`;
- npm license policy: 140 packages across 11 reviewed expressions.

Cargo-deny's duplicate dependency listings are the repository's existing
allowed warnings; the security command completed successfully.
