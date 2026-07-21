# Viewer M3 Organization and Comparison Design

- Status: Approved refinement of the accepted Product Spec and system architecture; drag architecture amended and approved 2026-07-20
- Date: 2026-07-16 (drag architecture amendment: 2026-07-20)
- Baseline: merged and verified M2 at `ff23c6e`
- Owned milestone: M3 Organization and Comparison

## 1. Purpose and scope

M3 turns Viewer from a read-only reviewer into a controlled local organizer without weakening the M1/M2 browsing, privacy, portability or performance boundaries. It delivers:

- two-to-four-image comparison with synchronized or independent viewport transforms;
- single and batch rename, copy, in-project move and macOS Trash;
- replace/prefix/suffix/number batch-rename previews and validation;
- explicit-handle internal move/Option-copy drag, equivalent command dialogs and native Finder copy export from the file surface;
- session undo for markers, favorites, rename and in-project move;
- current-project Watcher reconciliation for external create/modify/rename/move/delete;
- complete read-only capability enforcement and active-file recovery feedback;
- operation progress, conflicts, partial-failure summaries, close coordination and crash recovery.

M3 does not add cloud features, accounts, editing, permanent deletion, automatic updating, extra file formats, Windows code or a public distribution flow. M4 retains final accessibility/color/performance/security/release evidence, cross-computer fingerprint recovery after an external rename and the two-hour/user-folder acceptance run.

## 2. Approaches considered

### 2.1 File-operation orchestration

1. **Rust command coordinator over the accepted G2 primitives — selected.** React submits opaque entity IDs, destinations and conflict choices. A single Rust write lane revalidates disk identity, persists intent, performs filesystem work, commits portable metadata, synchronizes the disposable index and emits bounded progress. This is the only option that makes every durable journal state truthful and preserves the existing narrow Tauri boundary.
2. Frontend orchestration of small rename/copy commands. This makes cancellation and progress superficially simple, but exposes race windows between filesystem, metadata and index calls and cannot recover after a WebView/process failure.
3. Delegate organization to Finder. This loses batch rename rules, per-item conflict policy, portable marker moves, undo and deterministic recovery.

### 2.2 Compare implementation

1. **One React compare workspace backed by the existing opaque image service — selected.** It keeps two to four pane transforms in session UI state, requests viewport-sized proxies first, and never gives the frontend source paths.
2. Mount two to four copies of the current full-screen preview component. This duplicates global keyboard/listener state and makes synchronized pan/zoom and memory admission unreliable.
3. Embed a native Quick Look panel. It does not provide the required inline marker controls, deterministic multi-pane layout or shared transform model.

### 2.3 External-change handling

1. **Watcher hints plus targeted truth reconciliation — selected.** Existing debounced events produce the smallest safe project-relative roots. Rust re-reads those roots, compares them with the session index and commits one structured change set. Stable device/inode identity associates same-session renames; M4 adds content-fingerprint recovery across reopen/computer moves.
2. Apply FSEvents payloads directly. Events can be coalesced, incomplete or overflowed, so treating them as truth risks false deletes and stale paths.
3. Full-project rescan for every event. Correctness is possible, but frequent AI-generator writes would repeatedly discard useful derived work and violate the accepted smallest-safe-rescan architecture.

### 2.4 Drag-session ownership

1. **Separate internal and external drag initiators — selected, corrected after physical acceptance.** Dragging a dedicated file card/row body surface immediately starts a native copy-only Finder session. A visible organization handle starts a custom Pointer Events session to the folder tree; its mode is fixed to move by default or copy when Option is held at pointer-down. It never creates HTML `DataTransfer`, because Tauri's required native incoming file-drop handler and a complete WebView HTML5 drag/drop session are mutually incompatible. Each gesture has one owner from its first frame, so no WebView-to-AppKit handoff or native-handler/HTML-drop coexistence is attempted.
2. Add a global Organization/Export mode switch. This avoids competing sessions but introduces persistent mode errors and extra interaction cost for a frequent productivity-tool action.
3. Use one fully native AppKit session for both Viewer-internal and Finder destinations. This could eventually unify the gesture, but it requires a custom pasteboard type, native destination routing, modifier-aware operation negotiation and folder hit testing. It is disproportionate for Viewer 0.1 and increases M3 risk.

## 3. Architectural decomposition

M3 remains a modular-monolith milestone and is delivered in four reviewable slices that share one session model and one final gate:

1. **Transaction integration:** portable schema v3, truthful commit barriers, batch planning/execution, recovery, progress, conflict results and undo records.
2. **Organization UI:** rename/copy/move/Trash dialogs, batch rename preview, task summaries, shortcuts, explicit internal organization handles and native Finder drag initiation from file surfaces.
3. **Comparison:** reusable viewport transform, 2–4 pane workspace, inline marker controls and graceful removal.
4. **Reconciliation and lifecycle:** Watcher subscription, expected-change ledger, targeted re-indexing, selection/preview/compare repair, read-only recovery controls and close coordination.

Domain owns operation/compare/reconcile value types and state rules. Application owns planners, command/undo/reconcile services and ports. Infrastructure owns SQLite journals, filesystem executors and index reconciliation. `viewer-platform-macos` owns Darwin no-replace rename, volume checks, Trash, Watcher and the AppKit Finder-drag adapter. `src-tauri` is the composition root and converts stable DTOs/errors. React owns dialogs, focus, selection, explicit drag initiators, the internal Pointer drag coordinator and presentation only. Tauri's native incoming file-drop channel remains enabled exclusively for Finder-to-Viewer project import; it is not reused as the outgoing drag source.

## 4. Portable data and truthful operation states

Portable schema version 3 retains the accepted manifest and marker rows and extends the journal for user-facing outcome/recovery data:

- `operation_batches` records kind, lifecycle, requested/completed/failed/skipped counts and timestamps;
- `operation_items` retains source/destination/temporary paths, policy, evidence and state and adds a stable result code;
- all paths remain validated project-relative strings; no absolute/cache path, original bytes, text body, thumbnail or proxy is stored;
- marker paths move transactionally for successful rename/move; copies start unmarked; Trash leaves a non-visible marker row at the old relative path so a system restoration to the same path can recover it; normal scan projections ignore missing rows;
- migration from v2 creates a durable `metadata.sqlite.v2.bak` before schema changes and is idempotent.

The accepted seven-state protocol remains unchanged, but M3 makes the two logical barriers real:

1. the filesystem executor stops after the final path is verified;
2. the command coordinator updates marker/path metadata, then advances to `MetaCommitted`;
3. it applies or schedules the session-index change, then advances to `IndexSynced`;
4. only then is the item `Completed`.

No code may advance a barrier as a placeholder. Recovery replays only a unique safe action. Ambiguous paths, identities or evidence produce a recovery report and no guessed deletion/overwrite.

## 5. Batch command model

Every mutating request carries `sessionId`, `generation`, a unique ordered list of entity IDs and operation-specific parameters. Limits are 10,000 selected entities and one active file-operation batch. Sources are re-read from the session index and revalidated against the filesystem immediately before execution.

### 5.1 Rename

Single rename edits only the final filename in the same parent. It rejects empty names, `.`/`..`, `/`, NUL, hidden Viewer temporary prefixes, `.viewer` case variants and names that exceed the destination volume's `_PC_NAME_MAX` limit. It preserves the extension only when the user leaves extension editing disabled; the default single-rename field selects the stem.

Batch rename rules are mutually composable in this fixed order:

1. replace all literal occurrences of `find` with `replacement` when `find` is non-empty;
2. add prefix;
3. add suffix before the extension;
4. optionally append a sequence before the extension using current visible sort order, a start integer from 0 through 999999 and 1–6 zero-padding digits.

The preview returns every old/new relative-path pair plus per-row validation errors. Empty/no-op output, duplicate targets, case-fold collisions, occupied targets, reserved/invalid names and unsafe parents block execution. Valid cycles and case-only changes are staged through deterministic operation-ID temporary siblings and remain one user-visible batch/undo step.

### 5.2 Copy and move

The destination is an existing writable visible folder inside the active project and cannot be any selected source/descendant. Copy uses the accepted 1 MiB streaming+BLAKE3+no-replace protocol. Same-volume move uses Darwin no-replace rename; cross-volume move uses verified copy followed by moving the original to Trash only after destination verification and metadata preparation.

Conflict preflight returns each conflict before execution. `skip`, `keep_both` and `replace` are the only policies; a dialog choice may apply to one item or the remaining conflicts. Replace always moves the old destination to Trash before no-replace placement. Copy is never undoable. In-project move is undoable only when all completed reverse actions pass identity/path validation.

### 5.3 Trash

Delete and the Delete key always call the macOS Trash adapter. Directories are not selected in the M3 content grid for Trash; file operations target supported files, including corrupt/unpreviewable files. The confirmation states the item count and that restoration uses system Trash. Trash is not in Viewer undo history.

### 5.4 Cancellation and results

Cancel prevents not-yet-started items. An active stream copy checks cancellation between chunks, removes only its registered temporary and records `Failed(cancelled)`. Atomic rename/Trash steps already in progress finish and are reported. A completed item is never rolled back because another item failed.

The result DTO contains batch/task ID, lifecycle, counts and bounded item rows with entity ID, relative path, status and stable safe code. OS messages and absolute paths never cross IPC. At most 200 detail rows are returned per page.

## 6. Undo

The existing memory-only `UndoStack` becomes part of each `DesktopSession`.

- M2 marker/favorite commands record prior values as one batch after the portable write succeeds.
- Rename/move record final identity plus current/restore paths only for completed items.
- `Command+Z` validates the entire top batch before changing anything; if one action is unsafe, no reverse action starts and the batch remains available with a clear conflict message.
- A reverse file batch runs through the same journaled coordinator, expected-change ledger and final reconciliation as a forward move.
- Copy and Trash never enter the stack.
- Close clears the stack; a reopened project has no undo history.

## 7. Compare workspace

Compare entry requires exactly 2–4 selected JPG/PNG entities from the current content collection. `C` and the toolbar action open a right-workspace compare surface without changing the selected folder context.

- two images use two columns; three use one large left pane plus two stacked right panes; four use a 2×2 grid;
- every pane requests a fit proxy sized to its actual CSS viewport and display scale through the existing session image protocol;
- synchronized mode is default. A normalized transform `{scale, centerX, centerY}` is applied to every pane with per-image clamping; independent mode retains one transform per entity;
- fit, 100%, zoom and pan are available. Rotation remains display-only per pane;
- each pane exposes Keep/Pending/Reject/Clear/Favorite and remove controls without leaving comparison;
- removing a pane preserves remaining transforms and reflows the layout. At one image, compare closes and that image becomes the normal single preview; at zero, it returns to the content grid;
- external deletion or move uses entity identity to retain a pane when possible. A confirmed deletion removes it and shows one concise status message.

The image scheduler admits proxies under the existing four-way budget. Compare never unconditionally decodes four originals; 100% requests may be denied/degraded by `DecodeBudget` and must explain the downgrade.

## 8. Drag and Finder export

Internal organization and Finder export are distinct gestures with distinct session owners. A drag must never begin as HTML5 internal drag and later attempt to become an AppKit drag at the WebView edge.

### 8.1 Internal organization drag

Every image card and text-file row exposes a visible organization handle. Only that handle starts a custom Pointer Events organization session; it is explicitly non-HTML-draggable and creates no `DataTransfer` or Viewer MIME payload. The ordered selection remains in React state as opaque entity IDs. Dragging a selected item uses the complete frozen selection, while dragging an unselected item first selects only that item. Operation mode is sampled at pointer-down and remains fixed for the session: move by default, copy when Option is held.

After a 4 CSS px threshold, a safe count/mode floating label follows the pointer. Folder rows expose only opaque entity IDs for `elementFromPoint` hit testing and advertise valid/invalid state from writable status, ancestry and reserved-boundary checks. The virtual folder list auto-scrolls within a 32 CSS px vertical edge zone at no more than 18 CSS px per animation frame and re-hits after scrolling. Valid PointerUp calls the same preflight, conflict and execute commands as menu actions; invalid/no-target release, Escape, pointer cancel, window blur, workspace replacement and unmount have no filesystem effect. The handle is disabled in read-only or busy sessions.

File item outer option nodes are not draggable. A dedicated body export surface and the Pointer-only organization handle are siblings, guaranteeing that the handle cannot fall through to Finder export. Tauri's native incoming drag-drop handler remains enabled and no HTML5 internal `dragover`/`drop` protocol is used.

### 8.2 Native Finder export

The body of every image card and text-file row is the external drag initiator. Its `dragstart` handler freezes the selected entity IDs, calls `preventDefault()` to stop WebKit from owning an HTML session and immediately invokes the Viewer-owned Finder command. Dragging a selected item exports the complete selection; dragging an unselected item exports only that item. Read-only projects may export because the operation never writes to the project.

Rust validates `sessionId`, `generation`, entity kind, project containment, symlink/alias exclusions and the current device/inode immediately before native publication. The frontend never receives source paths and no broad filesystem capability is added. The macOS adapter starts one copy-only AppKit file-URL session from the window content view on the main thread. It derives the current mouse position from the window and creates the required `LeftMouseDragged` event for the session; it does not require `NSApplication.currentEvent` to survive frontend IPC or asynchronous revalidation. The drag image is bounded and derived from a system icon or existing safe representation, never a new copy of the original file.

Tauri's native incoming file-drop handler remains enabled so a Finder folder can still open a project. External export does not depend on DOM `drag`, `dragover`, window-edge thresholds or `dragleave`, and the former edge/depth handoff state is removed.

Finder export is always copy-only. A cancelled session or invalid Finder destination leaves Viewer unchanged and is not an error. Failure before native-session start produces a stable safe message; stale generation or changed/missing file identity produces a refresh-and-retry message. M3 cannot be approved until single image, multi-image, Markdown/TXT, cancellation and post-relaunch exports pass against a real Finder destination.

## 9. Watcher reconciliation

Opening an active project starts exactly one recursive `MacWatcherPort` subscription. Closing first stops publication, then drops the subscription before index/cache teardown.

Debounced event batches enter `ReconcilePlanner`. The coordinator:

1. rejects stale session/generation and unsafe/hidden/reserved paths;
2. minimizes affected roots and snapshots indexed nodes below them;
3. re-walks only those roots while preserving project-relative paths;
4. compares by session entity identity first, then exact relative path;
5. transactionally removes vanished session rows/FTS, upserts current nodes and invalidates changed image/text derived data;
6. migrates portable marker paths for an unambiguous same-session rename/move;
7. reschedules text/header derivation and refreshes folder/search projections;
8. emits one bounded `viewer://project-changed` summary with affected entity IDs and kinds.

Expected Viewer changes are registered before mutation and may label the reconciliation reason, but they never suppress the truth re-read. Overflow reconciles the smallest known common root or the whole project only when no narrower safe root exists.

React repairs active context deterministically: keep selection/preview/compare by entity ID when still present; otherwise remove missing selections, close a deleted preview, reflow compare and preserve the nearest surviving grid item/scroll anchor. Background changes remain silent unless they affect active context or cause a recoverable error.

## 10. Read-only, permissions and lifecycle

Read-only mode continues to allow browse, search/filter/sort, image/text preview, information and compare. Marker, rename, copy/move into the project, Trash, undo and internal drop are disabled in both UI and Rust. Existing `.viewer` markers remain readable; absent metadata is never created.

The persistent banner adds two actions:

- `重新选择目录` closes the current session and returns to the picker while preserving a safe explanation;
- `打开权限设置` opens the macOS Privacy & Security settings through a narrow platform adapter and only after a direct user click.

An unreadable root fails before workspace activation with a safe recovery message. Permission loss during a session causes the affected operation to fail per item; a failed write does not silently downgrade an already-open session or mutate metadata.

While a file-operation batch is active, explicit project close, Command-W and Command-Q show one choice surface: wait, cancel pending items then close, or remain open. Already committed items remain committed. After the write lane becomes idle, close stops Watcher, clears undo, closes databases/images and removes the session cache.

## 11. UI surfaces and keyboard behavior

M3 keeps the two-column layout and adds transient surfaces only:

- a compact action toolbar/context menu for Compare, Rename, Copy, Move, Trash and Info;
- a single-rename sheet;
- a batch-rename sheet with rule controls and a virtualized full preview table;
- a destination/conflict sheet using the existing folder tree data;
- compare in the right dynamic workspace;
- task-bar operation progress and a paged result drawer;
- concise active-file external-change and recovery banners.

`Enter` opens rename, `C` opens compare for 2–4 images, `Delete` opens Trash confirmation and `Command+Z` requests undo. Single-key shortcuts are disabled for text inputs, contenteditable regions, text preview selection and modal controls. Destructive actions require visible confirmation; no shortcut silently replaces a destination.

## 12. Errors, security and privacy

All file commands revalidate current entity, filesystem type, identity, canonical parent/root containment, `.viewer` exclusion, alias/symlink exclusion, destination permission and volume behavior in Rust. UI disabling is not a security boundary.

Stable error categories and item codes distinguish validation, conflict, environment, consistency and cancellation. Absolute paths, temporary names, raw SQLite errors, hashes and OS diagnostic strings remain internal. Operation result DTOs use relative paths only.

No broad filesystem/shell/network/database Tauri capability is added. Native Finder export, Trash and permission settings are exposed only as Viewer-owned commands operating on current-session entity IDs or fixed system destinations. No telemetry, updater, upload or automatic network request is introduced.

## 13. Testing and acceptance

Every implementation task follows red-green-refactor and ends in a focused commit. M3 adds:

- pure tests for rename rules, name validation, conflicts, compare cardinality/transforms and active-context repair;
- integration tests for truthful journal barriers, metadata path moves, copy/move/Trash partial failures, cancellation, undo validation and crash recovery at every state;
- Watcher tests for create/partial-write/modify/rename/move/delete, bursts, expected changes, overflow, marker relocation and generation teardown;
- desktop command tests for exact DTOs, read-only rejection, stale sessions, safe errors, one-write-lane behavior, close coordination and recovery reports;
- React tests for all dialogs, keyboard suppression, task/results UI, immediate dedicated-body native export, Pointer-only organization handles, frozen pointer-down move/Option-copy semantics, threshold/target/auto-scroll/cancel behavior, gesture-surface isolation, 2–4 comparison, inline markers and external-change recovery;
- security/license/scope checks and a portable-metadata validator upgraded for schema v3;
- packaged-app GUI acceptance for normal/conflict/partial-failure operations on disposable files, real macOS Trash, organization-handle internal drag/Option-copy, single/multi/image/text Finder export, cancelled Finder drag, Finder-folder project import, undo, compare, external Finder changes, read-only mode and cache cleanup.

The native drag adapter has focused tests proving copy-only operation, bounded item preparation and removal of the live-`currentEvent` precondition. The packaged app must prove that Finder receives bytes matching the source hash and that Viewer never mutates the source during export.

The exact `gate:m3` must include inherited `gate:m2`, all new suites, G2 crash matrix, G3 watcher/search gate, UI build/tests, fmt, strict Clippy, locked dependencies, license/audit/security/scope checks and schema-v3 live validation. M3 is approved only after the packaged Apple Silicon app passes, no Critical/Important finding remains and the branch is fast-forward merged to `main` with the exact gate passing again there.

## 14. Frozen M3 decisions

- One serial Rust file-operation lane; no frontend transaction orchestration.
- Stable entity IDs only across IPC; no source absolute paths in React.
- Destination folders are project-internal; Finder export is copy-only.
- File card/row bodies own native Finder export; explicit organization handles own entity-only internal drag. No session changes owner at the window boundary.
- Internal drag samples Option at drag start; default is move and Option is copy for the full session.
- Tauri native incoming drop remains enabled for project-folder import; outgoing export starts a Viewer-owned AppKit session immediately at `dragstart`.
- Rename/move retain markers; copies start unmarked; Trash is system-restored and not Viewer-undoable.
- Conflicts are only Skip, Keep Both or Replace-to-Trash.
- Batch rename rule order is replace, prefix, suffix, sequence.
- Compare accepts exactly 2–4 images and defaults to synchronized transforms/proxies.
- Watcher events are hints; targeted disk reconciliation is truth.
- Same-session device/inode handles external rename association; M4 owns fingerprint recovery after reopen/computer moves.
- Read-only enforcement exists independently in UI and Rust.
- M3 adds no dependency unless the existing Rust/macOS/Tauri stack cannot implement an accepted requirement and the dependency passes Apache-2.0 governance first.
