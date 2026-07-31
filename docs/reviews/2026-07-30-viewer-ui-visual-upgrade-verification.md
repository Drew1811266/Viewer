# Viewer UI Visual Upgrade Verification

- Specification: `docs/superpowers/specs/2026-07-30-viewer-complete-ui-visual-upgrade-design.md`
- Platform: macOS development build
- Implementation commit: `1a21283`

## Automated verification

| Command | Exit | Final result |
| --- | ---: | --- |
| `pnpm --dir ui check` | 0 | Biome and TypeScript passed (one existing deprecation notice). |
| `pnpm --dir ui test` | 0 | 60 files; 559 passed, 1 skipped. |
| `pnpm --dir ui build` | 0 | TypeScript and Vite production build passed. |
| `pnpm verify` | 0 | Policy, UI, Rust and security verification passed. |
| `pnpm tauri build --debug --bundles app` | 0 | Local, uninstalled macOS QA bundle rebuilt after `1a21283`; native compact recheck passed. |
| `pnpm verify:clean` | 0 | Full verification passed with a clean worktree. |

## State matrix

| Step 4 state group | Result | Viewport/evidence |
| --- | --- | --- |
| 1. No project, drag/drop, opening, light appearance | Pass | Same-viewport IAB no-project 1280×720; source boards 1440×900 and 1024×720; lifecycle/light tests. |
| 2. Shell and sidebar | Pass | Native 1229×768 display; native compact 1024×720 webview evidence. |
| 3. Content grid, selection, other files, drag target | Pass | Native 1229×768 fixture and compact 1024×720 webview; component tests. |
| 4. Filter, results, progress, pagination | Pass | Native 1229×768, `native-compact-filter-only-1024x720-webview.jpeg`, and final `native-latest-mutually-exclusive-popovers.jpeg`; mutual-exclusion regression test. |
| 5. Image preview and unavailable image | Pass | Native 1229×768 `native-image-preview.png`; preview tests. |
| 6. Comparison and read-only controls | Pass | 2/4/20 and read-only component/App tests; extended ignored QA fixture supplies 20 candidates. |
| 7. Text, errors, unsupported and inspector | Pass | Native 1229×768 unsupported/info; Markdown/TXT/encoding/truncation tests and extended QA fixture. |
| 8. Radial and conventional fallback | Pass with environment exception | Native click-mode primary ring and expanded 标记 secondary ring observed; WebKit menu precedes secondary-click DOM path, then Escape exposes Viewer ring; radial/context tests cover held pointer and fallback. |
| 9. Operation dialogs and settings | Pass | Native 1229×768 settings; dialog tests cover rename, batch, destination, trash and close states. |
| 10. Tasks, recovery, errors, read-only | Pass | Native 1229×768 task/error; App/component tests cover results, notices, row error and read-only strip. |

## Accessibility

Native checks covered Meta+F, Meta+I, Escape/focus restoration, preview controls and compact popover containment. After the final native rebuild/relaunch, opening 筛选 closed an already-open 视图; the final recheck is `target/visual-qa/native-latest-mutually-exclusive-popovers.jpeg`. UI coverage verifies Tab/focus-visible, named roles, non-color marker cues, arrow navigation in radial/context menus, Meta+A routing, reduced motion and read-only write disabling. The 1024×720 native compact captures keep filter/task/content critical actions visible.

## Visual comparison

| Approved board family | Reference viewport(s) | Implementation evidence | Visible mismatch / resolution |
| --- | --- | --- | --- |
| visual-density | 1440×900, 1024×720 | native default/compact | None. |
| content-browser | 1440×900, 1024×720 | native default/compact | None. |
| search-tasks | 1440×900, 1024×720 | native search/filter | Peer-popover overlap fixed in `1a21283`. |
| preview-compare | 1440×900, 1024×720 | native preview; compare tests | None. |
| text-info | 1440×900, 1024×720 | native info/unsupported; text tests | None. |
| radial-reference | 1440×900, 1024×720 | native primary/secondary ring | None; secondary-click exception below. |
| menus-dialogs | 1440×900, 1024×720 | native settings; dialog tests | None. |
| states-dialogs | 1440×900, 1024×720 | native task/error; state tests | None. |
| launch-loading | 1440×900, 1024×720 | IAB no-project; lifecycle tests | None. |
| empty-project | 1440×900, 1024×720 | normalized 1280×720 IAB pair | Default primary action fixed in `6836bb3`. |
| visual-system-motion | 1440×900, 1024×720 | native light surface; motion tests | None. |

The authoritative captures are `target/visual-qa/reference-*-1440x900.png` and `reference-*-1024x720.png`. Same-viewport IAB/browser comparison is available for the no-project pair at 1280×720. Native display captures are 1229×768; native compact webview captures are 1024×720.

## Remaining differences

Approved verification-environment exceptions only: Computer Use cannot hold a native secondary pointer through the full gesture and macOS WebKit displays its own menu before the secondary-click DOM path. Escape exposes the Viewer click-mode primary ring, and the 标记 secondary ring with five children was observed through native accessibility. Computer Use also cannot provide a native 1440×900 display capture. These limitations do not represent an unresolved Viewer visual difference; the specified paths are covered by native click-mode/secondary-ring evidence and automated held-pointer/fallback tests.
