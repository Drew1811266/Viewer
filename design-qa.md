# Design QA — Viewer complete UI visual atlas

## Comparison target

- Source visual truth:
  - `docs/superpowers/specs/2026-07-30-viewer-complete-ui-visual-upgrade-design.md`
  - `target/visual-qa/reference-integrated-content-browser-17-1024x720.png`
  - `target/visual-qa/reference-integrated-radial-reference-21-1024x720.png`
  - `target/visual-qa/reference-integrated-empty-project-minimal-23-1024x720.png`
- Implementation:
  - `docs/prototypes/viewer-complete-ui-visual-atlas.html`
  - Browser-rendered evidence: `target/visual-qa/atlas-browser-overview-1280x720.png`
  - Exact app-surface evidence: `target/visual-qa/atlas-implementation-content-browser-1024x720.png`
- Combined source/implementation input inspected:
  - `target/visual-qa/atlas-comparison-content-browser-1024x720.png`

## Viewport and normalization

| Artifact | CSS size | Capture pixels | Density |
| --- | ---: | ---: | ---: |
| Source content-browser board | 1024 × 720 | 1024 × 720 | 1× |
| Atlas content-browser app surface | 1024 × 720 | 1024 × 720 | 1× |
| Atlas shell browser evidence | 1280 × 720 | 1280 × 720 | 1× |

The atlas has an internal `#embed=<screen>&viewport=1024` review mode so the
Viewer app surface can be captured at exactly 1024 × 720 without the atlas
navigation or a scaled browser canvas. No density resampling was used for the
main comparison.

## State

The main same-state comparison covers the compact content browser with:

- 40 px workspace toolbar;
- compact project directory;
- visible image grid with real fixture assets;
- one selected thumbnail;
- selection summary;
- Filter/View/More top-level actions.

Additional browser-rendered states inspected:

- project overview and content browser;
- View and More popovers, including mutual exclusion and `Escape` dismissal;
- search and anchored filter surface;
- preview and compare with visible `完成`;
- text preview and information inspector;
- approved six-sector radial menu;
- destination/conflict, batch rename, trash, and close-task dialogs;
- read-only, local error, notice, and single task surface;
- no-project, drag, validating, and scanning states;
- 1024 × 720 and 1440 × 900 internal preview sizes.

## Full-view comparison evidence

`target/visual-qa/atlas-comparison-content-browser-1024x720.png` places the
approved source and exact-size implementation in one image. It was inspected at
original resolution. The toolbar, sidebar, selection outline, filenames,
status tags, and asset treatment remain readable at 1×, so a separate focused
crop was not required.

## Fidelity review

- Fonts and typography: the cross-platform system stack uses Inter/SF Pro/
  Segoe UI/PingFang/Microsoft YaHei fallbacks. Weight, size, line height, and
  compact hierarchy remain consistent across macOS and Windows conventions;
  no important label wraps or truncates in the inspected states.
- Spacing and layout rhythm: the 40 px toolbar, 24 px directory row, 220 px
  project sidebar, quiet dividers, 8–12 px control spacing, and compact image
  grid preserve the approved A density. No persistent control clips at either
  internal viewport.
- Colors and visual tokens: white and warm-gray surfaces, restrained indigo
  action/selection color, semantic green/yellow/red states, and elevation only
  for menus, dialogs, notices, and task surfaces match the approved system.
- Image quality and asset fidelity: the prototype reuses the approved Viewer
  fixture imagery and reference boards. No product imagery or radial-menu art
  was replaced by emoji, CSS drawings, placeholder blobs, or custom SVG art.
- Copy and content: toolbar labels, selection guidance, `完成`, destructive
  confirmations, loading copy, and the three-element no-project state match the
  consolidated specification.
- Icons and controls: icon treatment remains quiet and single-family; normal
  menu rows are borderless, destructive rows are semantic, and keyboard focus
  uses the same indigo token as other focus states.
- Accessibility and behavior: semantic buttons and form controls are
  keyboard-addressable, focus is visible, `Escape` closes open popovers, and
  reduced motion disables the skeleton animation.

## Finding and repair history

1. **P2 — drag-state label collided with the atlas state switcher.**
   The initial drag capture placed the drop-target label too close to the top
   review control. The label was moved from 16 px to 48 px below the drop
   boundary, and embed review mode now removes all atlas-only controls.
   Post-fix evidence:
   `target/visual-qa/atlas-implementation-drag-window-1024x720.jpg`.
2. **P2 — atlas navigation inherited the browser's orange focus ring.**
   A dedicated 2 px indigo `:focus-visible` rule now maps navigation focus to
   the Viewer accent token.
3. **P2 — exact visual evidence initially included the atlas shell.**
   An embed review mode was added and the implementation was recaptured as an
   exact 1024 × 720 app surface. Post-fix evidence:
   `target/visual-qa/atlas-implementation-content-browser-1024x720.png`.

No actionable P0, P1, or P2 findings remain.

## Primary interactions tested

- switch among all 12 visual sections;
- select a second thumbnail and observe the summary update from 1 to 2;
- right-click a thumbnail and open the single radial-menu reference;
- open View, switch to More, and verify only one popover remains;
- press `Escape` and verify the open popover closes;
- switch all dialog and launch/loading states;
- switch internal preview size between 1024 × 720 and 1440 × 900.

Browser console errors and warnings checked: none.

## Follow-up polish

- P3: a future implementation pass may add live sector hover/secondary-state
  animation to the radial reference. The approved static geometry and visual
  hierarchy are already represented in this atlas.

final result: passed
