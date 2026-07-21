# M3 Organization and Comparison Pointer-Drag Acceptance

## Verdict

**PASS.** The seven required physical checks passed against tested code commit
`453234a20515c3111c5ed75ec9327a8bc3d0c82e`. This record covers the
Pointer Events replacement for Viewer-internal organization drag, while
preserving native Finder export and Finder-folder import.

## Environment and automated verification

- Tested code commit: `453234a20515c3111c5ed75ec9327a8bc3d0c82e`
- Gate: `pnpm gate:m3` — PASS, exit 0, ending with
  `M3 organization and comparison gate passed`.
- Package: `pnpm build:macos` — PASS; Apple Silicon macOS 13-compatible
  `Viewer.app` and `Viewer_0.1.0_aarch64.dmg` built with the internal ad-hoc
  signature. Notarization is not required for internal 0.1.
- App executable SHA-256:
  `a9cedf21b1b07694bec889d4eb19eda47d2f4207bee822cbc78ba6cfa374d961`
- DMG SHA-256:
  `d5ffaff23139eb1d1d09240375c9e52bfdbe7ecd4dd63a5f3e89bfac46928b72`

## Fixture-relative integrity map

All paths below are relative to the disposable acceptance project or its
separate `destinations/` directory.

| File | SHA-256 |
| --- | --- |
| `source-images/acceptance-photo.jpg` | `ffb89121baa0aafd524f82eb0ed6d0338e596f9a5e2b20d0749348f9d22dc2c7` |
| `source-images/acceptance-alpha.png` | `f514f2a5563166aaa73d0549d3708a4fdf66a140d97ac57edefb050371699f77` |
| `source-text/acceptance-notes.md` | `920948f8e718da4b6563f1b217ba6db82b4a88597028074d2b7cdf4459584a28` |
| `source-text/acceptance-details.txt` | `d81f4bcffecf792dd856e5d09e6d6c2a8411084921bfe33e5a24c362c6ed1935` |

## Physical acceptance matrix

| # | Physical check | Evidence and result |
| --- | --- | --- |
| 1 | Multi-image marquee Finder export | A visible marquee selected `source-images/acceptance-photo.jpg` and `source-images/acceptance-alpha.png`; dragging one selected card body to Finder produced exactly `destinations/image-multi-export/acceptance-photo.jpg` and `destinations/image-multi-export/acceptance-alpha.png`. Both destination hashes match the integrity map. **PASS**. |
| 2 | Multi-text Finder export | Command-selecting `source-text/acceptance-notes.md` and `source-text/acceptance-details.txt`, then dragging one selected row body to Finder, produced exactly the corresponding files in `destinations/text-multi-export/`. Both hashes match the integrity map. **PASS**. |
| 3 | Cancelled Finder drag | In the sole restarted exact packaged Viewer, the selected `source-images/acceptance-alpha.png` card body was dragged to blank Viewer workspace and released outside any Finder or folder destination (not from `⋮⋮`, and without Escape). The project remained loaded and selected with no dialog or file command. `destinations/cancelled-drag/` remained empty; the project listing remained exactly `source-images/acceptance-alpha.png`, `organization-copy/acceptance-alpha.png`, `organization-move/acceptance-photo.jpg`, and the two `source-text/` files; all hashes remained mapped values. Destination listings added nothing: image multi-export has its two files, text multi-export has its two files, post-relaunch export has its PNG, and cancelled-drag and folder-import are empty. The earlier Escape cancellation remains a supplemental check. **PASS**. |
| 4 | Default internal move | Dragging the `acceptance-photo.jpg` `⋮⋮` handle without Option removed `source-images/acceptance-photo.jpg` and created `organization-move/acceptance-photo.jpg` with SHA-256 `ffb89121baa0aafd524f82eb0ed6d0338e596f9a5e2b20d0749348f9d22dc2c7`. **PASS**. |
| 5 | PointerDown-frozen Option copy | Option was down at PointerDown on the `acceptance-alpha.png` `⋮⋮` handle and released before PointerUp. Both `source-images/acceptance-alpha.png` and `organization-copy/acceptance-alpha.png` remained present with SHA-256 `f514f2a5563166aaa73d0549d3708a4fdf66a140d97ac57edefb050371699f77`. **PASS**. |
| 6 | Native Finder-folder import | After closing the project, dragging the project folder from Finder into empty packaged Viewer reopened it and indexed `source-images/`, `source-text/`, `organization-move/`, and `organization-copy/` (scan `9/9`). Native Finder-folder import passed. **PASS**. |
| 7 | Quit, relaunch, reimport, export | The exact packaged app was fully quit, relaunched, and used to reimport the fixture. A fresh export of `source-images/acceptance-alpha.png` created exactly `destinations/post-relaunch-export/acceptance-alpha.png` with the matching alpha PNG hash. **PASS**. |

## Portable operation-journal evidence

The internal operations reused the existing Rust preflight, conflict-planning,
verified execution, portable SQLite operation-journal, and reconciliation
pipeline. The journal recorded only fixture-relative paths:

| Operation | Completed journal evidence |
| --- | --- |
| Move | One completed `move` batch, requested/completed `1/1`, failed/skipped `0/0`, with one completed item from `source-images/acceptance-photo.jpg` to `organization-move/acceptance-photo.jpg`. |
| Copy | One completed `copy` batch, requested/completed `1/1`, failed/skipped `0/0`, with one completed item from `source-images/acceptance-alpha.png` to `organization-copy/acceptance-alpha.png`. |

## Component, task, and security review outcome

| Area | Outcome |
| --- | --- |
| Pointer Task 1 | Complete at `f218122`; review clean. |
| Pointer Task 2 | Complete at `db1cb5a`; review approved. |
| Pointer Task 3 | Complete at `dc50091` and `453234a`; review clean after the Important lifecycle fix. |
| Pointer Task 4 | This physical acceptance record: all seven checks PASS. |
| Rust file-operation boundary | Existing preflight, revalidation, transaction, journal, recovery, and watcher-reconciliation pipeline was reused; no new mutation protocol was introduced. |

Viewer-internal organization uses **Pointer Events** and no HTML5
`DataTransfer` or custom MIME type. The native incoming Finder-folder handler
remained enabled, as confirmed by the physical import pass. Finder exports
continue to use their established native boundary. Entity IDs remain opaque at
the frontend boundary; this evidence records only fixture-relative paths and
SHA-256 values. No absolute user, project, cache, or temporary path is
recorded here.
