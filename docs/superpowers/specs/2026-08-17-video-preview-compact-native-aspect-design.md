# Viewer Compact Native Video Preview Design

Date: 2026-08-17
Status: Approved direction; implementation pending

## Goal

Rebuild the video-preview chrome as a compact, QuickTime-like overlay that gives the media most of
the window, does not move under trackpad gestures, and constrains live window resizing to the
current video's display aspect ratio only while video preview is active.

This is a development build feature. Signing, notarization, distribution, and release gates are
out of scope.

## Source Of Truth

- User control reference:
  `/var/folders/hh/jj77kbxs0db1_j1kgh7hbd2c0000gn/T/codex-clipboard-f6d217de-ce8a-48bc-bb60-d4c5b5f38e75.png`
- Existing Viewer light surfaces, cobalt accent, typography, icon registry, and accessibility
  tokens remain authoritative for product identity.
- The reference supplies hierarchy and control-state inspiration. It does not require copying its
  purple color treatment or every secondary action.

## Selected Direction

Use an edge-anchored overlay rather than separate layout bands or a detached floating card.

- The native video surface owns the full preview content bounds.
- A compact metadata strip overlays the upper edge.
- A compact playback controller overlays the lower edge.
- Both overlays are anchored to the preview viewport and may auto-hide while playing.
- Navigation is integrated into the upper strip on wide layouts and moves into the lower safe area
  only when horizontal space is constrained.
- No chrome participates in document flow, so it cannot increase the video's aspect-constrained
  window size or create scrollable overflow.

## Visual Contract

### Upper overlay

- Visible height: 36 px on wide/compact layouts; no taller than 40 px at narrow widths.
- Filename is one line with ellipsis. Duration remains tabular and secondary.
- Done uses a compact visual button, not a large bordered card.
- Previous/next navigation uses small Apple-like line controls with a clear current-position label.
- Background is a restrained Viewer light material with sufficient contrast over bright and dark
  footage; it is not a full-width opaque dark band.

### Lower controller

- Visible height target: 60-64 px on wide layouts and at most 72 px on narrow layouts.
- Timeline and elapsed/total time share the upper portion of the controller.
- Transport and secondary settings share one lower row.
- Play/pause is the visual primary action. Volume, settings, and fullscreen are quiet secondary
  actions. Frame stepping is available on wide layouts and moves into the existing More menu on
  compact layouts.
- Visible icon/button bodies are 28-32 px. Each interactive control retains a minimum 44 x 44 CSS
  pixel hit target through transparent padding, satisfying accessibility without looking oversized.
- Viewer cobalt indicates progress, the play/pause primary action, keyboard focus, and selected
  states. Neutral controls use the existing Viewer icon assets and neutral borders.

### Control states

Every control has explicit idle, hover, pressed, keyboard-focus, selected, disabled, and reduced-
motion behavior. State changes use opacity, border, and subtle surface contrast. They do not rely on
scale animation that would shift layout.

Controls remain visible while paused, seeking, ended, failed, adjusting, keyboard-focused, or when
the pointer is active. Playing idle may hide chrome. Revealing or hiding chrome never changes the
native video viewport or window aspect.

## Scroll And Gesture Containment

Video preview is a fixed, inset-zero interaction island.

- `html`, `body`, `#root`, the preview overlay, stage, and chrome do not expose scrollable overflow
  while preview is active.
- Preview-owned wheel/trackpad gestures cannot scroll the hidden project view or move the overlay.
- Horizontal and vertical overscroll chaining are disabled.
- WebKit rubber-band elasticity is disabled only for the active preview and restored on close if CSS
  containment alone is insufficient on the real WKWebView.
- Timeline horizontal gestures remain functional because the timeline consumes its own pointer
  sequence; the scroll lock does not block pointer drag or keyboard controls.

## Native Window Aspect Contract

The media display aspect ratio is calculated from authoritative display dimensions after rotation.

- When a video preview opens, a macOS-only preview window coordinator stores the existing window
  resize constraints and frame.
- The coordinator computes a best-fit initial content size within the visible screen and constrains
  subsequent live resizing to the video's display aspect ratio.
- Because the overlays sit inside the video bounds, no fixed chrome height needs to be added to the
  aspect calculation.
- AppKit applies the constraint synchronously during live resize. React resize events do not resize
  the native surface and do not participate in the feedback loop.
- The native video surface remains contain-fit with no crop or stretch. Rotation 90/270 swaps the
  effective width and height before the ratio is installed.
- Fullscreen temporarily follows the display bounds. Exiting fullscreen restores the preview aspect
  constraint.
- Switching videos replaces the constraint with the new video's ratio without leaking the previous
  generation.
- Closing preview, project close, window close, failure, and cancelled open all restore the exact
  pre-preview resize policy. The rest of Viewer remains freely resizable.

## Component Boundaries

### React

- `VideoPreview` owns semantic chrome, visibility, keyboard focus, navigation, and scroll
  containment.
- `VideoControls` owns playback state presentation and accessible interaction targets.
- React does not calculate or publish native surface rectangles during live resize.

### Desktop command/runtime

- The runtime opens and closes one generation-bound native preview session.
- Authoritative display dimensions travel with the accepted generation.
- Stale open, close, resize, or navigation work cannot change the current window constraint.

### macOS platform

- The theater/surface path owns the video viewport and synchronous resize.
- A scoped window-aspect coordinator installs, updates, suspends for fullscreen, and restores the
  preview-only constraint.
- The coordinator must not replace or break Tauri's existing global window delegate. Any AppKit
  integration composes with the existing delegate or uses supported window constraints.

## Error And Recovery Behavior

- Missing or invalid display dimensions fail closed to the current unconstrained window; playback
  error UI remains usable and the global resize policy is not changed.
- If installing the native constraint fails, video playback remains available, an internal typed
  error is recorded, and closing preview still executes restoration.
- A failed or stale generation cannot leave overscroll disabled or a window constraint installed
  after preview closes.
- Ended state keeps the final frame/poster and preserves the current aspect constraint until Done.

## Testing Contract

Implementation proceeds test-first.

1. UI behavior tests prove preview scroll position remains zero under vertical and horizontal wheel
   input, while timeline drag remains operable.
2. Style-contract tests prove compact visible dimensions, 44 px hit targets, no layout-flow chrome,
   and no scrollable overflow.
3. Pure native tests prove best-fit sizing, rotation-aware ratios, generation replacement,
   fullscreen suspension, and exact restoration.
4. Desktop lifecycle tests prove open installs the constraint only after generation acceptance and
   all close/failure/cancellation paths restore it.
5. Real macOS QA verifies trackpad gestures, repeated live resize, video navigation across different
   aspect ratios, fullscreen round-trip, timeline seeking, ended state, and Done restoration.
6. Visual QA compares the supplied Apple control reference, the existing Viewer design tokens, and
   the same paused/playing viewport. P0-P2 mismatches must be closed in `design-qa.md` before handoff.

## Acceptance Criteria

- Top overlay is no more than 40 px and lower controller no more than 72 px at supported viewports.
- Visible button bodies do not exceed 32 px except the play/pause emphasis; all hit targets remain at
  least 44 x 44 px.
- Two-finger trackpad gestures do not translate the preview UI or reveal hidden project content.
- Window live resize remains locked to the current video's display aspect and feels synchronous.
- Opening another video updates the ratio; Done restores ordinary Viewer resizing.
- Controls remain readable and operable across bright/dark frames and all required interaction
  states.
- No transparent desktop region, black transition, crop, stretch, or ended-state black frame is
  visible.
