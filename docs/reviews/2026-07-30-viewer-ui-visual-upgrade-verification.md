# Viewer UI Visual Upgrade Verification

- Specification: `docs/superpowers/specs/2026-07-30-viewer-complete-ui-visual-upgrade-design.md`
- Current implementation commit: `08de91f`
- Platform: macOS development environment
- Evidence rule:
  - **Manual verified** means the implemented state was directly inspected at the recorded viewport.
  - **Automated only** means tests or static checks passed, but the required same-state manual inspection is absent.
  - **Unverified** means neither complete manual coverage nor a sufficient automated substitute exists. Automated evidence never substitutes for the required manual matrix.

## Automated verification

| Command | Exit | Final result |
| --- | ---: | --- |
| `pnpm --dir ui check` | 0 | Biome and TypeScript passed (one existing deprecation notice). |
| `pnpm --dir ui test` | 0 | 60 files; 575 passed, 1 skipped. |
| `pnpm --dir ui build` | 0 | TypeScript and Vite production build passed. |
| `pnpm verify:clean` | 0 | Policy, UI, Rust, security, and license gates passed on clean commit `08de91f`. |

The historical native bundle and captures were built from `1a21283`; no native bundle or visual capture was rebuilt after `08de91f`.

## State matrix

| Required state | Evidence class | Recorded evidence | Native 1440×900 |
| --- | --- | --- | --- |
| No-project resting/light appearance | Manual verified | Historical 1280×720 same-viewport IAB comparison; lifecycle/light tests on `08de91f`. | Unverified |
| Valid drag, invalid drag, and opening states | Automated only | Lifecycle and drop-state tests on `08de91f`. | Unverified |
| Shell and sidebar | Manual verified | Historical native 1229×768 display and 1024×720 compact webview captures from `1a21283`. | Unverified |
| Category/content grid | Manual verified | Historical native 1229×768 fixture and 1024×720 compact webview captures from `1a21283`. | Unverified |
| Select-all, selection summary, other-file expansion | Automated only | Component and App tests on `08de91f`. | Unverified |
| Organization and Finder drag target | Automated only | Component tests on `08de91f`. | Unverified |
| Filter and result list | Manual verified | Historical native 1229×768 and compact 1024×720 subset captures from `1a21283`; tests on `08de91f`. | Unverified |
| Progress and pagination | Automated only | Component and App tests on `08de91f`. | Unverified |
| Image preview fit/navigation | Manual verified | Historical native 1229×768 preview capture from `1a21283`; tests on `08de91f`. | Unverified |
| Image preview 100%, zoom, rotate, loading, and error | Automated only | Preview component and App tests on `08de91f`. | Unverified |
| Two-, four-, and twenty-file comparison | Automated only | Component and App tests on `08de91f`; ignored fixture supplies candidates. | Unverified |
| Compare transforms, synchronization, and read-only controls | Automated only | Compare component and App tests on `08de91f`. | Unverified |
| Unsupported preview and info inspector | Manual verified | Historical native 1229×768 captures from `1a21283`; tests on `08de91f`. | Unverified |
| Markdown, plain text, encoding, truncation, and two-file info | Automated only | Component and App tests on `08de91f`. | Unverified |
| Unavailable image and multi-file info | Automated only | Component and App tests on `08de91f`. | Unverified |
| Ordinary secondary click and Control-click fallback | Automated only | Both pointerup/contextmenu orders, compact deduplication, selection/focus restoration, and Control-click tests on `08de91f`. | Unverified |
| Held secondary radial gesture | Automated only | Both contextmenu orders with dwell/movement promotion and keyboard radial-menu tests on `08de91f`. | Unverified |
| Settings dialog | Manual verified | Historical native 1229×768 capture from `1a21283`; dialog tests on `08de91f`. | Unverified |
| Rename, batch, destination, trash, and close dialogs | Automated only | Dialog component and App tests on `08de91f`. | Unverified |
| Task stack and error card | Manual verified | Historical native 1229×768 capture from `1a21283`; tests on `08de91f`. | Unverified |
| Results, notices, and row-error states | Automated only | Component and App tests on `08de91f`. | Unverified |
| Recovery and read-only strip | Automated only | Component and App tests on `08de91f`. | Unverified |
| Scanning, indexing, loading, and recovery transitions | Automated only | Lifecycle component and App tests on `08de91f`. | Unverified |

No required state has current native 1440×900 evidence for `08de91f`.

## Accessibility

Historical manual checks on `1a21283` covered Meta+F, Meta+I, Escape/focus restoration, preview controls, and compact popover containment. Current `08de91f` accessibility evidence is automated only: named roles, focus-visible semantics, arrow navigation, Meta+A routing, reduced motion, read-only disabling, and compact containment tests pass. A complete manual accessibility pass on the current implementation has not been recorded.

## Visual comparison

| Board family | Reference viewport(s) | Evidence class | Current status |
| --- | --- | --- | --- |
| visual-density | 1440×900, 1024×720 | Historical manual subset | Current native 1440×900 comparison pending. |
| content-browser | 1440×900, 1024×720 | Historical manual subset plus current automated tests | Current selection, semantic-palette, and secondary-gesture changes require recapture. |
| search-tasks | 1440×900, 1024×720 | Historical manual subset plus current automated tests | Current native 1440×900 comparison pending. |
| preview-compare | 1440×900, 1024×720 | Historical manual preview plus current automated tests | Current toolbar state requires recapture; comparison states remain automated only. |
| text-info | 1440×900, 1024×720 | Historical manual subset plus current automated tests | Missing current full-state manual coverage. |
| radial-reference | 1440×900, 1024×720 | Historical manual click-mode subset plus current automated tests | Held-pointer state remains unverified manually. |
| menus-dialogs | 1440×900, 1024×720 | Historical manual settings subset plus current automated tests | Current compact-menu state requires recapture. |
| states-dialogs | 1440×900, 1024×720 | Historical manual task/error subset plus current automated tests | Missing current full-state manual coverage. |
| launch-loading | 1440×900, 1024×720 | Historical manual no-project subset plus current automated tests | Native 1440×900 and remaining lifecycle states pending. |
| empty-project | 1440×900, 1024×720 | Historical manual 1280×720 normalized pair | Current native required-viewports comparison pending. |
| visual-system-motion | 1440×900, 1024×720 | Historical manual light-surface subset plus current automated tests | Focus/palette changes through `08de91f` require recapture. |

The reference captures remain `target/visual-qa/reference-*-1440x900.png` and `reference-*-1024x720.png`. Same-viewport historical IAB comparison exists only for the no-project pair at 1280×720. Historical native display captures are 1229×768, and historical compact webview captures are 1024×720.

## Remaining differences

- No current native 1440×900 capture exists.
- The automation environment cannot inject and inspect the complete native held-secondary gesture.
- Several required state families have automated coverage but no current manual same-state inspection.
- These are pending verification gaps, not product-owner-authorized exceptions. Native 1440×900 and missing manual state coverage await explicit product-owner exception or new evidence.
