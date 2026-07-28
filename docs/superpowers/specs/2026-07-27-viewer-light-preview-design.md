# Viewer Light Preview Design

Date: 2026-07-27
Status: Approved

## Problem

Viewer's default workspace uses a light visual system, but opening an image
replaces it with a nearly black full-window surface and a dark toolbar. The
change is abrupt and makes preview feel like a separate application.

The mismatch is not caused by the operating-system appearance. The original
preview implementation hard-codes a dark background and white foreground on
the shared `.preview-overlay` and `.preview-toolbar` rules. Image preview
inherits both rules without a light override. Text preview already overrides
its document surface to light colors, but its toolbar remains dark.

## Goals

- Make image and text previews feel native to Viewer's default light workspace.
- Preserve the focused, full-window preview layout.
- Keep white-background images visually distinct from the preview canvas.
- Use consistent button, select, hover, and keyboard-focus treatments.
- Preserve all existing preview interactions, image rendering, and focus
  behavior.

## Non-goals

- Replacing the full-window preview with a centered modal or separate window.
- Changing image loading, caching, zoom, pan, rotation, or navigation logic.
- Changing text decoding, Markdown rendering, or external-link handling.
- Adding an application-wide theme system or a user-selectable preview theme.
- Restyling the compare workspace.

## Approved User Experience

Image and text previews remain full-window dialogs. Their presentation changes
from an opaque dark viewer to a light surface consistent with the workspace.

The shared preview surface uses `#f5f6f8` with `#1f2328` foreground text. The
toolbar and image-navigation footer use `#fbfcfd` surfaces separated from the
content by `#d8dce2` borders.

Toolbar buttons, footer buttons, and the text-encoding select use:

- white backgrounds and `#8a94a3` borders at rest;
- `#f3f5f7` backgrounds and `#747f8e` borders on hover;
- a visible `#2477d4` keyboard-focus outline;
- the existing compact dimensions and labels.

The image stage uses a slightly darker `#edf0f3` neutral canvas. The rendered
image receives a `#d8dce2` one-pixel boundary and a restrained
`0 8px 28px rgb(34 42 53 / 14%)` shadow. This keeps white source images
separate from the canvas without introducing a card-like frame or cropping
source pixels.

The image-navigation footer retains its centered previous/count/next layout.
The text preview retains its light document body and sticky toolbar, now using
the shared light toolbar treatment.

Loading text inherits the normal foreground color. Preview errors use
`#9d1c13` so they remain distinct and readable on the light surface.

## Architecture and Data Flow

This is a presentation-only change in `ui/src/styles/app.css`. The existing
`ImagePreview` and `TextPreview` component structures and class names provide
all required styling hooks:

- `.preview-overlay` owns the common light dialog surface and foreground;
- `.preview-toolbar` owns the shared toolbar and controls;
- `.image-preview-stage` owns the neutral image canvas;
- `.image-preview-stage img` owns the image boundary and shadow;
- `.image-preview > footer` owns the light navigation surface;
- `.text-preview` retains scrolling and document layout behavior.

No React state, props, event handling, backend requests, IPC, persistence, or
preview data flow changes.

## Error Handling

The existing loading and error states remain structurally and behaviorally
unchanged. Only their foreground colors are adapted to the light surface.
Image-budget and load failures still return to fit mode where currently
implemented, and text-decoding failures retain their existing messages and
recovery paths.

## Testing

CSS contract tests will verify:

- the shared preview surface and toolbar use the approved light colors;
- toolbar and footer controls expose default, hover, and `:focus-visible`
  treatments;
- the image stage, image boundary, image shadow, and footer use the approved
  light hierarchy;
- preview alerts use the approved error color;
- the text preview retains its sticky toolbar and light document surface.

Existing `ImagePreview` and `TextPreview` component tests will continue to
verify loading, fit/original behavior, rotation, navigation, text decoding,
link handling, Escape, and focus behavior. The complete UI suite, static
checks, and production build will be run before completion.

## Acceptance Criteria

- Double-clicking an image opens a light preview that visually belongs to the
  default Viewer workspace.
- White-background images remain visibly bounded against the canvas.
- The toolbar, controls, and footer no longer use the old black presentation.
- Text preview uses the same light toolbar and control system.
- Hover and keyboard-focus states remain clearly visible.
- Zoom, pan, rotation, previous/next navigation, closing, text decoding, and
  preview error behavior remain unchanged.
