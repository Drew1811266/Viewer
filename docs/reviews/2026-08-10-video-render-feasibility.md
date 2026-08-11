# macOS Native Video Render Feasibility Review

Date: 2026-08-10 (America/Los_Angeles)

Evidence generated: 2026-08-11T04:22:42.502Z

Command: `scripts/video/run-render-feasibility.sh`

Result: exit 0, all ten mandatory rows PASS

## Tested source

- Commit: `d09f79de6ea5f029ed19ce3f5d2ddc8834ca1148`
- Tree: `a0e9159c88a9b8e2b66e83ad237a0ad1155e7d25`
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

Initial callback counts are diagnostic, not fixed expectations. The VFR gate instead required strict relative increases 1 → 3 → 5 → 6 across the first forward, second forward, and backward renders. Its typed `time-pos` direction probe observed 66,667 → 133,333 → 66,667 µs.

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

## Signing gate

For each fixture build, the staged `libmpv.2.dylib` was ad-hoc signed, the debug app bundle was deep ad-hoc signed, then both gates ran successfully:

- Nested runtime: `codesign --verify --strict --verbose=4 .../libmpv.2.dylib`
- App bundle: `codesign --verify --deep --strict --verbose=4 .../Viewer.app`
- Logs: `target/video-render-feasibility/logs/verify-mpv-{h264-1080p,hevc-portrait,vfr-step}.log`
- Logs: `target/video-render-feasibility/logs/verify-app-{h264-1080p,hevc-portrait,vfr-step}.log`

The product release pipeline must preserve this coherence by signing the nested runtime before signing the outer app with the distribution identity; the feasibility run proves the bundle layout and verification sequence with an ad-hoc identity, not notarization.

## Preserved evidence

- Machine-readable result: `target/video-render-feasibility/matrix-result.json`
- H.264 first revealed: `target/video-render-feasibility/screenshots/h264-1080p-first-revealed.png` (`fb961295e1cd4324464e4af757920c2e7fb9ab325806b467cef6eea939f7c455`)
- H.264 Retina resize: `target/video-render-feasibility/screenshots/h264-retina-resize.png` (`5abab3361c1fa1957151a2cf000a8657120c43cb2d1edb2ae8daa73fcb7bba69`)
- HEVC first revealed: `target/video-render-feasibility/screenshots/hevc-portrait-first-revealed.png` (`aabafc9331ee5b0a7066bede8cee94b05e8f1196e6afe1254163fd93bab9bc6a`)
- VFR first revealed: `target/video-render-feasibility/screenshots/vfr-step-first-revealed.png` (`15e9ecfa2584c2bf245dd2c387782e5032376d63efa01c9bc80f076b231f2181`)
- VFR forward/backward: `target/video-render-feasibility/screenshots/vfr-forward-backward.png` (`5621617ad7caa555d0bb825857babf364d5ff4fb39590b41adbb512a38066049`)
- Lifecycle baseline: `target/video-render-feasibility/screenshots/lifecycle-30-baseline.png` (`c9e46856702e17edac5ea8544852d8bd2bb8ee30e8781bae07e957aceb786f98`)
- Native/build/test/signing logs: `target/video-render-feasibility/logs/`

Decision: PASS — product implementation may continue.
