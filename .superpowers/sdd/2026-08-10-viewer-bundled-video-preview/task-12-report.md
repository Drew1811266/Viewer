# Task 12 implementation report

Status: FIX ROUND 3 FROZEN — RE-REVIEW IN PROGRESS

Base: `4dacfa889d1ff81265457701d23fbddb37bf6358`

## Scope implemented

- Added a non-persisted Video Cache settings row that fetches runtime statistics only while Settings is mounted, displays usage against the fixed 1 GiB budget plus entry count, and clears through the existing confirmation surface.
- Added contained, retryable presentation for all six normalized playback-error categories without exposing paths. Done, Escape, and adjacent-video navigation remain available.
- Serialized preview teardown/reopen so Retry awaits closing the failed generation before reopening the same entity.
- Added neutral fixed-ratio thumbnail fallback for image-load failure without classifying a playable video as unavailable.
- Added the exact 12 Task 12 acceptance scene IDs, static frame artifacts, in-memory fake bridges, reduced-motion coverage, catalog entries, and registry tests. Acceptance scenes never start libmpv.
- Extended the native smoke matrix for real video-section, first-frame, control, navigation, error/retry, and cache-clear evidence.
- Kept the settings schema unchanged and did not begin Task 13.

## TDD evidence

- The plan command `pnpm ui:test -- ...` is not a repository script and failed with `Command "ui:test" not found`.
- The first equivalent repository test invocation exposed 12 intended behavior failures plus the missing `videoScenes` module. The retry-order regression specifically showed a second open before the deferred close settled.
- Core GREEN: SettingsDialog, VideoPreview, and VideoCard — 3 files / 25 tests passed.
- Acceptance GREEN: video scenes and acceptance catalog — 2 files / 6 tests passed.
- Integration regression: direct App coverage initially failed because the existing test bridge returned `undefined` instead of the typed cache-stat promise; the fake was corrected to the production bridge contract, then all 85 App tests passed.
- Review fix round 1 RED proved that an indexed terminal video exposed Retry without issuing any open operation, and that `video-reduced-motion` still selected the browser's `no-preference` media environment.
- Review fix round 1 GREEN made indexed Retry issue a same-entity open with viable stage geometry and made the exact reduced-motion scene exercise production `matchMedia` and data attributes.
- Review fix round 2 RED used initial indexed/session media with null dimensions and published `firstFrameReady` before authoritative Prepared media; it exposed both early UI replay and a native provisional-surface mount before the frontend could fit it.
- Review fix round 2 GREEN now re-probes the same authorized file identity on terminal Retry, fits the refreshed authoritative dimensions before native `prepare_surface`, and defensively buffers frontend first-frame publication until fitted `videoSetSurfaceRect` completes when open-session media is still null.
- Review fix round 3 RED deferred retry A's probe beyond navigation/open B and showed the missing pre-generation ownership API; the same test covers Done while the retry remains unresolved.
- Review fix round 3 GREEN adds bounded client-visible open-attempt IDs, pre-cancel-safe native tracking, a typed cancel command, cancellation propagation through the probe, and stale checks after every await through resolve/replace. Releasing stale A after B opens cannot close or replace B, and Done invalidates a pending retry before it can publish a generation.

## Focused verification

- Pre-review frozen gate:
  - `git diff --check` — PASS.
  - Affected App, settings, preview, card, acceptance-scene, catalog, and accessibility suite — PASS: 7 files / 120 tests.
  - `pnpm --dir ui check` — PASS: Biome checked 246 files and TypeScript completed. The existing Biome `recommended` deprecation notice was informational only.
  - `pnpm --dir ui build` — PASS: production build completed (188 modules transformed).
- Review-fix focused gate:
  - Native pre-prepare geometry unit — PASS: 1 / 1.
  - Native identity-bound retry and authorization boundary — PASS: 3 / 3.
  - Native generation/runtime lifecycle — PASS: 15 / 15.
  - Preview, bridge, and reduced-motion acceptance — PASS: 3 files / 30 tests.
  - Visual acceptance controller — PASS: 16 / 16.
  - `git diff --check` and `cargo fmt --check` — PASS.
- Open-attempt fix focused gate:
  - Deferred retry A vs navigation B plus Done — PASS inside native security boundaries: 4 / 4 total.
  - Native runtime lifecycle — PASS: 15 / 15.
  - Native pre-prepare geometry unit — PASS: 1 / 1.
  - Typed UI bridge, Preview lifecycle, bridge hook, acceptance bridge, and video scene — PASS: 5 files / 39 tests.
  - Visual acceptance controller — PASS: 16 / 16.
  - `pnpm --dir ui check` — PASS: 246 files plus TypeScript; existing Biome deprecation notice only.
  - `git diff --check` and `cargo fmt --check` — PASS.

## Review notes

- Initial single-reviewer verdict: Critical 0 / Important 2 / Minor 0. Indexed terminal Retry was inert, and the reduced-motion scene masked a `no-preference` browser environment.
- Fix-round-1 re-review: reduced-motion closed; Retry opened the same entity, but one Important remained because failed persisted metadata could not supply fitted native geometry before first frame.
- Fix-round-2 re-review: authoritative geometry closed; one new Important identified an unresolved retry A replacing navigation B after a late reprobe.
- Fix round 3 is frozen and returned to that same reviewer. No additional reviewer was started.

## Final verification

- The first final `pnpm verify` was run after the scoped review gate and found one unrelated-to-runtime documentation-coverage omission: `ui/src/visualMigrationCoverage.test.ts > tracks every audited state exactly once` compared the 104-entry catalog with a 92-entry ledger that had not registered Task 12's exact 12 video IDs. The remaining UI result was 104 passing files / 934 passing tests / 1 skipped test.
- Scoped RED: `pnpm --dir ui test visualMigrationCoverage` failed exactly at the catalog-versus-ledger equality assertion, showing the 12 missing video IDs.
- Scoped GREEN: the authoritative component-gap audit and migration ledger now include all 12 IDs with existing table columns and focused automated evidence. The migration coverage and catalog consumers now parse those exact IDs and preserve one-to-one, duplicate-free 104-state equality rather than filtering the video group.
- Focused GREEN: `pnpm --dir ui test visualMigrationCoverage acceptanceStateCatalog videoScenes` — 3 files / 9 tests passed.
- Final reviewer gate: stale Retry/navigation/Done review returned Critical 0 / Important 0 / Minor 0 (APPROVE); the follow-up final-gate coverage-fix review returned Critical 0 / Important 0 / Minor 0 (PASS).
- Final retry: fresh `pnpm verify` — PASS. Policy and clean-worktree checks passed; UI check passed (246 files, with only the pre-existing Biome deprecation info); UI tests passed (105 files / 935 tests, 1 skipped); UI production build passed (188 modules); Rust fmt, locked clippy, locked workspace tests, and security gate completed successfully.
