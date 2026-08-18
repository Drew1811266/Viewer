# Viewer Native Video Theater Visual Contract

Date: 2026-08-16
Status: Implemented; real macOS QA passed 2026-08-17

## Source Truth

- User failure evidence:
  `/var/folders/hh/jj77kbxs0db1_j1kgh7hbd2c0000gn/T/codex-clipboard-e1de389c-ae60-4985-ab25-9b95bf1aa5b3.png`
- Selected target: `docs/reviews/assets/viewer-native-video-theater-target.png`
- Selected target SHA-256:
  `d448f2a75811e4141ac765897153c2306827bc7c6599726093ab863695339c6e`
- Selected target pixels: 1586 × 992.

The target is the third displayed local design direction selected by the user. It is a visual
reference, not production evidence.

## Wide Layout Contract

- Standard macOS titlebar remains outside the player content hierarchy.
- One opaque deep-blue theater owns the complete content area.
- Top command bar is compact and aligned: filename/duration left, navigation center, Done right.
- The top bar is not a detached card and does not create a large empty header.
- Video is contain-fit and centered. Letterbox regions are the same native theater surface.
- Bottom controls form a flush inspector shelf rather than a floating oversized card.
- Timeline occupies the first shelf row with elapsed/total time aligned right.
- Transport is left aligned; secondary audio/rate/fullscreen controls are right aligned.
- Primary play/pause uses the Viewer cobalt accent. Other actions use clear 1 px boundaries and
  high-contrast white line icons.
- No transparent gap exists between top bar, media, or control shelf.

## Responsive Contract

### Compact: 1024 × 720

- Top filename truncates to one line; duration remains visible when space permits.
- Center navigation remains available but may reduce horizontal padding.
- Timeline/time keep a dedicated first control row.
- Play/pause, mute, fullscreen, and More remain visible.
- Frame stepping, volume, and playback rate move into the existing More popover.
- Exactly one accessible instance of every secondary control exists.

### Narrow: 720 × 720

- Top command bar remains one row with filename truncation and 44 px Done target.
- Navigation moves into the bottom safe region when center space would collide.
- Timeline/time remain readable and do not clip.
- The second row keeps play/pause, mute, fullscreen, and More.
- No control or popover extends outside the opaque theater.

## Interaction And State Contract

- Playing idle may hide overlay chrome; paused, seeking, ended, failed, adjusting, and focused
  states keep controls visible.
- Pointer movement and owned shortcuts restore chrome.
- Timeline click performs one exact commit and no preview seek.
- Timeline drag updates local time immediately, uses latest-wins preview, and commits immediately on
  release without waiting for preview completion.
- Ended retains the final frame and reports the exact duration.
- Loading/failure expose only the theater color behind semantic UI, never the desktop.

## Native Composition Contract

- AppKit owns the opaque theater and fitted video child.
- WKWebView owns overlay chrome only.
- React does not publish video geometry or cut a transparent aperture.
- Live resize stays entirely on the AppKit layout path.
- A missing/hidden/unmounted video child reveals the theater color.

## Acceptance Viewports

The later approved compact native-aspect specification supersedes fixed `1440 × 900`, `1024 × 720`,
and `720 × 720` content-area captures: the ordinary window itself now follows the media display
aspect. The equivalent real 16:9 captures are `1280 × 720`, `1024 × 576`, and `720 × 405`.
Portrait media is separately captured at `450 × 800`. The original viewport list remains below as
historical context for this contract.

The real Tauri application must be captured at:

- 1440 × 900 wide;
- 1024 × 720 compact;
- 720 × 720 narrow;
- fullscreen on the native display.

For the wide state, compare the selected target and implementation at the same crop and density.
For compact/narrow, compare against this responsive contract and record any deliberate disclosure.

## Blocking Visual Failures

- any desktop pixel visible inside the content area;
- black or transparent flash between theater, video, or controls;
- a top blank region materially larger than the selected target;
- detached/oversized controls that cover the media subject;
- timeline, navigation, or control overlap;
- controls below 44 × 44 CSS pixels;
- weak icon contrast or invisible disabled/pressed/focus state;
- contain-fit distortion or crop;
- final-frame black screen.

## Implementation Evidence

### Current source and normalization

- Exact user reference: `target/design-qa-video-compact-native/source-apple-controls.png`
  (`930 × 210`, SHA-256
  `eabd3b718edf0df3923acf06956289751d8f5d102dd5f579affa62ef6497edfc`). These are the
  exact bytes recovered from the original input image data URI after the clipboard temp path was
  unavailable.
- Final clean paused implementation: `target/design-qa-video-compact-native/14-final-paused-clean.jpeg`
  (`1072 × 603`, SHA-256
  `f0a19ecf29258204c8fcb1c0b9595f8128b55d691a66a7e7236a11ff6702b755`).
- Normalized same-state comparison:
  `target/design-qa-video-compact-native/comparison-paused-controls-normalized.png`
  (`1561 × 129`, SHA-256
  `e41085bdd55dd580c07fcfda2e0c1a7a8f2c5b56cc989efbdd3fda77f7786e14`). The source's
  `930 × 210` 2× controller crop is normalized to `465 × 105` at 1×. The final implementation's
  `1048 × 70` bottom shelf remains at captured 1× density and native aspect. Both are paused with a
  partially advanced timeline.

The compact/native-aspect design explicitly uses the Apple reference for hierarchy and state
inspiration, not for copying purple color or every secondary action. The implementation therefore
uses Viewer light neutral/cobalt tokens, a one-row compact shelf, existing icons, and More disclosure.

### Real macOS captures and interactions

| State | Evidence | Result |
| --- | --- | --- |
| Paused wide | `target/design-qa-video-compact-native/01-paused-wide.png` (`1280 × 720`) | Compact upper/lower overlays; no layout-flow band, seam, or subject-blocking card. |
| Playing wide | `target/design-qa-video-compact-native/02-playing-wide.png` (`1280 × 720`) | Native frame stays dominant; chrome does not alter media aspect. |
| Compact | `target/design-qa-video-compact-native/03-compact-1024x576.png` | Frame/volume/rate disclose through More; no duplicate visible controls or collision. |
| Narrow | `target/design-qa-video-compact-native/04-narrow-720x405.png` | Timeline/time and essential transport remain readable inside the stage. |
| Portrait | `target/design-qa-video-compact-native/05-portrait-ratio-450x800.png` | Rotation-aware 9:16 constraint replaces 16:9 exactly once, without crop/stretch. |
| Ended | `target/design-qa-video-compact-native/06-ended-wide.png` (`1280 × 720`) | Exact-duration useful final frame retained; no black terminal surface. |
| Done restored | `target/design-qa-video-compact-native/12-final-post-done-restored.jpeg` (`1229 × 768`) | Ordinary project frame and selection restored. |
| Free resize after Done | `target/design-qa-video-compact-native/13-final-post-done-free-resize.jpeg` (`1030 × 768`) | Non-16:9 ordinary resize succeeds; process and project state remain alive. |

Real Computer Use checks covered all four edges and four corners during live resize, vertical and
horizontal gestures over title/video/timeline/empty overlay, timeline click and drag, ratio
replacement, fullscreen enter/exit, ended retention, and Done restoration. The eight 16:9 live
resize results ranged from `1178 × 663` through `828 × 466` and remained ratio-constrained without a
React catch-up jump. Scroll gestures did not move the document or timeline. A click changed the
timeline `1.966667 → 0.566667`; a drag committed `1.533333` with the matching native frame.

### Final implementation assessment

- AppKit owns theater composition, native aspect policy, and live resize on the main thread.
  `MacVideoSurface` owns the aspect session; React does not publish a native rectangle.
- Fullscreen exit restores the active media ratio. Done restores the original frame and ordinary
  resize policy.
- Real QA exposed an AppKit crash when restoring the default `(0,0)` ratio by calling the ratio
  setter. Focused RED/GREEN coverage now restores an unconstrained window through its prior content
  resize increments instead; the final real build resizes freely after Done.
- The exact bundled reviewed runtime passed `scripts/video/verify-runtime.sh`, byte comparison,
  permissions, and arm64 checks. The fresh app was built with `--debug --no-sign`; no distribution,
  Developer ID, notarization, or stapling step ran.
- Current visual severity: P0 `0`, P1 `0`, P2 `0`.

final result: passed
