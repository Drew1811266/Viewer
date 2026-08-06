# Viewer Final UI Visual Acceptance

Date: 2026-08-05

Prototype: `docs/prototypes/viewer-complete-ui-visual-atlas.html`

Result: **Accepted — formal product migration and macOS tiered visual verification complete**

## Current product acceptance

- Formal product evidence commit: `9114b78515baa069914a6f6dc083a05a9982e9cf`.
- Complete browser comparison: 89 states at `1024 × 720` and `1440 × 900`, 178/178 combined reference/product images passed, with zero console or page errors.
- Browser evidence: `target/viewer-visual-acceptance/final-all-9114b78/9114b78515baa069914a6f6dc083a05a9982e9cf/`.
- Native acceptance controller commit: `7fa4005f2d9baab6805264ecbef5d51a0b429de3`.
- macOS native representative journeys: 15/15 passed at each target size, 30/30 total.
- Native evidence: `target/atlas-product-migration-acceptance/7fa4005f2d9baab6805264ecbef5d51a0b429de3/`.
- Final visual severity: P0 `0`, P1 `0`, P2 `0`.

The browser layer exhaustively verifies the visual state matrix; the native layer verifies representative window, focus, input, popover, radial-menu and screenshot paths. The historical Figma board below remains the approved atlas handoff, while the current product evidence above is the final implementation authority.

## Historical atlas acceptance scope

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
- `pnpm --dir ui test` — 77 files pass; 727 tests pass; 1 expected test skipped.
- `pnpm --dir ui build` — production build pass.
- `pnpm test:visual-acceptance` — 16/16 pass.
- `pnpm test:native-acceptance` — 99/99 pass.
- `git diff --check` — pass.
- Complete browser acceptance — 178/178 combined images, zero console or page errors.
- macOS native acceptance — 30/30 representative journeys pass.

## Visual acceptance conclusion

No blocking overlap, clipping, state ambiguity, or atlas-control obstruction
was found in either the complete product evidence or the accepted historical
atlas board. Filter, radial-menu, read-only, keyboard, progress, dialog,
target-size, inspector-width, and contrast-risk contracts have visual,
automated and representative macOS-native evidence.

This acceptance does not claim full WCAG conformance. Native macOS/Windows
screen-reader output, OS high-contrast rendering, platform font rasterization,
and physical input-device behavior require platform-specific testing in
packaged application builds. Windows-native verification remains a future
platform task because the Windows version is not yet under development.

## Historical Figma atlas audit board

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

## 2026-08-06 — Square Image Card Selection

### Inputs

- Approved design specification: `docs/superpowers/specs/2026-08-06-viewer-square-image-card-selection-design.md`.
- Authoritative visual source: `docs/prototypes/viewer-complete-ui-visual-atlas.html` at SHA-256 `723c542cff9d32ab63de571342e8abbbdf141dd491a3c56ba284dc313bab7ab5`.
- Supplemental user target: `/var/folders/hh/jj77kbxs0db1_j1kgh7hbd2c0000gn/T/codex-clipboard-d5bd1d52-517a-4846-900b-bf4128731924.png`; its red annotation defines the required complete-card boundary but is not used as a pixel-identical fixture.
- Implementation commit: `7ebf8496dea25ca8d104f634b322c2e74df87439` on `codex/square-image-card-selection`, captured from a clean worktree.
- Automated full-view evidence: `target/viewer-visual-acceptance/square-image-card-selection/7ebf8496dea25ca8d104f634b322c2e74df87439/{1024x720,1440x900}/{THU-04,THU-05,THU-06,THU-07}/combined.png`.
- Native full-view evidence: `target/atlas-product-migration-acceptance/7ebf8496dea25ca8d104f634b322c2e74df87439/1024x720/THU-05/combined.png`.
- Native focused-region evidence: `target/atlas-product-migration-acceptance/7ebf8496dea25ca8d104f634b322c2e74df87439/1024x720/THU-05/combined-focus.png`.

### Capture normalization

- Browser source and implementation captures use identical CSS viewports and 1× density: `1024 × 720` produces `1024 × 720` source/product images and a `2048 × 720` combined image; `1440 × 900` produces `1440 × 900` source/product images and a `2880 × 900` combined image.
- Native `native@2x.png` is `2048 × 1440` Retina output. The acceptance controller normalizes it to `1024 × 720` before combining it with the `1024 × 720` atlas reference, so geometry is judged at the same CSS size rather than by raw device pixels.
- The native `1440 × 900` window was not forced because the current display's safe application area caps it at `1440 × 847`. The accepted native state therefore uses the exact supported `1024 × 720` viewport without changing macOS display scaling, Dock settings, or global scrollbar settings; both approved sizes remain covered by browser component evidence.

### State and fidelity review

| State | Evidence | Result |
| --- | --- | --- |
| `THU-04` resting cards | Both browser viewports | Square card edges; no selection overlay or geometry shift. |
| `THU-05` single selection | Both browser viewports plus native `1024 × 720` | One 2 px accent boundary encloses the thumbnail and filename; no thumbnail-only ring remains. |
| `THU-06` multiple selection | Both browser viewports | Exactly one complete-card boundary appears on each of three selected cards. |
| `THU-07` keyboard focus | Both browser viewports | The complete-card selection boundary remains independent from the outer keyboard-focus treatment. |

- Typography: font family, size, weight, filename baseline, and selection-summary hierarchy remain unchanged; minor native rasterization differences are platform rendering, not token drift.
- Spacing and layout rhythm: moving the overlay does not change card measurement, image-stage height, filename row, grid gaps, or the selection-summary position.
- Color and tokens: the boundary uses `--viewer-accent`; the atlas uses its matching accent token. Forced-colors coverage moves to the same complete-card pseudo-element and uses `Highlight` at 2 px.
- Image quality and cropping: thumbnail assets retain `object-fit: contain`; no new crop, blur, scaling, or loading artifact is visible.
- Copy and content: source and native fixtures intentionally use different filenames and counts, while label hierarchy and the `已选择 1 项` guidance remain consistent.
- Interaction and layering: the organization handle stays above the non-interactive overlay, pointer events pass through the overlay, and focus remains separately visible.
- Scope protection: folder filmstrips, search-result rows, other-file rows, and non-image card families retain their existing geometry.

### Checklist

- [x] Resting content-grid cards have square corners.
- [x] The selection boundary encloses thumbnail and filename.
- [x] No inset thumbnail-only boundary remains.
- [x] Selection does not move card content or grid geometry.
- [x] Multi-selection draws one boundary per card.
- [x] Keyboard focus remains visually independent.
- [x] The organization handle remains above and operable.
- [x] Forced-colors styling follows the complete card.
- [x] Folder filmstrips and other card families are unchanged.

### Findings and comparison history

- Initial product finding: rounded image cards and a selection ring limited to the thumbnail stage did not match the approved complete-card intent.
- Implemented correction: the card radius is `0`, selection ownership remains on `aria-selected`, and the non-measuring overlay moved to `.image-cell::after` with `inset: 0`.
- Evidence correction during QA: the first `1024 × 720` reference export reused `THU-04` while changing URL fragments. Those stale images were rejected, each reference was reloaded in a fresh document, selected counts were asserted as `0/1/3/1`, and all eight combined states plus the native state were recaptured.
- Final severity: P0 `0`, P1 `0`, P2 `0`, P3 `0`.

Final result: passed

## 2026-08-06 — Five-Level Thumbnail Size Slider

### Accepted behavior

- Settings uses one accessible range slider with visible numeric stops `1` through `5`; the former `紧凑 / 标准 / 大图` radio cards are removed.
- The exact level-to-card-size contract is `1 = 96 px`, `2 = 132 px`, `3 = 168 px`, `4 = 204 px`, and `5 = 240 px`.
- Level 2 remains the default and invalid-value recovery level. The existing settings schema remains version 1, while all five values round-trip through the TypeScript model, Tauri DTO, Rust persistence, and JSON storage.
- Pointer input, Home/End and arrow-key changes share the same optimistic save behavior; stale completions cannot overwrite the latest user choice, and a failed latest write rolls the UI back to the last persisted value.

### Browser visual evidence

- Product/atlas evidence commit: `fa99c5d75e6c6cc04143b930f127fb8334774987` (later commits change only the native acceptance controller).
- Final browser run: 8 states at both `1024 × 720` and `1440 × 900`, 16/16 reference/product comparisons completed with no failed or unrun states.
- Evidence root: `target/viewer-visual-acceptance/five-level-thumbnail-slider-final-v2/fa99c5d75e6c6cc04143b930f127fb8334774987/`.
- Reviewed states: settings dialog (`DIA-01`), keyboard focus (`A11Y-01`), levels 1–5 (`THU-01`, `THU-02`, `THU-03`, `THU-08`, `THU-09`), and complete-card selection at maximum size (`THU-05`).
- Level 4 and 5 cards retain complete borders, filename rows, grid gaps, and selection geometry without clipping, overflow, or incomplete edge rendering at either viewport.

### macOS native evidence

- Level 5 grid (`THU-09`) passed at `1024 × 720`: `target/atlas-product-migration-acceptance/96d9a8c57d3da3335133d127a82f5ec728c6a687/1024x720/THU-09/combined.png`.
- Focused settings slider (`DIA-01`) passed at `1024 × 720`: `target/atlas-product-migration-acceptance/aea8957f451cbbdfe89a99747440a696d410738d/1024x720/DIA-01/combined.png`.
- The current display safe area limits the native Viewer window to `1280 × 800`, so a native `1440 × 900` capture was not forced. Browser component evidence covers that approved viewport. No macOS display scaling, resolution, Dock, or global scrollbar setting was changed.

### Final review

- Settings hierarchy, slider focus, accent thumb, filled track, numeric alignment, and close action match the approved visual atlas.
- Levels 4 and 5 increase only thumbnail/card scale; existing interaction logic, square card geometry, complete-card selection outline, filename treatment, and virtualization behavior remain intact.
- macOS current-stage severity: P0 `0`, P1 `0`, P2 `0`, P3 `0`.
- Windows-native rendering and input validation remain explicitly deferred until Windows development begins.

Final result: passed
