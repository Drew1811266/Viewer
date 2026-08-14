# Viewer Video Control Visibility Refinement Design

Date: 2026-08-13
Status: Approved

## Problem

The immersive video preview works, but the current full-width top and bottom
scrims visibly darken the video and read as black edge transitions. At the same
time, secondary transport, navigation, volume, rate, and fullscreen controls
can disappear against bright or high-detail frames because their surfaces and
icons do not establish enough local contrast.

## Confirmed Direction

Use localized high-contrast control surfaces instead of full-width image
shading. This preserves the immersive full-frame presentation while making
every interactive control immediately legible.

## Goals

- Remove the full-width top and bottom gradient scrims completely.
- Preserve the decoded video at its current contain-fit geometry without
  cropping, scaling, or changing the native render surface.
- Give the title, duration, Done action, page navigation, timeline, transport,
  volume, speed, and fullscreen controls reliable contrast over any frame.
- Make the primary play/pause action visually dominant.
- Preserve keyboard ownership, hover/focus/disabled states, reduced motion,
  forced colors, responsive layout, and automatic controls visibility.

## Non-goals

- Changing playback behavior, shortcuts, frame stepping, navigation policy,
  timeline requests, native video geometry, or media decoding.
- Reintroducing a permanent page footer or opaque full-width toolbar.
- Cropping video to fill the window or removing legitimate contain-fit
  letterboxing caused by a media/window aspect-ratio mismatch.
- Adding new icons, controls, dependencies, or settings.

## Considered Approaches

### 1. Localized floating control dock — selected

Remove both scrims. Put the timeline and lower controls on one compact,
high-opacity translucent dock. Keep title metadata, Done, and page navigation
as separate localized surfaces. This offers the strongest contrast without
darkening the frame.

### 2. Independent strong buttons without a dock

Remove the scrims and increase every button's opacity and border. This keeps
the lightest footprint but leaves the timeline and labels vulnerable on busy
frames and creates a visually fragmented control area.

### 3. Fixed opaque bottom bar

Place all lower controls in a full-width solid bar. This is highly readable but
recreates the black-edge appearance the refinement is intended to remove and
reduces visible video area.

## Visual Contract

### Full-frame video

The video remains visually unobstructed outside localized controls. The stage
has no top or bottom pseudo-element gradient. Native-surface transparency and
the first-frame reveal contract remain unchanged.

### Title and Done

The filename and duration share a compact dark translucent metadata capsule at
the upper-left. Done remains a separate capsule at the upper-right. Both use
high-contrast text, a visible one-pixel border, and a restrained shadow.

### Page navigation

The page pill remains centered above the playback dock. Its arrow buttons use
visible local surfaces, full-strength icons when enabled, and a clearly muted
but still recognizable disabled state.

### Playback dock

The timeline and control row sit inside one rounded translucent dock with:

- enough opacity to remain legible on bright footage;
- a light border and soft shadow to define the boundary;
- white icons and labels;
- a solid accent primary play/pause button;
- stronger secondary button borders and hover states;
- readable volume, rate, and elapsed/total-time controls.

The dock remains inset from every window edge and never becomes a full-width
black band.

## Responsive And Accessibility Behavior

- Existing `aria-label`, title, and keyboard shortcut contracts stay intact.
- Focus-visible styling remains explicit on buttons, timeline, inputs, and the
  rate select.
- Disabled page controls retain at least a recognizable outline and icon.
- Forced-colors mode continues to use system colors and explicit borders.
- Reduced-motion mode continues to disable control reveal transitions.
- Compact layouts may stack the settings row, but the dock remains within the
  clipped stage and all controls remain reachable.

## Implementation Boundaries

- Extend the existing semantic `--video-preview-*` token set; component styles
  must not introduce raw color literals.
- Update `videoPreview.css` and focused layout/style contracts only where
  required for the approved visual change.
- Keep `VideoPreview`, `VideoControls`, `VideoTimeline`, bridge commands, and
  native rendering behavior unchanged unless a semantic class hook is needed.

## Test Strategy

- RED: style contract rejects remaining stage scrims and insufficiently
  defined localized control surfaces.
- GREEN: focused layout, component, accessibility, and semantic-color tests.
- Render the same playing-controls acceptance state and compare it directly
  with the supplied running-product screenshot.
- Run UI check/build and the complete repository verification before handoff.

## Acceptance Criteria

- No full-width black gradient appears at the top or bottom of the video.
- The picture outside localized controls retains its original brightness.
- Every enabled playback and navigation button has a visible boundary and icon
  on both light and dark footage.
- Play/pause is the strongest control without obscuring adjacent actions.
- The lower dock remains inset and does not resemble a black window edge.
- Video playback, navigation, shortcuts, timeline, fullscreen, and native
  surface behavior remain unchanged.
