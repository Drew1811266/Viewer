# Task 9 report — dedicated video browsing

Worktree: `/Users/abc/Project/Viewer/.worktrees/video-preview`
Branch: `codex/video-preview`
Base: `cc468ae6970beb0ec0127ab108ba4db969e3d320`
Required commit: `feat: browse videos in a dedicated section`

## Scope and decisions

Task 9 activates Task 4's deliberately deferred native browse DTO fields, adds
the exact Task 8 frontend video command/event bridge, video-aware browse/search
types, a dedicated expandable video section, stable 16:9 cards, video-aware
selection scopes, and session-local disclosure preferences. It does not add the
Task 10 player, transport controls, or playback lifecycle state. Video open
gestures therefore emit only an entity ID through the dedicated `onOpenVideo`
boundary; they do not enter the existing generic preview state.

The native DTO now serializes required camelCase `videos`, `videoCount`, and
selection `types.videos` fields. Each `BrowserFileDto` includes required
`videoMetadata` (nullable for non-video files), and video metadata preserves the
indexed duration, display geometry, rotation, frame rate, codecs, probe status,
and normalized failure kind. `coverUrl` is currently an explicit `null` because
no browse cover artifact is available at this boundary. TypeScript mirrors the
required fields; an actual `VideoFile` further requires `kind: "video"` and
non-null metadata, and `isVideoFile` checks both.

## Delivered behavior

- Added all fourteen explicit Task 8 video bridge commands plus the single
  `viewer://video-event` listener, each forwarding only its matching native
  command contract.
- Removed Task 4's `#[serde(skip)]`/`deferred_until_task_9` DTO fields and
  exposed videos, video metadata, folder video counts, and selection video
  counts without changing runtime behavior.
- Added exact video metadata, playback session/event, request, cache, and file
  types; search now offers the `视频` file-kind filter.
- Partitioned browse content in stable image/video/other order. Video selection
  participates in Finder/organization/radial/file-operation safety paths and
  select-all exposes only scopes that actually contain files.
- Added a default-expanded video disclosure between images and other files.
  Empty sections render nothing; collapse does not clear hidden selection.
- Added a reserved 16:9 card stage, cover fade, centered play affordance,
  microsecond duration formatting, filename, active/selected styling, failed
  probe badge, reduced-motion handling, and bounded section scrolling.
- Added in-memory, per-project-session disclosure preference. Null and stale
  sessions cannot write a preference, and no preference path invokes probe,
  cover, or playback commands.

## RED / GREEN evidence

1. Video bridge contract:
   - RED: `pnpm --dir ui exec vitest run src/api/viewer.test.ts` failed 1/6
     because `videoOpen` and the remaining video bridge methods did not exist.
   - GREEN: the same file passed 6/6 after exact invoke/listen mappings landed.
2. Native browse DTO exposure:
   - RED: `cargo test -p viewer-desktop
     task_9_video_dto_serializes_workspace_metadata_and_counts -- --nocapture`
     exited 101 because `workspace_json["videos"][0]["name"]` was `Null`.
   - GREEN: the same exact test passed 1/1 after the video workspace, metadata,
     folder count, and selection count fields were serialized.
3. Required TypeScript DTO fixtures:
   - RED: `pnpm --dir ui exec tsc -b` exited 2 and identified every legacy
     fixture missing required `videos`, `videoMetadata`, `videoCount`, or
     selection video count fields.
   - GREEN: the same command exited 0 after fixtures and acceptance data were
     upgraded to the active native contract.
4. Browse, card, filter, model, and preference behavior:
   - RED: the first six-file focused run had three missing modules and six
     behavior failures for absent video grouping/filter/scope behavior.
   - GREEN: the same focused set passed 6 files / 110 tests after the dedicated
     section, cards, selection scopes, and preference hook were implemented.
5. Stable cover geometry:
   - RED: `videoBrowser.test.ts` failed 1/2 because the stylesheet lacked the
     explicit `aspect-ratio: 16 / 9` reservation.
   - GREEN: it passed 2/2 after the CSS contract was added.
6. Task 9/10 preview boundary found during self-review:
   - RED: the exact ContentBrowser regression failed because video double-click
     reused generic preview and the independent entity-ID callback was never
     called; the keyboard subcase likewise failed for Space.
   - GREEN: the focused regression passed after both gestures were routed only
     to `onOpenVideo`, leaving generic preview untouched.

## Focused verification before review

- Post-DTO Task 9 focused run: 10 files / 131 tests passed.
- Native video metadata publication regression: 1/1 passed.
- Native Task 9 serialization regression: 1/1 passed.
- `pnpm --dir ui check`: exit 0; 228 files checked (only the repository's
  existing Biome config deprecation notice).
- `pnpm --dir ui exec tsc -b`: exit 0 (also included in UI check).

The final post-self-review focused run and UI check are recorded below before
the single independent review begins.

## Review

The single fresh reviewer froze its first result at 0 Critical, 2 Important,
and 1 Minor:

1. Organization drag validation and drop execution looked up only image and
   other-file entities, so video moves/copies stopped before preflight.
2. Projection repair ordering and marker updates omitted workspace videos,
   leaving removed video selection and updated markers stale.
3. The empty-project copy still described only images, Markdown, and text.

Review-fix RED / GREEN evidence:

- Video organization drag RED: the exact App regression timed out with zero
  `preflightFileCommand` calls. GREEN: it passed 1/1 after both validation and
  execution lookup collections included videos.
- Video reducer bookkeeping RED: the focused reducer run failed 2/2; the
  video marker stayed unmarked and an externally removed video stayed
  selected. GREEN: both passed after stable workspace ordering and marker
  mapping included videos.
- Empty-state copy RED: the exact App test could not find the video-aware
  supported-file sentence. GREEN: it passed 1/1 after the copy was corrected.
- Post-fix App + reducer focused run: 2 files / 102 tests passed.
- Post-fix `pnpm --dir ui check`: exit 0; 228 files checked with only the
  existing Biome configuration deprecation notice.
- Post-fix `git diff --check`: exit 0.

The same reviewer re-read the scoped fixes and closed every original finding.
Final open counts are 0 Critical, 0 Important, and 0 Minor; verdict: APPROVE
and ready for the one permitted final verification. No second reviewer was
used.

## Final verification

The first required final `pnpm verify` exited 1 in the UI test stage. All work
before that failure passed: repository policy 29/29, clean-verifier tests
11/11, and UI check. The UI run passed 95 files and failed only
`acceptanceBridge.test.ts`: its exact complete bridge-key contract still listed
the pre-Task-9 methods and therefore rejected `listenVideo` plus all fourteen
new video bridge methods. At the stop point, 867 tests passed, 1 failed, and 1
was skipped; build, Rust, and security stages did not run.

The complete-key assertion was not weakened or removed. It now explicitly
lists `listenVideo` and every video method, and also proves `videoOpen` retains
the deterministic acceptance bridge's fail-fast behavior. Post-fix evidence:

- `pnpm --dir ui exec vitest run
  src/acceptance/acceptanceBridge.test.ts`: 1 file / 2 tests passed.
- `pnpm --dir ui check`: exit 0; 228 files checked with only the existing
  Biome configuration deprecation notice.
- `git diff --check`: exit 0.

The same reviewer approved this test-only contract fix with final open counts
of 0 Critical, 0 Important, and 0 Minor, confirming the exact assertion remains
strict and no production behavior changed.

The required `pnpm verify` retry exited 0. Evidence includes:

- repository policy 29/29 and clean-verifier tests 11/11;
- UI check, 96 test files / 868 passed + 1 skipped, and production build;
- locked workspace formatting, strict Clippy, and all workspace Rust tests,
  including the Task 9 DTO serialization regression;
- Tauri security boundaries 8/8;
- offline locked cargo-deny `bans ok, licenses ok, sources ok`;
- npm license policy: 140 packages across 11 reviewed expressions.

Cargo-deny's duplicate dependency listings are the repository's existing
allowed warnings; the security command completed successfully.

## Safety boundaries and non-goals

- No player surface, controls, playback hook, or Task 10 UI was added.
- The frontend never supplies a native media or cache filesystem path.
- Disclosure changes are presentation-only and cannot issue video commands.
- Covers use only DTO-provided opaque URLs and never change card geometry.
- Native changes are limited to direct browse DTO serialization and the tests
  whose destructuring consumed the former deferred field names.
