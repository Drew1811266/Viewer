# Viewer Pointer-Following Magnifier Revision Design

**Date:** 2026-08-07  
**Status:** Approved design  
**Scope:** Single-image preview magnifier interaction, animation, and persisted magnification values

## 1. Purpose

Physical testing found that enabling the magnifier with Q can leave the lens hidden
when the pointer is already stationary over the image. The current lens also replaces
the ordinary pointer by centering itself directly under it, which makes precise aiming
harder than requested. This revision makes the lens a pointer-adjacent inspection tool,
changes the available magnification values, and adds bounded enter/exit motion.

This document replaces the magnifier-specific decisions in
`2026-08-06-viewer-trackpad-zoom-and-image-magnifier-design.md`. Its trackpad zoom,
pan, original-detail ownership, safety-budget, navigation-lifetime, and image-coordinate
mapping decisions remain unchanged.

## 2. Approved Interaction

### 2.1 Activation and pointer tracking

- Unmodified Q and the preview-toolbar magnifier button continue to toggle one shared
  enabled state.
- The preview records the latest pointer position even while the magnifier is disabled.
  Enabling the magnifier immediately re-evaluates that recorded position. If it is
  inside the transformed image pixels, the lens appears without requiring another
  pointer movement.
- Pointer tracking is available for the lifetime of the Viewer surface rather than
  beginning only after the lens mounts. This covers opening a preview beneath a
  stationary pointer and then pressing Q.
- The ordinary mouse arrow remains visible at all times. The lens uses
  `pointer-events: none` and never becomes the pointer target.
- Leaving the transformed image pixels hides the lens but preserves the enabled state.
  Re-entering shows it again. Closing the preview still resets the enabled state.

### 2.2 Pointer-adjacent placement

- The pointer remains the exact source-sampling point. Moving the lens beside the
  pointer must not shift which original pixel is inspected.
- The preferred lens position begins 18 CSS px to the right and below the pointer.
- If the preferred position would cross the stage's right or bottom edge, that axis
  flips independently to the left or above the pointer.
- If neither side has enough room, the lens is clamped inside the stage. Shape and area
  changes immediately recompute the bounded placement.
- Position updates remain imperative and animation-frame-coalesced; pointer movement
  must not trigger a React render per sample.

### 2.3 Appearance and disappearance motion

- Every actual lens appearance uses a 140 ms ease-out transition from 85% scale and
  zero opacity to full size and opacity.
- Every actual lens disappearance uses a 110 ms ease-in transition back to 85% scale
  and zero opacity.
- The transform origin is the lens corner nearest the pointer, so the lens visually
  emerges from and returns toward the pointer.
- Enter and exit transitions are reversible. Rapidly crossing an image edge must not
  flash, restart from a stale position, or leave an invisible lens intercepting input.
- Pointer coordinates and original-detail sampling update immediately while the
  decorative scale/opacity transition runs.
- `prefers-reduced-motion: reduce` removes these transitions and shows or hides the lens
  immediately.

## 3. Magnification and Settings Schema

The bounded magnification choices become:

| Preference | Values | Default |
| --- | --- | --- |
| Magnification | `1.5`, `2`, `3` | `1.5` |

Lens shape remains `circle` or `rounded_rectangle`, defaulting to `circle`. Display
area remains `small`, `medium`, or `large`, defaulting to `small`, with the existing
fixed dimensions.

`ViewerSettings` advances to schema version 3. The frontend public type accepts only
the numeric union `1.5 | 2 | 3`, and the Rust domain uses an exact bounded value rather
than accepting arbitrary floating-point input.

Migration rules are explicit:

- valid version 1 and version 2 files preserve only their thumbnail density;
- their magnifier preferences reset to `circle`, `1.5`, and `small`;
- valid version 3 files load all exact bounded values;
- malformed or unknown versions follow the existing safe all-default fallback;
- writes remain complete, serialized, and atomic.

No compatibility mapping is required for the former 3x/4x/5x/6x magnifier values.

## 4. Architecture

### 4.1 Pointer source and preview orchestration

A focused pointer tracker owns the latest client-space point for the Viewer surface.
`ImagePreview` reads that point on Q/button activation, converts it through the current
stage bounds, and passes it through the existing transformed-image hit test. Pointer
movement, stage resize, image representation changes, viewport transforms, rotation,
navigation, and preference changes all re-evaluate placement from the same source of
truth.

The tracker stores coordinates in a ref and does not own magnifier enabled state. It
does not invoke the backend and does not install native AppKit cursor hooks.

### 4.2 Lens geometry and rendering

The lens receives two independent points:

- the source point under the arrow, used to position the original image inside the
  clipped lens; and
- the bounded display center beside the arrow, used to position the lens shell.

A pure geometry function computes display center, horizontal/vertical flip state, and
the transform-origin corner from pointer point, stage size, lens dimensions, and the
18 px gap. `ImageMagnifier` writes the resulting values to CSS custom properties in at
most one animation frame.

Visibility is represented separately from enabled state. CSS transitions opacity and
scale while preserving a reversible state; the component cancels pending frames on
unmount and never delays source-identity invalidation during navigation.

### 4.3 Settings boundaries

The application domain, JSON store, Tauri input/output DTOs, TypeScript bridge types,
settings provider, and Settings dialog share the same three exact numeric choices.
Schema migration occurs in the infrastructure store. UI code never interprets an old
value or silently clamps an invalid v3 value.

## 5. Failure, Safety, and Accessibility

- Original loading, 100 MP, and 700 MB limits remain unchanged.
- Loading and failure text appears inside the pointer-adjacent lens and follows the
  same placement rules.
- A budget or decode failure never substitutes the fit proxy for original detail.
- Q ownership, `aria-pressed`, live announcements, forced-colors behavior, and
  keyboard reachability remain unchanged.
- The visible arrow supplies the exact aim point; lens shape, enabled state, and errors
  are not communicated by motion or color alone.
- The lens remains bounded at supported small windows and 200% page zoom.

## 6. Verification

Implementation follows red-green-refactor and adds focused coverage for:

1. a pointer recorded before magnifier activation, followed by stationary Q activation,
   displays the lens without a second pointer event;
2. Q and toolbar activation remain equivalent, and the ordinary cursor is never hidden;
3. preferred right/bottom placement, independent axis flips, constrained-stage clamps,
   and nearest-pointer transform origins;
4. the source sample remains under the arrow while the shell is offset;
5. 140 ms enter, 110 ms exit, reversible visibility, and reduced-motion behavior;
6. exact `1.5 | 2 | 3` settings contracts with a `1.5` default;
7. v1/v2 migration preserves thumbnail density while resetting magnifier preferences;
8. original request cancellation, navigation lifetime, and budget-failure behavior do
   not regress.

The PRE-08 magnifier visual state and DIA-01 settings state are updated to the new
default. Final verification includes focused UI/Rust tests, visual contracts, the
repository's complete `pnpm verify`, a clean worktree, and relaunching the latest
development build for physical pointer and trackpad retesting.
