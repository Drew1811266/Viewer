# macOS Native Video Render Feasibility Review

Date: 2026-08-10 (America/Los_Angeles)

Evidence generated: 2026-08-11T06:01:32.644Z

Command: `scripts/video/run-render-feasibility.sh`

Result: exit 0, all ten mandatory rows PASS

## Tested source

- Commit: `d9735d2ef1af1401fe41eca79740599981ce5e42`
- Tree: `49d8719667957ad4e74011abc1865d35f44ad633`
- Working tree at matrix start: clean
- Machine-readable binding: `target/video-render-feasibility/matrix-result.json` → `source`

The later documentation-only evidence commit changes this review, not the tested executable source.

## Test host

- Model: Mac16,12 (MacBook Air)
- Chip: Apple M4
- macOS: 26.5.2 (25F84)
- Kernel: Darwin 25.5.0, arm64

## Fixtures

The runner rejected any missing, extra, or hash-mismatched fixture before native build or launch.

| Fixture | Stream | SHA-256 |
| --- | --- | --- |
| `h264-1080p.mp4` | H.264, 1920×1080, yuv420p, 30 fps, 2.000000 s | `972aff59c7183940dbdfae2d906421a26604a10b6bd24432ccf969a028674296` |
| `hevc-portrait.mp4` | HEVC, 1080×1920, yuv420p, 30 fps, 2.000000 s | `4e7abaf98918f862ea07c5b529a143cd7f208c3bba7b807dab7995ac8103d0ac` |
| `vfr-step.mp4` | H.264 VFR, 960×540, yuv420p, average 1620/89 fps, 2.966667 s | `ee9ede284fabb52570fdd51428bf5408fad35de9ef8a4aeaefbcbc577cfcdde9` |

## Actual decoded-frame and native properties

The hidden surface was revealed only when a render update coincided with mpv's typed `video-frame-info/picture-type` property. All three first revealed frames reported picture type `I`. The runner captured and pixel-validated each first revealed surface before polling hardware/video-output properties or performing any UI action.

| Fixture | First decoded frame | `hwdec-current` | `current-vo` | Initial rendered frames | Active resources |
| --- | --- | --- | --- | ---: | --- |
| H.264 1080p | `I` | `videotoolbox` | `libmpv` | 2 | 1/1/1 |
| HEVC portrait | `I` | `videotoolbox` | `libmpv` | 1 | 1/1/1 |
| H.264 VFR | `I` | `videotoolbox` | `libmpv` | 1 | 1/1/1 |

Initial callback counts are diagnostic, not fixed expectations: this run observed 2 for H.264 and 1 for HEVC/VFR. The VFR gate instead required strict relative increases 1 → 3 → 5 → 6 across the first forward, second forward, and backward renders. Its typed `time-pos` direction probe observed 66,667 → 133,333 → 66,667 µs.

The H.264 first-revealed color-frame extent was 152..872 CSS px before resize and 212..812 CSS px after resize. Overlay pixel analysis in logical region x=404..620, y=488..540 measured a 0.6068 neutral-pixel ratio and 0.3611 bright-neutral ratio above the saturated native frame. The HEVC first-revealed pixel probe measured a 0.2803 saturated ratio across extent 152..872; H.264 and VFR measured 0.8725.

## Mandatory matrix

| Row | Result | Evidence |
| --- | --- | --- |
| surface-at-dom-rect | PASS | First-revealed decoded H.264 extent 152..872 matches the 720 px DOM slot. |
| retina-resize | PASS | Resized decoded extent 212..812 matches the 600 px DOM slot; backing conversion applies scale once. |
| react-overlay-z-order | PASS | AX overlay exists and PNG pixels show neutral React controls above saturated native video. |
| first-frame-ready | PASS | Reveal/readiness requires typed decoded picture type `I`; the immediately captured first visible frame passes its fixture pixel signature before backend polling. |
| frame-step-forward | PASS | Typed frame-step increased both VFR `time-pos` and the relative rendered-frame count. |
| frame-step-backward | PASS | Typed frame-back-step reduced VFR `time-pos` from 133,333 to 66,667 µs and increased the render count. |
| h264-videotoolbox | PASS | Actual `hwdec-current=videotoolbox`, `current-vo=libmpv`. |
| hevc-videotoolbox | PASS | Actual `hwdec-current=videotoolbox`, `current-vo=libmpv`. |
| no-ipc-frame-buffer | PASS | Static gate found no frame-buffer/image-data transport in the Rust route or React scene. |
| 30-mount-unmount-baseline | PASS | Every close/reopen checked 0/0/0 or 1/1/1; final baseline is 0/0/0. |

Resource counters before the matrix: `0/0/0`. Resource counters after 30 mount/unmount cycles: `0/0/0` (clients/render contexts/surfaces).

## Failure cleanup and bundle boundary

Fixture, app-resource, transparency, session-mismatch, action, diagnostics, frame-event, and cancelled-response errors all take/drop the registered session on the main queue. Main-queue submission uses the non-fallible dispatch queue API, so the former callback-scheduling error path no longer exists. Runtime layout resolution accepts only `app.path().resource_dir()`; no environment or absolute-directory production override remains.

The runner launches the signed `.app` through `/usr/bin/open -n -W`, records the complete pre-launch PID snapshot, and limits cleanup to post-baseline processes whose command begins with the exact current-worktree executable path. Cleanup stops every such process found before and after launcher teardown, retries a failed targeted stop, bounds both launcher and helper waits, and appends any cleanup failure to the primary matrix error. Process-table calls have an OS-enforced one-second `SIGKILL` timeout plus an asynchronous lifecycle deadline, so a hung `ps` cannot prevent the already attributed Viewer, launcher, or helper from reaching teardown. Multiple candidates and process-table failures therefore cannot silently leak a LaunchServices-owned app or suppress a failing exit.

The helper validates the exact CG owner PID, window ID, and full frame before focus preparation. It activates only that PID through `NSRunningApplication` and a bounded `/usr/bin/osascript` child; it does not inject a global title-bar click. A hung activation child is terminated, escalated to `SIGKILL`, and reaped within a deadline reserved below the enclosing protocol timeout. The helper revalidates the unchanged full CG frame during and after activation, then requires both matching AXWindowNumber (when present) and matching AX geometry before it raises or focuses the target. This ordering handles a background Tauri window whose AX hierarchy is unavailable before activation without weakening PID, window-ID, geometry, or selector boundaries.

## Signing gate

For each fixture build, the staged `libmpv.2.dylib` was ad-hoc signed, the debug app bundle was deep ad-hoc signed, then both gates ran successfully:

- Nested runtime: `codesign --verify --strict --verbose=4 .../libmpv.2.dylib`
- App bundle: `codesign --verify --deep --strict --verbose=4 .../Viewer.app`
- Logs: `target/video-render-feasibility/logs/verify-mpv-{h264-1080p,hevc-portrait,vfr-step}.log`
- Logs: `target/video-render-feasibility/logs/verify-app-{h264-1080p,hevc-portrait,vfr-step}.log`

The product release pipeline must preserve this coherence by signing the nested runtime before signing the outer app with the distribution identity; the feasibility run proves the bundle layout and verification sequence with an ad-hoc identity, not notarization.

## Preserved evidence

- Machine-readable result: `target/video-render-feasibility/matrix-result.json`
- H.264 first revealed: `target/video-render-feasibility/screenshots/h264-1080p-first-revealed.png` (`ec9883588214f061cfdfbfb640823c8772fdf37288111e5ddf764561b382427f`)
- H.264 Retina resize: `target/video-render-feasibility/screenshots/h264-retina-resize.png` (`6a996cef44a752e3dd02fda3b31f4652a7eccd57a825b0b16092b59b322b8c9c`)
- HEVC first revealed: `target/video-render-feasibility/screenshots/hevc-portrait-first-revealed.png` (`f7f46941606b7ad4f7fc6b0a4057e41e0724d5d1be87e41d53d8f611d3f50f80`)
- VFR first revealed: `target/video-render-feasibility/screenshots/vfr-step-first-revealed.png` (`8903d032552314b59754780d7eef667ec9c8ff8c6190583cf98a70998c77638c`)
- VFR forward/backward: `target/video-render-feasibility/screenshots/vfr-forward-backward.png` (`82672254c0a91f7e62c8c51779185e644cee790dda45ea3c487e2c0ad54b2c90`)
- Lifecycle baseline: `target/video-render-feasibility/screenshots/lifecycle-30-baseline.png` (`b46cf8ce4076e3d4bf569c7b13fd8b897f014ca686c2e8524acc266ec49b6918`)
- Native/build/test/signing logs: `target/video-render-feasibility/logs/`

Decision: PASS — product implementation may continue.
