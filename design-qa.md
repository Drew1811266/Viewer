# Design QA — Viewer visual fidelity correction

## Visual truth and implementation

- Approved source boards:
  - `target/visual-qa/reference-integrated-empty-project-minimal-23-{1440x900,1024x720}.png`
  - `target/visual-qa/reference-integrated-content-browser-17-{1440x900,1024x720}.png`
  - `target/visual-qa/reference-integrated-search-filter-13-{1440x900,1024x720}.png`
  - `target/visual-qa/reference-integrated-preview-compare-14-{1440x900,1024x720}.png`
  - `target/visual-qa/reference-integrated-radial-reference-21-{1440x900,1024x720}.png`
  - `target/visual-qa/reference-integrated-menus-dialogs-20-{1440x900,1024x720}.png`
- Verified implementation: `codex/viewer-ui-visual-upgrade` at `7d04987`.
- Current implementation captures:
  - `target/visual-qa/implementation-current-empty-1440x900.png`
  - `target/visual-qa/implementation-current-empty-1024x720.png`
  - `target/visual-qa/implementation-current-selected-1024x720.png`
  - `target/visual-qa/implementation-current-filter-1024x720.png`
  - `target/visual-qa/implementation-current-preview-1024x720.png`
  - `target/visual-qa/implementation-current-radial-1024x720.png`
- Combined reference/implementation inputs inspected:
  - `target/visual-qa/comparison-empty-1440x900.png`
  - `target/visual-qa/comparison-content-selected-1024x720.png`
  - `target/visual-qa/comparison-preview-1024x720.png`
  - `target/visual-qa/comparison-radial-1024x720.png`

The source files are design boards with explanatory canvas around a framed app.
The combined inputs retain that context but compare the actual app surface,
layout, hierarchy, controls, and states. Test fixture thumbnails are deliberately
simple generated images; Viewer still renders the user's real local assets in
production and the thumbnail pipeline was not replaced.

## Viewports and density

| Surface | CSS viewport | Capture pixels | Effective density | Result |
| --- | ---: | ---: | ---: | --- |
| Browser, no project | 1440 × 900 | 1440 × 900 | 1× | passed |
| Browser, no project | 1024 × 720 | 1024 × 720 | 1× | passed |
| Native Viewer, project states | 1024 × 720 | 1024 × 720 | normalized 1× | passed |
| Native Viewer, resilience check | host maximum | 1318 × 768 | normalized 1× | passed |

The current Mac display cannot expose a native 1440 × 900 window, so the exact
large viewport was checked in the local browser and the native build was
stretched to the host maximum as an additional resilience check. This is a P3
coverage limitation rather than a visual defect; the exact compact native
target and both exact browser targets are covered.

## State and interaction coverage

The following current-build states were exercised:

- No project: only the product name, short description, primary open action,
  and secondary recent-project affordance remain.
- Project content: sidebar, compact toolbar, responsive three-column grid at
  1024 × 720, selected image, task completion summary, and automatic clean-task
  dismissal.
- Selection: the 2 px indigo selection ring is inset 6 px inside the image
  thumbnail only; the filename/card is not outlined. `Esc` clears selection and
  keyboard focus remains a separate state.
- File actions: ordinary right-click, Control-click, menu-key invocation, and
  held/moved secondary gestures all resolve to the same six-sector radial menu.
  There is no conventional rectangular file-action menu.
- Toolbar: `筛选`, `视图`, and `更多` are the only top-level actions. View and
  More use quiet, borderless, left-aligned command rows and mutually exclusive
  popovers.
- Preview: fit/100%/zoom controls form one segmented group, rotate is separate,
  and the right-side close action is visibly labelled `完成`.
- Filter, radial menu, View, More, preview, outside dismissal, and focus
  restoration were exercised in the native app. Browser console errors for the
  no-project surface were empty.

## Fidelity verdict

- Typography: the approved cross-platform system stack and compact hierarchy
  are preserved. Titles, commands, metadata, warnings, and shortcuts remain
  distinct without accidental wrapping or clipping.
- Spacing and layout: the 52 px top bar, 260 px sidebar, quiet dividers, card
  spacing, popover containment, and 1024 × 720 responsive grid are stable. No
  horizontal overflow or unreachable persistent control was observed.
- Colors and tokens: the white/neutral surface system, indigo selection/action
  accent, semantic red destructive state, focus roles, and shadows use shared
  semantic tokens.
- Images and icons: native thumbnails remain sharp and use the real file
  pipeline. The radial menu retains the approved segmented ring geometry and
  icon ordering.
- Copy: visible labels are concise and consistent. Selection guidance reads
  `右键打开圆盘菜单 · Esc 取消选择`; preview closes with `完成`.
- Accessibility and behavior: actionable controls expose roles, focus is
  restored after dismissals, `Esc` works, keyboard/context-menu invocation is
  covered, and reduced-motion contracts remain in the automated suite.

## Finding and repair history

1. **P1 — wrong file-action surface.** A rectangular contextual command list
   had replaced the approved radial interaction. `3b4fc87` made the radial menu
   the single file-action surface and added the ordinary/gesture/keyboard event
   paths. Post-fix evidence:
   `comparison-radial-1024x720.png`.
2. **P1 — full-card selection outline.** Selection incorrectly enclosed the
   image and filename. `852222a` moved the ring inside the thumbnail and
   separated selection from keyboard focus. Post-fix evidence:
   `comparison-content-selected-1024x720.png`.
3. **P2 — toolbar command rows remained boxed.** A higher-specificity popover
   selector overrode the intended borderless rows. `7d04987` corrected selector
   specificity and widened the compact View popover to prevent wrapping. Native
   View/More inspection after hot reload confirmed the repair.
4. **P2 — redundant toolbar and menu density.** `c5b4254` reduced the toolbar
   to Filter/View/More and simplified the two popovers.
5. **P2 — scattered task surfaces.** `58ee2b9` consolidated task feedback into
   one compact surface and made clean success transient.
6. **P2 — preview/compare chrome drift.** `81eb4de` regrouped the viewing
   controls and restored an explicit `完成` action. Post-fix evidence:
   `comparison-preview-1024x720.png`.
7. **Environment mismatch — stale installed-style bundle.** Native inspection
   initially opened an older bundle instead of the working-tree binary. The
   launch audit isolated that process, verified the current source through the
   development URL, and the canonical launcher now removes stale Viewer
   processes before starting one current-worktree development instance.

## Verification

- UI static check: passed.
- UI tests: 577 passed, 1 skipped.
- UI production build: passed.
- Launcher tests: 14 passed.
- Rust formatting: passed.
- Rust Clippy with warnings denied: passed.
- Rust workspace tests: passed.
- Current visual comparisons: no remaining actionable P0, P1, or P2
  differences.

Optional P3 follow-up: repeat the native 1440 × 900 capture on a larger physical
display. It is not required to ship this correction because the exact large
browser viewport, compact native viewport, and native maximum-width resilience
state all pass.

final result: passed
