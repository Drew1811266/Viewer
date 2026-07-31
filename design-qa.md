# Design QA — Viewer UI visual upgrade

## Comparison target and normalization

- Source visual truth: the approved Visual Companion boards captured at both
  1440 × 900 and 1024 × 720 CSS pixels in
  `target/visual-qa/reference-*-1440x900.png` and
  `target/visual-qa/reference-*-1024x720.png`. They cover shell/density,
  content, search/tasks, preview/compare, text/info, radial actions,
  menus/dialogs, loading/recovery, launch, and empty project.
- Historical rendered implementation: the locally bundled, non-installed macOS
  debug app at `target/debug/bundle/macos/Viewer.app`, built from `1a21283`.
  The current implementation at `08de91f` has not been rebuilt or recaptured
  natively.
- Native display captures: `target/visual-qa/native-*.png` at 1229 × 768;
  compact native webview evidence is
  `native-compact-content-default-1024x720-webview.jpeg` and
  `native-compact-filter-only-1024x720-webview.jpeg`, plus the final native
  mutual-exclusivity recheck at
  `native-latest-mutually-exclusive-popovers.jpeg`. Only the no-project
  IAB/browser pair is same-viewport; contact sheets with 1440 × 900 source
  boards are normalized review inputs, not a native 1440 × 900 claim.

The source boards include a documentation canvas and a mock window, while the
implementation captures contain only the real Viewer window. Comparisons
therefore exclude explanatory board chrome and judge the mock-window content
region against the Viewer content region. The one pixel-normalized no-project
pair uses a 1172 × 648 source crop, resized to 1280 × 720 implementation
pixels; its source and implementation are both browser-rendered at the same
effective 1280 × 720 CSS viewport (DPR 2). The native Computer Use surface
provided a 1229 × 768 Viewer display capture. The later compact native window
provided the configured 1024 × 720 webview target. Computer Use cannot provide
a native 1440 × 900 display capture or complete held-secondary-pointer
injection. These are recorded verification gaps; they have no exception status
without explicit product-owner approval.

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

**Historical findings and current automated status**

No actionable P0, P1, or P2 visual differences were identified in the recorded
historical subset. Because `08de91f` has not received the required native
1440 × 900 and full manual state inspection, this document cannot make a final
visual-pass claim for the current implementation.

- Typography: the inspected shell, menu, dialog, filter, result, preview and
  inspector all use the approved compact system sans hierarchy: strong page
  title, 14–16 px action text, quieter metadata and readable small controls.
  No clipping, unintended wraps, or browser-default button type remains.
- Spacing and layout rhythm: the 52 px shell, narrow sidebar, quiet dividers,
  card/grid gaps, task stack, popover containment, inspector and dialog all
  preserve the board's white-space-first rhythm. The compact test coverage
  confirms persistent controls remain reachable at 1024 × 720.
- Colors and tokens: automated source contracts confirm that component
  declarations consume semantic roles from `tokens.css` and contain no raw
  hex/rgb/hsl or named structural palette literals. Warning, danger, review,
  information, selection, control and shadow roles remain distinct. This is
  automated evidence, not a current manual visual confirmation.
- Image quality and assets: Viewer renders the fixture's actual images,
  transparent alpha artwork, Quick Look fallback, and unsupported JSON state;
  none were replaced by generated or handcrafted stand-ins. The radial symbols
  retain the previously approved segmented SVG implementation rather than a
  substitute icon set.
- Copy and content: Chinese product labels, shortcut hints, selection counts,
  disabled guidance, preview controls, error/task labels, and read-only-aware
  action wording remain coherent and fit their controls.

**Residual verification gaps**

- macOS Computer Use's secondary-click injector opens WebKit's text context
  menu before the DOM can be observed. The Viewer menu is visible after that
  native menu is dismissed. Current automated tests verify the ordinary
  contextmenu-before-pointerup and pointerup-before-contextmenu paths,
  Control-click fallback, dwell/movement promotion, deduplication, selection,
  focus restoration, and default prevention. The native held gesture remains
  unverified manually.
- The ignored QA project at `target/visual-qa/fixture-project/` adds Markdown,
  plain-text, unsupported JSON and 20 comparison candidates without changing
  the tracked fixture. Markdown/text, read-only, loading/recovery, comparison,
  focus and reduced-motion states are also covered by their rendered
  component/App tests and approved boards; the native fixture capture validates
  the shared shell and overlays used by those states.

## Interaction and accessibility checks

- Historical native (`1a21283`): Meta+F focuses search; Meta+I opens the
  inspector; filter, view and
  more controls open and Escape closes/restores trigger focus; search results,
  other-file expansion, image preview navigation controls, radial menu,
  unsupported-file recovery, task failure card and settings dialog were
  exercised.
- Current automated (`08de91f`): 60 UI test files passed (575 tests, 1
  skipped), including roles,
  keyboard/focus restoration, radial/context menu keyboard paths, compact
  containment, read-only write disabling, preview/compare, loading/recovery
  and reduced-motion semantics.
- Console/build: the current production UI build completed without errors. No
  native bundle was rebuilt after `08de91f`; the historical native bundle was
  not installed or deployed.

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
4. `1a21283` — P1: opening 筛选 while 视图 was open could leave overlapping
   peer popovers. A failing App-level regression test drove the shared toolbar
   popover coordination; the rebuilt native app recheck confirms 筛选 expanded
   while 视图 and 更多 are collapsed in
   `target/visual-qa/native-latest-mutually-exclusive-popovers.jpeg`.
5. `660818e` — final review findings were repaired with failing tests first:
   ordinary secondary click now opens the compact menu while held gestures open
   the radial menu; compact submenus remain viewport-contained; focus and
   palette tokens are valid and legacy values are absent; preview/compare
   toolbars follow the required structure. Static checks, 568 UI tests, build,
   and clean-tree repository verification pass. These automated results do not
   supply the missing manual evidence.
6. `08de91f` — the two remaining Important findings were repaired with failing
   tests first. Secondary gestures now preserve an early `contextmenu` signal
   until release, dwell, or meaningful movement decides compact versus
   pointer-radial behavior; both allowed event orders, Control-click, focus,
   selection, and deduplication are covered. Component stylesheet colors and
   shadows now consume semantic roles defined in `tokens.css`, with a
   declaration-level contract preventing raw component palette literals.
   Static checks, 575 UI tests, build, and clean-tree repository verification
   pass. No native or manual evidence was added.

## Implementation checklist

- [ ] Validate the current implementation against every visual-state family in
  the native Viewer at all required viewports.
- [x] Normalize board chrome and record viewport/density constraints.
- [x] Fix and re-check every P0/P1/P2 finding.
- [ ] Run final native-bundle verification and complete the manual state matrix
  on the current implementation.

final result: blocked

Blocker: native 1440×900 and missing manual state coverage await explicit product-owner exception or new evidence.
