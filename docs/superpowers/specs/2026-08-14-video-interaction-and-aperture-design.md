# Video Interaction Latency and Surface Aperture Design

Date: 2026-08-14
Status: proposed

## Problem

The current player still feels slow during timeline seeking and live window resize. A reproduced native sample shows that this is not a limitation of libmpv itself:

- `seek_absolute_us` waits inside libmpv while running on the AppKit main thread.
- each forced resize redraw performs an OpenGL render and buffer flush on the AppKit main thread;
- the draw path synchronously reads playback properties from libmpv on the AppKit main thread;
- geometry updates perform redundant OpenGL-context work.

The preview also leaks transparent window pixels. After the first frame, CSS makes the entire preview, stage, and document roots transparent, then tries to repaint the area outside the native surface with rounded matte borders. During resize, CSS/native rounding or update-order differences expose the desktop around the video.

## Goals

1. Timeline interaction gives immediate feedback and exact seeking never blocks AppKit.
2. Live resize tracks the window continuously without fixed-rate stair stepping.
3. Paused video redraws once after resize settles; playing video uses natural frame callbacks.
4. Only the exact native video aperture is transparent. Every other preview pixel is an opaque theater matte.
5. Old seek, resize, and frame results cannot overwrite a newer generation or request.

## Non-goals

- replacing libmpv or the existing OpenGL renderer;
- changing video codecs, thumbnail generation, controls, or the browser layout;
- adding a display-link render thread in this iteration;
- release signing or packaging changes.

## Architecture

### Serialized media command worker

The libmpv client command/property path moves to one dedicated serial worker. AppKit retains only NSView, NSOpenGLContext, and presentation work that macOS requires on the main thread.

- Seek requests enter a newest-wins mailbox with `(generation, request_id, phase, target_us)`.
- `preview` performs a fast keyframe seek.
- The render callback acknowledges the first visible frame for that request.
- Only after that acknowledgement does the worker perform the exact `commit` seek.
- A newer request invalidates the pending preview, acknowledgement, and commit.
- Playback time and EOF state are maintained as worker-owned cached snapshots. The main render path reads snapshots without calling synchronous libmpv property APIs.

This preserves existing generation safety while removing libmpv command-lock waits from AppKit.

### Resize presentation path

Each geometry request remains sequence-numbered and newest-wins.

- During live resize, AppKit applies only the newest NSView frame.
- The current fixed 30 Hz forced redraw loop is removed.
- Playing video is redrawn by normal libmpv frame callbacks.
- Paused video schedules one trailing retained-frame redraw after geometry has settled.
- Duplicate OpenGL-context updates are removed; each applied geometry has one context update boundary.

The trailing redraw is cancelled by a newer geometry sequence, session close, or generation change.

### Opaque theater with a single transparent aperture

The preview document and overlay remain opaque at all times. Transparency is limited to an explicit aperture whose integer pixel rectangle is derived from the same fitted `VideoSurfaceRect` sent to native code.

- `html`, `body`, `#root`, `.video-preview`, and `.video-preview-stage` keep the theater background.
- Four real opaque matte regions cover the area above, right, below, and left of the aperture.
- The aperture alone is transparent after the first frame is ready.
- Before first-frame readiness, an opaque reveal veil covers the aperture.
- Title, navigation, controls, loading, and errors always render above the matte/aperture layers.
- When geometry is unknown or invalid, there is no aperture and the whole stage remains opaque.

The aperture/matte values use the exact integer fitted rectangle, not separately rounded CSS calculations. A resize update publishes native geometry and aperture geometry from the same calculation.

## Data flow

### Seek

`pointer input -> UI preview request -> worker fast seek -> native frame callback -> request acknowledgement -> worker exact seek -> progress snapshot/event`

### Resize

`ResizeObserver -> fitted integer rect -> newest geometry mailbox -> AppKit NSView frame -> natural frame callback OR one paused trailing redraw`

### Surface visibility

`fitted integer rect -> native surface rect + CSS aperture variables -> first-frame gate -> aperture becomes transparent`

## Failure and cancellation behavior

- Worker shutdown cancels queued commands and joins before libmpv/client teardown.
- A failed media command publishes the existing normalized video failure without leaving the UI in a seeking state.
- A stale generation/request/geometry sequence is a successful no-op.
- If the first-frame acknowledgement never arrives, exact commit is not allowed to overtake the preview; a newer request or close cancels it.
- Invalid aperture geometry fails closed to a fully opaque stage.

## Verification

### Deterministic tests

- seek and property calls never execute on the AppKit main thread;
- preview frame acknowledgement precedes exact commit;
- rapid seeks retain only the newest request;
- resize bursts apply newest geometry and issue no periodic forced redraw;
- paused resize produces exactly one trailing redraw;
- close/generation changes cancel pending command and redraw work;
- document/overlay remain opaque and only the explicit aperture is transparent;
- aperture and native surface use identical integer coordinates, including fractional stage geometry and rotation.

### Native acceptance

- repeat the measured timeline-click sequence and confirm AppKit has no libmpv command-lock wait;
- continuously resize while playing and paused, checking smooth visual tracking and final frame correctness;
- seek repeatedly across distant positions and confirm immediate preview followed by exact convergence;
- inspect all four window edges during resize for zero desktop leakage;
- confirm ended video retains its final frame.

## Completion criteria

- no synchronous libmpv command/property call on AppKit in the seek/render hot paths;
- no fixed-rate forced GPU redraw during live resize;
- no visible transparent pixels outside the fitted video aperture;
- focused tests, strict lint/type checks, and native interaction acceptance pass.
