# Viewer Native Video Theater Refactor Design

Date: 2026-08-16
Status: Architecture and extreme compact light chrome approved
Supersedes:

- `2026-08-13-viewer-video-player-experience-redesign.md` for player composition ownership;
- `2026-08-14-video-interaction-and-aperture-design.md` for transparency/aperture ownership;
- the geometry and seek portions of `2026-08-14-viewer-video-interaction-performance-design.md`.

## Why This Is A Structural Refactor

The current player has repeatedly received local fixes for visible seams, transparent regions,
resize lag, and slow timeline seeking. Those fixes cannot make the system reliable because the
frontend and native renderer both try to own the same visible rectangle on different clocks.

Today the player does all of the following:

1. makes the Tauri window, WKWebView, document roots, preview, and stage transparent;
2. mounts a native `NSOpenGLView` below the WKWebView;
3. measures a DOM rectangle in React;
4. computes the fitted video rectangle in TypeScript;
5. sends that rectangle across Tauri IPC to Rust and AppKit;
6. paints four CSS matte rectangles around the predicted native rectangle.

During live resize the browser can commit its transparent aperture before AppKit moves the native
surface. The desktop is then visible through the mismatch. No extra border, overlap pixel, debounce,
or CSS transition can turn these independently scheduled updates into one atomic composition.

Timeline interaction has a similar ownership error. Pointer release waits for the current preview
seek to publish a frame and only then sends the exact commit seek. The user therefore pays for two
serial decode operations before the final picture is stable.

The playback engine remains libmpv. The problems are above the decoder boundary: ownership,
composition, and scheduling.

## Confirmed Direction

The player becomes one native-owned opaque theater with a transparent WebView overlay:

- AppKit owns the full theater background, native video surface, contain-fit calculation, and
  high-frequency live-resize geometry.
- The WKWebView remains above the theater and owns only semantic UI chrome: title, Done,
  navigation, loading/error messages, timeline, and controls.
- The browser never cuts a transparent video aperture and never predicts the native surface frame.
- Timeline click performs one exact seek. Timeline drag uses latest-wins preview seeks; release
  cancels preview ownership and issues the exact commit immediately.

This is not another visual patch. It deletes the cross-layer aperture contract.

## Goals

1. The desktop can never be visible through the player, including during live resize, loading,
   navigation, failure, ending, and close.
2. The video frame follows window resizing on AppKit's layout clock without DOM measurement or IPC.
3. Playback controls remain readable and balanced at wide, compact, and narrow sizes without the
   current large empty header and detached oversized dock.
4. Timeline click and drag communicate distinct intent and never serialize preview before commit.
5. Paused and ended media retain the current decoded frame while resizing.
6. Generation, seek request, and lifecycle invalidation remain fail-closed.
7. Acceptance is based on native captures and latency measurements, not CSS source inspection.

## Non-goals

- replacing libmpv, VideoToolbox, or the current bundled runtime;
- adding streaming, captions, playlists, or quality selection;
- changing thumbnail generation or the project browser outside the video card/preview handoff;
- changing the release-signing decision;
- preserving the current `video_set_surface_rect` API for compatibility when it has no remaining
  owner.

## Composition Architecture

### Current composition to remove

```text
NSWindow (transparent)
└── content view
    ├── NSOpenGLView (video, below)
    └── WKWebView (transparent)
        └── React stage
            ├── transparent aperture prediction
            ├── top matte
            ├── right matte
            ├── bottom matte
            └── left matte
```

The visible result depends on two independent rectangles matching every frame.

### Target composition

```text
NSWindow (opaque theater color)
└── content view
    ├── MacVideoTheater (opaque, fills content bounds)
    │   └── NSOpenGLView (native contain-fit child)
    └── WKWebView (transparent overlay)
        └── React chrome only
            ├── compact top bar
            ├── loading/error overlay
            ├── navigation
            └── integrated bottom controls
```

The theater exists before the media surface and remains after the surface is hidden or removed.
Even if WebView and AppKit updates land in different run-loop turns, the exposed pixel is always the
opaque theater color, never the desktop.

### Window and WebView contract

- The NSWindow is opaque and uses the theater background color. The existing transparent-window
  configuration is removed.
- The WKWebView under-page background remains clear so the native theater and video are visible.
- A failed WebView transparency configuration is still typed and prevents surface activation.
- React roots do not change global document backgrounds when preview opens.

### Native theater ownership

`MacVideoTheater` owns:

- an opaque `NSView` inserted below the WKWebView;
- an `NSOpenGLView` child for libmpv rendering;
- the active media display width, height, and rotation;
- contain-fit layout in AppKit coordinates;
- frame/bounds observation during live resize;
- first-frame hide/reveal and final-frame retention;
- removal of its observers before surface/context teardown.

The theater view autoresizes with the WKWebView/content view. A native frame-change observer or
equivalent AppKit layout callback recomputes the fitted child frame directly on the main thread.
It does not wait for React, Tauri IPC, Tokio, or a runtime transition lock.

The contain-fit result is integer-stable in backing pixels. Rotation 90/270 swaps the effective
media dimensions before fitting. Invalid or missing media geometry leaves the video child hidden
while the theater remains opaque.

### Render and resize rules

- Playing media relies on normal libmpv frame callbacks after the native child frame changes.
- Paused and ended media request one coalesced retained-frame redraw after native geometry settles.
- Repeated AppKit callbacks with the same fitted frame are no-ops.
- A generation change, close, project teardown, or WebView destruction cancels pending redraw work
  before releasing the render context.
- No production resize path calls `video_set_surface_rect`.

## Frontend Architecture

### Approved light chrome revision (2026-08-17)

The video preview must use the same light visual language as the rest of Viewer. The earlier dark
theater chrome is superseded. Only the decoded image and unavoidable contain-fit letterboxing may
remain dark; application-owned chrome is light.

- The top command bar is exactly 50 CSS pixels high. It uses `--viewer-surface`, a single
  `--viewer-border` divider, `--viewer-text`, and `--viewer-text-secondary`.
- The bottom control bar is exactly 88 CSS pixels high. It uses the same surface and divider and
  contains one compact timeline row above one compact transport row.
- The native viewport is the full window content bounds minus those 50 and 88 pixel reservations.
  AppKit, not the browser, continues to own that geometry.
- The non-video stage uses the existing Viewer canvas/application neutrals. No full-window dark
  background, translucent dock, detached card, heavy shadow, or dark metadata plaque remains.
- Ordinary buttons use the existing white/soft-surface controls and gray borders. Play/pause is the
  only persistent accent-filled control. Focus, hover, active, disabled, and forced-color states
  remain structural and accessible.
- At compact widths, frame stepping, the volume slider, and rate selector collapse behind More;
  play/pause, mute, fullscreen, timeline, current time, and duration remain directly accessible.
- The 50/88 reservations do not grow at narrow widths. Text truncates and secondary controls
  collapse instead of increasing chrome height.

The compact layout preserves the current native resize, first-frame, seek, generation, teardown,
and ended-poster behavior. This revision changes visual presentation and fixed native reservations,
not the playback engine or event architecture.

### Component boundaries

- `VideoPreview` owns dialog lifecycle, current file, player state, and overlay composition.
- `VideoPreviewTopBar` owns filename, duration, Retry, and Done.
- `VideoPreviewNavigation` owns previous/next video actions and the position label.
- `VideoControls` owns the single integrated control surface.
- `VideoTimeline` owns pointer/keyboard timeline semantics and thumbnail presentation.
- `useVideoBridge` owns lifecycle/events/commands but no longer measures DOM geometry.

The top bar and navigation can remain local components initially if extraction would not improve
behavior, but `VideoPreview` must no longer contain native geometry or aperture calculations.

### Visual hierarchy

The decoded picture is the primary surface. Chrome consists of two calm layers:

1. A compact top bar with filename/duration on the left, navigation in the center, and Retry/Done
   on the right.
2. One bottom safe-area group containing the two-row timeline and transport controls.

The redesign removes:

- the large empty top matte owned by CSS/native mismatch;
- four aperture matte elements;
- a control dock that visually reads as a detached footer;
- duplicated or weakly distinguished icon buttons;
- transparent gaps between the picture, navigation, and controls.

The controls use the existing Viewer tokens, Inter/SF/PingFang font stack, and Lucide asset registry.
No handcrafted icons or new visual dependency is introduced.

### Responsive layout

- **Wide (>= 1100 px):** timeline/time row; frame previous, play/pause, frame next on the left;
  mute/volume, rate, and fullscreen on the right. Navigation remains centered in the top bar.
- **Compact (760-1099 px):** timeline/time remain full width; play/pause, mute, fullscreen, and More
  remain visible; frame stepping, volume, and rate move into More.
- **Narrow (< 760 px):** title truncates to one line, Done remains 44 px minimum, navigation and
  controls stack without overlap, and only one accessible instance of each secondary control exists.

All visible actions have a minimum 44 by 44 CSS-pixel target, explicit border, hover, pressed,
disabled, and focus-visible states. The dock never extends outside the opaque theater.

### First-frame and failure states

- Before first frame, React may show semantic loading content, but it does not paint or remove a
  transparent aperture.
- The native video child stays hidden until a decoded picture is ready.
- On failure, the theater remains opaque and the localized error surface remains interactive.
- On ended, the generated useful cover overlays a source-authored black tail; controls report the
  final duration.
- On navigation, the old generation is closed before a new surface is revealed.

## Timeline Interaction Architecture

### Click

A pointer press/release that does not cross the drag threshold is a single exact commit:

```text
pointer up -> local time update -> invalidate preview epoch -> exact commit seek
```

No preview seek is issued for a click.

### Drag

Dragging updates local time and thumbnail position immediately. At most one preview request is in
flight and one newest replacement is pending:

```text
pointer move -> local CSS/visual update -> latest preview mailbox -> keyframe seek
pointer up   -> invalidate preview epoch -> exact commit immediately
```

Commit does not wait for a preview frame. A late preview frame or completion cannot overwrite the
commit because request identity and intent are checked at publication.

### Keyboard

Home, End, ArrowLeft, and ArrowRight are exact commits. Frame stepping remains a distinct transport
operation. Keyboard behavior does not create thumbnail preview work.

### Frontend update budget

Pointer movement updates one animation-frame-owned visual snapshot. It does not create multiple
independent React state commits for time, offset, width, dragging, and preview publication in the
same pointer event. Thumbnail requests remain debounced and generation/request/bucket scoped.

## API Changes

### Remove

- `videoSetSurfaceRect` from `ViewerBridge` and `VideoPreviewBridge`;
- `video_set_surface_rect` Tauri command and DTO;
- application `SurfaceRect` publication used only by the browser-resize path;
- CSS aperture variables and four matte elements.

### Add or reshape

- native theater mount input containing active media geometry rather than a browser rectangle;
- native theater diagnostics for bounds changes, fitted frame changes, and retained redraws;
- seek cancellation/publication rules where commit invalidates preview immediately.

Any temporary compatibility shim must be private, marked for deletion in the same change, and must
not be called by production UI.

## Error, Cancellation, And Teardown

- Lifecycle generation owns theater, render context, media worker, and event publication.
- Close invalidates resize observation and seek publication before context/client teardown.
- A stale native resize callback is a no-op.
- A stale preview result is a no-op after commit or generation change.
- A commit failure enters the existing normalized retryable player failure state.
- A WebView transparency failure leaves the window/theater opaque and does not attach video.
- Native view and observer removal occur on the AppKit main thread.

## Verification Contract

### Deterministic behavior tests

- native contain-fit covers landscape, portrait, 90/270 rotation, fractional bounds, and backing
  scale rounding;
- theater is opaque before surface mount and after surface unmount;
- NSWindow is not configured transparent;
- WebView transparency occurs before attaching the theater/video hierarchy;
- a resize burst performs native latest-frame layout without browser geometry commands;
- paused/ended resize coalesces one retained redraw;
- `VideoPreview` renders zero aperture matte elements and does not mutate root transparency;
- `useVideoBridge` opens/controls/closes without `ResizeObserver` or `videoSetSurfaceRect`;
- click emits one commit and zero previews;
- drag emits latest-wins previews and one immediate commit;
- preview completion after commit cannot publish state.

### Real native acceptance

Automated acceptance must launch the real Tauri app with the reviewed bundled runtime and capture:

1. continuous live resize while playing;
2. continuous live resize while paused;
3. resize after ended with final frame retained;
4. all four window edges and titlebar/content boundaries for desktop-pixel leakage;
5. timeline click latency from pointer release to first matching exact frame;
6. timeline drag responsiveness and exact release convergence;
7. wide, compact, narrow, and fullscreen control layouts.

Acceptance fails on any visible desktop pixel inside the content area, black/transparent flash,
stale frame replacement, control overlap, or unbounded resize/seek backlog.

CSS source-text tests are not evidence of visual correctness. Visual comparison uses the supplied
failure screenshot and a same-viewport post-refactor native capture side by side.

## Performance Budgets

- Live resize never crosses JavaScript-to-Rust IPC for video geometry.
- Native video frame reaches the newest theater bounds within two display refresh intervals after
  the last resize sample.
- Timeline click issues the exact seek in the pointer-release task without waiting for preview.
- Drag retains at most one in-flight preview and one pending replacement.
- One gesture issues exactly one exact commit.
- Play/pause and Done remain responsive while preview seek work is active.

## Local Visual Handoff

Figma is deliberately removed from this workflow. Account state and remote editor permissions must
not block the refactor.

The visual source of truth consists of:

- the user-supplied failure screenshot at its original viewport;
- a repository-local static target showing wide, compact, and narrow layouts with existing Viewer
  tokens and icon assets;
- same-viewport screenshots from the real Tauri application after implementation;
- a side-by-side visual review that records spacing, clipping, hierarchy, desktop leakage, control
  contrast, and responsive behavior.

The static target is a design reference rather than an alternative production frontend. Final
acceptance always uses the real native application and reviewed bundled runtime.

## Completion Criteria

- the browser aperture/matte ownership is deleted, not hidden behind another style;
- AppKit owns opaque theater composition and live video geometry;
- timeline click/drag follow the single-commit contract;
- the integrated overlay layout passes wide/compact/narrow visual acceptance;
- real native resize, leakage, seek, and final-frame acceptance pass;
- focused tests, UI check/build, strict Rust Clippy/workspace tests, and `pnpm verify` pass;
- no completion claim is made from unit tests alone.
