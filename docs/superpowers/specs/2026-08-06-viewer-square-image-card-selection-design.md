# Viewer Square Image Card Selection Design

Date: 2026-08-06
Status: Approved

## Decision

Viewer content-folder image cards use square outer corners. Selecting an image
draws one accent boundary around the complete file item, including the
thumbnail stage and filename area. The former inset boundary inside only the
thumbnail is removed.

This document is the authoritative narrow override for content-grid card
geometry and selection treatment. It supersedes conflicting thumbnail-only
selection clauses in:

- `2026-07-30-viewer-complete-ui-visual-upgrade-design.md`;
- `2026-07-30-viewer-visual-fidelity-correction-design.md`;
- `2026-08-02-viewer-atlas-to-product-complete-migration-design.md`.

All unrelated rules in those documents remain active.

## Scope

The change applies only to file items in the selected content folder's primary
image grid.

It does not change:

- folder-overview filmstrip thumbnails;
- preview or comparison surfaces;
- sidebar rows, menus, dialogs, task surfaces, or empty states;
- selection semantics, multi-selection, keyboard navigation, radial-menu
  invocation, double-click preview, drag organization, or virtualization.

## Card Geometry

- The complete file card has `0` corner radius.
- The thumbnail stage and filename area form one continuous rectangular item.
- The resting card retains the existing quiet one-pixel neutral boundary.
- The card remains shadowless and receives no selected fill or image tint.
- Existing card size, grid gap, thumbnail aspect-ratio calculation, and
  filename height do not change.

## Selection Boundary

- `aria-selected="true"` on the file item is the single source of selection
  state for accessibility and visual styling.
- A non-interactive absolute overlay draws
  `2px solid var(--viewer-accent)` around the complete card at `inset: 0`.
- The overlay has `0` corner radius and uses border-box sizing.
- The boundary includes the thumbnail stage and filename area.
- The old thumbnail-frame `data-selected` state and inset rounded boundary are
  removed.
- The overlay does not participate in measurement and therefore cannot change
  the grid, cause layout shift, or move filenames when selection changes.
- Each selected item draws its own complete boundary during multi-selection.

## Focus and Interaction Layering

Selection and keyboard focus remain distinct:

- selection uses the complete-card accent boundary;
- keyboard focus keeps the existing outer `:focus-visible` outline and does
  not change `aria-selected`;
- a focused selected item may show both boundaries;
- the selection overlay uses `pointer-events: none`;
- the organization handle remains visible and operable above the overlay when
  its existing hover, focus, or drag conditions apply.

Clicks, modifier-clicks, marquee selection, double-click preview, secondary
click, radial-menu gestures, keyboard commands, and drag organization retain
their existing behavior.

## Considered Approaches

### Complete-card overlay — selected

This matches the approved annotated target, includes the filename, and avoids
layout movement because it does not alter measured border width.

### Changing the card's physical border width — rejected

Changing the resting one-pixel border to a two-pixel selected border risks
size compensation, content movement, and virtual-grid measurement drift.

### External CSS outline — rejected

An external outline avoids measurement changes but can intrude into grid gaps
or be clipped at a virtualized viewport edge. It also appears visually
detached from the requested card boundary.

## Accessibility and Platform Behavior

- The DOM option continues to expose `aria-selected` for assistive technology.
- Selection remains identifiable without changing image color or opacity.
- Forced-colors support must map the complete-card boundary to a system color.
- macOS and future Windows builds use the same Viewer-owned square geometry;
  no platform-specific radius is introduced.
- Reduced-motion behavior is unchanged because selection has no animation.

## Verification

Automated regression coverage must prove:

- the content-grid file card has zero corner radius;
- the selected overlay belongs to the complete `aria-selected` card;
- the selected overlay is inset zero, square, two pixels, non-interactive, and
  absolutely positioned;
- the thumbnail frame no longer owns a selected-state attribute or overlay;
- the filename is inside the selected card boundary;
- selection does not change measured geometry;
- keyboard focus remains independently styled;
- existing single-selection, multi-selection, radial-menu, preview, and drag
  behavior stays green.

Visual acceptance must compare the annotated target and the current
implementation in the same input at a matching folder-grid state. Acceptance
requires square resting cards, no inner thumbnail ring, and one complete-card
selection boundary enclosing the filename.
