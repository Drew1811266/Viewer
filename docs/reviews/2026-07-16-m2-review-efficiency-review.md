# M2 Review Efficiency Stage Review

- Status: Passed
- Date: 2026-07-16
- Base: `c738ed4` (accepted M1 on `main`)
- Reviewed branch head: `fef4de9`
- Review method: M2 requirement traceability, exact aggregate gate, portable-metadata validation, Apple Silicon package inspection, release-app acceptance on writable/copied/read-only fixtures, cache/network inspection and complete branch diff review
- Decision: **Approved for local fast-forward merge to `main`**

## Exit-criteria traceability

| M2 requirement | Evidence | Result |
| --- | --- | --- |
| `REQ-OBJECT-MARKER` | File and folder review state/favorite combinations, independent inheritance, single/batch writes, reopen and copied-project recovery are covered by Rust/UI tests; the release app visibly retained a Pending+Favorite file and Keep+Favorite folder | Pass |
| `REQ-FLOW-SEARCH` | Release-app search found a CJK filename and a CJK Markdown body term; project/subtree scope, fuzzy name/path, every declared filter, chips, grouped/flat results and bounded excerpts are covered by query/controller suites | Pass |
| `REQ-FLOW-SORT` | Natural 2-before-10 ordering, every sort key/direction and stable grouped/flat results are covered by query/UI tests; packaged-app sort/layout changes worked and reset to filename/ascending/grouped on reopen | Pass |
| `REQ-FLOW-SEARCH-FEEDBACK` (M2) | Debounce, request revision, stale-result rejection, progressive index feedback and actionable empty states are covered by controller/runtime suites; no body or source image decode is present in result-page DTOs | Pass |
| `REQ-FLOW-ORGANIZE` (M2) | Selection-scoped single/batch review commands update portable truth before disposable projections and return per-entity state; M3 still owns filesystem mutation | Pass |
| `REQ-FLOW-SHORTCUTS` (M2) | `1/2/3/0/F` act on the current selection, suppress writes for editable focus/read-only state and retain selection after refresh; file and folder shortcuts were exercised in the release app | Pass |
| `REQ-IA-FILE-INFO` (M2) | Command-I showed relative-path-only single-file information and marker state in the release app; aggregate count/size/type/common-marker behavior is covered by component and desktop integration tests | Pass |
| `REQ-IA-FOLDER-CARDS` (M2) | Release-app cards showed folder markers, descendant reviewed totals and Keep/Pending/Reject/Unmarked/Favorite counts propagating through arbitrary-depth ancestors | Pass |
| `REQ-TECH-PORTABLE-METADATA` (M2) | Versioned manifest plus schema-v2 SQLite store, FULL durability, migrations, stable UUID/path evidence, read-only behavior, copied-project identity and exact allow-list validator all pass; M4 retains external-rename fingerprint recovery | Pass |

## Review findings

No Critical or Important product defect was found during packaged-app acceptance.

Computer Use does not expose modifier state on pointer clicks, so it could not faithfully synthesize Command-click/Shift-click in the packaged grid. Single-file and single-folder selection/marking, keyboard shortcuts and information display were exercised in the release app. Multi-selection range/toggle, batch marking, preserved selection and aggregate information are covered by `ContentBrowser`, `MarkerControls`, `InfoOverlay`, reducer/controller and desktop-runtime tests. One physical modifier-click batch review remains an explicit M4 manual release check rather than being represented as GUI automation evidence.

The runtime contains no updater, telemetry, analytics or application network dependency. The only frontend capability remains native folder selection plus event listen/unlisten; CSP restricts connections to the Tauri IPC origin and images to local application/opaque image-protocol origins. An `lsof` inspection of the M2 release process while a project was active reported no TCP or UDP socket.

## Packaged-application acceptance

The writable fixture contained folders at four content depths, an empty folder, CJK/natural-sort names, JPG/PNG with sRGB, Display P3, alpha and EXIF orientation, Markdown/TXT, and one corrupt image. Observed in the release application:

- the folder-only tree represented arbitrary depth and the corrupt image failed independently;
- CJK filename search returned two images, and a CJK term found only inside Markdown returned the independent text file;
- file type, marker, favorite, orientation, pixel-dimension and time filters were exposed; filter chips, every sort family, direction, grouped/flat layout and session reset behaved as specified;
- a folder was marked Keep+Favorite, an image Pending+Favorite, and the folder/ancestor cards immediately showed partial progress and aggregate counts;
- Command-I exposed relative path, type, size and marker information without an absolute path;
- closing and reopening preserved portable markers while clearing query, filter, sort, layout and selection state.

The whole project was copied to `/tmp/viewer-m2-copy.6qa2tl/project-copy`. Both source and copy validated with project ID `fd25a3a3-4b74-4fee-92df-88c24dfe3fe5`, schema version 2, two markers and identical metadata database hashes. The copied release-app view retained the folder marker and descendant statistics.

Two permission fixtures were then opened from `/tmp/viewer-m2-ro.eBR00R`:

- existing `.viewer`: marker state remained readable, all marker controls were disabled, search worked and a shortcut attempt left the metadata SHA-256 unchanged at `5caf30011f1f52567f7217823e801a298da5ccd9dca6633b34db776fd8ba103c`;
- absent `.viewer`: browsing/search worked in visible read-only mode and no `.viewer` directory was created before or after close.

For each active project, Viewer created only its owned session SQLite/images directory below `~/Library/Caches/com.viewer.desktop/sessions`. Explicit close returned to the empty import surface and removed the complete session directory. The portable directory contained only `project.json` and `metadata.sqlite`; the validator and string inspection found no original file copy, text body, thumbnail/proxy, absolute project path or cache path.

## Package and automated evidence

```text
pnpm gate:m2                         PASS twice before GUI acceptance and fresh in Task 14
  inherited M1 aggregate gate       PASS
  repository policy                 PASS
  UI                                46 tests + production build
  portable metadata policy          19 Node tests + live source/copy validation
  cargo fmt / strict Clippy          PASS
  locked Rust workspace tests        PASS
  M2 portable/projection/index/query/browse suites PASS
  desktop runtime/security boundary  PASS
  dependencies/licenses/audits       PASS
  G3 indexed-search gate             PASS across 20 fresh sessions
pnpm build:macos                     PASS; Viewer.app + Viewer_0.1.0_aarch64.dmg
file / lipo                          Mach-O 64-bit arm64 / arm64 only
Info.plist / LC_BUILD_VERSION        macOS 13.0 minimum
bundle                               com.viewer.desktop / 0.1.0
codesign                             ad-hoc (expected for internal unsigned build)
hdiutil verify                       VALID
portable metadata source/copy       same project ID, schema 2, two markers
read-only existing metadata hash     unchanged
read-only absent metadata            no .viewer created
session cache after project close    empty
lsof release process TCP/UDP         no sockets
```

## Accepted limitations and later ownership

- M3 owns rename/copy/move/Trash, operation progress/conflicts, undo, comparison, watcher reconciliation and the complete filesystem-mutation read-only matrix.
- M4 owns external-rename fingerprint recovery, physical modifier-click batch acceptance, accessibility/keyboard completion, final color and real-folder performance evidence, crash-residue cleanup, network-deny release evidence and final distribution governance.
- The synthetic fixture validates integrated M2 behavior but does not replace M4 acceptance with the user's supplied approximately 10 MiB production images.
- The application and DMG are ad-hoc signed and not notarized because Developer ID credentials were not provided; this is allowed for the current internal-only distribution scope.

## Decision

The fresh exact M2 gate, production build, arm64/macOS 13 package inspection and `main...HEAD` review all pass. The complete diff was reviewed for architecture direction, durable portable writes, stale-request handling, read-only behavior, error redaction, security/capability boundaries, test quality and M2/M3 scope separation. No unresolved Critical or Important finding remains. M2 meets its exit criteria and may be fast-forward merged to `main`; M3 planning may begin only after the merged `main` reruns the exact M2 gate successfully.

Post-approval evidence: `codex/m2-review-efficiency` was fast-forward merged at `873134f`; `pnpm install --frozen-lockfile` and the exact `pnpm gate:m2` both passed from merged `main` with no worktree changes.
