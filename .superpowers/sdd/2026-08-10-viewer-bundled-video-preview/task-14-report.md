# Task 14 implementation report

Status: native VFR no-FRAME fix implemented; focused non-native checks are green.
Task 14 is not marked complete by this report. No commit has started.

## Review fix round 1

Reviewer findings 0C/3I/2M were verified against the executable data flow.
The preserved matrix and generation diagnostic carried only commit/status and
timestamps, not the dirty tracked bytes, untracked Task 14 files, or a shared
machine/fixture/app/runtime/run identity. They cannot be rebound after the fact,
so the prior core development PASS and its numeric latency/performance claims
are superseded by `UNVERIFIED` until the next native run.

Schema 3 now binds matrix, generation diagnostic, and strict smoke artifact to
the same run, current source identity (HEAD plus binary tracked diff hash plus
untracked path/mode/content manifest), machine, fixture IDs/hashes, executable,
runtime inventory, and runtime lock. The collector rejects every mismatch,
requires each passed row to use that exact run ID, verifies the smoke artifact
hashes both native inputs, and has no timestamp or environment-bit fallback.

The lifecycle scene now samples its real closed baseline and performs 30
alternating H.264/HEVC navigation cycles. Each mount must be exactly baseline+1
and each close exactly baseline for the only instrumented live resources:
clients, render contexts, and surfaces. Decoder/audio have no live counters in
this gate. Worker cancellation and artifact revocation remain independently
covered by `setup_failure_cancels_and_awaits_worker_before_session_teardown`,
`project_close_closes_video_before_artifact_revoke_and_session_close`, and
`close_failure_still_resets_session_and_revokes_thumbnail_artifacts`; they are
not described as 30-cycle measurements.

Task 13's bundle verifier now has a shared `development|signed` audit. Both
modes require exact runtime inventory/hashes, architecture equality, loader and
rpath containment, Apple-only absolute dependencies, and offline bundled
ffprobe. Signed mode additionally retains nested/deep signature, Developer ID
plus timestamp, stapler, and `spctl` checks. Development smoke runs this audit
before launch and emits the only collector-accepted bound smoke artifact.

Source-grep tests in the touched provenance, smoke, lifecycle, and performance
sequencing scope were replaced with executable temporary artifacts or direct
behavior tests. The unsupported 4K latency statement and unbound historical
numeric performance assertions were removed from current acceptance docs.

Focused RED/GREEN ledger for this round:

- provenance collector: RED 1/8 passing, GREEN 8/8;
- lifecycle React behavior: RED 0/1, GREEN 1/1;
- strict bundle audit behavior: RED 0/7, GREEN 7/7;
- smoke behavior: GREEN 3/3 after the shared audit integration;
- performance generator behavior: GREEN 2/2 against executable fake tools;
- Rust evidence consumer: RED 2/3, GREEN 3/3.

No native application, application build, full verification, review, or commit
was run in this fix scope.

## Review fix round 2

Skip-build no longer pairs current source metadata with an arbitrary stale app.
A non-skip run writes an atomic build attestation only after build, final nested
runtime signing, inventory refresh, and final app signing. It binds the exact
source identity and digest, executable, inventory, lock and inventoried runtime
files, bundle identity, and build configuration. Skip-build verifies all those
current identities before native work; missing or stale source/app/runtime
attestations reject.

Every default and skip-build launch now follows a shared strict development
bundle audit. The verifier atomically emits a machine artifact after the audit;
the runner verifies that artifact against the build attestation and binds both
digests into matrix/diagnostic evidence. Development smoke consumes that exact
artifact and rejects missing, stale, or post-audit-mutated bundles. It does not
mint audit booleans. Signed-mode Developer ID, timestamp, stapler, and `spctl`
checks remain fail-closed.

The collector now requires all 30 adjacent lifecycle fixture IDs to differ, so
a grouped 29+1 path rejects. Remaining render-runner source-text assertions
were removed in favor of real temp repository/app/runtime attestation tests,
injected ordering behavior, and exported performance fixture sequencing.

Round-2 RED/GREEN ledger:

- build attestation/orchestration: RED module absent, GREEN 7/7;
- exact lifecycle alternation: RED grouped 29+1 accepted, GREEN rejected;
- verifier audit artifact: RED usage exit 64, GREEN focused 1/1;
- smoke artifact consumption: RED 0/2, GREEN 2/2.

No native application, application build, full verification, review, or commit
was run in round 2.

## Review fix round 3

Offline evidence no longer originates from `NETWORK_DISABLED=1` or an emitter
boolean. The actual runner launch boundary selects either the canonical macOS
`sandbox-exec` deny-network profile or the ordinary unrestricted `open` path.
Only the sandbox path can produce an offline launch artifact, and only after
the exact attested/audited app has spawned, its exact target process is
attributed, and its stable native window is bound.

The atomic artifact binds run and source identity digest, build-attestation and
bundle-audit digests, current app/runtime and bundle identities, machine,
canonical sandbox profile bytes/hash, exact launch executable/arguments,
launcher PID, target PID, and window identity. Matrix and diagnostic bindings
carry its digest. Smoke only reads and verifies the artifact; the collector and
Rust consumer require `networkPolicy: deny-all`. Missing/stale run, source,
build, audit, app/runtime, profile, process/window, or artifact digest rejects,
as does an unrestricted launch even when a legacy true bit is present.

This is evidence of launching the bound app through the macOS sandbox
deny-network command boundary. It is not described as independent kernel
packet-capture proof.

Round-3 RED/GREEN ledger:

- network launch artifact: RED module absent, GREEN behavior fixtures cover the
  fake sandbox/open executables and stale mutations;
- smoke emitter: RED could mint offline true without a launch artifact, GREEN
  requires and copies the verified artifact;
- collector: RED accepted unrestricted launch plus legacy true, GREEN rejects;
- Rust consumer: RED rejected the new valid fixture before understanding the
  launch digest/policy, GREEN validates both.

No native application, application build, full verification, review, or commit
was run in round 3.

## Native VFR no-FRAME fix

The bound development run `2026-08-13T07:45:46.718Z` passed initial H.264
generation 1 and HEVC generation 2, then stalled while switching to VFR
generation 3. The stalled generation reached mount return, one render-context
update callback, and one main-thread draw entry, but produced no
`MPV_RENDER_UPDATE_FRAME`, decoded picture, reveal, or frame event for more
than ten seconds. Its client/render-context/surface counters remained exactly
`1/1/1`. This places the failure between libmpv's asynchronous file load and
its first frame update, rather than in the React listener, acceptance polling,
generation ownership, or cleanup.

Both the normal macOS adapter and feature-only feasibility route previously
issued synchronous `loadfile` and then immediately set `pause=true`. libmpv
documents that `loadfile` returns before the previous file has stopped and the
new file has finished loading. Therefore those two calls did not establish the
intended "new file opened paused" ordering: pause could take effect before the
new file decoded its initial frame. That race explains the observed single
non-FRAME wake and the historical intermittency across cold and VFR switches.

A stateful client-contract fake makes the asynchronous boundary deterministic.
It models a pre-paused load as producing the initial decoded frame, and a
load-then-pause sequence as pausing before that deferred decode. After a typed
`open_local_file_paused` stub was added with the old ordering, the focused test
failed with decoded picture `None` instead of `Some("I")`. The production method
now validates and initializes the client, sets pause before `loadfile`, and then
loads the canonical local file. `MacVideoRenderSession`, the normal
`MacOsLibmpvAdapter`, and the feasibility route all consume that one typed
operation. Render-context-before-load ordering, first-frame hiding/reveal,
generation cleanup, and `hwdec=auto-safe` are unchanged.

Focused RED/GREEN ledger:

- missing typed API: expected compile RED;
- old `loadfile` then pause implementation: behavioral RED, decoded picture
  `None` versus expected `Some("I")`;
- corrected pause-before-load implementation: client contract 2/2 PASS;
- platform first-frame gate: focused 1/1 PASS;
- feature-gated feasibility route: focused 10/10 PASS;
- Rust formatting and `git diff --check`: PASS.

No app build, native rerun, full matrix, full repository verification, review,
or commit was run in this fix scope. Native confirmation remains pending the
explicit post-focused-test approval gate.

## User-scoped decision boundary

The current goal is functional development completion and normal focused tests,
not formal release acceptance. Developer ID signing, notarization, stapling,
`spctl`, a signed clean-machine run, and full repository/build verification are
deferred. No evidence below promotes them to PASS.

AV1 decode and 4K performance remain `UNVERIFIED`. The preserved native
artifacts cannot establish current readiness because they predate the source,
app/runtime, machine, fixture, and run binding contract.

## Superseded pre-review record

The chronology below records how the review findings were discovered. Its
unbound native outcomes are historical context, not current acceptance evidence.

## Fixture and acceptance implementation chronology

1. Added the nine-entry schema-2 fixture manifest, bundled-runtime generator,
   deterministic embedded VP9/AV1 seeds, hashes, provenance, and redistribution
   declarations. The generator uses only the reviewed bundled tools and rejects
   hash, codec, rotation, VFR, damaged, or unsupported-shape drift.
2. Added Rust fixture-matrix, performance, lifecycle, and shared evidence
   consumers plus the development/signed smoke contract. Performance samples are
   generated sequentially with hardware encoders and do not fall back to host
   tools.
3. Extended the feature-gated feasibility route with typed play/pause latency,
   libmpv dropped/mistimed counters, 1080p60/4K fixtures, and generation-bound
   mount/update/draw/FRAME/picture/reveal/event diagnostics. The typed mpv
   properties have a client-contract test; no product bridge or arbitrary
   property API was added.
4. Native evidence established H.264 and HEVC VideoToolbox first frames, VFR
   forward/backward movement, a real timeline preview, 30-cycle exact cleanup,
   and H.264 1080p60 performance. The originating matrix run ID for these rows is
   `2026-08-13T04:00:26.091Z`.
5. A dedicated generation diagnostic recorded initial and reopened HEVC 4K30
   stages, but it predates the binding contract and is not current evidence.
6. The feasibility React scene originally registered its listener and mounted
   concurrently. A frame-ready event could be missed or overwritten by an older
   false mount snapshot; late old-fixture work could also republish. A deferred
   listener/mount behavior test failed before the fix, then passed after listener
   registration was awaited, scene-local ownership was checked, and same-fixture
   readiness became monotonic.
7. One later 4K-only resume failed in the native acceptance helper while
   activating Viewer, before media selection. It retained inherited row run IDs,
   left `hevc-4k30-performance` false with a null run ID, and collected no 4K
   performance sample. This acceptance-infrastructure failure does not negate the
   separate 4K functional diagnostic.

## Scoped evidence schema TDD

The first Node contract run failed 0/4 because the original collector required
the false 4K row to be true and had no per-sample status or diagnostic binding.
The final Node suite passes 4/4 and proves:

- missing or false required functional/lifecycle/H.264 performance rows reject
  the document;
- every passed required row retains a non-empty source run ID;
- only `hevc-4k30-performance` may be `unverified`;
- unverified 4K performance requires an explicit false row, null source run,
  absent measurement, the activation-before-media failure, and complete native
  initial/reopen diagnostic evidence.

The Rust RED compile failed because no parsing/validation function existed. The
Rust GREEN suite passes 2/2 and independently proves a missing/false core field
fails and H.264 cannot use the 4K-only `unverified` allowance.

The former schema-2 target output is rejected by the schema-3 collector and is
not current evidence. Its unbound performance and lifecycle values are not
reported as acceptance results.

The ignored performance and lifecycle consumers previously accepted those
unbound values. They now require schema-3 evidence and have not been run against
a current artifact; only their parser behavior is claimed in this review fix.

## Smoke and release separation

Development smoke now runs the shared strict inventory/hash/architecture/loader
audit before its network-denied restricted-`PATH` launch. It emits a bound
artifact for the collector and was not run during this review fix.

Signed mode remains fail-closed: runtime inventory, Developer ID identity,
strict nested/app signature, stapler, and `spctl` remain required. No signed mode
behavior was weakened.

## Diagnostic instrumentation decision

The generation diagnostic is retained because it is restricted to the
`video-feasibility` feature, covered by Rust/UI behavior tests, and provides the
auditable 4K functional source needed to distinguish readiness from the missing
dropped-frame window. Typed latency and dropped/mistimed properties remain in
the native wrapper because the Task 14 performance consumer needs them and the
public wrapper still exposes no arbitrary property strings.

## Documentation

`docs/reviews/2026-08-10-video-preview-acceptance.md` maps all 12 approved design
criteria to commands or named evidence, includes every fixture hash and the
performance/status table, and records development UNVERIFIED/release-signing
DEFERRED. The native smoke matrix, product spec, requirement scope matrix,
technical foundations, and documentation index now carry the same boundary.

## Focused verification ledger

- Node collector behavior: PASS, 4/4.
- Rust evidence parser behavior: PASS, 2/2.
- Generated evidence collector against preserved artifacts: superseded; the
  preserved schema-2 artifact is rejected and no current schema-3 artifact exists.
- Ignored performance and lifecycle consumers: not run against native evidence;
  their parser behavior is covered by focused tests only.
- Fixture generator `--verify`: PASS with bundled tools.
- Fixture matrix: PASS, 1/1; eight required decode/failure outcomes passed and
  AV1 reported probe PASS with decode UNVERIFIED/hardware-dependent.
- Documentation policy/scope: PASS, 29/29 policy tests and all 48 requirement
  IDs mapped exactly once.
- Feature-gated native contracts: PASS, 1/1 typed mpv client contract and 10/10
  feasibility-route unit tests.
- Feasibility acceptance UI: PASS, 7/7.
- UI check: PASS across 246 files plus TypeScript; the existing Biome
  `recommended` deprecation is informational.
- Rust format, shell syntax, documentation decision endings, and `git diff
  --check`: PASS.
- Developer smoke: not run by explicit scope.
- Native app/build/full `pnpm verify`: not run by explicit scope.
- Release signing/notary/stapler/`spctl`: deferred by user scope.

## Pre-review concerns

- The 4K30 dropped-frame window still needs a future stable acceptance-controller
  run on an appropriate reference machine before it can become PASS.
- AV1 hardware decode needs a machine with the required hardware capability.
- Formal release work must run the unchanged signed-mode checks and a signed
  clean-machine acceptance; current core development evidence is UNVERIFIED.

## Bound native acceptance and performance reset correction

After the reviewed runtime was rebuilt against the current lock, the fresh
bound development run `2026-08-13T08:03:46.973Z` passed the strict bundle
attestation/audit, deny-network sandbox launch, H.264/HEVC VideoToolbox paths,
VFR forward/backward frame steps, production timeline, and thirty alternating
H.264/HEVC lifecycle cycles with the measured resource baseline restored.

That run stopped before performance measurement because the performance helper
used an unrelated HEVC fixture as an intermediary merely to recreate the
current H.264 session. The intermediary never reached first-frame readiness;
therefore neither H.264 nor 4K performance was claimed and no schema-3
development evidence was emitted.

The acceptance-only reset now exposes one explicit “重新挂载当前样本” operation.
It serializes `close` then `mount` for the unchanged fixture, preserves the
generation/event guard, and closes a late mount if React cleanup invalidates the
operation. The runner removes all intermediary fixture switching and waits for
the native diagnostic generation to increase before accepting the remounted
first frame. A controlled React test first failed because the action did not
exist, then passed with the exact same-fixture `close`/`mount` sequence. The
pure runner plan test proves both H.264 and 4K retain their own fixture IDs.

Focused correction verification:

- `videoFeasibilityScene.test.tsx`: PASS, 8/8.
- `build-attestation.test.mjs`: PASS, 10/10.
- `node --check scripts/video/render-feasibility.mjs`: PASS.
- `pnpm --dir ui check`: PASS across 246 files plus TypeScript.
- `git diff --check`: PASS.

No native app rebuild/rerun, ignored evidence consumer, full `pnpm verify`, or
commit is claimed by this correction checkpoint.

The scoped review of that checkpoint found two real generation races. A late
invalidated remount could close a newer same-fixture session because native
close ownership used only the fixture ID, and an old same-fixture frame event
could temporarily satisfy the reset checks. It also found that performance
evidence reused the backend strings sampled before the reset.

The follow-up RED tests reproduced both boundaries: an old generation restored
`firstFrameReady` while generation 8 was pending, the late cleanup request had
no native generation, and the desktop test could not import a generation
ownership predicate. The GREEN implementation adds an optional
`expectedGeneration` only to the feature-gated feasibility request. A guarded
close whose expected generation no longer owns `ACTIVE_SESSION` is a successful
no-op, so the generic error cleanup cannot tear down the successor. React tracks
the minimum accepted native generation, rejects lower same-fixture reports, and
sends the exact generation returned by a late mount when cleaning it up.

The runner now re-reads generation diagnostics, first-frame/I-frame state,
resource counts, rendered frames, VideoToolbox, and libmpv after the same-sample
remount. A pure validation function requires all observations to belong to the
strictly newer generation; the performance row uses these post-reset backend
values rather than the earlier session.

Fresh focused results after this review fix:

- feasibility React behavior: PASS, 10/10, including stale same-fixture event
  rejection and generation-owned late cleanup;
- runner/attestation/assertion behavior: PASS, 16/16;
- feasibility Rust behavior: PASS, 11/11, including generation-owned close;
- UI check, Rust format, Node syntax, and diff check: PASS.

The second scoped review closed the report/evidence finding but found one final
ownership path: close still performed fallible fixture/bundle/window setup
inside the generic “error clears session” wrapper, and its mismatch branch used
fallible live diagnostics. A stale close could therefore still clear its
successor before or during the guard.

Close is now dispatched before all bundle/window setup and outside that generic
cleanup wrapper. A generation mismatch returns an infallible snapshot assembled
only from stored session state and atomics; it cannot touch the active session.
If the IPC receiver disappears, cleanup similarly compares the exact expected
generation before removing `ACTIVE_SESSION`. The focused native ownership test
covers both guarded-close ownership and cancelled-command ownership. Fresh
post-fix results remain Rust 11/11, UI 10/10, Node 16/16, UI check/format/syntax/
diff all green. No native/full run is claimed at this point.

## Final development-scope native result

The single post-review default-build run `2026-08-13T08:28:27.758Z` passed the
source/app/runtime attestation, strict bundle audit, canonical deny-network
sandbox launch, DOM geometry, Retina resize, React overlay z-order, first-frame
gate, forward/backward VFR steps, H.264 and HEVC VideoToolbox paths, no-frame-
buffer IPC boundary, production timeline, and thirty alternating lifecycle
cycles with the measured clients/render-contexts/surfaces baseline restored
from `0/0/0` to `0/0/0`.

The run stopped in H.264 performance sampling because the post-warmup rendered
delta was zero, producing `NaN%`; 4K performance therefore did not run. Per the
user-approved development-stage scope, both performance rows are recorded as
UNVERIFIED rather than retried or promoted. AV1 hardware decode and formal
release signing remain UNVERIFIED/DEFERRED. Core video functionality is accepted.

## Final verification and handoff

The final scoped reviewer closed the stale-generation close ownership fix with
`0 Critical / 0 Important / 0 Minor` and approved the development-scope change.

The first final `pnpm verify` run found one acceptance-only accessibility
contract failure: the feasibility button's visible `↻` glyph was not in the
audited icon registry. The visible label was changed to `重挂` while retaining
the accessible name `重新挂载当前样本`; the exact accessibility and feasibility
tests then passed 14/14 and `pnpm --dir ui check` passed.

The single necessary full retry of `pnpm verify` exited 0. It passed policy and
scope checks, the clean verifier, UI checks, all 105 UI test files (941 passed,
1 skipped), the 188-module production build, locked Rust formatting and strict
workspace Clippy, all workspace tests and doc tests, the 8 Tauri security tests,
`cargo-deny` bans/licenses/sources, npm license policy, and the bundled video
runtime license check. Only the repository's existing informational Biome
deprecation and allowed duplicate-dependency warnings remained.

No formal signing, notarization, stapling, or `spctl` release acceptance is
claimed. H.264/4K performance metrics and AV1 hardware decode remain explicitly
non-blocking UNVERIFIED items under the user-approved development-stage scope.
