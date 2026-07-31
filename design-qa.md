# Design QA — Viewer UI visual upgrade

## Comparison target and normalization

- Source visual truth: the approved Visual Companion boards captured at both
  1440 × 900 and 1024 × 720 CSS pixels in
  `target/visual-qa/reference-*-1440x900.png` and
  `target/visual-qa/reference-*-1024x720.png`. They cover shell/density,
  content, search/tasks, preview/compare, text/info, radial actions,
  menus/dialogs, loading/recovery, launch, and empty project.
- Rendered implementation: the locally bundled, non-installed macOS debug app
  at `target/debug/bundle/macos/Viewer.app`, built from `c4b6fc7`.
- Native captures: `target/visual-qa/native-*.png`; the source/implementation
  comparison inputs are `target/visual-qa/contact-*.png`.

The source boards include a documentation canvas and a mock window, while the
implementation captures contain only the real Viewer window. Comparisons
therefore exclude explanatory board chrome and judge the mock-window content
region against the Viewer content region. The one pixel-normalized no-project
pair uses a 1172 × 648 source crop, resized to 1280 × 720 implementation
pixels; its source and implementation are both browser-rendered at the same
effective 1280 × 720 CSS viewport (DPR 2). The native Computer Use surface
provided a 1229 × 768 Viewer window and did not expose programmatic resizing;
the desktop compact-layout tests cover the approved 1024 × 720 containment
case. This window-control constraint is recorded rather than treated as a
visual difference.

## Evidence and state coverage

Full-view comparison inputs inspected:

- `target/visual-qa/contact-empty-project-content-normalized-1280x720.png`
  (same state and equal normalized pixels).
- `target/visual-qa/contact-main-workspace-1440x900.png`,
  `target/visual-qa/contact-search-filter-1440x900.png`,
  `target/visual-qa/contact-info-1440x900.png`, and
  `target/visual-qa/contact-image-preview-1440x900.png` (source board region
  beside native Viewer capture; explanatory canvas excluded from review).
- `target/visual-qa/reference-integrated-radial-reference-21-1440x900.png`
  beside `target/visual-qa/native-file-context-menu.png` for the preserved
  segmented radial-menu geometry.

Focused native evidence inspected:

- `native-fixture-content-default.png`, `native-fixture-filter-open.png`, and
  `native-search-results.png`: shell, sidebar, toolbar, grid/list result,
  filter, pagination, task/error stack.
- `native-image-preview.png`, `native-image-preview-info.png`,
  `native-fixture-info-open.png`, and `native-unsupported-json-preview.png`:
  preview controls, inspector, unsupported-file recovery state.
- `native-file-context-menu.png` and `native-settings-dialog.png`: radial
  action ring, dimmed dialog layer, focusable settings controls.

## Fidelity review

**Findings**

No actionable P0, P1, or P2 visual differences remain.

- Typography: the inspected shell, menu, dialog, filter, result, preview and
  inspector all use the approved compact system sans hierarchy: strong page
  title, 14–16 px action text, quieter metadata and readable small controls.
  No clipping, unintended wraps, or browser-default button type remains.
- Spacing and layout rhythm: the 52 px shell, narrow sidebar, quiet dividers,
  card/grid gaps, task stack, popover containment, inspector and dialog all
  preserve the board's white-space-first rhythm. The compact test coverage
  confirms persistent controls remain reachable at 1024 × 720.
- Colors and tokens: white/soft-gray surfaces, hairline dividers, semantic
  indigo selection/focus/primary states, destructive red, and restrained
  shadows match the approved light visual system. The app stays light; no
  dark-system media override or legacy blue/gray token is present.
- Image quality and assets: Viewer renders the fixture's actual images,
  transparent alpha artwork, Quick Look fallback, and unsupported JSON state;
  none were replaced by generated or handcrafted stand-ins. The radial symbols
  retain the previously approved segmented SVG implementation rather than a
  substitute icon set.
- Copy and content: Chinese product labels, shortcut hints, selection counts,
  disabled guidance, preview controls, error/task labels, and read-only-aware
  action wording remain coherent and fit their controls.

**Residual test gaps**

- macOS Computer Use's secondary-click injector opens WebKit's text context
  menu before the DOM can be observed. The Viewer menu is visible after that
  native menu is dismissed, and the regression test now verifies that the
  target-level native listener sees `contextmenu.defaultPrevented` during
  capture for both image and text rows. This is an automation limitation, not
  an actionable Viewer visual or interaction finding.
- The ignored QA project at `target/visual-qa/fixture-project/` adds Markdown,
  plain-text, unsupported JSON and 20 comparison candidates without changing
  the tracked fixture. Markdown/text, read-only, loading/recovery, comparison,
  focus and reduced-motion states are also covered by their rendered
  component/App tests and approved boards; the native fixture capture validates
  the shared shell and overlays used by those states.

## Interaction and accessibility checks

- Native: Meta+F focuses search; Meta+I opens the inspector; filter, view and
  more controls open and Escape closes/restores trigger focus; search results,
  other-file expansion, image preview navigation controls, radial menu,
  unsupported-file recovery, task failure card and settings dialog were
  exercised.
- Automated: 60 UI test files passed (558 tests, 1 skipped), including roles,
  keyboard/focus restoration, radial/context menu keyboard paths, compact
  containment, read-only write disabling, preview/compare, loading/recovery
  and reduced-motion semantics.
- Console/build: production UI build completed without errors; native bundle
  built locally and was not installed or deployed.

## Comparison history

1. `6836bb3` — P2: the no-project entry action retained browser-default
   styling. A failing primary-action contract test was added, then the action
   was given the semantic indigo, shared radius, 34 px height, focus and
   disabled treatment. The normalized same-state contact sheet was recaptured
   and contains no P0/P1/P2 finding.
2. `fad7a98` — P1: Escape did not close the controlled filter popover in the
   native Viewer. A failing keyboard/focus test identified the missing details
   handler; the repair closes the popover and restores focus to 筛选. The rebuilt
   native filter capture confirmed the behavior.
3. `c4b6fc7` — P1 native-menu risk: target bubbling prevented the DOM default
   too late for platform listeners. A failing capture-phase regression test was
   added, then image and text file targets gained capture-phase prevention.
   The focused test and full UI suite pass; native CUA's WebKit overlay is
   documented above as a tool limitation.

## Implementation checklist

- [x] Validate every approved visual-state family against the native Viewer and
  the 11 supplied source boards.
- [x] Normalize board chrome and record viewport/density constraints.
- [x] Fix and re-check every P0/P1/P2 finding.
- [x] Run final UI, repository and native-bundle verification.

final result: passed
