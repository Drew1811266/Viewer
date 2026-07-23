# Viewer Radial Menu Context-Menu Fallback Design

Date: 2026-07-22  
Status: Approved

## Problem

The file radial menu currently opens only from a secondary-button `pointerdown`.
The file's `contextmenu` handler merely suppresses the native menu. macOS WebView
input paths such as trackpad secondary click and Control-click can therefore
reach `contextmenu` without a matching `pointerdown` whose `button` is `2`.
Right-clicking the organization drag handle is also intercepted before the file
card receives the event.

## Desired Behavior

- Secondary-button `pointerdown` continues to open the radial menu immediately
  so press-drag-release marking remains available.
- `contextmenu` opens the same radial menu as a click-mode fallback when the
  current gesture did not already open it.
- A standard right-click that emits both events opens exactly one menu session.
- The complete image or text file item is a valid target, including the
  organization drag handle.
- Left-button organization dragging, selection, preview, focus restoration, and
  keyboard operation remain unchanged.

## Design

`ContentBrowser` will route both event types through one helper that freezes the
same context selection and emits the same `RadialMenuRequest`.

The secondary `pointerdown` path records the target entity for a bounded
one-second deduplication window and includes its pointer id, preserving held
radial gestures. A subsequent `contextmenu` for the same entity consumes that
record and only suppresses the native menu. The record also expires
automatically so a missing `contextmenu` cannot suppress a later independent
gesture. If no matching record exists, `contextmenu` creates a click-mode
request with a null pointer id.

The organization drag handle will only consume unmodified primary-button input
used for dragging. Secondary-button and Control-click input will bubble to the
file item and use the same radial menu paths as the rest of the card.

## Testing

Component tests will verify:

- secondary `pointerdown` still creates one held-gesture request;
- a standalone `contextmenu` creates one click-mode request;
- paired `pointerdown` and `contextmenu` events do not create duplicates;
- Control-click fallback works;
- secondary input on the organization handle reaches the radial menu;
- image and text file items share the behavior;
- primary-button organization dragging remains unchanged.

The full repository verification command must pass after implementation.
