# Viewer Progressive Loading Acceptance

## Build identity

- Branch: `codex/viewer-atlas-product-migration`
- Browser-acceptance commit: `17834f88cdd79192dbff9820e9463ff25512ac4a`
- Native executable: `/Users/abc/Project/Viewer/.worktrees/viewer-atlas-product-migration/target/debug/viewer-desktop`
- Canonical launcher result: one development process at launch, PID `40834`, sourced from
  `codex/viewer-atlas-product-migration @ 17834f8`.

## Deterministic contracts

| Contract | Command | Observed result |
| --- | --- | --- |
| Artificial initial delay | Focused reducer/controller batch | `45/45` passed. The first projection is dispatched immediately after project open; the fixed `600ms` hold is absent (`0ms` fixed hold). |
| Non-blocking thumbnail work | `pnpm --dir ui exec vitest run src/App.test.tsx` | `83/83` passed. The project root, folder tree, content option and item-local placeholder remain visible while the thumbnail Promise is pending; the TaskBar remains non-blocking. |
| Delayed folder feedback | `useDelayedProjectionProgress.test.tsx` in the affected batch | The line stays hidden through `119ms`, appears at `120ms`, restarts for a replacement request and clears on settle/unmount. |
| Progressive items | `ContentBrowser.test.tsx` in the affected batch | A resolved thumbnail remains visible while the second cell retains `缩略图加载中`; each mounted request remains at one call across shelf toggles. |
| Request coalescing | `projectThumbnailCache.test.ts` | `4/4` passed. Two identical pending/resolved keys make one loader call; file version, pixel size and scale are independent keys, and failures can retry. |
| LRU and session isolation | Cache tests plus App integration tests | The resolved LRU is capped at `512`; a touched entry survives the next eviction. The same category/content key makes `1` bridge call in one session and reopening makes the expected second call (`2` total). An old cleared generation cannot overwrite a new request. |
| Newest folder wins | Focused reducer/controller batch | Late success and failure from older requests do not replace the newest folder; optimistic selection is immediate and the final workspace matches the newest target. |
| Affected deterministic batch | Eleven state, App, cache, component and style test files | `274/274` passed in one Vitest process. |
| Complete repository gate | `pnpm verify` | Exit `0`. UI: `79/79` files, `740` passed and `1` intentional skip; production build passed. Rust format, Clippy, all workspace tests, security, dependency and license policies passed. |

`pnpm --dir ui check` checked `196` files with no fixes and exited `0`. It reported only the
existing informational Biome migration notice for the deprecated `recommended` configuration
field.

## Visual evidence

Both states were captured once at `1024x720` from a clean commit. Each manifest reported the
expected viewport, no horizontal overflow, `0` console errors and automation exit `0`.

| State | Evidence | Visual verdict |
| --- | --- | --- |
| `LAU-05` | `target/viewer-visual-acceptance/17834f88cdd79192dbff9820e9463ff25512ac4a/1024x720/LAU-05/` | Pass after one combined-image inspection. The product shows the real initial project/workspace skeleton and scan TaskBar without exposing a false loaded tree. |
| `LAU-06` | `target/viewer-visual-acceptance/17834f88cdd79192dbff9820e9463ff25512ac4a/1024x720/LAU-06/` | Pass after one combined-image inspection. The real content grid stays present with item-local placeholders, filenames and non-blocking TaskBar feedback; there is no full-grid concealment layer. |

## Bounded native observation

The native sequence was performed exactly once against
`target/atlas-product-migration-fixture/ViewerAcceptance`:

1. Opened the project with the native folder picker.
2. Selected the previously unvisited `衣服/A01`, then `衣服/A02`.
3. Returned to `衣服/A01`.
4. Rapidly selected `角色/B01` → `目标/Source` → `衣服/A02`.
5. Switched from `衣服/A02` to the project overview, then opened `衣服/A01` again.

Observed results:

- First visits selected the requested row immediately. The next accessibility/screenshot capture
  (approximately `0.5s`) showed the correct stable grid: `A01` with its product images and `A02`
  with its accessory images. No white workspace flash or input-blocking overlay was observed.
- Returning directly to `A01` showed its images immediately and no new loading TaskBar, consistent
  with same-session thumbnail reuse.
- The three rapid selections settled on `A02`; both the highlighted row and the displayed accessory
  grid matched that newest request.
- Overview → `A01` preserved the workspace structure and returned to the populated image grid
  without a white flash.
- The development launcher log contained no `ERROR`, panic or `failed to` entries. The computer
  control resolver briefly launched the same worktree's bundled Viewer while closing by app name;
  both explicit worktree Viewer PIDs and their development host were terminated. A final process
  check found no Viewer, Tauri dev or Vite process from this worktree.

## System restoration check

No System Settings pane was opened and no display or Dock setting was changed.

| Setting | Before | After | Result |
| --- | --- | --- | --- |
| Dock autohide | `0` | `0` | Match |
| Physical pixels | `2940 x 1912` | `2940 x 1912` | Match |
| Logical resolution | `1470 x 956 @ 60.00Hz` | `1470 x 956 @ 60.00Hz` | Match |
| Retina mode | `spdisplays_2560x1664Retina` | `spdisplays_2560x1664Retina` | Match |
| Main / mirror / online | `yes / off / yes` | `yes / off / yes` | Match |
