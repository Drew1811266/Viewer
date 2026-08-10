# Viewer Bottom Shelf And Stable Preview Loading Design

Date: 2026-08-09
Status: Approved

## Problem

Two remaining visual problems affect content browsing and image preview:

1. In a mixed folder, the collapsed `其它文件` disclosure is followed by a
   large blank region. The disclosure itself is only about 38 pixels high, but
   the image slot is currently allowed to shrink to the laid-out image rows.
   The unused height is therefore left below the shelf on the same surface and
   looks as though the shelf occupies the lower part of the window.
2. Opening an image can first render a small preview for roughly 0.5–1 second
   and then jump to the final fitted size. The small render is approximately
   576×384 pixels, which is the 90% fit result produced by the preview's
   temporary 640×480 stage fallback. A later `ResizeObserver` notification
   supplies the real stage dimensions and causes the visible jump.

The previous progressive-preview correction keeps display geometry stable
when a fit proxy is replaced by original pixels. It does not cover the earlier
transition from a temporary stage size to the real stage size.

## Goals

- Keep the mixed-folder `其它文件` disclosure pinned to the bottom of the
  content area.
- Let the image browser own every pixel above the disclosure or expanded shelf.
- Size an expanded mixed shelf from its actual rows, with the existing strict
  20% content-area cap and internal scrolling beyond the cap.
- Never render an image against an invented or zero preview-stage size.
- Show a useful dynamic loading state while the initial image and real stage
  geometry are being prepared.
- Reveal the image once at its final fitted size without a small-to-large jump.
- Keep fit-proxy-to-original replacement geometry stable.
- Preserve selection, image-grid scroll position, recovered dimensions,
  thumbnail work and cache state across shelf changes.
- Respect reduced-motion preferences and existing accessibility behavior.

## Non-goals

- Reporting byte-accurate or percentage image-loading progress. The current
  image request API exposes completion promises, not trustworthy byte progress.
- Changing thumbnail density, image-card sizing or proportional row layout.
- Redesigning the preview toolbar, navigation controls, magnifier or zoom model.
- Delaying preview entry until an image has been preloaded.
- Adding a user-resizable shelf or changing the session-level expand preference.
- Adding a third-party progress, layout or animation dependency.

## Considered Approaches

### 1. Minimal CSS and fallback adjustment

Restore image-slot flex growth and remove the visible 640×480 fallback while
leaving the existing loading logic otherwise unchanged.

This is a small patch, but it leaves readiness decisions distributed across
representation, source-dimension and stage-measurement branches. A future
timing change could reintroduce another intermediate visible geometry.

### 2. Explicit preview readiness gate

Restore the bounded mixed layout and represent initial preview readiness
explicitly. The preview image is not mounted until a nonzero real stage
measurement and a valid first representation/source geometry are available.

This is the selected approach because it addresses the timing cause directly,
makes the loading transition deterministic and supports focused regression
tests.

### 3. Preload before entering preview

Keep the content browser visible until the fit representation and preview
geometry are ready, then open the preview.

This can hide loading inside the preview, but makes the user's activation feel
unresponsive, complicates cancellation and navigation, and increases cache and
memory pressure. It is not selected.

## Selected Layout Behavior

### Mixed folder, collapsed

- The image slot is a `flex: 1 1 auto` bounded viewport.
- The `其它文件` disclosure is an intrinsic-height bottom shelf of about 38
  pixels.
- The disclosure is visually and structurally the final item in the content
  column, so no unowned blank region appears below it.
- The image grid receives the measured height of the full remaining image slot.
- Sparse image content may naturally leave unused canvas inside the image
  viewport, but that space remains above the shelf and is no longer presented
  as part of the other-file region.

### Mixed folder, expanded

- The expanded shelf participates in layout below the image viewport and never
  overlays image cards.
- Its requested height is the disclosure height plus the actual other-file row
  height.
- Total shelf height remains capped at 20% of the available content-body height.
- Rows beyond the cap scroll inside the mounted virtual list.
- One other file therefore consumes only one row plus the disclosure rather
  than a fixed or flex-grown lower pane.

### Other-file-only folder

The existing full-height primary list behavior remains unchanged and is not
subject to the mixed-folder 20% cap.

### State preservation

The image slot and `AspectVirtualGrid` must not be keyed or remounted when the
shelf expands or collapses. Only the measured viewport height changes. This
preserves DOM scroll position, selection, keyboard ownership, pending thumbnail
work and recovered image dimensions.

## Preview Readiness Model

The preview distinguishes the following inputs:

- `stageReady`: the mounted preview stage has a finite, nonzero measurement
  obtained from its actual layout;
- `representationReady`: a fit proxy or original representation is available;
- `sourceReady`: valid source dimensions are available from authoritative file
  metadata or the selected representation;
- `previewReady`: all three conditions above are true and final fit geometry has
  been committed for the current entity.

The 640×480 fallback is not a renderable stage measurement. Zero or unavailable
measurements remain pending instead of being replaced with invented dimensions.

Stage measurement occurs at layout time and continues through
`ResizeObserver`. Only finite, positive sizes are published. The preview image
is mounted only after `previewReady` becomes true, so the first visible image
already uses the real stage and final fitted geometry.

Changing to another preview entity resets entity-specific readiness and error
state. A valid stage measurement may be reused because the preview shell did
not change size, but the next image still waits for its own representation and
source dimensions.

## Loading Presentation

While the current image is previewable but `previewReady` is false, the center
of the preview stage shows a compact loading state containing:

- the existing message `正在准备高清预览…`;
- a slim indeterminate progress track;
- a continuously moving highlight that communicates active work without a
  fabricated percentage.

The loading state is available immediately and has no artificial minimum
duration. Fast loads therefore remain fast; longer real loads provide the
requested dynamic feedback.

When `previewReady` becomes true:

- the progress state fades out;
- the final fitted image fades in once;
- the transition lasts approximately 180ms;
- no scale animation is used, so the image cannot appear to grow from the
  temporary size.

The proxy may later be replaced by the original representation. That upgrade
changes only the representation URL and pixel source; it keeps the same source
geometry, viewport state and visible fitted size.

## Motion And Accessibility

- The progress container uses `role="status"`; the progress track uses
  `role="progressbar"` without percentage-valued ARIA attributes.
- Loading text remains concise and announced once per image load.
- The image keeps its current accessible name when mounted.
- Under `prefers-reduced-motion: reduce`, the moving highlight and fade
  transition are disabled. The static track and loading copy remain visible
  until readiness.
- No loading element receives keyboard focus, and existing preview shortcuts
  continue to belong to the preview dialog.

## Failure And Race Behavior

- If the fit request fails while the original request is still pending, the
  loading state remains until the original succeeds or fails.
- If either representation becomes usable, preview readiness proceeds from
  that representation without waiting for the other request.
- If no representation can be produced, loading stops and the existing fatal
  error presentation appears.
- Late results for an entity outside the allowed preview window remain ignored
  through the existing request/cache guard.
- A resize after the first reveal recomputes fitted geometry normally; this is
  an intentional response to a real window-size change, not an initial-load
  fallback transition.

## Implementation Boundaries

Expected implementation areas are:

- `ui/src/styles/adaptiveOtherFilePanel.css` for the bottom-shelf/image-slot
  flex relationship;
- `ui/src/components/ContentBrowser.tsx` and its layout tests for mixed-grid
  viewport behavior;
- `ui/src/components/ImagePreview.tsx` for actual stage measurement, preview
  readiness and loading presentation;
- the existing preview stylesheet or a focused companion stylesheet for the
  indeterminate progress and reveal motion;
- focused component and stylesheet tests, followed by the existing complete
  verification and quality-report commands.

The implementation must not change the desktop image API or persistence
schema.

## Test Strategy

### Content browser

- A collapsed mixed shelf remains one disclosure row at the bottom of the
  content area.
- The image slot grows into the height above that row.
- An expanded shelf with one file uses disclosure plus one row.
- A long expanded list is capped at 20% and scrolls internally.
- Toggling the shelf preserves the same grid node and scroll offset.
- Image-only and other-file-only modes retain their current height behavior.

### Image preview

- A zero initial stage rectangle does not publish 640×480 and does not mount an
  image.
- The loading status and indeterminate progressbar are visible while readiness
  is pending.
- The first finite nonzero stage measurement commits one final fitted image
  size; no 576×384 intermediate image commit occurs.
- A small fit proxy upgraded to a large original preserves visible geometry.
- Original-first, fit-first, fit-failure/original-success and total-failure
  paths settle correctly.
- Navigation resets entity readiness without retaining the prior image.
- Reduced-motion styling removes movement while retaining loading feedback.

### Verification

- Run focused layout and preview unit tests during implementation.
- Run the full UI verification suite and repository quality report.
- Launch the latest development binary and physically confirm both reported
  screenshots' scenarios at the user's window size.

## Acceptance Criteria

1. In the reported mixed folder, the collapsed `其它文件 · 1` bar sits at the
   bottom of the window and does not visually own a large blank region.
2. Expanding that one-file shelf shows only one row plus its disclosure.
3. Opening the reported 6308×4205 image never displays the temporary small
   image shown in the screenshot.
4. A dynamic, non-percentage progress bar is visible during a perceptible load.
5. The image first appears at its final fitted size and does not resize when
   original pixels replace the proxy.
6. Existing preview zoom, pan, magnifier, navigation, rotation and close
   behavior remain functional.
