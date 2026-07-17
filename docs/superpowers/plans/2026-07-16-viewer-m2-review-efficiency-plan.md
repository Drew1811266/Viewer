# Viewer M2 Review Efficiency Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` to implement this plan task-by-task. This repository session does not delegate to subagents. Keep the checkboxes and review evidence current.

**Goal:** Deliver the complete M2 review workflow: portable file/folder review states and favorites, project/subtree fuzzy and full-text search, complete filtering and sorting, batch marking shortcuts, folder progress, and multi-selection information without weakening M1 security or performance.

**Architecture:** M2 keeps portable user truth and rebuildable search state separate. A versioned `.viewer/project.json` and `.viewer/metadata.sqlite` own stable project identity and markers; the existing session SQLite database owns scan/search/derived image and text metadata and is deleted on close. Marker writes commit to portable storage first and then synchronize the disposable index. Search pages remain bounded and never contain text bodies. Visible body matches may request one separately bounded, plain-text excerpt. Image filter metadata comes from Image I/O header probes, never a search-time/full-image decode.

**Tech Stack:** Existing Rust/Tokio/Tauri 2 modular monolith, rusqlite/SQLite FTS5, nucleo-matcher, Image I/O, React 19, TypeScript 6, Vite 8, Vitest, Testing Library. No new direct dependency is planned.

## Frozen M2 Decisions

- `Apache-2.0` remains the repository and package license. No source copied from another application is introduced.
- Writable projects atomically create/reuse `.viewer/project.json` and `.viewer/metadata.sqlite`. Read-only projects may read existing metadata but never create or modify `.viewer`.
- Portable metadata uses SQLite rollback journaling, `synchronous=FULL`, foreign keys, a five-second busy timeout, schema backups before migration, and relative paths only.
- A marker consists of one optional review state (`keep`, `pending`, `reject`) plus an independent favorite boolean. Files and folders may be marked; folder marks never inherit.
- Portable marker identity is a stable marker UUID plus relative path, kind, size/mtime evidence, and an optional BLAKE3 fingerprint column reserved for M3/M4 reconciliation. A copied project recovers M2 marks by stable project ID and unchanged relative paths. External-rename fingerprint recovery is completed in M4.
- Review-state ascending order is `unmarked`, `keep`, `pending`, `reject`; descending reverses it. Pixel sorting uses pixel count, then width, height, natural path, and entity ID.
- Natural ordering is case-insensitive with numeric runs compared numerically (`2` before `10`), then original spelling/path and entity ID for stable ties. Folders always use natural ascending order.
- Search ORs selections within one filter category and ANDs categories. It supports file kind, review state, favorite, unmarked, orientation, width/height, size, and modified-time bounds.
- Grouped results sort content-folder groups naturally and sort files inside each group. Flat results apply the selected file sort globally. Pages are capped at 200 rows.
- Search request revisions are separate from the project scan generation. The Rust runtime and React controller both suppress stale revisions; changing a query never invalidates the active project scan.
- Base scan, text indexing, and image-header indexing publish independent progress. Partial results are useful immediately and remain labelled as updating until relevant indexing finishes.
- Search result DTOs contain node/marker/image metadata, match field, score, and filename/path match ranges only. They contain no text body. A separate command returns at most 160 Unicode scalar values of normalized plain text for one visible body hit.
- Text indexing reads at most 10 MiB and supports the same UTF-8/UTF-16/GB18030 detection as M1 preview. Search never reads originals directly.
- Closing a project resets query, scope, filters, sort, grouping, selection, snippets, progress, and all session index/cache state.
- M3 still owns rename/copy/move/Trash, marker undo, compare, watcher reconciliation, and file-operation progress. M2 does not pull those surfaces forward.

## File Map

### Portable truth

- `crates/viewer-application/src/metadata.rs`: marker and project-metadata ports plus batch marker service.
- `crates/viewer-infrastructure/src/portable/{mod,schema,identity,markers}.rs`: atomic project manifest, shared portable schema/migration, and marker repository.
- `crates/viewer-infrastructure/migrations/portable/0002_markers.sql`: project and marker tables added to the existing operation journal database.

### Disposable projections and search

- `crates/viewer-domain/src/{file,search}.rs`: marker/image metadata and complete query/sort/filter/result contracts.
- `crates/viewer-application/src/{browse,search}.rs`: enriched browser projections, natural ordering, and coordinated search/snippet ports.
- `crates/viewer-infrastructure/migrations/session/0001_initial.sql`: disposable derived metadata columns and indexes.
- `crates/viewer-infrastructure/src/search/{index,query,text}.rs`: hydration, batch metadata updates, complete search algebra, natural sorting, progress, and bounded snippets.

### Desktop and React

- `src-tauri/src/{state,dto,error}.rs`: portable store composition, derived-index workers, safe M2 DTOs.
- `src-tauri/src/commands/{search,markers}.rs`: bounded M2 commands.
- `ui/src/api/{types,viewer}.ts`: exact M2 bridge.
- `ui/src/state/{viewerReducer,useViewerController}.ts`: session search/selection/marker state and stale suppression.
- `ui/src/components/{SearchToolbar,SearchResults,MarkerControls}.tsx`: review workflow.
- `ui/src/components/{ContentBrowser,FolderOverview,InfoOverlay}.tsx`: marker/progress/aggregate enhancements.

---

### Task 1: Freeze M2 executable contracts and baseline

**Files:**
- Create: `docs/superpowers/plans/2026-07-16-viewer-m2-review-efficiency-plan.md`
- Modify: `docs/superpowers/plans/2026-07-16-viewer-0.1-roadmap.md`

- [x] **Step 1: Verify the isolated M2 worktree starts at merged M1**

Run: `git merge-base --is-ancestor main HEAD && pnpm install --frozen-lockfile && pnpm gate:m1`

Expected: exit 0 with the exact M1 gate passing before any M2 product code.

- [x] **Step 2: Record the M2 architecture decisions and owned requirements**

Map `REQ-OBJECT-MARKER`, `REQ-FLOW-SEARCH`, `REQ-FLOW-SORT`, the M2 portion of `REQ-FLOW-SEARCH-FEEDBACK`, `REQ-FLOW-ORGANIZE`, `REQ-FLOW-SHORTCUTS`, `REQ-IA-FILE-INFO`, `REQ-IA-FOLDER-CARDS`, and `REQ-TECH-PORTABLE-METADATA` to the tasks below. Record explicitly that M2 search-body excerpts are separate bounded requests, not result-page fields.

- [x] **Step 3: Commit the approved M2 plan**

```bash
git add docs/superpowers/plans/2026-07-16-viewer-m2-review-efficiency-plan.md docs/superpowers/plans/2026-07-16-viewer-0.1-roadmap.md
git commit -m "docs: plan M2 review efficiency"
```

### Task 2: Version and migrate portable project metadata

**Files:**
- Create: `crates/viewer-infrastructure/migrations/portable/0002_markers.sql`
- Create: `crates/viewer-infrastructure/src/portable/{mod,schema,identity}.rs`
- Modify: `crates/viewer-infrastructure/src/lib.rs`
- Modify: `crates/viewer-infrastructure/src/operation/journal.rs`
- Test: `tests/m2_portable_metadata.rs`

**Interfaces:**
- Produces: `PortableProjectIdentity::open(root, access, now_ms)`, `PortableDatabase::open`, schema v2 migration, and migration backup evidence.

- [ ] **Step 1: Write failing manifest/migration/read-only tests**

Cover first writable open, stable reopen, project-directory copy, existing read-only metadata, absent read-only metadata, malformed/forward manifest rejection, v1 database migration, one-time v1 backup, and operation-journal reopen after v2 migration. Assert the manifest/database contain no absolute/cache path.

Run: `cargo test --test m2_portable_metadata identity migration -- --nocapture`

Expected: RED because portable identity/schema v2 do not exist.

- [ ] **Step 2: Implement atomic `project.json`**

Use format `{ schemaVersion: 1, projectId, createdAtMs }`. Create the `.viewer` directory only for read-write access. Write a same-directory temporary file, `sync_all`, rename atomically, and sync the directory. Reject symlinks, case variants, non-regular files, unsupported versions, and project-ID/database disagreement. Read-only without metadata returns an ephemeral identity and no writable store.

- [ ] **Step 3: Centralize portable database initialization**

Move the existing journal initializer behind one shared opener. Upgrade v1 to v2 inside one transaction after creating `metadata.sqlite.v1.bak` with exclusive creation and syncing it. Keep `journal_mode=DELETE`, `synchronous=FULL`, and foreign keys. Both marker and operation repositories must accept schema v2 and reject newer versions.

- [ ] **Step 4: Verify GREEN and compatibility**

Run: `cargo test --test m2_portable_metadata && cargo test -p viewer-infrastructure operation_journal && cargo fmt --check`

Expected: all identity, backup, migration, journal, and read-only cases pass.

- [ ] **Step 5: Commit**

```bash
git add crates/viewer-infrastructure tests/m2_portable_metadata.rs
git commit -m "feat: version portable project metadata"
```

### Task 3: Add durable file/folder marker storage

**Files:**
- Modify: `crates/viewer-infrastructure/migrations/portable/0002_markers.sql`
- Create: `crates/viewer-application/src/metadata.rs`
- Create: `crates/viewer-infrastructure/src/portable/markers.rs`
- Modify: `crates/viewer-application/src/{lib,ports}.rs`
- Modify: `crates/viewer-infrastructure/src/portable/mod.rs`
- Test: `tests/m2_portable_metadata.rs`

**Interfaces:**
- Produces: `Marker { review_state, favorite }`, `MarkerTarget`, `MarkerPatch`, `PortableMetadataPort::markers_for_paths`, `apply_batch`, and `MarkerService`.

- [ ] **Step 1: Write failing marker truth tests**

Cover all review/favorite combinations, file and folder rows, clear review without clearing favorite, favorite toggle without changing review, atomic multi-selection update, duplicate targets, rollback on invalid path, reopen, project copy, and no source-file content/mtime mutation.

Run: `cargo test --test m2_portable_metadata markers -- --nocapture`

Expected: RED because marker storage is absent.

- [ ] **Step 2: Implement schema and repository**

Store a marker UUID, canonical relative path, kind, review state, favorite, optional size/mtime/BLAKE3 evidence, and update time. Enforce unique relative paths, valid enums, and no `.viewer` path. Batch changes use one immediate transaction and return committed rows. Clearing both fields may remove the marker row.

- [ ] **Step 3: Implement the application service**

Validate non-empty unique targets and write capability. Commit portable truth first, then call a disposable-index synchronization port. If index sync fails, return a typed `CommittedButProjectionStale` error so the desktop can rehydrate rather than retry the durable mutation blindly.

- [ ] **Step 4: Verify GREEN**

Run: `cargo test --test m2_portable_metadata markers && cargo test -p viewer-application metadata && cargo clippy --locked --workspace --all-targets -- -D warnings`

- [ ] **Step 5: Commit**

```bash
git add crates/viewer-application crates/viewer-infrastructure tests/m2_portable_metadata.rs
git commit -m "feat: persist portable review markers"
```

### Task 4: Enrich and hydrate the disposable session index

**Files:**
- Modify: `crates/viewer-domain/src/{file,search}.rs`
- Modify: `crates/viewer-infrastructure/migrations/session/0001_initial.sql`
- Modify: `crates/viewer-infrastructure/src/search/index.rs`
- Test: `tests/m2_session_projection.rs`

**Interfaces:**
- Produces: `Marker`, `ImageMetadata { width, height }`, `IndexedNode`, `IndexProgress`, batch marker hydration, image-header updates, and text-status progress.

- [ ] **Step 1: Write failing hydration/progress tests**

Prove scan upsert preserves marker/image/text-derived columns, hydration matches portable rows by exact relative path, stale portable paths are ignored, batch marker sync is atomic, image dimensions reject invalid/overflow values, and progress is monotonic.

Run: `cargo test --test m2_session_projection -- --nocapture`

Expected: RED because enriched projections/progress do not exist.

- [ ] **Step 2: Extend the disposable schema and exact index API**

Add width, height, image metadata status, text metadata status, review/favorite indexes, modified/size indexes, and batch update methods. Keep session WAL/NORMAL and make all pages obtain marker/image values from the disposable database, never by opening portable SQLite from the UI path.

- [ ] **Step 3: Implement post-scan hydration**

After each committed scan file/folder batch, query portable markers for those relative paths and synchronize matches. A projection-sync failure marks the index stale and schedules one complete rehydrate; it never overwrites portable truth.

- [ ] **Step 4: Verify GREEN and M1 regression**

Run: `cargo test --test m2_session_projection && cargo test --test m1_browse_queries && cargo test --test progressive_scan`

- [ ] **Step 5: Commit**

```bash
git add crates/viewer-domain crates/viewer-infrastructure tests/m2_session_projection.rs
git commit -m "feat: hydrate disposable review projections"
```

### Task 5: Complete derived text and image-header indexing

**Files:**
- Modify: `crates/viewer-infrastructure/src/search/{text,index}.rs`
- Modify: `src-tauri/src/state.rs`
- Modify: `src-tauri/src/dto.rs`
- Test: `tests/m2_derived_indexing.rs`
- Test: `tests/m2_desktop_runtime.rs`

**Interfaces:**
- Produces: bounded text extraction for M1 encodings, header-only image metadata indexing, `viewer://index-progress`, and cancellation on close.

- [ ] **Step 1: Write failing derived-work tests**

Cover UTF-8/UTF-16/GB18030 text, invalid/over-10-MiB text isolation, valid/corrupt JPG/PNG headers, no rendered image artifact during search indexing, progressive counters, generation cancellation, and close cleanup.

Run: `cargo test --test m2_derived_indexing && cargo test -p viewer-desktop --test m2_desktop_runtime derived -- --nocapture`

Expected: RED because M1 runtime does not schedule derived indexing.

- [ ] **Step 2: Share strict bounded encoding behavior**

Reuse the M1 text decoding policy without preview HTML or source mutation. Store normalized indexed text only in session FTS. Index failures become per-item statuses and do not stop other work.

- [ ] **Step 3: Schedule bounded lower-priority derived work**

Queue text jobs at P3 and image probes at P3/P4 after base rows exist. Probe through `ImagePort::probe`, record width/height only, and never call render. Cap concurrent work and validate session/project generation immediately before publication.

- [ ] **Step 4: Publish coalesced safe progress**

Emit counts/status at no more than 20 Hz, plus final events. DTOs contain no absolute path, text, image content, or cache location.

- [ ] **Step 5: Verify GREEN**

Run: `cargo test --test m2_derived_indexing && cargo test -p viewer-desktop --test m2_desktop_runtime derived && ./scripts/run-g1-image-gate.sh`

- [ ] **Step 6: Commit**

```bash
git add crates/viewer-infrastructure src-tauri tests/m2_derived_indexing.rs tests/m2_desktop_runtime.rs
git commit -m "feat: index bounded review metadata"
```

### Task 6: Implement complete search, filter, sort, grouping, and snippets

**Files:**
- Modify: `crates/viewer-domain/src/search.rs`
- Modify: `crates/viewer-application/src/{search,ports}.rs`
- Modify: `crates/viewer-infrastructure/src/search/{query,index}.rs`
- Test: `tests/m2_search_queries.rs`

**Interfaces:**
- Produces: complete `SearchQuery`, `SearchSort`, `SearchLayout`, `SearchFilters`, bounded `SearchPage`, match ranges, and `text_snippet`.

- [ ] **Step 1: Write failing query-matrix tests**

Cover exact/fuzzy Unicode filename and path, CJK body search, project/subtree scope, every filter independently, OR-within/AND-between algebra, favorite/unmarked distinction, image orientation/dimension bounds, size/time bounds, natural name ordering, modified/size/pixel/review sorts in both directions, grouped/flat layout, stable ties, offset/limit cap, and partial index status.

Run: `cargo test --test m2_search_queries -- --nocapture`

Expected: RED because G3 contracts cover only a subset.

- [ ] **Step 2: Extend domain contracts without exposing bodies**

Return enriched node metadata plus match field, score, group relative path, and normalized filename/path match ranges. Reject limits over 200 and invalid ranges. A blank query with filters/sort is valid.

- [ ] **Step 3: Implement SQL prefilter plus Rust rank/sort**

Use parameterized SQL for scope and filters, existing FTS/nucleo matching for text, and one deterministic natural comparator for all user-visible ordering. Group before page slicing in grouped mode. Search never probes files or decodes images.

- [ ] **Step 4: Implement the separate bounded body-snippet port**

Require current session/generation, entity kind Markdown/TXT, a body match for the normalized query, and an indexed FTS row. Return escaped/plain normalized context of at most 160 scalar values with no absolute path. Result pages themselves remain body-free.

- [ ] **Step 5: Verify GREEN and performance prototype**

Run: `cargo test --test m2_search_queries && cargo test --test search && ./scripts/run-g3-scan-search-gate.sh`

Expected: all query cases pass and the existing indexed-search p95 remains within 100 ms.

- [ ] **Step 6: Commit**

```bash
git add crates/viewer-domain crates/viewer-application crates/viewer-infrastructure tests/m2_search_queries.rs
git commit -m "feat: complete indexed search and sorting"
```

### Task 7: Enrich browsing, folder statistics, and multi-selection information

**Files:**
- Modify: `crates/viewer-application/src/browse.rs`
- Modify: `crates/viewer-infrastructure/src/search/index.rs`
- Modify: `src-tauri/src/dto.rs`
- Test: `tests/m2_browse_projections.rs`

**Interfaces:**
- Produces: marker-aware `BrowserFile`, `FolderTreeItem`, `ContentFolderCard`, `FolderReviewProgress`, and `SelectionInfo`.

- [ ] **Step 1: Write failing projection tests**

Cover natural folder/file order, folder marker independence, descendant file review counts, favorite counts, empty and mixed folders, progressive updates, and multi-selection total size/type/common-review/common-favorite values.

Run: `cargo test --test m2_browse_projections -- --nocapture`

Expected: RED because M1 projections omit markers/statistics.

- [ ] **Step 2: Implement enriched projections**

Cards show keep/pending/reject/unmarked/favorite counts across supported descendant files but never infer a folder's own marker. Info aggregation returns relative paths only and a common value only when every selected item agrees.

- [ ] **Step 3: Use the same natural comparator everywhere**

Tree/card folders are always ascending. Content files honor the active session sort. Folder switches preserve sort; project close restores natural ascending.

- [ ] **Step 4: Verify GREEN**

Run: `cargo test --test m2_browse_projections && cargo test --test m1_browse_queries`

- [ ] **Step 5: Commit**

```bash
git add crates/viewer-application crates/viewer-infrastructure src-tauri/src/dto.rs tests/m2_browse_projections.rs
git commit -m "feat: project review statistics"
```

### Task 8: Expose narrow M2 desktop commands and safe errors

**Files:**
- Create: `src-tauri/src/commands/{search,markers}.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/{state,dto,error,lib}.rs`
- Test: `tests/m2_desktop_runtime.rs`
- Test: `tests/security_boundaries.rs`

**Interfaces:**
- Produces: `search_project`, `search_text_snippet`, `set_review_state`, `toggle_favorite`, and `selection_info`.

- [ ] **Step 1: Write failing runtime/security tests**

Cover exact camelCase DTO shapes, 200-row cap, wrong-session/stale revision suppression, batch marker success, committed-but-stale recovery, read-only rejection, unknown IDs, mixed file/folder targets, project-copy reopen, snippet bounds, and no path/body/SQLite details in errors.

Run: `cargo test -p viewer-desktop --test m2_desktop_runtime -- --nocapture`

Expected: RED because commands are absent.

- [ ] **Step 2: Compose portable identity/store before session activation**

Make `ActiveProject.project_id` the stable manifest ID when available. The runtime owns at most one portable store and closes it before cache teardown. Writable first-open metadata creation happens only after root validation/probe succeeds.

- [ ] **Step 3: Implement request revision tracking and command validation**

Keep one atomic latest search revision per active session. Check session/project generation and revision before work and publication. Validate entity IDs against the current index and re-resolve relative paths before durable marker writes.

- [ ] **Step 4: Register only narrow commands/events**

Do not add frontend filesystem/SQL/network permissions. Map read-only writes to a capability error and all internals to existing safe error categories.

- [ ] **Step 5: Verify GREEN and security regression**

Run: `cargo test -p viewer-desktop --test m2_desktop_runtime && cargo test -p viewer-desktop --test security_boundaries && ./scripts/check-tauri-security.sh`

- [ ] **Step 6: Commit**

```bash
git add src-tauri tests/m2_desktop_runtime.rs tests/security_boundaries.rs
git commit -m "feat: expose safe review commands"
```

### Task 9: Add typed frontend search and marker state

**Files:**
- Modify: `ui/src/api/{types,viewer}.ts`
- Modify: `ui/src/state/{viewerReducer,useViewerController}.ts`
- Test: `ui/src/state/viewerReducer.test.ts`
- Test: `ui/src/state/useViewerController.test.tsx`

**Interfaces:**
- Produces: exact search/marker/progress bridge, session-only query model, 120 ms debounce, and stale-request cancellation.

- [ ] **Step 1: Write failing reducer/controller tests**

Cover Cmd-F intent, query/scope/filter/sort/group changes, chip removal/clear all, folder-switch persistence, close reset, rapid-typing out-of-order responses, progress events, visible-only snippet requests, selection-preserving marker responses, and read-only write suppression.

Run: `pnpm --dir ui test -- viewerReducer.test.ts useViewerController.test.tsx`

Expected: RED because M2 frontend state is absent.

- [ ] **Step 2: Add exact bridge types and fakeable methods**

No `any`, filesystem path, body, or backend-error escape hatch. Keep production Tauri imports inside `viewer.ts`.

- [ ] **Step 3: Implement deterministic session state**

Debounce text input by 120 ms; filter/scope/sort changes may run immediately. Increment request revision, ignore old results, request snippets only for visible body hits, and clear every M2 field on project close/event.

- [ ] **Step 4: Verify GREEN**

Run: `pnpm --dir ui test -- viewerReducer.test.ts useViewerController.test.tsx && pnpm --dir ui build`

- [ ] **Step 5: Commit**

```bash
git add ui/src/api ui/src/state
git commit -m "feat: coordinate session review state"
```

### Task 10: Build the search/filter/sort UI

**Files:**
- Create: `ui/src/components/{SearchToolbar,SearchResults}.tsx`
- Create: `ui/src/components/{SearchToolbar,SearchResults}.test.tsx`
- Modify: `ui/src/App.tsx`
- Modify: `ui/src/styles/app.css`
- Test: `ui/src/App.test.tsx`

- [ ] **Step 1: Write failing interaction/accessibility tests**

Cover Cmd-F focus, fuzzy query, scope selector, every filter control, chips, clear all, sort key/direction, grouped/flat toggle, partial-index status, result highlighting, bounded body snippet rendering, pagination, useful zero state with clear/expand actions, and return to folder context.

Run: `pnpm --dir ui test -- SearchToolbar.test.tsx SearchResults.test.tsx App.test.tsx`

Expected: RED because components do not exist.

- [ ] **Step 2: Implement a compact top toolbar and virtual results**

Keep the two-column shell. Search replaces only the right workspace while preserving tree selection. Use removable chips and Chinese labels. Group headings display validated relative folder paths. Do not render hidden full result bodies.

- [ ] **Step 3: Implement search feedback**

Show `结果仍在更新` while base/text/image metadata is incomplete. Empty state states the query, scope, and active filters and offers `清除筛选` and, for subtree searches, `搜索整个项目`.

- [ ] **Step 4: Verify GREEN and build**

Run: `pnpm --dir ui test -- SearchToolbar.test.tsx SearchResults.test.tsx App.test.tsx && pnpm --dir ui build`

- [ ] **Step 5: Commit**

```bash
git add ui/src/components ui/src/App.tsx ui/src/styles/app.css
git commit -m "feat: add indexed search workspace"
```

### Task 11: Build batch markers, shortcuts, statistics, and information UI

**Files:**
- Create: `ui/src/components/{MarkerControls,MarkerControls.test}.tsx`
- Modify: `ui/src/components/{ContentBrowser,FolderOverview,FolderTree,InfoOverlay}.tsx`
- Modify: corresponding component tests
- Modify: `ui/src/App.tsx`
- Modify: `ui/src/styles/app.css`

- [ ] **Step 1: Write failing batch/shortcut tests**

Cover buttons and `1/2/3/0/F`, single/multi-selection, file/folder targets, independent favorite/review values, no shortcut while input/textarea/contenteditable owns focus, read-only disabled state, folder progress updates, and multi-selection information.

Run: `pnpm --dir ui test -- MarkerControls.test.tsx ContentBrowser.test.tsx FolderOverview.test.tsx FolderTree.test.tsx InfoOverlay.test.tsx`

Expected: RED because M2 marker controls/statistics are absent.

- [ ] **Step 2: Implement marker controls and keyboard policy**

Apply review/favorite changes to all selected IDs. Preserve selection and active item after refresh. Use labelled icons plus text/status, not color alone. Folder selection may be marked from the tree/card action without marking descendants.

- [ ] **Step 3: Render review progress and aggregate info**

Cards show reviewed/total plus state/favorite counts. The information overlay shows total bytes, type counts, and common marker values for multi-selection; it never shows an absolute path.

- [ ] **Step 4: Verify GREEN and complete UI suite**

Run: `pnpm --dir ui test && pnpm --dir ui build`

- [ ] **Step 5: Commit**

```bash
git add ui/src
git commit -m "feat: add batch review workflow"
```

### Task 12: Add the exact M2 gate and portable-metadata validator

**Files:**
- Create: `scripts/run-m2-review-gate.sh`
- Create: `scripts/validate-m2-portable-metadata.mjs`
- Create: `scripts/m2-portable-metadata.test.mjs`
- Modify: `package.json`
- Modify: `docs/milestones/viewer-0.1-scope-matrix.md` only if test ownership needs a clarified filename, not scope changes

- [ ] **Step 1: Write the failing validator policy test**

Generate valid and invalid `.viewer` fixtures. Reject originals, text bodies, thumbnails/proxies, absolute paths, symlinks, unknown files, wrong schema versions, invalid review states, and missing Apache license metadata. Accept only the manifest, database, SQLite transient sidecars during a live transaction, and versioned migration backup.

Run: `node --test scripts/m2-portable-metadata.test.mjs`

Expected: RED because the validator is absent.

- [ ] **Step 2: Implement the M2 gate**

`pnpm gate:m2` must run M1 gate, portable policy tests, complete UI tests/build, fmt, strict Clippy, all locked workspace tests, M2 integration/runtime/security tests, dependency/license/audit checks, scope coverage, G3 search performance, and portable metadata validation.

- [ ] **Step 3: Verify exact gate GREEN twice**

Run: `pnpm gate:m2 && pnpm gate:m2`

Expected: two clean consecutive passes with no uncommitted generated artifact.

- [ ] **Step 4: Commit**

```bash
git add package.json scripts docs/milestones/viewer-0.1-scope-matrix.md
git commit -m "test: gate M2 review efficiency"
```

### Task 13: Perform packaged-app M2 acceptance

**Files:**
- Create: `docs/reviews/2026-07-16-m2-review-efficiency-review.md`
- Modify: `docs/superpowers/plans/2026-07-16-viewer-m2-review-efficiency-plan.md`

- [ ] **Step 1: Build the Apple Silicon release artifact**

Run: `pnpm build:macos`

Inspect `.app` and DMG architecture/minimum macOS metadata; require arm64 and macOS 13.0 minimum.

- [ ] **Step 2: Run real GUI acceptance on a writable fixture**

Using the packaged app, open a deep project, search names/paths/CJK text, exercise all filters/sorts/grouping, verify partial progress, mark file/folder single and batch, use shortcuts, inspect card statistics and multi-info, close/reopen, and confirm marks persist while session filters reset.

- [ ] **Step 3: Run copied-project and read-only acceptance**

Copy the whole project to another path and verify stable project ID/marks. Open a read-only project with and without existing `.viewer`; browsing/search must work, marker actions must be unavailable, and absent metadata must not be created.

- [ ] **Step 4: Inspect lifecycle and content boundaries**

After close, confirm the session cache/index is deleted. Validate `.viewer` contains no original, text body, thumbnail/proxy, absolute path, or cache path. Confirm no network request, telemetry, updater, or broad Tauri capability was introduced.

- [ ] **Step 5: Record evidence and defects**

The review lists every M2 requirement, automated command output, GUI evidence, package inspection, and severity. Critical/Important defects block approval; fixes return to the relevant TDD task and rerun the exact M2 gate.

### Task 14: Final M2 review, approval, and local merge

**Files:**
- Modify: `docs/reviews/2026-07-16-m2-review-efficiency-review.md`
- Modify: `docs/superpowers/plans/2026-07-16-viewer-0.1-roadmap.md`
- Modify: `docs/superpowers/plans/2026-07-16-viewer-m2-review-efficiency-plan.md`

- [ ] **Step 1: Run verification-before-completion evidence**

Run fresh: `git status --short && pnpm gate:m2 && pnpm build:macos`

Expected: clean worktree before generated artifacts, exact gate pass, valid arm64/macOS 13 package, and no stale output substituted for current evidence.

- [ ] **Step 2: Review the complete branch diff**

Run: `git diff --check main...HEAD && git diff --stat main...HEAD && git log --oneline main..HEAD`

Review architecture boundaries, correctness, durability, read-only behavior, security/privacy, accessibility baseline, test quality, and scope exclusions. Record no unresolved Critical/Important finding.

- [ ] **Step 3: Approve and commit the stage review**

Mark M2 complete in the roadmap only after the review is approved.

```bash
git add docs/reviews/2026-07-16-m2-review-efficiency-review.md docs/superpowers/plans/2026-07-16-viewer-0.1-roadmap.md docs/superpowers/plans/2026-07-16-viewer-m2-review-efficiency-plan.md
git commit -m "docs: approve M2 review efficiency"
```

- [ ] **Step 4: Merge locally to `main` and verify the merge**

From `/Users/abc/Project/Viewer`:

```bash
git checkout main
git merge --ff-only codex/m2-review-efficiency
pnpm install --frozen-lockfile
pnpm gate:m2
```

Expected: fast-forward merge and exact M2 gate pass on `main`.

- [ ] **Step 5: Begin M3 only after M2 is merged**

Create the M3 Organization and Comparison plan/worktree from the verified `main`. Do not implement any M3 feature before that plan is committed.

## M2 Exit Criteria

- All Task 1–14 checkboxes are complete and correspond to committed evidence.
- Portable identity/markers survive reopen and whole-project copy; read-only mode never writes.
- Search, filters, sort, grouping, partial feedback, snippets, batch marks, shortcuts, folder statistics, and multi-selection info pass integrated UI/runtime tests.
- Search result pages leak no body content; snippets are separately bounded; search performs no image render/full decode.
- `.viewer` contains only portable metadata/journal artifacts and migration backups, never originals or derived content.
- The exact M2 gate passes on the feature branch and merged `main`.
- The packaged Apple Silicon app passes real GUI acceptance with no open Critical/Important defect.
