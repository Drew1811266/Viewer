# Viewer Filmstrip Scrollbar and Thumbnail Progress Design

Date: 2026-08-05
Status: Approved

## Problem

Folder overview filmstrips currently expose their native horizontal scrollbar
at rest. On systems configured to always show scrollbars, the thumb remains
visually prominent even when the user is not interacting with that folder row.
The scrollbar competes with the thumbnails and makes different rows appear
inconsistently weighted.

The background-task surface can also show `正在生成缩略图` at `0 / N` after
thumbnail requests have actually completed. Viewer mounts the UI under React
`StrictMode`. Its development-only effect preflight runs an effect cleanup and
then sets the effect up again. `ContentBrowser` currently changes its
`mounted` ref to `false` during that cleanup but never restores it. Promise
settlements therefore stop updating the completed and failed counters even
though thumbnail loading itself continues.

## Goals

- Keep native horizontal scrolling, trackpad inertia, drag behavior, scroll
  anchoring, and virtualization unchanged.
- Hide a folder filmstrip scrollbar at rest.
- Reveal it when the pointer is anywhere inside that folder row.
- Keep it visible while keyboard focus is anywhere inside that row.
- Fade it after both hover and focus leave the row.
- Count every visible-content thumbnail request as completed or failed when
  its promise settles during the active component lifetime.
- Preserve protection against state updates after a real unmount.
- Remain correct under React `StrictMode` effect preflight.

## Non-goals

- Replacing the native scrollbar with a custom draggable control.
- Changing filmstrip sizing, layout, scrolling distance, or image loading.
- Counting folder-overview, preview, compare, or cached thumbnail work in the
  content workspace's `正在生成缩略图` task.
- Changing task-bar dismissal timing or task aggregation.
- Adding retries, cancellation, or backend image-pipeline changes.

## Approved Scrollbar Behavior

The existing `.folder-filmstrip` remains the native `overflow-x: auto`
viewport. Its scrollbar track stays transparent. The thumb is transparent in
the resting state and transitions its color over 180 ms.

The thumb uses the normal subtle scrollbar color when either condition is
true:

1. `.folder-filmstrip-row:hover`
2. `.folder-filmstrip-row:focus-within`

The hover target is the complete row, including the identity column and image
area. When neither condition is true, the thumb returns to transparent. The
scrollbar keeps a stable gutter so rows do not change height during the fade.
The implementation supplies the WebKit scrollbar rules used by the Tauri
webview and a standards-based `scrollbar-color` fallback for future engines.

Reduced-motion users receive the same visibility states without the color
transition. The scrollbar remains a native control and retains platform input
and accessibility behavior.

## Approved Progress Lifecycle

`ContentBrowser` continues to own counters for thumbnail requests made by its
visible image grid:

- `requested` increments immediately before calling `requestThumbnail`.
- `completed` increments when that request resolves.
- `failed` increments when that request rejects.
- task status is running while `completed + failed < requested`.
- task status becomes complete or failed after all requested work settles.

The mounted-lifetime ref is set to `true` in the effect setup and to `false`
in that setup's cleanup. React `StrictMode` can therefore execute
setup → cleanup → setup without leaving the live component disabled. A real
unmount still leaves the ref false, so late promise settlements cannot update
state.

The project-level thumbnail cache remains outside this counter. A cache hit or
coalesced in-flight request still returns a promise to `ContentBrowser`; its
settlement counts for the request the visible grid made. No backend or cache
protocol changes are required.

## Error Handling

Resolved requests increment only `completed`. Rejected requests increment only
`failed` and preserve the existing thumbnail error behavior. Counter updates
remain functional-state updates so concurrent settlements cannot overwrite one
another. Promise settlements after a real unmount remain ignored.

## Verification

Automated regression coverage must prove:

- the resting filmstrip thumb is transparent;
- row hover and `focus-within` select the visible thumb color;
- the thumb declares a fade transition and reduced-motion removes it;
- a `ContentBrowser` rendered in `StrictMode` reports a running thumbnail task;
- resolving a pending thumbnail advances its completed count;
- rejecting a pending thumbnail advances its failed count;
- the task reaches the correct terminal state instead of remaining at `0 / N`;
- existing filmstrip scrolling, cache, task-bar, and content-browser tests stay
  green.

After focused tests pass, Viewer must pass the repository's complete
`pnpm verify` command. The development app is then restarted through
`pnpm start:viewer`, and native visual acceptance confirms the scrollbar
visibility states and live thumbnail progress in the real Tauri webview.
