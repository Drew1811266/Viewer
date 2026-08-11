# macOS Native Video Render Feasibility Review

Date: 2026-08-10 (America/Los_Angeles)

Evidence generated: 2026-08-11T03:48:19.954Z

Command: `scripts/video/run-render-feasibility.sh`

Result: exit 0, all ten mandatory rows PASS

## Test host

- Model: Mac16,12 (MacBook Air)
- Chip: Apple M4
- macOS: 26.5.2 (25F84)
- Kernel: Darwin 25.5.0, arm64

## Fixtures

| Fixture | Stream | SHA-256 |
| --- | --- | --- |
| `h264-1080p.mp4` | H.264, 1920×1080, yuv420p, 30 fps, 2.000000 s | `972aff59c7183940dbdfae2d906421a26604a10b6bd24432ccf969a028674296` |
| `hevc-portrait.mp4` | HEVC, 1080×1920, yuv420p, 30 fps, 2.000000 s | `4e7abaf98918f862ea07c5b529a143cd7f208c3bba7b807dab7995ac8103d0ac` |
| `vfr-step.mp4` | H.264 VFR, 960×540, yuv420p, average 1620/89 fps, 2.966667 s | `ee9ede284fabb52570fdd51428bf5408fad35de9ef8a4aeaefbcbc577cfcdde9` |

## Actual native properties

These values are queried from mpv's runtime properties after decoded fixture frames, not copied from the configured `auto-safe` preference.

| Fixture | `hwdec-current` | `current-vo` | Initial rendered frames | Active resources |
| --- | --- | --- | ---: | --- |
| H.264 1080p | `videotoolbox` | `libmpv` | 2 | 1/1/1 |
| HEVC portrait | `videotoolbox` | `libmpv` | 2 | 1/1/1 |
| H.264 VFR | `videotoolbox` | `libmpv` | 2 | 1/1/1 |

The VFR direction probe observed `time-pos` move 66,667 → 133,333 → 66,667 µs. The forward and backward renders reached 4 and 8 callbacks respectively. The H.264 color-frame extent was 152..872 CSS px before resize and 212..812 CSS px after resize. Overlay pixel analysis in logical region x=404..620, y=488..540 measured a 0.6068 neutral-pixel ratio and 0.3611 bright-neutral ratio above the saturated native frame.

## Mandatory matrix

| Row | Result | Evidence |
| --- | --- | --- |
| surface-at-dom-rect | PASS | Decoded color extent 152..872 matches the 720 px DOM slot. |
| retina-resize | PASS | Resized decoded extent 212..812 matches the 600 px DOM slot; backing conversion applies scale once. |
| react-overlay-z-order | PASS | AX overlay exists and PNG pixels show the neutral React controls above saturated video. |
| first-frame-ready | PASS | Readiness advances only after a post-load draw; the pre-load render-context wake remains hidden. |
| frame-step-forward | PASS | Typed frame-step advanced VFR `time-pos` and rendered-frame count. |
| frame-step-backward | PASS | Typed frame-back-step reduced VFR `time-pos` from 133,333 to 66,667 µs. |
| h264-videotoolbox | PASS | Actual `hwdec-current=videotoolbox`, `current-vo=libmpv`. |
| hevc-videotoolbox | PASS | Actual `hwdec-current=videotoolbox`, `current-vo=libmpv`. |
| no-ipc-frame-buffer | PASS | Static gate found no frame-buffer/image-data transport in the Rust route or React scene. |
| 30-mount-unmount-baseline | PASS | Every close/reopen checked 0/0/0 or 1/1/1; final baseline is 0/0/0. |

Resource counters before the matrix: `0/0/0`. Resource counters after 30 mount/unmount cycles: `0/0/0` (clients/render contexts/surfaces).

## Signing gate

For each fixture build, the staged `libmpv.2.dylib` was ad-hoc signed, the debug app bundle was deep ad-hoc signed, then both gates ran successfully:

- Nested runtime: `codesign --verify --strict --verbose=4 .../libmpv.2.dylib`
- App bundle: `codesign --verify --deep --strict --verbose=4 .../Viewer.app`
- Logs: `target/video-render-feasibility/logs/verify-mpv-{h264-1080p,hevc-portrait,vfr-step}.log`
- Logs: `target/video-render-feasibility/logs/verify-app-{h264-1080p,hevc-portrait,vfr-step}.log`

The product release pipeline must preserve this coherence by signing the nested runtime before signing the outer app with the distribution identity; the feasibility run proves the bundle layout and verification sequence with an ad-hoc identity, not notarization.

## Preserved evidence

- Machine-readable result: `target/video-render-feasibility/matrix-result.json`
- H.264 initial: `target/video-render-feasibility/screenshots/h264-1080p.png` (`4f67e4d8e7eb0aa8da3a4969a666585bea5723e5ebb3dccdcd812293c9d80ee1`)
- H.264 Retina resize: `target/video-render-feasibility/screenshots/h264-retina-resize.png` (`b0c3cde4aaf486cb705bae30a0cd36f170426efa14d781343d4089bf1ccb50ad`)
- HEVC portrait: `target/video-render-feasibility/screenshots/hevc-portrait.png` (`11df6063cd869afc5404111315fc6de5c6222ac9cd864edf1e1a6f60696442fb`)
- VFR forward/backward: `target/video-render-feasibility/screenshots/vfr-forward-backward.png` (`6617b7ed051ec34ebce0d3d91856c082a8ea91c4199ec568a2c4881819cca657`)
- Lifecycle baseline: `target/video-render-feasibility/screenshots/lifecycle-30-baseline.png` (`7f12f955531b0ca042dfb285563887066e59d3671b17b4ae24ab6e9504bd9ac8`)
- Native/build/test/signing logs: `target/video-render-feasibility/logs/`

Decision: PASS — product implementation may continue.
