# M3 Organization and Comparison Stage Review

- Status: Task 18 code findings corrected and verified; mandatory physical Finder drag pending
- Date: 2026-07-19
- Base: `1642957` (paused M3 checkpoint)
- Acceptance head: `13b86c0` plus the uncommitted Task 18 corrections listed below
- Review method: exact M3 aggregate gate, Apple Silicon package inspection, release-app acceptance on writable/copied/read-only fixtures, real filesystem and Trash operations, portable-metadata validation, cache/network inspection and final complete-diff review
- Decision: **Not yet approved or merged; physical Finder drag is the only open Important acceptance item**

## Exit-criteria traceability

| M3 requirement | Automated and packaged-app evidence | Result |
| --- | --- | --- |
| `REQ-FLOW-ORGANIZE`, `REQ-FLOW-BATCH-RENAME` | M3 command, preflight, transaction and UI suites cover single/batch rename, cycles, case-only names, copy/move/Trash, all conflict policies, partial results and cancellation. The packaged app completed a four-file prefix rename and Command-Z, a case-only `a.jpg -> A.jpg -> a.jpg` cycle, deep copy/move, Skip/Keep Both/Replace and a permission-denied item without a destination or temporary residue. | Pass |
| `REQ-FLOW-UNDO` | Rust/UI suites cover marker/favorite/rename/move LIFO undo, partial batches, drift and unsafe-refusal cases. Packaged Command-Z restored batch rename, case-only rename and a deep move; copy and Trash remained excluded. | Pass |
| `REQ-FLOW-DRAG-DROP` | UI and Rust suites cover opaque multi-selection drag, move/Option-copy feedback, stale/read-only suppression, Finder boundary hand-off, entity revalidation and copy-only AppKit source masks. Computer Use could not synthesize a WebView HTML5 drag gesture, so no physical Finder drop is claimed. | Automated pass; physical gesture pending |
| `REQ-FLOW-COMPARE` | Model/component suites cover exactly 2–4 unique images, bounded proxy requests, layouts, transforms, sync/independent mode, inline markers and fallback. The packaged app showed two side-by-side, three asymmetric and four-grid layouts; zoom/rotation, sync toggle, inline Keep, removal and single-preview/grid fallback worked. | Pass |
| `REQ-FLOW-EXTERNAL-CHANGES` | Watcher/reconciliation suites cover expected/unexpected changes, generations and selection/preview/compare repair. Packaged deep-copy/move/undo, a Finder Trash restoration and destination mutations refreshed the tree/grid counts without reopening. | Pass |
| `REQ-FLOW-READONLY-ERRORS` | Rust/UI dual-layer capability matrices reject markers and every file mutation while retaining browse/search/preview/compare. The packaged read-only fixture displayed `只读项目`; marker, rename, copy, move and Trash controls were disabled while preview remained usable. | Pass |
| `REQ-FLOW-LIFECYCLE` | Runtime/UI suites cover wait/cancel/stay, close blocking, stale-event rejection and teardown. Packaged close returned to the empty import surface, reopen did not restore a directory and `~/Library/Caches/com.viewer.desktop/sessions` was empty after close/quit. | Pass |
| `REQ-TECH-FILE-CONSISTENCY`, `REQ-TECH-PATH-SECURITY` | Journal, fault-matrix, recovery, identity, symlink, portable validator and security tests pass. Packaged copy/move/replace hashes matched their sources; no operation silently overwrote, and a recoverable ambiguous batch was surfaced for inspection instead of replayed destructively. | Pass |
| `REQ-TECH-MEMORY`, `REQ-RELEASE-ACCEPTANCE` | Compare uses viewport proxies and never unconditionally decodes four originals. The exact M3 gate passes, and the app/DMG are arm64-only, macOS 13.0, strict-valid ad-hoc signed artifacts with no release-process socket. | Pass |

## Packaged-application acceptance

The disposable writable fixture contained JPG/PNG files at arbitrary folder depth, Markdown/TXT files, a corrupt JPEG, comparison folders with two, three and four images, deep operation destinations and portable schema-v3 state. Observed in the release application:

- folder-only navigation, root overview, thumbnail loading and corrupt-file isolation remained responsive;
- original-image preview, next navigation, 156% zoom, rotation and close worked without copying an original into portable metadata;
- explicit Select All selected two, three and four images, and comparison reflowed correctly as panes were removed;
- a four-file batch rename showed the complete preview before execution and was reversible; a case-only rename was also reversible after the Task 17 regression fix;
- two files copied to a third-level destination with exact SHA-256 equality, and Skip kept destination hashes unchanged, Keep Both created distinct exact copies, and Replace completed through macOS filesystem APIs without Finder automation;
- a deep move retained exact content and Command-Z restored the source while removing the destination;
- a mode-`000` source produced `permission_denied`, no target and no temporary file; cancellation tests preserved completed work and canceled only pending/active copy work according to the contract;
- Viewer moved `viewer-restore-proof.png` to the macOS Trash, Finder exposed `放回原处`, restoration returned it to its original folder, and Viewer reconciled the count;
- the copied project retained project identity and its Keep marker; read-only operation controls were disabled; startup/close always returned to the empty surface.

Computer Use attempted internal and Viewer-to-Finder drag gestures, but its instantaneous pointer primitive did not emit the WebView HTML5 `dragstart` event. The product's complete drag contract remains covered by `App`, `ContentBrowser`, `FolderTree` and `m3_drag_export` tests, including copy-only AppKit source masks and entity/path revalidation. The frozen M3 design nevertheless requires a human physical drag into Finder before approval; this review does not represent automated contract coverage as GUI evidence and does not defer the requirement to M4.

## Defects found and corrected during acceptance

1. **Unsigned bundle:** Tauri produced a binary without a valid distributable application signature. The macOS bundle now requests explicit ad-hoc signing, and repository policy locks that internal-build decision.
2. **Inaccessible bulk comparison selection:** Computer Use exposed missing focus/selection affordances. Shift+Arrow, Command-A, click focus and an explicit `全选当前文件夹` action were added with component tests.
3. **Case-only rename undo false collision:** undo treated the same file's case-folded current path as a foreign occupant. Prevalidation now permits only that canonical same-file alias, with a real-filesystem regression test.
4. **Finder automation Trash hang:** the default Trash route could launch Finder/AppleScript and block internal builds behind Automation consent. The adapter now selects `NSFileManager`; real Trash and Finder `放回原处` were both verified.
5. **Portable entity-ID false rejection:** stable filesystem-derived UUID-shaped entity IDs are canonical but need not carry RFC version/variant bits. The validator now distinguishes entity IDs from project/operation IDs and retains strict shape, with positive and negative policy tests.

Every correction followed a failing focused test, implementation, focused green verification and a fresh exact M3 aggregate gate.

## Independent Task 18 review findings

The complete branch review reported no Critical findings and four Important findings. Three code findings are corrected on the branch and covered by new regressions:

1. **Concurrent same-inode rewrites:** file snapshots now carry nanosecond modification/change evidence; Replace destinations are fingerprinted and revalidated immediately before Trash; copy verifies the current source fingerprint against the copied bytes; and cross-volume move revalidates the source immediately before Trash. Same-length in-place source and destination rewrites now fail without deleting either current source or destination.
2. **Replace recovery marker ownership:** recovery no longer depends on the disposable session index to identify the replaced destination. A narrow portable-metadata path deletion clears the old destination marker before Copy/Rename/Move Replace projection, including a real close/reopen with an initially empty index.
3. **Display-preflight retention:** the display-only preview path now disposes its prepared backend batch, and invalid/blocked start paths do the same. Repeated previews retain zero prepared batches.

The fourth finding is the still-open physical Finder gesture. It cannot be closed by unit/integration evidence alone. Before M3 approval, a human must verify both single- and multi-file drag from the packaged Viewer into Finder, confirm copies appear at the Finder destination, and confirm the Viewer project sources remain present.

## Package, integrity and privacy evidence

```text
pnpm gate:m3                         PASS; exit 0
  repository policy                 7/7
  UI                                128 tests + production build
  portable schema-v3 policy         45 Node tests + live source/copy validation
  cargo fmt / strict Clippy          PASS
  locked Rust workspace tests        PASS
  M3 command/undo/drag/compare/lifecycle suites PASS
  G2 transaction fault matrix        PASS
  G3 Watcher/search gate             PASS
  dependency/license/audit/security/scope checks PASS
pnpm build:macos                     PASS; Viewer.app + Viewer_0.1.0_aarch64.dmg
file / lipo                          Mach-O 64-bit arm64 / arm64 only
Info.plist / LC_BUILD_VERSION        macOS 13.0 minimum
bundle                               com.viewer.desktop / 0.1.0
codesign --verify --deep --strict    PASS; ad-hoc internal signature
hdiutil verify                       VALID
portable metadata source             schema 3, 1 marker, 25 operations, no transient/backup residue
portable metadata copied fixture     same identity/marker; expected ambiguous interrupted operation surfaced
session cache after close             empty
lsof release process TCP/UDP          no sockets
```

The `.viewer` validator accepts only the manifest, schema-v3 SQLite database, exact SQLite sidecars and approved prior-schema backup. It rejects originals, text bodies, thumbnails/proxies, absolute/cache paths, unknown tables/columns/enums/result codes/files and symlinks. Static policy and runtime socket inspection found no updater, analytics, crash upload or application network behavior.

## Scope and remaining review work

- No Windows implementation or cloud behavior was introduced.
- The app and DMG remain ad-hoc signed and are not notarized because Developer ID credentials are outside the internal-only 0.1 scope.
- The synthetic fixture validates correctness but does not replace M4 acceptance with the user's supplied approximately 10 MiB production images.
- Task 18 must review `main...HEAD`, classify the physical Finder-drag automation gap, rerun the exact gate and package build from a clean committed branch, and leave no unresolved Critical or Important finding before approval.

## Task 17 decision

The exact gate, package checks, real file/Trash operations, compare/read-only/lifecycle behavior and portable/privacy boundaries pass after five acceptance-found defects were corrected. The independent Task 18 review then found three additional code defects; all three now have failing-before/green-after regressions and pass the fresh exact gate and package checks. Task 17 remains open only for the mandatory physical Finder drag, which is explicitly disclosed rather than inferred from automation.
