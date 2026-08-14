# Viewer Video Player Experience Redesign

Date: 2026-08-13
Status: Approved
Figma: https://www.figma.com/design/gWuePv7qwGnu32aRryxdyg/Untitled?node-id=9-2

## Problem

The player is functionally complete, but its chrome still behaves like a set of
independent overlays rather than one coherent playback surface. On bright or
high-detail frames the title, navigation, timeline, transport, volume, rate,
and fullscreen actions compete with the picture. At narrower window widths the
controls compress rather than reprioritize, and the current near-black stage
creates an abrupt black transition before the native first frame appears.

The result is visually heavy, difficult to scan, and fragile across window
sizes even though the underlying playback commands and native surface are
working.

## Confirmed Direction

Treat video as the primary content and render chrome as a small number of
stable, localized layers:

1. an ambient matte that owns all pre-first-frame and letterbox space;
2. a quiet metadata/header layer;
3. a bottom chrome region containing navigation, timeline, and transport;
4. responsive disclosure that preserves the primary playback actions and moves
   secondary settings into a compact More panel.

The React stage and native surface continue to share the existing fitted
geometry contract. This redesign does not add a second media rectangle or
attempt to correct native geometry with CSS.

## Goals

- Eliminate pure-black flashes and full-width black-band transitions.
- Keep the decoded picture visually dominant at every supported window size.
- Establish one clear hierarchy: play/pause, timeline, time, navigation, then
  secondary settings.
- Keep every visible target at least 44 by 44 CSS pixels.
- Make the layout deliberate at wide, compact, and narrow widths instead of
  allowing controls to collide or overflow.
- Keep paused, playing, ended, loading, failure, reduced-motion, keyboard, and
  forced-colors states explicit.
- Preserve generation-safe playback, first-frame gating, native contain-fit,
  frame stepping, timeline thumbnails, shortcuts, and automatic idle hiding.

## Non-goals

- Changing media decoding, ffmpeg/libmpv, native render-surface creation, video
  metadata, cache behavior, or IPC commands.
- Cropping the video to fill the stage.
- Replacing the existing Lucide icon registry or adding UI dependencies.
- Rebuilding the rest of the application shell.
- Adding captions, playlists, quality selection, or other new player features.

## Layout Contract

### Ambient stage

The stage uses a deep blue-gray ambient matte rather than pure black. The same
semantic matte color covers the loading shell and the four letterbox insets
derived from `matteInsets`. A short 120 ms reveal veil fades away only after the
active generation reports its first frame. Reduced-motion mode removes this
transition without changing the gate.

### Header

Filename and duration form a compact upper-left metadata capsule. Done remains
the single upper-right exit action. Neither element creates a full-width bar.
In fullscreen, header chrome is absent while playback remains keyboard
accessible.

### Bottom chrome

Navigation and playback controls share one bottom safe region and never overlap
each other or extend beyond the clipped stage. The control dock has two rows:

- timeline plus elapsed/total time;
- transport on the left and settings/fullscreen on the right.

The dock remains inset from window edges and uses a localized translucent
surface, border, and shadow. It must not look like a footer or expose desktop
content below the native surface.

### Responsive priority

- **Wide (`>= 1100px`)**: previous frame, play/pause, next frame, mute, volume,
  rate, and fullscreen remain inline.
- **Compact (`760px`–`1099px`)**: play/pause, timeline, time, mute, fullscreen,
  and More remain visible; frame stepping, volume, and rate move into More.
- **Narrow (`< 760px`)**: timeline/time form the first row; play/pause, mute,
  fullscreen, and More form the second row. The More panel contains frame
  stepping, volume, and playback rate without duplicating accessible controls.

The responsive branch is driven by one viewport-match hook, not by two visible
copies of the same form controls.

## Interaction And State Contract

- The existing 2.5 second idle timer hides playback chrome only while playing.
- Paused, seeking, adjusting, focused, ended, and failed states keep controls
  visible.
- Opening More counts as adjustment/focus and prevents idle dismissal.
- Pointer movement and owned keyboard shortcuts reveal controls.
- Fullscreen hides metadata/navigation and hides the cursor together with idle
  controls; pointer movement restores both.
- Escape closes More first through the existing popover contract, then retains
  the existing fullscreen-first/preview-close shortcut behavior.
- No visual transition is allowed to reveal an unfitted native surface or
  bypass the active-generation first-frame gate.

## Component Boundaries

- `VideoPreview` owns stage state, matte/reveal attributes, header, navigation,
  and the bottom chrome container.
- `VideoControls` owns timeline, transport, settings, and the responsive branch.
- `VideoControlsMoreMenu` owns the compact-only secondary controls and reuses
  `ViewerPopover` plus existing button/input primitives.
- `useVideoResponsiveLayout` owns the `1100px` viewport breakpoint and responds
  to live media-query changes.
- `useVideoControlsVisibility` remains the source of idle visibility; More feeds
  its existing adjusting/focus inputs instead of creating a second timer.
- `useVideoBridge` remains the only source of media geometry, playback state,
  and commands.

## Accessibility

- All visible controls retain Chinese accessible names and existing keyboard
  shortcuts.
- More exposes `aria-expanded` and restores focus to its trigger on Escape or
  outside dismissal.
- Range and select controls remain native form elements with labels.
- Focus-visible treatment remains explicit against both the dock and the video.
- Forced-colors mode uses Canvas, CanvasText, and Highlight instead of translucent
  surfaces.
- Reduced-motion mode disables reveal and chrome transitions.

## Acceptance Criteria

- Opening a video shows an ambient matte, then the fitted first frame without a
  pure-black flash or exposed desktop.
- No header, navigation, control, popover, or timeline element overlaps or clips
  at 720×720, 1024×720, 1440×900, or fullscreen.
- Wide layout shows all playback controls inline.
- Compact/narrow layouts expose one More button and only one accessible instance
  of frame-step, volume, and rate controls.
- Every enabled visible action has a clear 44×44 target, boundary, hover state,
  and focus state.
- Controls auto-hide after 2.5 seconds only while playing and restore on pointer
  or owned keyboard activity.
- Playback, seek, frame step, mute, rate, fullscreen, navigation, retry, and Done
  retain their existing command behavior.
- Focused UI tests, accessibility/style contracts, UI check/build, visual
  acceptance captures, and `pnpm verify` pass before handoff.
