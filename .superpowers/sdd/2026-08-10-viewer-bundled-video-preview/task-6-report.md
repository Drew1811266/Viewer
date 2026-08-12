# Task 6 Report: Safe Video Covers and Timeline Frames

Date: 2026-08-11
Base: `d0f28f86191d405c7dc54a6488567a38ba8ed37c`
Branch: `codex/video-preview`
Scope: Task 6 only

## Implemented

- Added a canonical-source, size, mtime, output-kind, timeline-bucket, and
  algorithm-version keyed `VideoCacheKey`.
- Added a marker-verified `video/` cache with a fixed 1 GiB budget, atomic PNG
  publication, access-stamp LRU eviction, safe stats/clear, symlink rejection,
  and one shared operation lock for every handle opened on the same canonical
  cache root.
- Added deterministic cover selection at the approved early fractions. Analysis
  uses width-160 grayscale frames and the approved luminance/variance thresholds;
  the selected cover is rendered at width 640. Timeline PNGs use width 320 and
  500 ms buckets.
- Added `VideoThumbnailService` checks for active session generation, Task 5
  `MediaFileIdentity`, cancellation before cache publication, playback-active
  yielding, and one-at-a-time thumbnail extraction.
- Extended `BundledMediaTools` with a typed, fixed-argument, identity-bound,
  cancelable frame API. It reuses the existing one-permit media-tools semaphore,
  kills and awaits canceled children, bounds stdout/stderr, and binds ffmpeg to
  its packaged file identity and runtime-inventory SHA-256 just like ffprobe.
  Task 6 does not use the existing raw diagnostic `ffmpeg(arguments)` method.
- Added verified video-PNG registration through `ImageArtifactRegistry`, plus a
  generation-linearized public Tauri helper that returns only a session-scoped
  `viewer-image://localhost/<session>/<token>` URL. Cache paths never enter the
  URL or protocol response.
- Updated the Task 5 fake runtime fixture to contain the now-required bound
  ffmpeg executable and inventory digest. The approved Task 3 MP4/MOV fixture
  bytes were not modified.

## Files

- Created `crates/viewer-infrastructure/src/video_cache.rs`.
- Created `crates/viewer-infrastructure/src/video_thumbnail.rs`.
- Created `crates/viewer-infrastructure/tests/video_cache.rs`.
- Created `crates/viewer-infrastructure/tests/video_thumbnail.rs`.
- Modified `crates/viewer-infrastructure/src/lib.rs`.
- Modified `crates/viewer-infrastructure/src/image_cache.rs`.
- Modified the test fixture builder in
  `crates/viewer-infrastructure/src/video_probe.rs`.
- Modified `crates/viewer-video-mpv/src/lib.rs` and
  `crates/viewer-video-mpv/src/process.rs` to provide the safe frame boundary
  required to avoid bypassing Task 5 identity/cancellation/semaphore controls.
- Modified `src-tauri/src/image_protocol.rs`, `src-tauri/src/state/mod.rs`, and
  `src-tauri/src/state/video_index.rs`.

## RED / GREEN Evidence

1. Cache and selector RED:
   `cargo test -p viewer-infrastructure --test video_cache --test video_thumbnail`
   exited 101 with unresolved `video_cache` and `video_thumbnail` modules.
   Initial GREEN passed cache 7/7 and selector/quantizer 4/4.
2. Thumbnail-service RED:
   `cargo test -p viewer-infrastructure --test video_thumbnail` exited 101 with
   missing extractor/service/error/output contracts. GREEN passed 10/10,
   including 160-gray to 640-PNG cover selection, 320-PNG quantized reuse,
   playback yield, max concurrency one, cancellation, and stale-generation
   publication rejection.
3. Bundled ffmpeg RED:
   `cargo test -p viewer-video-mpv ffmpeg_frame_` exited 101 with missing
   `MediaFrameOutput` and `video_frame_identity_bound`. GREEN passed 3/3 for
   fixed arguments, source replacement rejection, and kill-and-await
   cancellation. The full package later passed 18 unit tests plus all runnable
   integration/doc tests.
4. Protocol-registration RED:
   `cargo test -p viewer-infrastructure --test video_cache` exited 101 with
   missing `register_video_png` and `UnverifiedVideoCache`. GREEN passed 9/9;
   desktop protocol passed 10/10; generation-bound URL registration passed 2/2.
5. Cross-handle clear/write RED:
   `cargo test -p viewer-infrastructure --test video_cache
   clear_waits_for_an_atomic_write_started_by_another_cache_handle -- --nocapture`
   failed because clear raced the writer and the writer returned `Err(Io)`.
   After sharing the operation lock by canonical cache root, the same command
   passed 1/1.

Systematic-debugging corrections during GREEN were limited to evidence-backed
root causes: Rust 1.85 does not permit `Ord::min` in this `const fn`; a macOS
test compared `/var` against canonical `/private/var`; and the Task 5 fake
runtime did not yet mirror the newly bound ffmpeg entry.

## Verification

- Clean-base `pnpm verify`: exit 0 before implementation.
- Focused cache/thumbnail suites: passing.
- `cargo test -p viewer-video-mpv`: passing; 3 staged-runtime tests remain
  conditionally ignored because no staged ViewerVideoRuntime is present.
- `cargo test -p viewer-infrastructure video_`: passing after the fixture
  correction.
- `cargo test -p viewer-desktop image_protocol`: 10/10 passing.
- `cargo test -p viewer-desktop registered_video_png_url`: 2/2 passing.
- `cargo fmt --check && cargo clippy --locked --workspace --all-targets --
  -D warnings`: exit 0 at the implementation checkpoint.
- Final fresh focused suites, strict Clippy, and full `pnpm verify` will be run
  again after independent review and any fixes.

## Independent Review

The first fresh read-only review completed with **0 Critical, 6 Important, and
2 Minor** findings. Verdict: not ready to merge. No review fix was started
before the user requested a pause.

Important findings to close with RED/GREEN tests before re-review:

1. Hold verified cache identity through rename, removal, clear, and registration
   so intermediate-component or entry replacement cannot create a symlink/path
   TOCTOU.
2. Account for and remove orphan temporary-only entry directories so `stats`,
   eviction, and `clear` cannot report zero while bytes remain on disk; the
   1 GiB budget must include all owned cache bytes.
3. Recheck cancellation and generation at the actual cache-hit return and
   final cache-publication boundary.
4. If playback becomes active after extraction starts, cancel and reap the
   thumbnail ffmpeg child before yielding.
5. Bind PNG registration to a trusted cache/artifact identity and the owning
   session/generation rather than accepting independently supplied candidate
   marker root, session, generation, and path values.
6. Remove the ffmpeg verify-then-spawn pathname replacement window; executable
   bytes used by the child must remain bound to the reviewed bundled identity.

Minor findings to address in the same fix round:

- Replace ffprobe-specific timeout/cancellation diagnostic wording in the
  ffmpeg frame path with neutral media-tool diagnostics.
- Keep synchronous cache filesystem work off async executor paths.

The frozen reviewed implementation passed its focused suites and strict
Clippy before review, but the full post-review `pnpm verify`, re-review, and
Task 6 commit are intentionally outstanding.

## Pause / Resume Point

- Paused by user on 2026-08-11.
- Current HEAD/base: `d0f28f86191d405c7dc54a6488567a38ba8ed37c`.
- Working tree is intentionally dirty and uncommitted with Task 6 changes.
- Task 5 remains complete and verified; Task 7 has not started.
- Resume by technically validating each review finding, adding stable REDs,
  applying minimal fixes, running the same reviewer again, then fresh strict
  Clippy and full `pnpm verify`. Commit only after approval and exit 0.

## Review Fix Round 1/5: fd-bound cache and orphan accounting

- Technical verification: findings 1 and 2 reproduced. Cache operations checked
  pathnames and later reopened/renamed/recursively removed them; scanning skipped
  a valid key directory when `artifact.png` was absent.
- RED: `cargo test -p viewer-infrastructure --test video_cache
  temp_only_owned_entries -- --nocapture` ran 2 tests and failed 2/2: stats
  reported `entry_count = 0` and clear left the temp-only directory behind.
- RED: `cargo test -p viewer-infrastructure adversarial_tests --lib --
  --nocapture` failed to compile because deterministic pre-commit/pre-remove
  test hooks were not present.
- GREEN: cache roots and key directories are retained as no-follow directory
  descriptors. Creation, open, rename, unlink, and directory removal use
  descriptor-relative operations; device/inode binding is checked at commit and
  removal, and the canonical root identity is rechecked before returning.
- GREEN: scanning is descriptor-bound and accounts for every owned regular file
  (`artifact.png`, access stamp, and valid temp artifacts). Temp-only entries now
  participate in stats, eviction, and clear.
- A first behavioral run exposed that duplicated directory descriptors share a
  directory offset. The evidence was clear returning success without observing
  the entry after a prior scan. `rewinddir` on each descriptor-bound scan fixed
  that root cause. A test fixture initially used a non-generated temp suffix;
  changing it to the production 32-hex format made it test the owned-temp
  contract.
- Final focused evidence:
  `cargo test -p viewer-infrastructure video_cache::adversarial_tests --lib --
  --nocapture` passed 4/4 (entry and intermediate-root swaps for publication and
  clear); `cargo test -p viewer-infrastructure --test video_cache --
  --nocapture` passed 12/12.

## Review Fix Round 2/5: publication boundaries and playback preemption

- Technical verification: findings 3 and 4 reproduced. Cache-hit branches
  returned immediately after cache I/O; the synchronous put combined staging
  and rename; playback was observed only before extractor launch.
- RED: `cargo test -p viewer-infrastructure publication_boundary_tests --lib
  -- --nocapture` failed to compile because the deterministic after-hit and
  after-stage boundary hooks did not exist. RED:
  `cargo test -p viewer-infrastructure --test video_thumbnail
  playback_activation_after_launch_cancels_and_reaps_before_retrying --
  --nocapture` failed 0/1 by timing out after playback became active; the
  launched extractor never received cancellation.
- GREEN: cache-hit I/O and PNG staging run on bounded blocking tasks. Both cover
  and timeline requests revalidate cancellation, source identity, session, and
  generation after a hit. Long temp writes finish before a generation-held
  commit; a final callback rechecks cancellation and Task 5 source identity
  immediately before descriptor-relative rename. Staged temps self-clean when
  the request is rejected, and eviction runs off the async worker after commit.
- GREEN: extraction now uses a child cancellation token and monitors playback
  for the entire child lifetime. Playback activation cancels the child, awaits
  it (the real bundled-media implementation kills and reaps on token
  cancellation), yields until idle, then retries while preserving the original
  request cancellation.
- Focused evidence: publication-boundary tests passed 4/4 (hit cancellation,
  hit generation invalidation, post-stage cancellation, post-stage generation
  invalidation). The playback transition test passed 1/1 and asserted extractor
  active count returned to zero before retry. Full cache/thumbnail integration
  suites passed 12/12 and 11/11.
- Minor async-I/O finding is also addressed here: cache get, temp write/stage,
  eviction scan/removal, and clear execute via `spawn_blocking`; only the short
  validated rename/access-stamp commit remains inside the generation
  linearization critical section.

## Review Fix Round 3/5: trusted artifact registration

- Technical verification: finding 5 reproduced. The old API accepted an
  independently supplied session and path, then reconstructed trust by opening
  any candidate parent containing the predictable marker.
- RED: the capability-oriented integration test failed to compile because
  `register_video_png` still expected `SessionId` and `&Path`, and registered
  artifacts had no generation/request identity.
- GREEN: registration now requires both the application-held `VideoCache` and
  the non-publicly-constructible `VideoThumbnailArtifact`. Session, generation,
  request ID, and path are read only from that artifact; URL registration
  linearizes its embedded identity through `TaskCoordinator`.
- The cache binds the PNG through its retained root/entry descriptors and
  returns an open verified file. The registry retains that file descriptor, and
  the image protocol serves video PNG bytes through positional reads from the
  bound file instead of reopening a pathname.
- Focused evidence: capability/session/request preservation passed 1/1;
  registration through a different cache capability was rejected 1/1; desktop
  URL tests passed 2/2; protocol passed 1/1 after renaming the registered PNG
  and placing different bytes at its old pathname, proving the response still
  came from the bound verified file.

## Review Fix Round 4/5: executable identity through spawn

- Technical verification: finding 6 reproduced. SHA-256 and identity were
  checked on one pathname open, while `Command::new` later resolved the bundle
  pathname again.
- RED: the deterministic validation-to-spawn replacement test failed to compile
  because no before-spawn boundary existed. A first fd-exec implementation then
  produced a real macOS `EACCES`: macOS exposes `/dev/fd/N` for reads but does
  not execute it and has no `fexecve`/`execveat`. This platform failure was not
  treated as GREEN.
- GREEN: at runtime binding, the reviewed bundle executable is retained open,
  hashed against inventory, and copied by positional reads from that verified
  file into a freshly created process-private `0700` launch directory. The
  copied executable is `0500`, retained open, identity/hash revalidated for
  every launch, and used by `Command`; argv[0] remains the reviewed bundle path.
  Original bundle path identity is also revalidated before each request, so a
  replacement is rejected on subsequent calls but cannot change bytes used by
  an already validated launch.
- GREEN: the replacement-window test passed 1/1 after replacing the bundle
  pathname immediately after validation; child stdout was exactly
  `verified-executable`, not the replacement payload. The fixed-argument/frame
  test still proves the actual child argv and identity-bound stdin.
- Full `cargo test -p viewer-video-mpv -- --nocapture` passed 20/20 unit tests,
  all runnable integration/doc tests, with only the same 3 staged-runtime tests
  ignored. This includes ffmpeg and ffprobe cancellation kill-and-reap, timeout
  cleanup, independent output bounds, and source-file descriptor binding.
- Minor diagnostic finding is closed: cancellation and timeout strings are now
  media-tool neutral, with an exact-string test passing 1/1.

## Review Fix Round 5/5: focused verification checkpoint

- `cargo fmt --all -- --check` and `git diff --check`: exit 0.
- Cache/thumbnail integration suites: 12/12 and 11/11 pass.
- `cargo test -p viewer-infrastructure video_ -- --nocapture`: all 29 matching
  library tests pass, including 4 adversarial cache swap tests and 4 publication
  boundary tests; all matching integration tests pass.
- Desktop image protocol and registered URL focused suites: pass.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: exit 0 with
  no `allow(dead_code)` or equivalent warning suppression added.
- A full pre-re-review `pnpm verify`, same-reviewer scoped re-review, and final
  post-review fresh verification remain outstanding at this checkpoint.

Pre-re-review full gate: fresh `pnpm verify` exited 0. Policy tests passed;
UI check/build passed; UI tests passed 92 files with 851 passed and 1 skipped;
workspace format, strict Clippy, and all workspace tests passed; security and
license gates completed successfully. The existing Biome deprecated-config
message remained informational only.

## Scoped Re-review and Review Fix Round 2/5

The same reviewer returned **0 Critical, 2 Important, and 1 Minor**. The two
remaining Important findings were retained registered-file storage escaping
cache accounting after unlink, and a final snapshot-path check followed by a
second pathname resolution in `Command::spawn`. The Minor finding required
removing all three Task 6 `too_many_arguments` suppressions.

### Registered PNG storage is reclaimed without losing immutable content

- Technical validation: the first registration fix retained an `Arc<File>`.
  Clearing or evicting the cache removed the pathname and made `stats` report
  zero, but the open inode still retained its physical blocks outside the
  enforceable 1 GiB budget.
- RED: `cargo test -p viewer-infrastructure --test video_cache
  clearing_a_registered_png_reclaims_the_cache_entry_without_losing_immutable_bytes
  -- --nocapture` failed to compile because the registry had no immutable-byte
  or retained-file behavior to assert.
- GREEN: cache registration now reads the already descriptor-bound PNG by
  positional reads into `Arc<[u8]>`, validates the PNG and size while holding
  the cache operation lock, and closes the cache file before registration.
  The registry stores only those immutable bytes. The same test passes 1/1:
  after `clear`, cache stats are zero and the path is absent, while the
  registered response remains byte-identical and retains no cache file.
  Trusted cache/artifact capability, session, generation, and path-hiding
  checks remain unchanged. Focused cache tests pass 13/13 and the desktop
  protocol tests pass 10/10.

### Executed Mach-O bytes remain identity-bound through spawn

- Technical validation: macOS has no `fexecve`/`execveat`. Executing the
  reviewed ffmpeg Mach-O through `/dev/fd/N` returned `EACCES` even with an
  explicitly inheritable descriptor, proving the failure was not limited to
  script shebang handling. The process-private snapshot therefore remained the
  portable executable source, but its check followed by `Command::spawn` still
  allowed the snapshot pathname itself to be replaced.
- RED: `ffmpeg_frame_rejects_snapshot_replacement_after_final_validation`
  first failed to compile because no post-final-validation/pre-spawn boundary
  existed. Its deterministic hook renames the verified snapshot and puts a
  different Mach-O at the launch pathname in that exact window.
- GREEN: Mach-O frame children use `posix_spawn` with
  `POSIX_SPAWN_START_SUSPENDED`. Before the child can execute user code,
  `proc_pidpath` identifies its selected image; that pathname is opened with
  `O_NOFOLLOW|O_CLOEXEC` and its device/inode identity is compared with the
  retained reviewed snapshot. A mismatch is sent `SIGKILL` and reaped before
  returning `ExecutableChanged`; a match alone receives `SIGCONT`. Fixed argv,
  descriptor-bound media input, the shared semaphore, cancellation, timeout,
  and independent output bounds remain in force.
- The snapshot-swap test passes 1/1. Three ad-hoc-signed Mach-O helper tests
  pass 3/3 and prove cancellation, timeout, and stdout overflow each kill and
  reap the suspended-launch child (`kill(pid, 0)` returns `ESRCH`). The full
  `viewer-video-mpv` focused suite passes 24/24 unit tests plus all runnable
  integration tests.

### Request structure replaces warning suppression

- RED: after removing the three Task 6 suppressions,
  `cargo clippy -p viewer-infrastructure --all-targets -- -D warnings` reported
  `timeline` as 9/7 and `publish_if_current` as 8/7; the cover method already
  fell below the threshold once `self` was counted separately.
- GREEN: `VideoThumbnailContext` now carries the inseparable session,
  generation, canonical source identity, Task 5 media identity, and
  cancellation token. `CoverThumbnailRequest` and `TimelineThumbnailRequest`
  carry only operation-specific values, and the same context reaches cache-hit
  validation and final publication. No Task 6 `too_many_arguments` or
  dead-code suppression remains. Cache/thumbnail tests pass 13/13 and 11/11,
  and strict workspace Clippy exits 0.

## Load-bearing Bundled PNG Runtime Correction

The scoped review work exposed a production gap not visible through the fake
extractor: the previously staged reviewed ffmpeg had no PNG encoder. Task 6
requires real width-320 and width-640 PNG output, so the controller authorized
the smallest direct expansion beyond the brief's initial file list:
`scripts/video/build-macos-runtime.sh`, the runtime lock/validator/verifier, and
the existing staged-runtime integration test. No Task 7 code was added.

- RED: the new runtime-contract test failed because the lock lacked
  `--enable-zlib`; the real staged integration test failed with `Unknown
  encoder 'png'` for `MediaFrameOutput::Png320`.
- GREEN: the locked FFmpeg configuration now explicitly includes only the
  needed system-zlib closure, `--enable-zlib` and `--enable-encoder=png`.
  Runtime lock validation and `verify-runtime.sh` require both options, and the
  verifier also requires the actual `png` encoder table entry. Node runtime
  policy tests pass 13/13.
- A single clean rebuild used the existing pinned HTTPS + SHA-256 builder with
  fresh work and stage directories. Builder verification passed every inventory
  entry. Buildconf contains all four prohibited-feature disables plus both PNG
  enables; the encoder table contains `VF...D png`. `otool` reports only Apple
  frameworks, `/usr/lib/libSystem.B.dylib`, and the approved system
  `/usr/lib/libz.1.dylib`; there is no PATH/Homebrew or production network
  fallback.
- `codesign --verify --strict` passes for ffmpeg and ffprobe. The ad-hoc ffmpeg
  CDHash was `d01e5b804d02d9fcd83bdc4085c52778e30a6e88`; ffprobe was
  `0ab94de591004f2924b8a1ea07f62a88bc25de67`. A fresh
  `shasum -a 256 -c runtime.inventory.sha256` reported every entry `OK`.
- The real staged Rust regression now passes 1/1. It creates a real video using
  the reviewed ffmpeg, runs both identity-bound frame modes through the
  suspended-spawn path, validates both PNG signatures, and parses the IHDR
  widths as exactly 320 and 640.

## Round 2 Pre-final-review Verification

- `cargo fmt --all -- --check` and `git diff --check`: exit 0.
- Runtime lock/verifier Node tests: 13/13 pass.
- `cargo test -p viewer-video-mpv -- --nocapture`: 24/24 unit tests and all
  runnable integration/doc tests pass; the three explicitly staged tests remain
  ignored in the ordinary package run.
- Cache/thumbnail integration: 13/13 and 11/11 pass. Infrastructure `video_`
  library selection: 29/29 pass.
- Desktop image protocol: 10/10 plus its security-boundary test pass;
  generation-bound registered URL tests: 2/2 pass.
- The separately invoked real staged PNG test passes 1/1 with both exact widths.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: exit 0.
- The existing reviewer final scoped re-review, one final post-review
  `pnpm verify`, and Task 6 commit remain outstanding.

## Scoped Re-review Fix Round 3/5: suspended child code identity

- Technical validation: `proc_pidpath` identifies a pathname associated with
  the suspended child, but reopening that pathname observes the current
  directory entry rather than the executable image already selected by the
  child. A concurrent directory update can restore the retained reviewed entry
  before that reopen, so matching path/device/inode checks alone do not prove
  the child's selected code identity.
- RED: the deterministic regression moves the process-private reviewed launch
  directory aside after final validation, places a different signed Mach-O at
  the original pathname, lets the suspended child select it, and restores the
  entire reviewed directory after `proc_pidpath` but before the old pathname
  reopen. `cargo test -p viewer-video-mpv
  ffmpeg_frame_rejects_snapshot_replacement_restored_after_pidpath --
  --nocapture` failed with `unexpected ABA result:
  Ok(MediaToolOutput { status: ExitStatus(unix_wait_status(9)), stdout: [],
  stderr: [] })`, proving the old check resumed a child whose code did not come
  from the reviewed snapshot.
- GREEN: snapshot creation now parses every SHA-256 CodeDirectory from the
  retained, inventory-hashed Mach-O bytes (thin and universal formats) and
  stores their 20-byte CDHashes. While the child remains
  `POSIX_SPAWN_START_SUSPENDED`, launch requires the public macOS
  `csops(pid, CS_OPS_STATUS)` result to contain `CS_VALID` and
  `csops(pid, CS_OPS_CDHASH)` to match one of those retained snapshot hashes.
  Identity unavailable or mismatched fails closed; the caller sends `SIGKILL`
  and waits before returning `ExecutableChanged`. Only a match receives
  `SIGCONT`.
- Focused evidence: `cargo test -p viewer-video-mpv process::tests --
  --nocapture` passes 24/24, including the precise directory-restore regression
  and cancellation, timeout, and output-overflow kill/reap checks. The real
  rebuilt runtime test passes 1/1 with exact 320/640 IHDR widths through the
  same suspended/CDHash launch path:
  `VIEWER_VIDEO_TEST_RESOURCES=target/task6-review-resources cargo test -p
  viewer-video-mpv --test bundled_loader
  staged_client_accepts_the_locked_down_local_media_options -- --ignored --exact
  --nocapture`.
- `cargo clippy -p viewer-video-mpv --all-targets -- -D warnings` exits 0. Test
  launch hooks are grouped into a structure; no argument-count, dead-code, or
  other warning suppression was introduced. Fixed argv, descriptor-bound
  input, shared semaphore, cancellation, timeout, output limits, and mismatch
  child reap remain covered.

## Residual Scope Notes

- Task 6 exposes the generation-bound PNG URL registration boundary publicly;
  Task 7 will consume it from the video session service. This task does not add
  the Task 7 playback state machine, commands, player surface, or UI.
- The commit will be titled `feat: cache video covers and timeline frames`.
  Its resulting SHA is reported in the controller handoff because a commit
  cannot embed its own final SHA without changing that SHA.

## Final Review, Verification, and Commit Gate

- The same scoped reviewer closed the sole remaining Important executable
  identity finding. Final assessment after the round 3 implementation was
  **0 Critical, 0 Important, 1 Minor; ready to merge**. The Minor was only the
  staged IHDR test name in this report; it was corrected to
  `staged_client_accepts_the_locked_down_local_media_options` before the final
  gate, with no code change or re-review required.
- The single fresh post-review `pnpm verify` exited 0 end-to-end: repository
  policy 29/29, clean-wrapper 11/11, UI check, 92 UI test files with 851 passed
  and 1 intentional skip, production build, locked formatting, strict workspace
  Clippy, all workspace and doc tests, Tauri security 8/8, cargo-deny
  bans/licenses/sources, and npm licenses (140 packages / 11 expressions).
- Task 6 is ready for the requested commit
  `feat: cache video covers and timeline frames`; the resulting SHA is recorded
  in the controller handoff. Task 7 has not been started.
