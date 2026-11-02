# ADR 0011: Restrained Material Design 3 design system

> Status: Active (0.3.0)
>
> Decision: Viewer 0.3.0 adopts a restrained, light-only Material Design 3
> language implemented as hand-authored semantic CSS tokens. It uses four
> interface colors plus white, a separate eight-color annotation palette,
> rectangular 8px-radius buttons (no pills), and no `color-mix()` or runtime
> Material theming library.
>
> Date: 2026-11-01

## Context

0.2.5 grew from a document-style desktop tool: dense toolbars, mixed radii,
legacy teal accents, and an annotation layer with a fixed coral-red 2px stroke
and yellow sticky-note text. The 0.3.0 cycle adds five annotation tools, a
style bar, a native wgpu/Metal viewport, offline dictation, and a full visual
refresh. Without a single visual contract, the annotation workbench looked like
a different product from the rest of the app.

The planning phase considered a standard Material 3 theme generated from a
seed color with `@material/material-color-utilities`, plus MUI. That route
would have introduced a large dependency, a generated token surface, and M3
components that do not map cleanly to a desktop file browser (dense menu rows,
desktop popovers, a 320px review rail). It also risked `color-mix()` and
dynamic color, which are unsafe on the macOS 13 WebKit baseline.

## Decision

- **Light-only, neutral-first.** The app canvas is `#FBFBFB`; cards, inputs,
  and rails are white. Interface color is limited to `#FBFBFB`, `#E8F9FF`,
  `#C4D9FF`, `#C5BAFF`, plus semantic status colors (success/warning/error)
  and neutral text/outline. Annotation colors are a separate fixed palette and
  never change with the theme.
- **Semantic CSS variables, hand-authored.** `ui/src/styles/tokens.css` is the
  single source for `--viewer-*` tokens. A test (`tokens.test.ts`) fails when a
  consumed variable is undefined. No Material color utility, no runtime theme
  provider, no generated stylesheet.
- **Typography.** Roboto for Latin and the macOS system CJK face (PingFang)
  at runtime; the Pen canvas substitutes Noto Sans SC. Headline 28/36, page
  title 24/32, section 16/24, body 14/20, body-small/label 12/16, label-small
  11/16. Titles 600, body 400. No 700/800 weights.
- **Shape and density.** Dialog 28; card 12; input 8; menu 4; popover 16;
  32px chip 6; compact label 6. Buttons are rectangles with an 8px radius;
  pill buttons are not used. Desktop density: 32px toolbar buttons, 40px
  dialog actions, 48px menu rows, 22px folder rows, 320px review rail.
- **No `color-mix()`.** macOS 13.0–13.2 WebKit lacks reliable support. Every
  translucent surface is a precomputed rgba token (for example
  `--viewer-surface-translucent`, `--viewer-warning-container-translucent`).
  A CSS contract test rejects new `color-mix()` usage.
- **Annotation visual system v2.** The annotation layer gets its own documented
  construction (frames 35–37): 1.5px white halo, 2px/1.5px/4px main stroke,
  10% fill, 20px badge with a 1.5px color ring, lavender selection frame
  offset 5px, 8px handles with a 24px hit area, 4px rounded rectangles, and a
  white note with a 3px accent bar. Annotation colors and UI colors are
  intentionally independent; the style bar uses white/lavender chrome rather
  than the annotation color for selection.
- **Design source and artifacts.** The Pen file
  `/Users/abc/Project/9.22/viewer.pen` is the editable source and stays outside
  Git. The repository keeps 38 exported 1440×900 PNGs under
  `docs/design/0.3.0/pen/frames/` plus the frame map. The `.pen` file is not a
  build dependency; the exported frames and this ADR are the reviewable
  artifacts.
- **Motion.** Short fades and small transforms; annotation commit 160ms,
  selection 120ms, hover 120ms. `prefers-reduced-motion` removes movement and
  keeps opacity only.

## Consequences

- The visual language is reproducible without a theme runtime and works on the
  macOS 13 baseline. The cost is manual token curation: a new role must be
  added to `tokens.css` and covered by the guard test.
- M3 component libraries (MUI) are not adopted; Viewer keeps its own
  `ViewerButton`, `ViewerIconButton`, `ViewerChoiceChip`, `ViewerDialog`, and
  review-specific components. This preserves desktop density and existing
  accessibility/focus contracts.
- The annotation palette is deliberately not theme-derived. A future dark mode
  would require a second ADR for annotation contrast and canvas rendering, not
  just a token swap.
- Exported PNGs can drift from the `.pen` source if a frame is edited without
  re-exporting. The pen README records the frame IDs and the regeneration
  procedure; the design frame is not treated as an implementation guarantee.
- The style bar and annotation canvas share the same numbers (10% fill, 20px
  badge, 5px selection offset, 8px handle); changing one without the other is a
  visual regression covered by the annotation tests and acceptance captures.

## Alternatives considered

- **Generated M3 theme from a seed with `@material/material-color-utilities`.**
  Rejected: adds a dependency and generated surface for a palette that is
  intentionally four colors; dynamic color conflicts with the neutral-first
  direction.
- **MUI or another component library.** Rejected: desktop density, menu row
  height, and the review rail do not map cleanly; migration would override most
  component styles anyway.
- **Dark mode in 0.3.0.** Rejected: annotation colors, evidence PNGs, and the
  native surface would need a second contrast system; 0.3.0 is light-only.
- **Material Expressive / large shapes.** Rejected: the product is a dense
  review tool, not a marketing surface; pills and oversized radii reduce
  scanability.
- **Runtime `color-mix()` for translucent surfaces.** Rejected on the macOS 13
  WebKit baseline; precomputed rgba tokens are predictable and testable.
