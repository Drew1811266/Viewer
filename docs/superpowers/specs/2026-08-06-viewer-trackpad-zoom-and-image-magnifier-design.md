# Viewer Trackpad Zoom and Image Magnifier Design

**Date:** 2026-08-06
**Status:** Approved design
**Scope:** Single-image preview, persisted Viewer display settings, and acceptance coverage

## 1. Purpose

Viewer's single-image preview already supports fitted display, explicit 100% display,
toolbar zoom buttons, rotation, bounded pointer dragging, and previous/next navigation.
This design extends that surface with native-feeling MacBook trackpad zoom and pan,
plus an original-detail magnifier that can be toggled from either the keyboard or the
preview toolbar.

The feature must preserve the current image safety budget and the existing light
preview layout. It must not turn a pinch gesture into page zoom, add another preview
window, or move preview state into a second native UI implementation.

## 2. Approved Product Decisions

### 2.1 Trackpad interaction

- A two-finger pinch continuously zooms the image around the gesture center.
- A two-finger scroll pans a zoomed image in both axes.
- Panning stops at the computed image boundary and never navigates to the previous or
  next image.
- Trackpad input, pointer dragging, and the toolbar minus/plus controls update one
  viewport transform model.
- The existing zoom range remains 10% through 800%.
- The existing fitted, original-100%, free-zoom, rotation, loading, failure, and
  navigation behaviors remain available.
- Free zoom continues to use the existing fit representation. It does not
  automatically request a full original; the magnifier is the explicit original-detail
  tool.

On macOS, trackpad pinch is exposed to web content as a wheel event with `ctrlKey`
set. The preview owns that event only while the pointer is over its image stage. The
implementation must register a non-passive listener, prevent the WebView default only
for owned gestures, normalize wheel delta modes, and update through animation frames.

### 2.2 Magnifier activation and lifetime

- Pressing Q toggles the magnifier on. Pressing Q again toggles it off.
- A new preview-toolbar magnifier button controls the same state.
- Key auto-repeat does not cause repeated toggles.
- Modified Q shortcuts are ignored so Command-Q remains the application quit command.
- While the magnifier is enabled and the pointer is inside the actual rendered image,
  the ordinary pointer is hidden and the lens is centered on the pointer.
- Moving onto the gray stage, toolbar, navigation, or outside the window restores the
  ordinary pointer and hides the lens without changing the enabled state.
- Re-entering the actual image restores the lens when the enabled state is still true.
- Previous/next navigation preserves the enabled state during one preview session.
- Closing the entire preview resets the enabled state. A newly opened preview starts
  with the magnifier off.
- Unsupported, unavailable, or removed images disable the toolbar button and ignore Q.

The toolbar button sits beside the existing zoom controls. It uses a real Lucide
magnifier asset, exposes `aria-pressed`, and includes Q in its accessible description
and tooltip.

### 2.3 Original-detail definition

The lens is independent of the main viewport zoom. It samples the oriented
`original100_percent` representation, not a second enlargement of the fit preview.
The selected 3x, 4x, 5x, or 6x value is a linear scale relative to the oriented
100%-original pixels. For example, at 4x a source pixel occupies four CSS pixels in
each axis inside the lens.

Rotation must remain visually consistent: cursor coordinates are mapped through the
inverse viewport transform to oriented source coordinates, and the lens shows the
same orientation as the main image.

### 2.4 Persisted preferences

The existing Settings dialog keeps its single `显示与外观` category and adds a
`图片预览` group. It does not add empty or speculative top-level categories.

The group persists three preferences:

| Preference | Values | Default |
| --- | --- | --- |
| Lens shape | `circle`, `rounded_rectangle` | `circle` |
| Magnification | `3`, `4`, `5`, `6` | `4` |
| Display area | `small`, `medium`, `large` | `small` |

Area values use fixed CSS geometry so screenshots, keyboard semantics, and small
window behavior remain deterministic:

| Area | Circle | Rounded rectangle |
| --- | --- | --- |
| Small | 160 px diameter | 180 x 120 px |
| Medium | 220 px diameter | 240 x 160 px |
| Large | 300 px diameter | 330 x 220 px |

The rounded rectangle uses the existing Viewer radius language. Each preference uses
a labelled, keyboard-operable bounded choice control. Shape, magnification, and area
changes are applied immediately and saved through the existing optimistic settings
flow.

## 3. Architecture

### 3.1 Component boundaries

`ImagePreview` remains the preview orchestrator. New behavior is divided into focused
units instead of expanding the existing component into one mixed interaction file:

- `useImageViewport` owns viewport mode, zoom, translation, rotation, anchoring, and
  boundary calculations.
- `usePreviewGestures` normalizes non-passive wheel input, pointer drag input, and
  toolbar actions into viewport actions.
- `ImageMagnifier` renders lens geometry, load/failure content, the source image, and
  the pointer-relative crop.
- A pure geometry module maps stage coordinates to oriented source coordinates and
  exposes testable clamp, inverse-transform, and lens-window calculations.

Each unit consumes explicit values and callbacks. It does not read App state or invoke
Tauri directly.

### 3.2 Viewport model

The viewport model contains:

- display mode: `fit`, `original`, or `free`;
- free zoom scalar;
- clockwise rotation in 90-degree increments;
- x/y translation in stage CSS pixels;
- measured stage and rendered-source dimensions.

All zoom inputs use one anchor-preserving operation. The operation converts the
gesture point into image-local coordinates before changing scale, then adjusts
translation so that local point stays under the same stage point. Translation is
clamped again after zoom, rotation, resize, representation change, and navigation.

The gesture adapter treats `wheel + ctrlKey` as pinch zoom and ordinary two-axis
wheel deltas as pan. A gesture is consumed only in the preview stage; it never leaks
into image navigation or application page zoom.

### 3.3 Magnifier state and rendering

The preview session owns:

- `magnifierEnabled`;
- pointer-inside-image state;
- the latest stage pointer coordinates;
- the current original representation load state;
- one current request identity or abort controller.

Enabling the magnifier requests the current image's existing
`original100_percent` representation. Only one current original request and one
decoded current original are retained. Navigating cancels or invalidates the stale
request and releases the previous decoded source. Returning to an earlier image may
reuse the artifact cache, but the UI never retains several decoded originals.

Pointer movement never invokes the backend. It writes coordinates to refs/CSS custom
properties and schedules at most one visual update per animation frame. The lens
clips a duplicate rendering of the current original and positions that rendering
from pure source-coordinate math. This avoids canvas pixel copies and avoids React
state updates for every pointer sample.

### 3.4 Settings schema and data flow

`ViewerSettings` advances from schema version 1 to version 2 and adds typed enums for
shape, magnification, and area. The frontend public settings shape mirrors those
bounded values.

Version handling is explicit:

- missing or malformed settings use all defaults;
- version 1 preserves its valid thumbnail density and receives the three new defaults;
- version 2 requires the exact bounded shape;
- unknown future versions fall back safely rather than being partially interpreted.

The settings bridge saves one complete `ViewerSettings` value atomically. The
provider keeps its existing serialized write queue, optimistic display, confirmed
snapshot, and latest-write-wins rollback behavior. A field-specific write must not
recreate the complete value from a stale snapshot and overwrite another preference.

The settings provider passes the three magnifier preferences through `App` to
`ImagePreview`. It never persists `magnifierEnabled`; enabled state remains preview
session state.

## 4. Loading, Failure, and Safety

The fit preview remains visible while an original is requested. When an enabled lens
is under the pointer but the original is not ready, the lens shell follows the
pointer and displays `正在载入原图`. It must not enlarge the fit proxy and present it
as original detail.

If the original exceeds the existing 700 MB / 100 MP decode budget, the main preview
continues normally. The lens stays enabled but shows a compact `原图超出安全预览限制`
state, and the stage emits one contained local explanation. Viewer never bypasses
the budget or begins an unbounded decode. Other original failures use `无法载入原图`.

Failure feedback is scoped to the current entity and request identity. It does not
flash again on every pointer entry. Navigation clears the current failure and tries
the next image normally. Removal or invalidation cancels the request, hides the lens,
and uses the existing unavailable-image state.

Settings-save failure restores the last confirmed complete settings value and uses
the current `无法保存设置` feedback. It does not leave only one field rolled back.

## 5. Accessibility and Responsive Behavior

- The toolbar button has a stable `放大镜` name, `aria-pressed`, and Q shortcut hint.
- A polite live region announces `放大镜已开启` and `放大镜已关闭`.
- The button provides the non-pointer route to the complete feature.
- Q is handled only by the focused preview dialog, ignores modifiers, composition,
  repeat, already-prevented events, and disabled image states.
- Forced-colors mode retains a visible lens boundary and readable loading/error text.
- Reduced-motion mode removes decorative transitions; cursor tracking itself remains
  immediate.
- At 200% page zoom and the supported small viewport, all settings controls, preview
  toolbar controls, the lens, and the floating image navigation remain reachable and
  bounded.
- Lens shape and active state are never communicated by color alone.

## 6. Verification Strategy

### 6.1 Pure and component tests

- Anchor-preserving zoom at center and edge points.
- Delta normalization, 10%-800% clamping, and both-axis pan bounds.
- 0/90/180/270-degree inverse source mapping.
- Circle and rounded-rectangle geometry for all three area values.
- Q/button synchronization, repeat/modifier suppression, and `aria-pressed`.
- Pointer image entry, image exit, stage entry, window exit, and cursor restoration.
- Enabled-state continuity across previous/next navigation and reset on preview close.
- One-current-original request ownership, stale completion rejection, and navigation
  cancellation.
- Loading, budget failure, decode failure, removal, and recovery on the next image.
- Schema-v1 migration, schema-v2 exact round trip, bounded value rejection, ordered
  multi-field writes, and full-value rollback after save failure.

### 6.2 Formal visual acceptance

- Add `PRE-08` for an active, loaded, default circle/4x/small magnifier over a real
  image.
- Extend `DIA-01` so the formal Settings dialog includes the real image-preview
  controls and retains its existing focus/footer contract.
- Include magnifier loading and error structure in component evidence without making
  failure text a permanent normal-state visual.
- Re-run both required acceptance viewports and the existing A11Y-05 high-zoom state.

### 6.3 Native and hardware acceptance

Automated DOM wheel tests prove the normalized event contract but do not claim real
trackpad fidelity. Completion requires a real MacBook trackpad check covering:

1. pinch in/out around the finger center;
2. horizontal, vertical, and diagonal two-finger pan;
3. hard stop at every image boundary with no image navigation;
4. Q and toolbar-button toggle parity;
5. pointer exit and re-entry while enabled;
6. previous/next navigation while enabled;
7. circle and rounded-rectangle rendering at every size and magnification;
8. no visible cursor desynchronization or stale original after fast movement and
   navigation.

If the current WebKit wheel contract fails this real-hardware gate, the fallback is a
small macOS-only adapter obtained through Tauri's documented `with_webview` escape
hatch. The native adapter may normalize magnification events into the same frontend
viewport actions; it must not create a second viewport state model or switch to
whole-page WKWebView magnification.

## 7. Non-goals

- Magnifier support in multi-image compare.
- Persisting whether the magnifier is currently enabled.
- Free-form lens resizing or arbitrary numeric magnification.
- Offset-lens placement modes.
- Replacing the existing full-original safety budget with an unbounded or tiled image
  engine in this feature.
- Trackpad gestures for previous/next navigation.
- Native AppKit rendering unless the documented hardware fallback gate is triggered.

## 8. Reference Basis

- MDN documents that trackpad zoom sends wheel events with `ctrlKey` set:
  <https://developer.mozilla.org/en-US/docs/Web/API/Element/wheel_event>
- Apple documents that `WKWebView.allowsMagnification` defaults to false:
  <https://developer.apple.com/documentation/webkit/wkwebview/allowsmagnification>
- Tauri documents macOS WKWebView access through `WebviewWindow::with_webview`:
  <https://docs.rs/tauri/latest/tauri/webview/struct.WebviewWindow.html#method.with_webview>

