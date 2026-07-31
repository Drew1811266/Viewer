# Viewer Final UI Visual Acceptance

Date: 2026-07-31

Prototype: `docs/prototypes/viewer-complete-ui-visual-atlas.html`

Result: **Accepted for Figma audit-board handoff**

## Acceptance scope

This pass verifies the highest-priority visual-atlas corrections against the
approved Viewer visual-upgrade specification. The accepted screenshots were
captured after the corrections and are stored in:

`target/final-design-acceptance-2026-07-31/`

The compact captures use a 1024×720 Viewer viewport. The wide captures use the
atlas's 1440×900 Viewer viewport, fitted inside a 1280×720 outer browser
capture.

## Corrected findings

| Step | Surface | Evidence | Health | Acceptance note |
| --- | --- | --- | --- | --- |
| 01 | Collapsed sidebar | `01-collapsed-sidebar-1024x720.jpg` | Pass | The 52 px rail now exposes one centered, accessible expand control; project text no longer collides with it. |
| 02 | Advanced filter | `02-advanced-filter-1024x720.jpg` | Pass | Trigger and panel both derive `6` from the same active-condition model; the panel opens in place. |
| 03 | Read-only menu | `03-readonly-menu-1024x720.jpg` | Pass | Unavailable actions are visibly muted, carry a read-only label, and use disabled semantics. |
| 04 | Radial click | `04-radial-click-1024x720.jpg` | Pass | The primary radial menu is visible in the Viewer shell instead of a rectangular substitute. |
| 05 | Radial mark | `05-radial-mark-1024x720.jpg` | Pass | The mark secondary fan uses the approved radial geometry and remains attached to its primary sector. |
| 06 | Radial organize | `06-radial-organize-1024x720.jpg` | Pass | The organize secondary fan is visually distinct and preserves the same center and sector rhythm. |
| 07 | Radial disabled | `07-radial-disabled-1024x720.jpg` | Pass | Dashed boundaries, reduced emphasis, and the unavailable label make the state unambiguous. |
| 08 | Radial read-only | `08-radial-readonly-1024x720.jpg` | Pass | The center and affected actions clearly communicate read-only mode. |
| 09 | Radial keyboard | `09-radial-keyboard-1024x720.jpg` | Pass | Keyboard focus is visible on a sector without being confused with selection. |
| 10 | Keyboard selection | `10-keyboard-selection-1024x720.jpg` | Pass | Enter/Space selection is visible on the image card and reflected by `aria-selected`. |
| 11 | Scanning progress | `11-scanning-progress-1024x720.jpg` | Pass | Status and progress are visible and expose live-region/progressbar semantics. |
| 12 | Batch dialog | `12-batch-dialog-1024x720.jpg` | Pass | The modal scrim covers the entire Viewer shell, including toolbar and sidebar; dialog labelling is explicit. |
| 13 | Forced-colors | `13-forced-colors-1024x720.jpg` | Pass | System color tokens and explicit boundaries preserve hierarchy without relying on shadows. |
| 14 | 200% reflow | `14-zoom-200-1024x720.jpg` | Pass | The real Viewer shell reflows at 200%; the primary action remains reachable. |
| 15 | Wide main atlas | `wide-00-main-atlas-1280x720.jpg` | Pass | The atlas state switcher occupies reserved space and no longer overlays product controls. |
| 16 | Wide browser | `wide-01-browser-1280x720.jpg` | Pass | Main navigation, content rail, and image grid remain aligned at the 1440×900 Viewer viewport. |
| 17 | Wide filter | `wide-02-advanced-filter-1280x720.jpg` | Pass | The filter popover stays anchored and unclipped in the wide shell. |
| 18 | Wide radial mark | `wide-03-radial-mark-1280x720.jpg` | Pass | Primary and secondary radial geometry stays centered and legible at the wide viewport. |
| 19 | Wide preview | `wide-04-preview-zoom-1280x720.jpg` | Pass | Preview content, chrome, and zoom controls retain clear separation. |
| 20 | Wide dialog | `wide-05-batch-dialog-1280x720.jpg` | Pass | Modal scale, focus hierarchy, and full-shell scrim remain correct at the wide viewport. |
| 21 | Wide results | `wide-06-results-1280x720.jpg` | Pass | Result density and hierarchy remain stable without overlap or clipping. |
| 22 | Wide 200% reflow | `wide-07-zoom-200-1280x720.jpg` | Pass | The accessibility demonstration remains a real reflowed shell rather than an explanatory card. |

## Automated verification

- `pnpm --dir ui check` — pass.
- `pnpm --dir ui exec vitest run src/visualAtlas.test.ts` — 10/10 pass.
- `pnpm --dir ui test` — 60 files pass; 587 tests pass; 1 existing test skipped.
- `pnpm --dir ui build` — production build pass.
- `git diff --check` — pass.
- In-app browser console after the acceptance run — no errors or warnings.

## Visual acceptance conclusion

No blocking overlap, clipping, state ambiguity, or atlas-control obstruction
was found in the accepted compact and wide evidence sets. Filter, radial-menu,
read-only, keyboard, progress, dialog, target-size, inspector-width, and
contrast-risk contracts now have both visual and automated evidence.

This acceptance does not claim full WCAG conformance. Native macOS/Windows
screen-reader output, OS high-contrast rendering, platform font rasterization,
and physical input-device behavior still require later testing in packaged
application builds.

## Figma audit board

- File:
  [Viewer Final UI Visual Acceptance — 2026-07-31](https://www.figma.com/design/oCtWdesfu5wPx2m9QW1g6Y)
- Section: `Viewer Final UI Visual Acceptance — 2026-07-31`
- Structure: 22 accepted screenshot cards, each with step number, health, name,
  and finding-specific acceptance note.
- Layout verification: 15 cards in row one, 7 cards in row two, 200 px
  horizontal gaps, and a 600 px row gap.
- Visual verification:
  `target/final-design-acceptance-2026-07-31/figma-audit-board.png`
- Structural verification: all 22 cards contain exactly one accepted
  screenshot; the audit section is the only top-level canvas node.
