# Viewer Intelligent Compare Layout Design

Date: 2026-07-28
Status: Approved

## Problem

Viewer currently chooses its multi-image comparison layout only from the image
count:

- two images use two columns;
- three images use one asymmetric large pane and two smaller panes;
- four images use a `2 × 2` grid;
- more than four images are rejected.

This policy ignores image aspect ratios and the available workspace shape. Four
portrait images in a wide desktop window therefore receive only half of the
available height, while most of each pane becomes empty side space. The user
entered comparison specifically to inspect large images and details, so this is
a structural layout failure rather than a color or spacing defect.

The fixed four-image limit also prevents larger review sets. Simply raising the
limit without changing rendering would mount and decode every selected image,
creating unnecessary memory and interaction cost.

## Goals

- Select a comparison layout from oriented image aspect ratios, image count and
  available workspace dimensions.
- Keep all images visible when a readable no-scroll layout exists.
- Switch to a large-image scrolling layout when fitting every image would make
  any image too small.
- Give four portrait images a single-row layout in the reported wide-window
  case instead of a `2 × 2` grid.
- Preserve the user's source or selection order in every layout.
- Use a horizontal, equal-height filmstrip for many portrait images.
- Use a two-column vertical flow for many landscape or square images.
- Use a horizontal, equal-height, variable-width filmstrip for mixed aspect
  ratios.
- Support comparison of 2–20 JPG or PNG images.
- Disable the radial menu's comparison action when more than 20 items are
  selected instead of opening comparison and reporting an error afterward.
- Preserve the existing light appearance, zoom, pan, rotation, synchronized
  transform, review-state and favorite behavior.
- Bound mounted preview nodes and decoded images through virtualization.
- Keep layout calculation pure, bounded, testable and independent of image
  decoding or per-pane DOM measurement.

## Non-goals

- Cropping images to make a layout look denser.
- Reordering or grouping selected images by orientation.
- Making one compared image visually more important than another.
- Changing single-image preview behavior.
- Changing review-state, favorite or original-image safety semantics.
- Supporting non-JPG/PNG comparison in this change.
- Removing the explicit 20-image capacity boundary.
- Adding a third-party layout, masonry or virtualization dependency.
- Persisting a user's last automatically chosen layout as a preference.
- Adding a manual layout picker in the first version.

## Approved User Experience

### Entry capacity and radial-menu state

Comparison has one shared capacity contract:

```text
MIN_COMPARE_IMAGES = 2
MAX_COMPARE_IMAGES = 20
```

The radial menu's `并排对比` action is enabled only when all of the following
are true:

1. the file-content comparison context is available;
2. no blocking file operation is running;
3. 2–20 items are selected;
4. every selected item is a supported JPG or PNG image.

When more than 20 items are selected, the action is visibly disabled with the
existing disabled visual treatment, exposes native/semantic disabled state,
cannot be activated by pointer or keyboard, and reports the disabled reason
`最多同时对比 20 张图片`.

Disabled-reason precedence is:

1. busy operation;
2. unavailable comparison context;
3. more than 20 selected items;
4. fewer than 2 selected items;
5. one or more unsupported items.

The fewer-than-two reason is `请选择 2–20 张图片`. The unsupported-item reason
is `仅支持 JPG 或 PNG 图片`.

The direct application entry point and compare model repeat validation as
defense in depth. No reducer or model silently truncates a list to 20. An
unexpected invalid programmatic request is rejected and leaves the current
workspace state intact.

### No-scroll comparison

The layout engine first tries to show every image without outer scrolling. It
generates equal-weight candidates:

- one row;
- one column;
- uniform grids with 2–4 columns and source-order wrapping.

The last grid row is centered and keeps the same pane size as preceding rows.
It is never stretched to fill unused columns. The former three-image
asymmetric layout is removed because it gives the first item different visual
weight.

Every candidate displays the complete image with contain semantics. No
candidate may crop, stretch or reorder an image.

### Scrolling comparison

If no no-scroll candidate meets the minimum viewing size, the engine chooses a
scrolling layout:

| Image composition | Scrolling layout |
| --- | --- |
| All portrait | One equal-height row with horizontal scrolling |
| All landscape or neutral/square | Two-column vertical flow |
| Mixed orientations or ratios | One equal-height, variable-width row with horizontal scrolling |

The landscape/square flow becomes a single column when the available workspace
width is below 900 CSS px.

Mixed content always preserves original order. Landscape items occupy wider
cards and portrait items narrower cards at one common image-stage height.
Cards retain enough width for existing pane controls; extremely narrow images
may therefore have control-driven side space, but the image itself is never
distorted.

### Layout changes and focus anchoring

The active image is the scroll anchor. When resizing, adding/removing images or
switching between no-scroll and scroll layouts:

- the active entity remains active;
- the active entity remains visible and is kept near its previous visual
  position where possible;
- the view does not jump back to the first image;
- transform, rotation, review and favorite state remain associated with the
  same entity ID.

Removing items down to one or zero preserves existing transitions to
single-image preview and the file grid.

## Orientation and Metadata

The engine uses `BrowserFile.imageMetadata` when dimensions are finite and
positive. EXIF orientation has already been applied by that metadata path.
Viewer then applies the compare pane's explicit quarter rotation:

```text
ratio = orientedWidth / orientedHeight
rotation 0 or 180: effectiveRatio = ratio
rotation 90 or 270: effectiveRatio = 1 / ratio
```

Aspect classes are initial tunable constants:

```text
portrait: effectiveRatio < 0.90
neutral:  0.90 <= effectiveRatio <= 1.10
landscape: effectiveRatio > 1.10
```

The classes only select the scrolling fallback family. Candidate scoring uses
the continuous effective ratio rather than the class.

If metadata is missing or invalid, the engine temporarily uses `1:1`. When a
visible or overscanned pane recovers valid natural dimensions, Viewer records
that ratio by entity and modification identity and solves once more. The normal
12% switching rule still applies unless the current layout has become invalid.

## Layout Solver

### Inputs and output

`solveCompareLayout` is a pure function. Its input contains:

- available workspace width and height after the compare toolbar;
- pane header, footer and gap geometry tokens;
- ordered entity IDs;
- effective aspect ratio for every entity;
- the previous valid layout kind and score, if any.

User zoom and pan are deliberately absent. Rotation is represented only through
the effective aspect ratio.

The output is a `CompareLayoutPlan` containing:

- layout kind;
- scrolling axis or `none`;
- pane or row geometry;
- total scroll extent;
- item rectangles or deterministic geometry inputs;
- chosen candidate score;
- whether the previous layout was retained by hysteresis;
- a safe focus-anchor target.

Invalid zero, negative or non-finite workspace inputs do not produce invalid
geometry. The hook keeps the previous valid plan or uses a safe one-column
fallback if none exists.

### Candidate simulation

For every item `i` with ratio `r` in a candidate image-stage rectangle
`W × H`:

```text
displayWidth[i]  = min(W, H × r)
displayHeight[i] = min(H, W / r)
displayArea[i]   = displayWidth[i] × displayHeight[i]
```

The same image is also simulated in a single-pane full-workspace stage. Its
candidate area divided by that single-pane area is the normalized display area
used for fairness across aspect ratios.

The initial readable-size constraints are:

```text
minimum display area = 120,000 CSS px²
minimum displayed long edge = 360 CSS px
```

A no-scroll candidate is eligible only when every item meets both constraints.
If the workspace itself cannot satisfy them even for a scrolling item, the
engine returns the best possible safe scrolling geometry rather than failing.

These values are named configuration constants. They may be tuned against real
Viewer fixtures without changing the solver contract.

### Candidate score

Eligible candidates use:

```text
score =
  0.45 × minimum normalized display area
  + 0.25 × mean normalized display area
  + 0.20 × image-area viewport utilization
  + 0.10 × layout continuity
```

`layout continuity` is `1` when the candidate keeps the current layout family
and `0` otherwise.

The solver generates at most six candidates. Its complexity is `O(n × k)`,
where `n <= 20` and `k <= 6`. It never enumerates image permutations.

### Hysteresis

Geometry inside the current layout updates continuously as the workspace
changes. A layout-family switch happens only when:

- the current family no longer satisfies hard constraints; or
- the new family's score is at least 12% higher than the current score.

This prevents repeated switching near a breakpoint while still responding
immediately when images have become unreadably small.

## Scrolling Geometry and Virtualization

### Horizontal filmstrip

The horizontal filmstrip uses a geometry table:

```text
width[i]  = clamped width required by common stage height and ratio[i]
offset[0] = inlinePadding
offset[i + 1] = offset[i] + width[i] + gap
```

Pure portrait sets normally produce equal or similar widths. Mixed sets retain
variable widths. Widths are clamped only for pane-control usability and to
prevent one landscape card from consuming an unbounded track fraction; image
contain behavior remains unchanged.

The viewport binary-searches monotonic offsets and mounts:

- the visible interval;
- one viewport of overscan before it;
- one viewport of overscan after it;
- the focused item when it is outside that interval.

The active/focused item is at most one additional mounted pane, keeping the
render count bounded.

### Vertical landscape/square flow

The vertical flow pairs consecutive items into two columns, or one column
below the 900 CSS px breakpoint. Each row computes the height needed by its
items at the available column width and stores a monotonic row offset.

Visibility binary-searches row offsets and applies the same one-viewport
overscan rule. The last unpaired item remains one normal-width column item; it
does not stretch across both columns.

### Loading policy

Layout uses metadata and does not wait for pixels. Visible panes request image
representations first. Overscan panes are mounted after the visible interval.
Items beyond overscan create no image request or decoded image node.

When a virtual pane leaves overscan:

- its unfinished request receives cancellation;
- the pane DOM is unmounted;
- its transform, rotation, review and favorite state remain in the compare
  model by entity ID.

Opening comparison does not wait for all selected images. Each visible pane can
complete independently.

## Scrolling, Zoom and Pan

- Native trackpad horizontal movement scrolls a horizontal filmstrip.
- `Shift + wheel` scrolls horizontally.
- A plain vertical mouse wheel over the filmstrip is translated into horizontal
  movement while the track can continue in that direction. At a track edge it
  is not trapped.
- The landscape/square flow uses native vertical scrolling.
- Keyboard focus movement scrolls the target pane into view.
- The virtual region exposes an accessible name, total item count and per-item
  position.

Zoom never participates in layout scoring. The existing synchronized mode
continues to apply normalized scale and center relative to each pane's fit
state. Off-window panes receive the stored transform when mounted.

Dragging a zoomed image pans that image. Outer scrolling owns wheel/trackpad
movement and does not compete for the image drag gesture. `适应窗口` resets
image transforms without forcing a different stable outer layout.

## Performance Requirements

- Solver work uses only in-memory scalar metadata and bounded candidates.
- `ResizeObserver` notifications are coalesced with one
  `requestAnimationFrame`; React state changes only when the plan's meaningful
  geometry or layout kind changes.
- No solver path reads image pixels, waits for image decoding or measures every
  pane DOM node.
- Geometry arrays and visibility lookup remain linear to build and logarithmic
  to query.
- Mounted panes are bounded by visible content, two one-viewport overscan
  regions and at most one focused pane.
- Image representation requests are bounded by mounted panes rather than the
  selected count.
- A local benchmark on the current development computer targets less than
  2 ms to solve the maximum 20-image set.
- CI verifies bounded candidate count and complexity invariants instead of a
  hardware-sensitive wall-clock threshold.
- Rapid workspace resizing must not create a long task from layout solving or
  visibly flash between layout families.

## Architecture

### Shared capacity contract

A shared UI-domain module exports `MIN_COMPARE_IMAGES`,
`MAX_COMPARE_IMAGES` and compare-candidate validation. It is consumed by:

- radial menu model and disabled reasons;
- application compare entry;
- workspace reducer;
- compare model creation and reconciliation;
- user-facing validation copy.

The reducer removes its current `.slice(0, 4)` behavior. Invalid cardinality is
rejected rather than truncated.

### Pure layout engine

`ui/src/components/compareLayoutEngine.ts` owns:

- ratio validation and rotation-aware effective ratios;
- candidate generation;
- fit simulation;
- readable-size eligibility;
- score calculation;
- hysteresis;
- scrolling-family choice;
- horizontal and vertical geometry tables;
- visible-range and anchor calculations.

It has no React, DOM, file-request or application-state dependency.

### Measurement hook

`ui/src/components/useCompareLayout.ts`:

- observes the compare content region;
- coalesces resize notifications;
- assembles solver inputs from ordered files and compare rotations;
- preserves the previous valid plan;
- exposes the current plan to the workspace.

Only the outer content region is measured. Pane metrics continue to serve zoom
and pan clamping but do not drive outer-layout selection.

### Specialized virtual viewport

`ui/src/components/CompareVirtualViewport.tsx` owns horizontal and vertical
compare virtualization, scrolling, wheel translation, focus anchoring and
accessible collection metadata.

The existing generic `VirtualList` remains unchanged. It assumes one fixed
vertical row height and is not expanded to carry horizontal variable-width and
variable-row comparison semantics.

### Workspace and pane integration

`CompareWorkspace` retains toolbar actions and compare-state orchestration. It
renders either:

- a no-scroll equal-weight grid from the plan; or
- `CompareVirtualViewport` for a scrolling plan.

`ComparePane` remains responsible for one mounted image, its pane chrome,
representation request, transform interaction, review controls and load/error
state. Virtual mounting does not change its public state identity.

## Error Handling

- Missing metadata uses a neutral placeholder ratio and may recover once.
- A corrupt or failed image retains its pane and existing error UI so other
  geometry and scroll positions do not jump.
- Invalid solver inputs keep the previous plan or use one safe column.
- A removed active image transfers activity to the nearest surviving neighbor.
- A canceled off-window request does not report a user-visible load failure.
- Original-image budget rejection continues to fall back to the fit proxy.
- More than 20 selected items never start comparison.
- Direct invalid entry reports a concise status instead of partially opening a
  comparison.
- No error DTO or diagnostic adds absolute paths to the visible UI.

## Accessibility

- Disabled radial actions expose real disabled semantics and the correct
  reason; grey appearance is not the only signal.
- Compare collections expose an accessible name and ordered set size.
- Virtual panes expose position-in-set and preserve logical source order.
- Focused panes are mounted and automatically scrolled into view.
- Existing pane buttons retain names, pressed states, focus treatment and
  keyboard operation.
- Keyboard zoom shortcuts and Escape continue to operate from the compare
  workspace.
- Motion from re-layout is not animated in the first version, avoiding
  distracting or inaccessible large-area transitions.

## Testing

### Capacity and entry

- Radial compare is enabled for 2 and 20 supported images.
- It is disabled for 0, 1 and 21 supported images.
- A 21-image selection renders the grey disabled state, cannot activate and
  exposes `最多同时对比 20 张图片`.
- Mixed supported/unsupported selections remain disabled.
- Busy and unavailable-context reasons preserve their higher priority.
- Direct entry rejects more than 20.
- Reducer and model do not silently slice invalid input.

### Pure solver

- Four `3:4` portrait images in the reported wide workspace choose one row.
- Four landscape images choose the best eligible grid.
- Six portrait images cross the readable threshold in representative workspace
  sizes and use a horizontal filmstrip when required.
- Landscape/square overflow uses two columns and falls to one below 900 px.
- Mixed ratios preserve entity order and produce variable filmstrip widths.
- Portrait, neutral and landscape threshold boundaries.
- Rotation at 90/270 degrees inverts the effective ratio.
- Missing, zero, negative, overflowing and non-finite dimensions.
- Exact area and long-edge readable thresholds.
- Score weighting and 12% hysteresis on both sides of the boundary.
- Current invalid layouts switch regardless of hysteresis.
- Candidate count remains at most six for 20 items.

### Geometry and virtualization

- Fractional horizontal widths do not accumulate rounding drift.
- Horizontal and vertical geometry offsets remain monotonic.
- Binary-search visible ranges at the start, middle and end.
- One viewport of overscan on each side.
- A focused off-window pane is mounted as the only allowed extra item.
- Scroll anchoring across resize, add/remove and layout-family changes.
- Last unpaired vertical item retains normal column width.
- Image requests are issued only for mounted panes and canceled on virtual
  unmount.

### Interaction and component integration

- Horizontal trackpad/wheel translation and edge release.
- Native vertical flow scrolling.
- Active entity and normalized transforms survive virtual unmount/remount.
- Synchronized and independent zoom continue to match current behavior.
- Zoom and pan do not trigger an outer layout-family switch.
- Removing items transitions to single preview and grid at one and zero.
- Corrupt and delayed images do not reorder or collapse neighboring panes.
- Accessible set size, item position, disabled reasons and focus restoration.

### Performance

- A deterministic test verifies no more than six candidates and no DOM/image
  access from the solver.
- A 20-item component fixture verifies that mounted pane and request counts are
  bounded by viewport geometry rather than equal to 20.
- A development benchmark records solver time for portrait, landscape, square
  and mixed 20-image sets, with a current-machine target below 2 ms.
- Rapid resize tests verify frame coalescing and no repeated state commit for an
  equivalent plan.

## Acceptance Examples

1. Four portrait images in a wide compare workspace appear in one horizontal
   row and use substantially more vertical image area than the former grid.
2. Four landscape images remain in a layout that uses width effectively.
3. A larger portrait set becomes a horizontally scrolling large-image strip
   when the no-scroll candidates fall below the readability threshold.
4. A larger landscape/square set becomes a virtualized two-column vertical
   flow.
5. A mixed set remains in source order in an equal-height variable-width
   horizontal strip.
6. Selecting 21 images leaves `并排对比` grey and inactive in the radial menu.
7. Scrolling a 20-image set mounts and requests only visible and overscanned
   panes while preserving off-window compare state.
