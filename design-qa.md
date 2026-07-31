# Design QA — Viewer complete UI visual atlas

## Comparison target

- Source visual truth:
  - `docs/superpowers/specs/2026-07-30-viewer-complete-ui-visual-upgrade-design.md`
  - `target/visual-qa/reference-integrated-content-browser-17-1024x720.png`
  - `target/visual-qa/reference-integrated-radial-reference-21-1024x720.png`
  - `target/visual-qa/reference-integrated-empty-project-minimal-23-1024x720.png`
- Implementation:
  - `docs/prototypes/viewer-complete-ui-visual-atlas.html`
  - Browser overview: `target/visual-qa/atlas-complete-browser-overview-1280x720.jpg`
  - Exact content-browser surface:
    `target/visual-qa/atlas-complete-implementation-browser-1024x720.jpg`
- Combined source/implementation inputs inspected:
  - `target/visual-qa/atlas-complete-comparison-content-browser-1024x720.png`
  - `target/visual-qa/atlas-complete-comparison-content-browser-focus-1024x360.png`
  - `target/visual-qa/atlas-complete-comparison-radial-1024x720.png`
  - `target/visual-qa/atlas-complete-comparison-no-project-1024x720.png`

## Viewport and normalization

| Artifact | CSS size | Capture pixels | Density |
| --- | ---: | ---: | ---: |
| Approved content-browser board | 1024 × 720 | 1024 × 720 | 1× |
| Atlas content-browser app surface | 1024 × 720 | 1024 × 720 | 1× |
| Approved radial board | 1024 × 720 | 1024 × 720 | 1× |
| Atlas radial app surface | 1024 × 720 | 1024 × 720 | 1× |
| Approved no-project board | 1024 × 720 | 1024 × 720 | 1× |
| Atlas no-project app surface | 1024 × 720 | 1024 × 720 | 1× |
| Browser atlas shell | 1280 × 720 | 1280 × 720 | 1× |

The atlas embed review mode removes atlas-only navigation and renders the
Viewer surface at its requested 1024 × 720 CSS size. The in-app browser capture
was center-cropped from its 1280 × 720 browser viewport to that exact app
surface; no resampling or density conversion was used. The focused comparison
uses the same 1024 × 360 top region from source and implementation.

## State and coverage

The final atlas contains two visual-foundation pages plus all 17 approved
coverage groups. Those groups expose 89 switchable states, including:

- no-project, valid/invalid drag, opening, scanning, thumbnail generation,
  empty project, error, and recovery;
- expanded, resized, collapsed, and drop-target sidebars;
- project root, category, content folder, descendant aggregate, and folder
  bands;
- compact, standard, and large thumbnails with none, single, multiple, and
  keyboard-focus selection;
- collapsed/expanded other files and internal organization drag;
- grouped, flat, indexing, paging, and empty search;
- zero, one, multiple, and advanced filters;
- View, More, and read-only menus;
- seven radial invocation/secondary/disabled/keyboard states;
- seven image-preview states, four compare counts, seven document states, and
  single/multiple information inspectors;
- seven dialogs, five task outcomes, five result/notice states, and five
  accessibility review states.

## Full-view comparison evidence

The three 2072 × 720 combined inputs place the approved visual board and exact
1024 × 720 implementation surface in one image. They were inspected at original
resolution. The no-project state preserves the exact three-element resting
content. The radial page retains the approved six-sector raster as visual truth
and adds a semantic state panel without redrawing the radial geometry. The
content browser preserves the two-column shell, top-level action order, quiet
white/warm-gray surfaces, and thumbnail-only selection outline.

## Focused region comparison evidence

`target/visual-qa/atlas-complete-comparison-content-browser-focus-1024x360.png`
compares the same top 1024 × 360 region. It verifies toolbar/side-column
alignment, 24 px directory rows, file-name separation, real product imagery,
semantic tags, and the 6 px-inset selection outline. This crop was required
because these details are too small to judge reliably in the full-view board.

## Required fidelity surfaces

- Fonts and typography: the cross-platform stack uses Inter/SF Pro/Segoe UI
  Variable/PingFang/Microsoft YaHei fallbacks. Small UI copy remains legible at
  both internal sizes; weights, line heights, truncation, and hierarchy do not
  drift between states.
- Spacing and layout rhythm: the workspace toolbar remains 40 px, preview
  toolbar 52 px, and directory rows 24 px. Top and body columns stay aligned at
  1024 × 720 and 1440 × 900. Menus, dialogs, task surfaces, and inspectors use
  elevation only where the specification permits it.
- Colors and visual tokens: white and warm-gray application surfaces,
  restrained indigo action/focus/selection, and separate success, warning, and
  danger semantics match the approved token system.
- Image quality and asset fidelity: eleven approved fixture/reference rasters
  are embedded as local data URIs. Product imagery and radial art were not
  replaced by emoji, CSS drawings, custom SVG, gradients, or placeholder art.
- Copy and content: toolbar labels, `完成`, cross-platform Trash/Recycle Bin
  language, selection guidance, disabled reasons, recovery copy, and
  no-project content match the consolidated specification.
- Icons and controls: the atlas avoids invented icon art; text-only controls
  are used where the approved design does not require a sourced icon. Menus
  remain borderless rows, danger appears only in relevant states, and focus is
  structurally visible.
- Accessibility and behavior: real buttons, menus, menuitems, labels, alt text,
  visible focus, reduced-motion behavior, forced-color review, focus
  restoration guidance, and 200% zoom states are represented. Core state
  controls and navigation were exercised in the browser.

## Finding and repair history

1. **P2 — the previous atlas was representative rather than complete.**
   It exposed twelve coarse pages, omitted multiple approved state families,
   and depended on `/files/` runtime image paths. The atlas now publishes all 17
   coverage groups, 89 switchable states, and eleven embedded data-URI assets.
   Post-fix evidence: all four Vitest contract tests pass, and all nineteen
   atlas navigation entries render one complete scene in the browser.
2. **P2 — operation results were shown as a centered heavy panel.**
   The specification requires a 320–340 px right-side non-modal inspector so
   browsing can continue. The operation result state now uses a 334 px right
   inspector with the image workspace still visible, compact outcome rows, and
   a result action.
3. **P2 — the Other Files area was initially treated as a bordered card.**
   The specification calls for a bottom panel introduced by a quiet top
   divider. The enclosing border and radius were removed while the compact
   expanded rows and drag state were retained.
4. **P2 — exact browser evidence initially included the atlas navigation.**
   The implementation was reloaded in embed mode and captured as an exact
   1024 × 720 Viewer app surface. A focused, same-region comparison was also
   generated for the dense toolbar/sidebar/thumbnail details.

No actionable P0, P1, or P2 findings remain.

## Primary interactions and checks

- clicked all nineteen atlas navigation entries and verified every scene
  rendered;
- exercised the 89-state controls through the JSDOM interaction contract;
- switched thumbnail density, selection, focus, search, filter, menu, radial,
  preview, compare, document, dialog, task, result, launch, and accessibility
  states;
- verified the selection outline remains inside the image stage and does not
  surround the file name;
- verified 1024 × 720 and 1440 × 900 internal preview sizes;
- loaded the canonical HTML through a local browser server with no console
  errors or warnings;
- verified the canonical HTML contains no external or root-relative image
  source and therefore remains self-contained when opened directly.

## Follow-up polish

- P3: the approved radial geometry remains a raster reference in this visual
  atlas. Live sector hit-testing and motion belong to the later product
  implementation pass, not this visual-only HTML approval artifact.

final result: passed
