# Viewer Native Video Theater Visual Contract

Date: 2026-08-16
Status: Selected target; implementation pending

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

Not captured yet. This section must record the source/implementation comparison paths, viewports,
pixel dimensions, density normalization, focused-region comparisons, and final design-QA result.
