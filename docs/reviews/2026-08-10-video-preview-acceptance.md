# Bundled Video Preview Development Acceptance

Date: 2026-08-12 (America/Los_Angeles)

Scope: development functionality and verification. Developer ID signing,
notarization, stapling, `spctl`, a signed clean-machine run, AV1 hardware decode,
and a fresh 4K30 dropped-frame window are outside this decision. A false result
is never counted as a pass; the only allowlisted `unverified` performance row is
`hevc-4k30`.

## Evidence binding

- Current bound functional run: `2026-08-13T08:28:27.758Z`. Its matrix binds the
  exact dirty source manifest, final app/runtime build attestation
  `7d4edf204a4c…`, strict development bundle audit `5dc6e7cf74a8…`, and canonical
  deny-network sandbox launch `42edf84d9e5e…`.
- A future non-skip run atomically writes a build attestation only after build,
  final runtime signing, inventory refresh, and final app signing. Skip-build
  runs must match its exact source, executable, runtime files, inventory, lock,
  bundle identity, and build configuration before any native work.
- Machine-readable inputs: `target/video-render-feasibility/matrix-result.json`
  and `target/video-render-feasibility/task14-generation-diagnostic.json`. A
  current native run writes the same binding and build/audit/launch digests
  into both.
- Strict smoke input: `target/video-acceptance/development-smoke.json`, which
  consumes the shared verifier's machine artifact; it cannot mint audit flags.
  The artifact binds the final bundle identity to the build attestation and both
  native input files.
- Offline status additionally requires the runner-produced network-launch
  artifact. It is written only after the exact audited app is spawned through
  the canonical macOS `sandbox-exec` deny-network profile and the target process
  and stable window are bound. It records the profile bytes/hash, launch
  executable/arguments, process/window identities, machine, run, source, build,
  audit, app, and runtime bindings. This proves the sandbox launch boundary; it
  does not claim independent kernel packet-capture evidence.
- Derived, target-only evidence:
  `target/video-acceptance/development-evidence.json` (schema 3).

The collector rejects stale or mismatched source bytes, machine, fixture,
app/runtime, run, row, diagnostic, and smoke bindings. It has no timestamp or
environment-variable fallback. The current run proves the core native function
rows, but intentionally emits no all-green development evidence because both
performance rows remain unverified.

## Fixture inventory

`VIEWER_VIDEO_RUNTIME_DIR="$PWD/target/task6-review-resources/ViewerVideoRuntime"
scripts/video/generate-test-fixtures.sh --verify` verifies these
redistribution-safe synthetic fixtures with the bundled `ffprobe`.

| Fixture | Contract | SHA-256 | Current status |
| --- | --- | --- | --- |
| `h264-aac.mp4` | MP4, H.264/AAC | `8b71e9463fa04b38548ae06307537a7d0e8896f7d2b2c2793b66557b2d1f60a8` | Contract covered; native decode UNVERIFIED |
| `hevc-portrait.mov` | MOV, HEVC, rotation 90° | `b8b11ada2c72524ba11250763a633239291f93ae603256f88a24ac54bfdb0fe1` | Contract covered; native decode UNVERIFIED |
| `prores.mov` | MOV, ProRes | `de22b6775e783b980901377ab214189e0019b2e3c34635821360dd3825c70651` | Contract covered; native decode UNVERIFIED |
| `vp9-opus.webm` | WebM, VP9/Opus | `b850f2783deb533336c252a10600b2cf8bdf70ac2a11bb0d496eab888c895001` | Contract covered; native decode UNVERIFIED |
| `av1.mkv` | MKV, AV1 | `d6f0f096a1a2ebb2ebf9366e45f0263698200adf356e85bc698b3bfa9322b100` | Contract covered; native decode UNVERIFIED |
| `vfr.mp4` | MP4, variable frame rate | `c30c7818caac334e4ecbb0084fa5f4ce9cfc29d5b4518a0fdb829c19a0d04ba6` | Contract covered; native behavior UNVERIFIED |
| `silent.mp4` | MP4, no audio | `1f2a8a4c10ea9a1c094050004c45dd2115f9f355524f3d9d68b6985c25b6f36b` | Contract covered; native behavior UNVERIFIED |
| `truncated.mp4` | MP4, generated truncation | `faa80af460b0947f45e83de40e2a953544c7b0b70dc462696a1e1341e957287f` | Normalization contract covered; native matrix UNVERIFIED |
| `unsupported-codec.mkv` | MKV, unsupported codec ID | `820e36bd6f267dfd6e4632d3cbbd37e7771886487b2328e6ef2efcd1daaccd94` | Normalization contract covered; native matrix UNVERIFIED |

The ignored matrix consumer is:

```sh
VIEWER_VIDEO_RUNTIME_DIR="$PWD/target/task6-review-resources/ViewerVideoRuntime" \
  cargo test -p viewer-desktop --test video_fixture_matrix \
  fixture_matrix_matches_manifest_expectations -- --ignored --nocapture
```

## Native performance and lifecycle

| Sample | Status | Codec/path | Command latency | Post-warmup window | Dropped frames | Source run |
| --- | --- | --- | ---: | ---: | ---: | --- |
| 1920×1080@60 | UNVERIFIED | Bound remount reached the native session, but the sampled rendered-frame delta was zero (`NaN%`) | — | no valid window | — | `2026-08-13T08:28:27.758Z` |
| 3840×2160@30 | UNVERIFIED | Not run after the H.264 performance precondition failed | — | — | — | none |

No dropped-frame or 4K latency claim is made. These performance targets are
non-blocking under the user-approved development-stage scope.

`VIEWER_VIDEO_ACCEPTANCE_EVIDENCE="$PWD/target/video-acceptance/development-evidence.json"
cargo test -p viewer-desktop --test video_performance_gate
common_local_samples_meet_native_playback_budgets -- --ignored --nocapture`
will validate the per-sample status after a schema-3 artifact exists. No such
current artifact exists, so this consumer is not presently claimed as passed.

The current native run sampled the actual starting diagnostics, alternated
H.264 and HEVC fixtures for all 30 navigation cycles, and returned measured
client/render-context/surface counts from `0/0/0` to `0/0/0`. Those are the only
live resources instrumented by this gate. Worker cancellation and artifact
revocation are covered independently by
`setup_failure_cancels_and_awaits_worker_before_session_teardown`,
`project_close_closes_video_before_artifact_revoke_and_session_close`, and
`close_failure_still_resets_session_and_revokes_thumbnail_artifacts`; they are
not claimed as 30-cycle measurements. Decoder/audio ownership has no live
counter in this gate and is not claimed.

## Twelve design criteria

| # | Development status | Command or named evidence |
| ---: | --- | --- |
| 1 | PASS for required development codecs; AV1 hardware decode UNVERIFIED | Bound run proves H.264 and HEVC VideoToolbox first frames; fixture contracts cover the broader matrix. |
| 2 | PASS | Browse/card contracts and bound native first-frame/geometry rows pass. |
| 3 | PASS | Bound first-frame, Retina resize, DOM rect, and overlay rows pass with generation-safe publication. |
| 4 | PASS for H.264/HEVC; AV1 and performance UNVERIFIED | Bound H.264/HEVC VideoToolbox rows pass. |
| 5 | PASS | Bound VFR forward/backward frame steps and production timeline pass; controls/shortcuts remain covered by UI tests. |
| 6 | PASS | Video service reopen/reset, end-state, no-persistence, and video-only navigation tests. |
| 7 | PASS | Cover selection, cache identity/invalidation, generation-safe probe scheduling, and browse-publication tests. |
| 8 | PASS | Cache/concurrency contracts and the bound production timeline row pass. |
| 9 | PASS | Fixture contracts plus retry/error-card/normalization tests pass; AV1 hardware decode remains explicitly UNVERIFIED. |
| 10 | PASS for measured resources | Bound run completes 30 alternating H.264/HEVC cycles and restores clients/render contexts/surfaces from `0/0/0` to `0/0/0`; other close tests remain independent evidence. |
| 11 | DEFERRED for release signing | Runtime/license/bundle layout audits from Task 13 remain applicable. Developer ID signing, notarization, stapling, `spctl`, and signed clean-machine execution are not claimed. |
| 12 | PASS | Platform-neutral `VideoEngine` contract/fake-adapter tests and architecture checks; macOS/libmpv types stay below the adapter boundary. |

## Scope notes

- The developer smoke runs the strict inventory/hash/architecture/loader audit
  before launch, then requires the runner-produced macOS sandbox launch artifact
  for a network-denied restricted-PATH launch. A legacy environment bit or
  emitter boolean cannot satisfy this gate. It was not run in this review-fix
  scope.
- Signed mode remains fail-closed and retains runtime inventory, Developer ID,
  strict nested/app signature, stapler, and `spctl` checks for future release
  work.
- Diagnostic generation counters are retained only in the
  `video-feasibility` feature. They are reviewer-useful because they bind 4K
  mount, FRAME, picture, reveal, and event publication to one generation.

Development decision: PASS — core bundled video preview functionality is complete and tested; H.264/4K performance metrics and AV1 hardware decode remain non-blocking UNVERIFIED items.
Release signing acceptance: DEFERRED by user scope.
