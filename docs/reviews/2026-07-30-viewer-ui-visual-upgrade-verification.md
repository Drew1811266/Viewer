# Viewer UI Visual Upgrade Verification

- Specification: `docs/superpowers/specs/2026-07-30-viewer-complete-ui-visual-upgrade-design.md`
- Current implementation commits: `b097976` (visual alignment) and `8e957af` (overlay-test synchronization)
- Platform: macOS development environment
- Evidence rule:
  - **Manual verified** means the implemented state was directly inspected at the recorded viewport.
  - **Automated only** means tests or static checks passed, but the required same-state manual inspection is absent.
  - **Unverified** means neither complete manual coverage nor a sufficient automated substitute exists. Automated evidence never substitutes for the required manual matrix.

## Automated verification

| Command | Exit | Final result |
| --- | ---: | --- |
| `pnpm --dir ui check` | 0 | Biome and TypeScript passed (one existing deprecation notice). |
| `pnpm --dir ui test` | 0 | 60 files; 590 passed, 1 skipped. |
| `pnpm --dir ui build` | 0 | TypeScript and Vite production build passed. |
| `pnpm test:policy` | 0 | 28 repository-policy tests passed; 47 scope requirements mapped exactly once. |
| `pnpm verify:clean` | 0 | Policy, UI, Rust, security, and license gates passed on clean commit `8e957af`. |

The current native development build was inspected after the `b097976` product changes; `8e957af`
changes only asynchronous test assertions and does not alter production output.

## State matrix

| Required state | Evidence class | Recorded evidence | Native 1440×900 |
| --- | --- | --- | --- |
| No-project resting/light appearance | Manual verified | Historical 1280×720 same-viewport IAB comparison; lifecycle/light tests on `8e957af`. | Unverified |
| Valid drag, invalid drag, and opening states | Automated only | Lifecycle and drop-state tests on `8e957af`. | Unverified |
| Shell and sidebar | Manual verified | Current native 1229×768 expanded and collapsed captures from `b097976`, each inspected in a combined reference/native comparison. | Unverified |
| Category/content grid | Manual verified | Current native 1229×768 content grid, selection outline, and selection summary from `b097976`. | Unverified |
| Select-all, selection summary, other-file expansion | Automated only | Component and App tests on `8e957af`. | Unverified |
| Organization and Finder drag target | Automated only | Component tests on `8e957af`. | Unverified |
| Filter and result list | Manual verified | Historical native 1229×768 and compact 1024×720 subset captures from `1a21283`; tests on `8e957af`. | Unverified |
| Progress and pagination | Automated only | Component and App tests on `8e957af`. | Unverified |
| Image preview fit/navigation | Manual verified | Historical native 1229×768 preview capture from `1a21283`; tests on `8e957af`. | Unverified |
| Image preview 100%, zoom, rotate, loading, and error | Automated only | Preview component and App tests on `8e957af`. | Unverified |
| Two-, four-, and twenty-file comparison | Automated only | Component and App tests on `8e957af`; ignored fixture supplies candidates. | Unverified |
| Compare transforms, synchronization, and read-only controls | Automated only | Compare component and App tests on `8e957af`. | Unverified |
| Unsupported preview and info inspector | Manual verified | Historical native 1229×768 captures from `1a21283`; tests on `8e957af`. | Unverified |
| Markdown, plain text, encoding, truncation, and two-file info | Automated only | Component and App tests on `8e957af`. | Unverified |
| Unavailable image and multi-file info | Automated only | Component and App tests on `8e957af`. | Unverified |
| Ordinary secondary click and Control-click fallback | Manual subset plus automated | Current native radial-menu capture for ordinary secondary click; both pointerup/contextmenu orders, compact deduplication, selection/focus restoration, and Control-click tests on `8e957af`. | Unverified |
| Held secondary radial gesture | Automated only | Both contextmenu orders with dwell/movement promotion and keyboard radial-menu tests on `8e957af`. | Unverified |
| Settings dialog | Manual verified | Historical native 1229×768 capture from `1a21283`; dialog tests on `8e957af`. | Unverified |
| Rename, batch, destination, trash, and close dialogs | Automated only | Dialog component and App tests on `8e957af`. | Unverified |
| Task stack and error card | Manual verified | Historical native 1229×768 capture from `1a21283`; tests on `8e957af`. | Unverified |
| Results, notices, and row-error states | Automated only | Component and App tests on `8e957af`. | Unverified |
| Recovery and read-only strip | Automated only | Component and App tests on `8e957af`. | Unverified |
| Scanning, indexing, loading, and recovery transitions | Automated only | Lifecycle component and App tests on `8e957af`. | Unverified |

No required state has current native 1440×900 evidence for `8e957af`.

## Accessibility

Historical manual checks on `1a21283` covered Meta+F, Meta+I, Escape/focus restoration, preview controls, and compact popover containment. Current `8e957af` accessibility evidence combines automated named-role, focus-visible, arrow-navigation, Meta+A, reduced-motion, read-only, and compact-containment tests with current native inspection of the named collapse/expand controls and radial command surface. A complete manual accessibility pass on the current implementation has not been recorded.

## Current targeted native acceptance

The highest-priority shell corrections were inspected in the running macOS development build with
the same interaction state placed beside the approved atlas reference:

| State | Native evidence | Combined comparison | Result |
| --- | --- | --- | --- |
| Expanded sidebar, one selected image | `target/final-design-acceptance-2026-08-02/native-fresh-expanded-selected-1229x768.jpg` | `target/final-design-acceptance-2026-08-02/reference-native-expanded-comparison.png` | Fresh restart confirms the 40 px header, 220 px default sidebar, project header action, 24 px directory geometry, thumbnail-only selection outline, and floating selection summary. |
| Collapsed navigation rail | `target/final-design-acceptance-2026-08-02/native-collapsed-selected-1229x768.jpg` | `target/final-design-acceptance-2026-08-02/reference-native-collapsed-comparison.png` | 52 px rail, non-wrapping expand action, compact root/folder navigation, and four-column natural grid at the available width conform. |
| One-file radial command surface | `target/final-design-acceptance-2026-08-02/native-radial-menu.png` | Direct native inspection against the atlas radial family and Figma card 01 | Six sectors, center cancel target, disabled compare state, destructive command treatment, dimmed backdrop, and selected-thumbnail outline conform. |

The combined comparisons use the same shell and selection state, but the atlas and native build use
different fixture images. They are visual-structure acceptance evidence rather than pixel-diff tests.

## Visual comparison

| Board family | Reference viewport(s) | Evidence class | Current status |
| --- | --- | --- | --- |
| visual-density | 1440×900, 1024×720 | Current native 1229×768 targeted pair plus historical subset | Expanded and collapsed density corrected; current native 1440×900 comparison pending. |
| content-browser | 1440×900, 1024×720 | Current native 1229×768 targeted pair plus current automated tests | Selection outline and summary recaptured; complete required-viewport matrix remains pending. |
| search-tasks | 1440×900, 1024×720 | Historical manual subset plus current automated tests | Current native 1440×900 comparison pending. |
| preview-compare | 1440×900, 1024×720 | Historical manual preview plus current automated tests | Current toolbar state requires recapture; comparison states remain automated only. |
| text-info | 1440×900, 1024×720 | Historical manual subset plus current automated tests | Missing current full-state manual coverage. |
| radial-reference | 1440×900, 1024×720 | Current native click-mode capture plus current automated tests | Held-pointer state remains unverified manually. |
| menus-dialogs | 1440×900, 1024×720 | Historical manual settings subset plus current automated tests | Current compact-menu state requires recapture. |
| states-dialogs | 1440×900, 1024×720 | Historical manual task/error subset plus current automated tests | Missing current full-state manual coverage. |
| launch-loading | 1440×900, 1024×720 | Historical manual no-project subset plus current automated tests | Native 1440×900 and remaining lifecycle states pending. |
| empty-project | 1440×900, 1024×720 | Historical manual 1280×720 normalized pair | Current native required-viewports comparison pending. |
| visual-system-motion | 1440×900, 1024×720 | Historical manual light-surface subset plus current automated tests | Focus/palette changes through `8e957af` require recapture. |

The reference captures remain `target/visual-qa/reference-*-1440x900.png` and `reference-*-1024x720.png`. Same-viewport historical IAB comparison exists only for the no-project pair at 1280×720. Historical native display captures are 1229×768, and historical compact webview captures are 1024×720.

## Remaining differences

- No current native 1440×900 capture exists; current targeted native evidence is 1229×768.
- The automation environment cannot inject and inspect the complete native held-secondary gesture.
- Several required state families have automated coverage but no current manual same-state inspection.
- These are pending verification gaps, not product-owner-authorized exceptions. Native 1440×900 and missing manual state coverage await explicit product-owner exception or new evidence.
