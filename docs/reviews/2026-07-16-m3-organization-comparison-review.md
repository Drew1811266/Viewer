# M3 Organization and Comparison Stage Review

> **Historical governance note (2026-07-23):** References to M4 or Viewer 0.1 release acceptance below describe the plan at the time this evidence was recorded. ADR 0005 cancelled M4 and replaced release acceptance with continuous development governance. The measured evidence in this document is unchanged.

- Status: **Complete and integrated into `main`.**
- Date: 2026-07-22
- Base: `1642957` (paused M3 checkpoint)
- Acceptance code head: `2fb612a85d38a8154b5311ad6a461a4f2090c9a9` (final safety corrections and enforced staged-copy policy)
- Review method: exact M3 aggregate gate, Apple Silicon package inspection, release-app acceptance on writable/copied/read-only fixtures, real filesystem and Trash operations, portable-metadata validation, cache/network inspection and final complete-diff review
- Decision: **All implementation, review, physical acceptance and integration exit criteria pass. M3 is accepted.**

## Exit-criteria traceability

| M3 requirement | Automated and packaged-app evidence | Result |
| --- | --- | --- |
| `REQ-FLOW-ORGANIZE`, `REQ-FLOW-BATCH-RENAME` | M3 command, preflight, transaction and UI suites cover single/batch rename, cycles, case-only names, copy/move/Trash, all conflict policies, partial results and cancellation. The packaged app completed a four-file prefix rename and Command-Z, a case-only `a.jpg -> A.jpg -> a.jpg` cycle, deep copy/move, Skip/Keep Both/Replace and a permission-denied item without a destination or temporary residue. | Pass |
| `REQ-FLOW-UNDO` | Rust/UI suites cover marker/favorite/rename/move LIFO undo, partial batches, drift and unsafe-refusal cases. Packaged Command-Z restored batch rename, case-only rename and a deep move; copy and Trash remained excluded. | Pass |
| `REQ-FLOW-DRAG-DROP` | UI and Rust suites cover opaque multi-selection drag, move/Option-copy feedback, stale/read-only suppression, Finder boundary hand-off, entity revalidation and copy-only AppKit source masks. The exact packaged app physically passed visible-marquee multi-image export, multi-text export, cancelled export, default internal move, PointerDown-frozen Option copy, native Finder-folder import and post-relaunch export with matching source/destination SHA-256 values. | Pass |
| `REQ-FLOW-COMPARE` | Model/component suites cover exactly 2–4 unique images, bounded proxy requests, layouts, transforms, sync/independent mode, inline markers and fallback. The packaged app showed two side-by-side, three asymmetric and four-grid layouts; zoom/rotation, sync toggle, inline Keep, removal and single-preview/grid fallback worked. | Pass |
| `REQ-FLOW-EXTERNAL-CHANGES` | Watcher/reconciliation suites cover expected/unexpected changes, generations and selection/preview/compare repair. Packaged deep-copy/move/undo, a Finder Trash restoration and destination mutations refreshed the tree/grid counts without reopening. | Pass |
| `REQ-FLOW-READONLY-ERRORS` | Rust/UI dual-layer capability matrices reject markers and every file mutation while retaining browse/search/preview/compare. The packaged read-only fixture displayed `只读项目`; marker, rename, copy, move and Trash controls were disabled while preview remained usable. | Pass |
| `REQ-FLOW-LIFECYCLE` | Runtime/UI suites cover wait/cancel/stay, close blocking, terminal cleanup warnings, stale-event rejection and teardown. The rebuilt package imported and scanned the fixture, created one known session cache, then received a real Cmd+Q; the process exited, that exact cache disappeared, the cache root was empty, and relaunch showed the empty import surface. | Pass |
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

The final clean disposable fixture was exercised through the exact packaged
application with macOS Accessibility-authorized, smooth native pointer events.
All seven required physical checks passed. Finder destinations contained the
expected JPG, PNG, Markdown and TXT copies with exact source hashes; a cancelled
drag left no destination; the JPG organization handle moved the file; the PNG
Option-at-PointerDown handle copied it while retaining the source; a Finder
folder drop reopened the project; and a full process quit/relaunch returned to
the empty import surface before a successful reimport and export. The complete
relative-path and SHA-256 record is in the linked M3 acceptance document.

## Defects found and corrected during acceptance

1. **Unsigned bundle:** Tauri produced a binary without a valid distributable application signature. The macOS bundle now requests explicit ad-hoc signing, and repository policy locks that internal-build decision.
2. **Inaccessible bulk comparison selection:** Computer Use exposed missing focus/selection affordances. Shift+Arrow, Command-A, click focus and an explicit `全选当前文件夹` action were added with component tests.
3. **Case-only rename undo false collision:** undo treated the same file's case-folded current path as a foreign occupant. Prevalidation now permits only that canonical same-file alias, with a real-filesystem regression test.
4. **Finder automation Trash hang:** the default Trash route could launch Finder/AppleScript and block internal builds behind Automation consent. The adapter now selects `NSFileManager`; real Trash and Finder `放回原处` were both verified.
5. **Portable entity-ID false rejection:** stable filesystem-derived UUID-shaped entity IDs are canonical but need not carry RFC version/variant bits. The validator now distinguishes entity IDs from project/operation IDs and retains strict shape, with positive and negative policy tests.
6. **Native Quit retained the active session cache:** macOS can reach final `RunEvent::Exit` without a preventable `ExitRequested`. The runtime now performs synchronous final session teardown and a Viewer-owned cache sweep on final exit. Normal close reports a terminal cleanup warning without resurrecting a closed backend session, and UI/native window/application/close-command paths all converge on the same committed-close decision.
7. **Path validation outlived the bound filesystem object:** Trash could follow a moved parent outside the project, and copy placement could lose its temporary identity after staging. Trash now owns the canonical project root and revalidates the bound parent plus file reference immediately before `NSFileManager`; copy now returns an identity-bound staged lease that owns safe cleanup through placement.
8. **Post-rename error cleanup could delete the official destination:** when rename succeeded but directory sync or final validation failed, generic cleanup followed the staged reference and removed the recoverable destination. The lease now retains a destination only when parent path/identity, file-reference path and staged snapshot all match; escaped or replaced identities are still cleaned without touching replacements.

Every correction followed a failing focused test, implementation, focused green verification and a fresh exact M3 aggregate gate.

## Independent Task 18 review findings

The initial complete-branch review reported no Critical findings and four Important findings. Three code findings were corrected first and covered by new regressions:

1. **Concurrent same-inode rewrites:** file snapshots now carry nanosecond modification/change evidence; Replace destinations are fingerprinted and revalidated immediately before Trash; copy verifies the current source fingerprint against the copied bytes; and cross-volume move revalidates the source immediately before Trash. Same-length in-place source and destination rewrites now fail without deleting either current source or destination.
2. **Replace recovery marker ownership:** recovery no longer depends on the disposable session index to identify the replaced destination. A narrow portable-metadata path deletion clears the old destination marker before Copy/Rename/Move Replace projection, including a real close/reopen with an initially empty index.
3. **Display-preflight retention:** the display-only preview path now disposes its prepared backend batch, and invalid/blocked start paths do the same. Repeated previews retain zero prepared batches.

The follow-up review then found two deeper Important consistency windows, both corrected in `ba5148e`:

4. **Replace destination mutation during source staging:** Rename/Move Replace now carries the preflight destination snapshot+hash into the executor, calculates fresh strong evidence after the source is staged and immediately before Trash, and restores the source plus terminalizes the journal if the destination changed. A mutation-hook regression rewrites the destination in place with the same byte length during staging and verifies that the current destination and source both remain while Trash is never called.
5. **Replace marker/journal crash window:** destination-marker deletion, source-marker movement and `Verified -> MetaCommitted` now commit in one SQLite `IMMEDIATE` transaction. A committed barrier replays as a no-op, and a close/reopen regression confirms the moved source marker cannot be erased by recovery.

The final independent review of `ba5148e` reported Critical 0, Important 0 and Minor 0 and marked the code READY. Its focused verification passed `m3_file_commands` 21/21, `m3_desktop_runtime` 12/12, `file_transactions` 19/19 and `m3_operation_projections` 6/6.

A later whole-branch safety review identified three additional code-level
Important findings plus stale acceptance evidence. Commit `b103070` removed the
path-only cleanup boundary and migrated the legacy executor to identity-bound
copy/cleanup; retained and revalidated temporary identities through native
Finder publication; added final file-reference, device/inode, alias and
containment checks; made terminal file batches refresh browse/search exactly
once; and guarded completion against replaced project epochs/sessions. Focused
re-review of these corrections reported Critical 0, Important 0 and Minor 0.

The refreshed physical acceptance against `b103070` then passed all seven
required gestures and verified every resulting file by SHA-256. The only
remaining pre-merge action is a fresh whole-branch review that includes this
final evidence update.

The first final-evidence aggregate-gate run exposed an asynchronous test race:
the batch-rename invalid row is focused in a React effect, while its test
asserted synchronously as soon as the row appeared. The assertion now waits for
the focus effect. The focused file passed 20 consecutive runs, the complete
184-test UI suite passed, and the fresh aggregate gate passed at `aaab801`.

A final adversarial re-review then found the native-Quit cache fallback, Trash
parent/leaf reparenting, staged-copy lease lifetime and post-rename recovery
issues described above. Commit `47908bd` closes those boundaries and adds
focused regressions; `2fb612a` updates repository policy so the production
executor must use the staged lease. The independent re-review returned
`Ready to commit`. The fresh aggregate gate passed at `2fb612a`, and the exact
rebuilt package passed the active-session Cmd+Q cache-removal and empty-relaunch
check on 2026-07-22.

## Package, integrity and privacy evidence

```text
pnpm gate:m3                         PASS; exit 0
  repository policy                 9/9
  UI                                185 tests + production build
  portable schema-v3 policy         54 Node tests + live source/copy validation
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
Viewer executable SHA-256             d58863b9e1e772f8c3e7afb53a68667456f0834c52852932185c4400b491ecdb
Viewer DMG SHA-256                    482e71878a8c53d7c1f2c6b4b68537bff97c28f50e6ca2f2a2fffe7fe687824d
```

The exact low-concurrency gate and package build above were repeated from clean
committed code head `2fb612a`. The `.viewer` validator accepts only the manifest,
schema-v3 SQLite database, exact SQLite sidecars and approved prior-schema
backup. It rejects originals, text bodies, thumbnails/proxies, absolute/cache
paths, unknown tables/columns/enums/result codes/files and symlinks. Static
policy and runtime socket inspection found no updater, analytics, crash upload
or application network behavior.

## Scope and remaining review work

- No Windows implementation or cloud behavior was introduced.
- The app and DMG remain ad-hoc signed and are not notarized because Developer ID credentials are outside the internal-only 0.1 scope.
- The synthetic fixture validates correctness but does not replace M4 acceptance with the user's supplied approximately 10 MiB production images.
- All physical acceptance is complete. The final whole-branch evidence review
  returned `Ready to commit and merge`; no M3 review work remains.

## Task 17 decision

The exact gate, package checks, real file/Trash operations,
compare/read-only/lifecycle behavior, portable/privacy boundaries and seven
physical drag checks all pass. The successive independent reviews and focused
re-reviews found no remaining Critical, Important or Minor code finding after
`2fb612a`. Tasks 17 and 18 are complete.

## Local integration result

- Final evidence commit: `8433df74b048d7972bc6f89b9f771d51cb08283e`.
- `codex/m3-organization-comparison` fast-forwarded local `main` from
  `7b12cf23e70df7dad0c975552c7317d1e5cbcc76` to `8433df7` without conflict.
- The exact low-concurrency `pnpm gate:m3` was rerun from merged `main` and
  ended with `M3 organization and comparison gate passed`.
- The merged checkout was clean after the gate; the owned M3 worktree was
  removed and its local feature branch deleted.
