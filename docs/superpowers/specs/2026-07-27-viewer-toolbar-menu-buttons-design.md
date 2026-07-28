# Viewer Toolbar Menu Buttons Design

Date: 2026-07-27
Status: Approved

## Problem

The workspace header exposes three interactive menu triggers:
`筛选与排序`, `结果视图`, and `•••`. They are implemented with
`<details>/<summary>` and already behave like buttons, but their summaries
currently have only padding and a border radius. Without a visible background
or border, the controls look like static labels instead of clickable actions.

## Goals

- Make all three menu triggers immediately recognizable as buttons.
- Keep their appearance consistent with the existing settings button.
- Distinguish default, hover, expanded, and keyboard-focus states.
- Preserve the current menu behavior, accessible names, and keyboard handling.
- Support the existing light and dark color schemes.

## Non-goals

- Replacing `<details>/<summary>` with a new popover implementation.
- Changing the contents, positioning, or behavior of any menu.
- Restyling unrelated content-view controls or buttons elsewhere in Viewer.
- Reorganizing the workspace header.

## Approved Design

The existing `SearchToolbar` and project-menu markup remains unchanged. CSS
targets only the direct summaries of `.search-options-panel`,
`.search-view-panel`, and `.project-menu`.

The two text controls use intrinsic width. The project-menu trigger uses a
compact square footprint suitable for `•••`. All three controls share:

- a white background, neutral border, and 6 px corner radius by default;
- a light neutral background and stronger border on hover;
- a light blue background and blue border while their parent `<details>` is
  open;
- a visible blue `:focus-visible` outline for keyboard navigation;
- a pointer cursor and the same minimum height as adjacent toolbar buttons.

Equivalent dark-scheme colors preserve the same state hierarchy and sufficient
contrast. Disabled styling is unnecessary because these triggers do not expose
a disabled state.

## Architecture and Data Flow

This is a presentation-only change in `ui/src/styles/app.css`. Existing React
state continues to control each `<details>` element's `open` attribute, and CSS
uses the parent element's `[open]` state to render the expanded appearance.
No component props, application state, events, persistence, or backend commands
change.

## Error Handling

The change introduces no new runtime operations or failure paths. If a browser
does not support `:focus-visible`, existing keyboard activation still works;
supported desktop WebView versions receive the enhanced focus indicator.

## Testing

CSS contract tests will verify:

- the three summaries receive the shared button treatment;
- hover, `[open]`, and `:focus-visible` selectors are present;
- the `•••` trigger receives its compact square sizing;
- dark-scheme rules cover the new controls.

Existing `SearchToolbar` and `App` interaction tests will continue to verify
menu activation and expanded-state behavior. The complete UI test suite, type
check, and production build will be run before completion.

## Acceptance Criteria

- The three identified controls visibly read as buttons at rest.
- Hovering a trigger provides feedback.
- Opening a trigger leaves it visibly selected until the menu closes.
- Keyboard focus is clearly visible.
- The `•••` button aligns visually with the adjacent settings button.
- Existing click and keyboard menu behavior remains unchanged.
