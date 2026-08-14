# Viewer Video Interaction Performance Design

Date: 2026-08-14
Status: Pending written review

## Problem

Viewer uses libmpv's render API with hardware decoding, but high-frequency
interactions are routed through the same serialized command path as lifecycle
operations. Window resize samples and timeline seeks cross React, Tauri IPC,
the runtime transition lock, the application service lane, and a synchronous
AppKit main-thread dispatch before reaching the native surface or libmpv.

This causes two user-visible failures:

- rapid resize produces cumulative geometry lag because each accepted rectangle
  must finish a full command round trip before the next one can be applied, and
  changing the native view frame does not itself guarantee an immediate redraw
  of the already decoded picture;
- timeline dragging produces many `absolute+exact` seeks. Old seeks are not
  superseded, so expensive precise decoding work and completion events can trail
  behind the pointer.

The playback core is not being replaced. The performance work is confined to
the interaction and native-surface coordination architecture around libmpv.

## Confirmed Direction

Retain libmpv and split high-frequency resize and scrub traffic from the
lifecycle command lane.

- Geometry becomes a latest-value native update: a newer rectangle replaces an
  older pending rectangle instead of waiting behind it.
- Timeline dragging uses fast keyframe previews. Pointer release sends exactly
  one precise seek.
- Resize and seek completions are generation- and request-bound so stale work
  cannot alter the active picture or application state.
- Native geometry application redraws the current decoded picture instead of
  waiting for a future media frame.

## Goals

- Keep video geometry visually synchronized with continuous window resize.
- Prevent high-frequency inputs from creating an unbounded command backlog.
- Make scrub feedback respond to the newest pointer position, not earlier
  positions in the drag sequence.
- Preserve exact final positioning after pointer release.
- Prevent resize, scrub, play/pause, navigation, close, and project teardown
  from blocking or resurrecting one another.
- Add measurements that identify latency at the UI, IPC, AppKit, libmpv, and
  first-render boundaries.
- Preserve the current native contain-fit surface, hardware decoding, first
  frame gate, ending-frame retention, controls, shortcuts, and security model.

## Non-goals

- Replacing libmpv with AVPlayer, VLC, a browser video element, or another
  playback framework.
- Adding streaming, subtitles, playlists, quality selection, or codec support.
- Cropping video to fill the stage.
- Changing the player visual design completed by the existing experience
  redesign.
- Making precise seek instantaneous for damaged, unindexed, remote, or
  unusually long-GOP media.
- Adding a new dependency for scheduling or metrics.

## Architecture

### 1. High-frequency geometry coordinator

`useVideoBridge` continues to calculate the fitted DOM rectangle, but geometry
publication no longer waits in the general `VideoRuntime::execute` transition
lane.

The desktop runtime owns a generation-scoped geometry coordinator with these
semantics:

- `publish(generation, sequence, rect)` validates the active generation and
  overwrites the pending rectangle;
- at most one AppKit main-thread drain is scheduled at a time;
- the drain takes the newest rectangle, applies it, and checks once for a newer
  value before becoming idle;
- rectangles older than the active generation or latest sequence are ignored;
- close, replacement open, project close, and application shutdown invalidate
  pending geometry before native teardown begins.

The coordinator is a bounded latest-value mailbox, not a FIFO queue. A burst of
one hundred measurements may apply one or several current rectangles, but it
must not execute one hundred historical rectangles.

The native surface applies the frame and updates the OpenGL drawable on the
AppKit main thread. The render loop then redraws the already decoded current
picture for the resized drawable. Redraw requests are coalesced so resize does
not create concurrent OpenGL renders.

The normal lifecycle lane remains authoritative for mount, reveal, close,
generation replacement, fullscreen state, and teardown. The geometry path may
change only the active surface rectangle and request a redraw.

### 2. Two-phase seek coordinator

The timeline exposes two distinct intents:

- `previewSeek(timeUs, requestId)` while the pointer is dragging;
- `commitSeek(timeUs, requestId)` on pointer release or keyboard activation.

Preview requests are latest-only and use libmpv `absolute+keyframes`. A newer
preview request invalidates the previous request before its completion can
publish UI state. The native coordinator permits at most one in-flight preview
and retains only one pending replacement.

Commit requests invalidate all pending preview work and use
`absolute+exact`. One drag gesture emits exactly one commit. The application
remains in `Seeking` until the first rendered frame associated with the commit
request is observed, then returns to the appropriate paused or playing state.

Every seek carries:

- active video generation;
- monotonically increasing seek request ID;
- intent (`preview` or `commit`);
- bounded target time.

Progress, frame-ready, and seek-completed events include the request identity
needed by the runtime to reject stale results. A completion from a previous
generation, an older request, or a preview superseded by a commit cannot update
the displayed time or state.

Keyboard seeking is a commit operation. Frame stepping keeps its current exact
semantics and remains on the lifecycle/transport lane.

### 3. Command-lane ownership

The command paths have explicit responsibilities:

- **Lifecycle lane:** open, close, replace, retry, fullscreen transition, and
  project/application teardown.
- **Transport lane:** play, pause, frame step, volume, mute, and rate.
- **Geometry coordinator:** latest active surface rectangle and redraw only.
- **Seek coordinator:** latest preview and one precise commit.

Lifecycle invalidation wins over every other lane. Transport commands remain
generation guarded but do not wait for historical resize or scrub input.
Geometry and preview failures are contained to their request and do not close
the active video. A precise commit failure transitions the active preview to
the existing normalized playback failure state.

No lane may hold a Tokio mutex while synchronously waiting on unrelated work
from another lane.

## Data Flow

### Continuous resize

```text
ResizeObserver
  -> one measurement per browser animation frame
  -> fitted active-generation rectangle
  -> native latest-value geometry publication
  -> one scheduled AppKit drain
  -> NSView/OpenGL drawable update
  -> coalesced redraw of current decoded picture
```

### Timeline drag and release

```text
pointer move
  -> local displayed time updates immediately
  -> newest preview request replaces pending preview
  -> libmpv absolute+keyframes
  -> only matching preview frame may publish

pointer release
  -> invalidate preview requests
  -> one absolute+exact commit
  -> wait for matching rendered frame
  -> publish final time and stable playback state
```

## Error And Lifecycle Handling

- A transient invalid resize sample is discarded; the last valid fitted surface
  remains visible.
- AppKit geometry failure records a bounded diagnostic and permits a newer
  rectangle to retry. It does not tear down playback.
- Preview seek failure is ignored when already superseded. If it is still the
  latest preview, controls remain usable and release can still issue the exact
  commit.
- Commit seek failure uses the existing safe playback error and retry surface.
- Video navigation, Done, project close, WebView reload, and application quit
  invalidate coordinators before releasing the native session.
- Paused and ended videos redraw their retained frame during resize; they do not
  reveal the ambient matte or a black frame.
- Playback state events remain generation scoped. Request identity adds a
  narrower seek boundary and does not replace generation ownership.

## Performance Instrumentation

Debug/test instrumentation records monotonic timestamps for:

- browser measurement and seek intent creation;
- desktop command receipt;
- latest-value replacement or acceptance;
- AppKit geometry application;
- libmpv seek issue;
- first matching rendered frame;
- stale request rejection.

Tests consume structured measurements rather than parsing user paths or media
contents. Production errors remain path-safe. Instrumentation must not expose
absolute source paths through IPC or UI events.

## Acceptance Criteria

### Bounded behavior

- A burst of 120 resize measurements retains at most one pending rectangle and
  schedules at most one AppKit drain concurrently.
- A burst of 120 pointer moves retains at most one pending preview seek and one
  in-flight preview seek.
- A complete drag gesture issues exactly one precise commit seek.
- Old geometry, preview, commit, frame, and completion events cannot alter a
  newer generation or request.

### Visible behavior

- Continuous resize does not accumulate historical geometry work; after input
  stops, the native surface reaches the newest fitted rectangle within two
  display refresh intervals under the focused acceptance fixture.
- Playing, paused, and ended frames remain visible and fitted throughout live
  resize without a black flash.
- Timeline time text follows the pointer immediately during drag.
- Scrub previews favor responsiveness and may land on a nearby keyframe.
- Releasing the pointer produces one exact final frame and stable time.
- Resize activity does not delay play/pause, and seek activity does not delay
  geometry publication.

### Verification

- TypeScript unit tests prove rAF measurement coalescing, one commit per gesture,
  and stale request rejection.
- Rust tests prove bounded latest-value coordinator behavior, generation
  invalidation, lane independence, keyframe-preview/exact-commit command flags,
  and redraw after geometry application.
- Native focused acceptance exercises continuous resize while playing, paused,
  and ended, plus rapid scrub and exact release on the reviewed local H.264
  fixture.
- Focused tests, UI check/build, strict Rust Clippy, workspace tests, native
  acceptance evidence, and `pnpm verify` pass before completion is claimed.

## Rejected Alternatives

### React-only throttling

Additional debounce or longer delays reduce command volume but leave the IPC,
runtime locks, AppKit dispatch, exact-seek cost, and redraw pacing unchanged.
It can hide symptoms but cannot provide bounded native work.

### Replace libmpv with AVPlayer

AVPlayer would provide a more platform-native layer but require a broad engine,
codec, event, test, and packaging rewrite. It would not solve serialized DOM-to-
native high-frequency commands by itself. The current failures are above the
decoder boundary, so replacing the playback core is not justified.

### Apply every resize and exact seek

Preserving every high-frequency input guarantees backlog under load. Resize and
scrub semantics require the newest value, not historical replay; only the final
seek requires exactness.
