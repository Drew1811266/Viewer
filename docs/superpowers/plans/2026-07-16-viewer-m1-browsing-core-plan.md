# Viewer M1 Browsing Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver the complete M1 browsing core: one local project session, progressive folder-only navigation, folder overviews, virtualized image/text browsing, safe image and text preview, task feedback, and disposable session-cache lifecycle.

**Architecture:** Implement M1 as vertical slices through the frozen modular-monolith boundaries. Domain/Application add only platform-neutral browse, text, and session contracts; Infrastructure owns SQLite queries and rebuildable caches; `viewer-platform-macos` owns native project access and external URL opening; `viewer-desktop` owns Tauri state, narrow commands/events, safe DTOs, and protocol registration; React is a recoverable projection of Rust state.

**Tech Stack:** Rust stable, Tokio, Tauri 2, SQLite/rusqlite, Quick Look Thumbnailing, Image I/O, `encoding_rs`, `pulldown-cmark`, React 19, TypeScript 6, Vite 8, Vitest, Testing Library.

## Global Constraints

- Viewer 0.1 builds only for Apple Silicon (`aarch64-apple-darwin`) and requires macOS 13 or newer.
- Runtime content remains local; no login, telemetry, automatic crash upload, update check, or application-originated network request.
- Supported content is limited to JPG, PNG, Markdown, and TXT.
- One application session opens at most one project root, and every new window starts empty.
- A readable but non-writable root opens in a visibly read-only session; M1 has no mutation surface and never attempts `.viewer` writes.
- Original files remain in the project; Viewer never creates a second complete asset library.
- Thumbnails, proxies, and full-text indexes are session data and are deleted when the project session closes.
- The frontend receives no filesystem, shell, database, or HTTP permission; folder selection is the single reviewed dialog capability and all file access occurs through Viewer commands.
- Every project-relative path is revalidated at the Rust boundary; links, aliases, hidden entries, and `.viewer` never enter normal browsing.
- Image work uses bounded representations; text preview reads at most 10 MiB and never edits source files.
- Every feature follows TDD and ends with focused tests plus a small commit.
- M2 search/sort/review metadata, M3 file mutation/compare/watcher UI, and M4 final accessibility/color/performance/release work are not pulled into M1.

---

## File Map

### Application and domain contracts

- `crates/viewer-application/src/project.rs`: project-session use case and immutable active-session descriptor.
- `crates/viewer-application/src/browse.rs`: folder tree/workspace projections and `BrowseIndexPort`.
- `crates/viewer-application/src/text.rs`: bounded text-preview request/result/encoding contracts and port.
- `crates/viewer-application/src/ports.rs`: re-export the new narrow ports only.
- `crates/viewer-application/src/lib.rs`: public module surface.

### Infrastructure and macOS adapters

- `crates/viewer-infrastructure/src/session_cache.rs`: disposable session directory, stale cleanup, size-accounted image representation cache.
- `crates/viewer-infrastructure/src/search/index.rs`: browse queries and entity lookup over the existing session index.
- `crates/viewer-infrastructure/src/text/preview.rs`: bounded UTF-8/UTF-16/GB18030 reader.
- `crates/viewer-platform-macos/src/lib.rs`: hardened root probe and validated external URL opener.

### Tauri composition root

- `src-tauri/src/error.rs`: the only IPC error DTO and safe internal-error mapping.
- `src-tauri/src/dto.rs`: camelCase command/event DTOs with no absolute path or cache path.
- `src-tauri/src/state.rs`: one active `DesktopSession`, generation/cancellation, index/cache/image services.
- `src-tauri/src/commands/{mod,project,browse,preview}.rs`: narrow Viewer commands.
- `src-tauri/src/markdown.rs`: CommonMark conversion, local-resource rewriting, and ammonia sanitization.
- `src-tauri/src/image_protocol.rs`: active-session-aware artifact resolver.
- `src-tauri/src/lib.rs`: composition and lifecycle hooks only.

### React presentation

- `ui/src/api/{types,viewer}.ts`: exact IPC/event types and injectable bridge.
- `ui/src/state/{viewerReducer,useViewerController}.ts`: deterministic UI projection and command orchestration.
- `ui/src/components/EmptyProject.tsx`: picker/drop entry.
- `ui/src/components/FolderTree.tsx`: flattened, fixed-row virtual folder-only tree.
- `ui/src/components/FolderOverview.tsx`: category/content-folder cards.
- `ui/src/components/VirtualGrid.tsx`: reusable fixed-row grid windowing.
- `ui/src/components/ContentBrowser.tsx`: image grid, independent text list, selection, keyboard entry to preview.
- `ui/src/components/{ImagePreview,TextPreview,InfoOverlay,TaskBar}.tsx`: dynamic workspaces and feedback.
- `ui/src/styles/app.css`: two-column macOS-oriented layout and responsive states.
- `ui/src/App.tsx`: surface routing and no business logic.

---

### Task 1: Project-session application use case

**Files:**
- Create: `crates/viewer-application/src/project.rs`
- Modify: `crates/viewer-application/src/lib.rs`
- Test: `crates/viewer-application/src/project.rs`

**Interfaces:**
- Consumes: `ProjectProbePort::probe`, `ProjectSession`, `TaskCoordinator::begin_session`.
- Produces: `ActiveProject { project_id, session_id, generation, root, display_name, access }`, `ProjectSessionService<P>::open`, `active`, and `close`.

- [x] **Step 1: Write the failing use-case tests**

```rust
#[test]
fn open_canonicalizes_one_real_directory_and_rejects_a_second_open() {
    let root = tempfile::tempdir().unwrap();
    let service = ProjectSessionService::new(AllowProbe, Arc::new(TaskCoordinator::default()));
    let opened = service.open(root.path()).unwrap();
    assert_eq!(opened.root, root.path().canonicalize().unwrap());
    assert_eq!(opened.access, ProjectAccess::ReadWrite);
    assert!(matches!(service.open(root.path()), Err(ProjectOpenError::AlreadyOpen)));
}

#[cfg(unix)]
#[test]
fn open_rejects_a_symlinked_root_before_probe() {
    let actual = tempfile::tempdir().unwrap();
    let parent = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(actual.path(), parent.path().join("linked")).unwrap();
    let service = ProjectSessionService::new(AllowProbe, Arc::new(TaskCoordinator::default()));
    assert!(matches!(service.open(&parent.path().join("linked")), Err(ProjectOpenError::UnsafeRoot)));
}
```

- [x] **Step 2: Run the focused tests and verify RED**

Run: `cargo test -p viewer-application project::tests -- --nocapture`

Expected: FAIL because `project` and `ProjectSessionService` do not exist.

- [x] **Step 3: Implement the minimal application service**

```rust
pub struct ActiveProject {
    pub project_id: ProjectId,
    pub session_id: SessionId,
    pub generation: Generation,
    pub root: PathBuf,
    pub display_name: String,
    pub access: ProjectAccess,
}

pub struct ProjectSessionService<P> {
    probe: P,
    coordinator: Arc<TaskCoordinator>,
    state: Mutex<(ProjectSession, Option<ActiveProject>)>,
}

impl<P: ProjectProbePort> ProjectSessionService<P> {
    pub fn open(&self, requested: &Path) -> Result<ActiveProject, ProjectOpenError>;
    pub fn active(&self) -> Option<ActiveProject>;
    pub fn close(&self) -> Result<Option<ActiveProject>, ProjectOpenError>;
}
```

Implementation rules: use `symlink_metadata` before `canonicalize`, reject a link or non-directory, call the probe only after canonicalization, derive `display_name` from the final path component, begin exactly one coordinator generation, and drive every failure through `fail_open`/`finish_close`.

- [x] **Step 4: Run application tests and verify GREEN**

Run: `cargo test -p viewer-application project::tests`

Expected: PASS including read-write, read-only, failed-open cleanup, close cancellation, and second-open cases.

- [x] **Step 5: Commit**

```bash
git add crates/viewer-application/src/project.rs crates/viewer-application/src/lib.rs
git commit -m "feat: add project session use case"
```

### Task 2: Disposable session cache and representation reuse

**Files:**
- Create: `crates/viewer-infrastructure/src/session_cache.rs`
- Modify: `crates/viewer-infrastructure/src/lib.rs`
- Test: `crates/viewer-infrastructure/src/session_cache.rs`

**Interfaces:**
- Consumes: `SessionId`, `ImageCacheKey`, rendered artifact paths.
- Produces: `SessionCache::create`, `index_path`, `image_root`, `lookup_image`, `insert_image`, `cleanup`, and `cleanup_stale`.

- [x] **Step 1: Write failing cache-lifecycle tests**

```rust
#[test]
fn cleanup_removes_only_the_owned_session_directory() {
    let base = tempfile::tempdir().unwrap();
    let first = SessionCache::create_in(base.path(), SessionId::new()).unwrap();
    let sibling = base.path().join("keep-me");
    fs::create_dir(&sibling).unwrap();
    let owned = first.root().to_owned();
    first.cleanup().unwrap();
    assert!(!owned.exists());
    assert!(sibling.exists());
}

#[test]
fn representation_cache_reuses_a_live_artifact_and_evicts_lru_over_budget() {
    let cache = fixture_cache_with_limit(8);
    cache.insert_image(key(1), fixture_artifact(1, 6)).unwrap();
    cache.insert_image(key(2), fixture_artifact(2, 6)).unwrap();
    assert!(cache.lookup_image(key(1)).is_none());
    assert!(cache.lookup_image(key(2)).is_some());
}
```

- [x] **Step 2: Run the focused tests and verify RED**

Run: `cargo test -p viewer-infrastructure session_cache::tests -- --nocapture`

Expected: FAIL because `SessionCache` does not exist.

- [x] **Step 3: Implement owned cleanup and a 2 GiB LRU**

```rust
pub const SESSION_CACHE_LIMIT_BYTES: u64 = 2 * 1024 * 1024 * 1024;

pub struct SessionCache {
    root: PathBuf,
    images: Mutex<ImageLru>,
}

impl SessionCache {
    pub fn create_in(base: &Path, session_id: SessionId) -> Result<Self, SessionCacheError>;
    pub fn index_path(&self) -> PathBuf;
    pub fn image_root(&self) -> PathBuf;
    pub fn lookup_image(&self, key: ImageCacheKey) -> Option<CachedImage>;
    pub fn insert_image(&self, key: ImageCacheKey, image: CachedImage) -> Result<(), SessionCacheError>;
    pub fn cleanup(self) -> Result<(), SessionCacheError>;
    pub fn cleanup_stale(base: &Path, active: Option<SessionId>) -> Result<usize, SessionCacheError>;
}
```

Only delete descendants whose directory name parses as `SessionId`; validate canonical containment before removal. Missing artifacts are removed from the map on lookup. Eviction deletes only paths under this session's `images` directory.

- [x] **Step 4: Run cache and security tests and verify GREEN**

Run: `cargo test -p viewer-infrastructure session_cache::tests && cargo test --test security_boundaries project_paths`

Expected: PASS with no deletion outside the owned cache root.

- [x] **Step 5: Commit**

```bash
git add crates/viewer-infrastructure/src/session_cache.rs crates/viewer-infrastructure/src/lib.rs
git commit -m "feat: add disposable session cache"
```

### Task 3: Browse projections and SQLite query port

**Files:**
- Create: `crates/viewer-application/src/browse.rs`
- Modify: `crates/viewer-application/src/lib.rs`
- Modify: `crates/viewer-infrastructure/src/search/index.rs`
- Test: `tests/m1_browse_queries.rs`

**Interfaces:**
- Consumes: indexed `FileNode` rows.
- Produces: `FolderTreeItem`, `BrowserFile`, `ContentFolderCard`, `FolderWorkspace`, `BrowseIndexPort`, and `BrowseService`.

- [x] **Step 1: Write failing deep-tree and category/content tests**

```rust
#[test]
fn category_query_returns_descendant_content_folders_at_arbitrary_depth() {
    let index = indexed_fixture(&[
        directory("catalog"), directory("catalog/shoes"),
        directory("catalog/shoes/id-001"), jpeg("catalog/shoes/id-001/front.jpg"),
        text("catalog/shoes/id-001/prompt.md"), directory("empty"),
    ]);
    let service = BrowseService::new(&index);
    let tree = service.folder_tree().unwrap();
    assert_eq!(paths(&tree), ["catalog", "catalog/shoes", "catalog/shoes/id-001", "empty"]);
    let workspace = service.folder_workspace(id("catalog")).unwrap();
    let FolderWorkspace::Category { folders } = workspace else { panic!() };
    assert_eq!(folders[0].relative_path.as_str(), "catalog/shoes/id-001");
    assert_eq!((folders[0].image_count, folders[0].text_count), (1, 1));
}
```

- [x] **Step 2: Run the integration test and verify RED**

Run: `cargo test --test m1_browse_queries -- --nocapture`

Expected: FAIL because the browse port/projections are missing.

- [x] **Step 3: Add exact platform-neutral browse interfaces**

```rust
pub trait BrowseIndexPort: Send + Sync {
    fn all_folders(&self) -> Result<Vec<FileNode>, BrowseIndexError>;
    fn node(&self, entity_id: EntityId) -> Result<Option<FileNode>, BrowseIndexError>;
    fn node_by_relative_path(&self, path: &RelativePath) -> Result<Option<FileNode>, BrowseIndexError>;
    fn direct_children(&self, folder: Option<EntityId>) -> Result<Vec<FileNode>, BrowseIndexError>;
    fn descendants(&self, folder: Option<EntityId>) -> Result<Vec<FileNode>, BrowseIndexError>;
}

pub enum FolderWorkspace {
    Category { folders: Vec<ContentFolderCard> },
    Content { images: Vec<BrowserFile>, text_files: Vec<BrowserFile> },
    Empty,
}
```

`BrowseService::folder_workspace(Option<EntityId>)` supports the project root as `None`, derives parents by validated relative path, includes empty folders in the tree, treats a folder with direct supported files as content, treats an ancestor with descendant content folders as category, and uses the first four image rows in stable `name COLLATE NOCASE, relative_path, entity_id` order for card representatives. `node_by_relative_path` is the only Markdown-resource lookup and returns no body or absolute path.

- [x] **Step 4: Implement `BrowseIndexPort for SessionIndex` with parameterized SQL**

Add entity lookup, all-folder, direct-child, and prefix-bounded descendant queries. Do not return text bodies or absolute paths.

- [x] **Step 5: Run browse plus G3 regression tests and verify GREEN**

Run: `cargo test --test m1_browse_queries && cargo test --test progressive_scan && cargo test --test search`

Expected: PASS for arbitrary depth, duplicate names, empty folders, 0–4 representative images, and existing scan/search behavior.

- [x] **Step 6: Commit**

```bash
git add crates/viewer-application/src/browse.rs crates/viewer-application/src/lib.rs crates/viewer-infrastructure/src/search/index.rs tests/m1_browse_queries.rs
git commit -m "feat: query folder browsing projections"
```

### Task 4: Bounded multi-encoding text preview

**Files:**
- Create: `crates/viewer-application/src/text.rs`
- Modify: `crates/viewer-application/src/lib.rs`
- Create: `crates/viewer-infrastructure/src/text/mod.rs`
- Create: `crates/viewer-infrastructure/src/text/preview.rs`
- Modify: `crates/viewer-infrastructure/src/lib.rs`
- Modify: `Cargo.toml`
- Modify: `crates/viewer-infrastructure/Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `THIRD_PARTY_NOTICES.md`
- Test: `tests/m1_text_preview.rs`

**Interfaces:**
- Consumes: one canonical project file and optional `TextEncoding` override.
- Produces: `TextPreviewPort::read`, `TextPreview { text, encoding, truncated }`, and typed content errors.

- [x] **Step 1: Write failing encoding/boundary tests**

```rust
#[test]
fn reader_detects_utf8_utf16_and_gb18030_without_mutating_source() {
    for (bytes, expected, encoding) in fixtures() {
        let path = write_fixture(bytes);
        let before = fs::read(&path).unwrap();
        let preview = TextPreviewReader.read(&path, None).unwrap();
        assert_eq!(preview.text, expected);
        assert_eq!(preview.encoding, encoding);
        assert_eq!(fs::read(&path).unwrap(), before);
    }
}

#[test]
fn reader_stops_at_ten_mib_on_a_character_boundary() {
    let preview = TextPreviewReader.read(&oversized_utf8_fixture(), None).unwrap();
    assert!(preview.truncated);
    assert!(preview.text.len() <= MAX_TEXT_PREVIEW_BYTES);
    assert!(std::str::from_utf8(preview.text.as_bytes()).is_ok());
}
```

- [x] **Step 2: Run the integration test and verify RED**

Run: `cargo test --test m1_text_preview -- --nocapture`

Expected: FAIL because the preview port/reader are missing.

- [x] **Step 3: Add and review `encoding_rs` as a direct dependency**

Run: `cargo add encoding_rs@0.8 --package viewer-infrastructure`

Record its exact locked version, MPL-2.0/Apache-2.0 license expression, purpose, and upstream in `THIRD_PARTY_NOTICES.md`; run `./scripts/check-locked-dependencies.sh` before committing.

- [x] **Step 4: Implement the bounded reader**

```rust
pub const MAX_TEXT_PREVIEW_BYTES: usize = 10 * 1024 * 1024;

pub enum TextEncoding { Utf8, Utf16Le, Utf16Be, Gb18030 }

pub trait TextPreviewPort: Send + Sync {
    fn read(&self, source: &Path, encoding: Option<TextEncoding>)
        -> Result<TextPreview, TextPreviewError>;
}
```

Read at most `MAX_TEXT_PREVIEW_BYTES + 4`, prefer BOM, then strict UTF-8, then strict GB18030. A manual encoding bypasses detection for this call only. Normalize CRLF/CR to LF. Return `EncodingRequired` rather than replacement characters when strict decoding fails.

- [x] **Step 5: Run text, dependency, and formatting checks and verify GREEN**

Run: `cargo test --test m1_text_preview && ./scripts/check-locked-dependencies.sh && cargo fmt --check`

Expected: PASS for empty, BOM, UTF-8, UTF-16 LE/BE, GB18030, invalid bytes, truncation, and I/O isolation.

- [x] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock THIRD_PARTY_NOTICES.md crates/viewer-application crates/viewer-infrastructure
git commit -m "feat: add bounded text preview reader"
```

### Task 5: Safe desktop DTOs, errors, and runtime state

**Files:**
- Create: `src-tauri/src/error.rs`
- Create: `src-tauri/src/dto.rs`
- Create: `src-tauri/src/state.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/Cargo.toml`
- Test: `tests/m1_desktop_runtime.rs`

**Interfaces:**
- Consumes: frozen internal error types and Tasks 1–4 services.
- Produces: `CommandError { code, category, user_message, retryable, task_id, item_id }`, one `DesktopRuntime`, and exact serializable project/browse/task DTOs.

- [x] **Step 1: Write failing DTO/error tests**

```rust
#[test]
fn command_error_is_safe_and_has_the_frozen_shape() {
    let secret = PathBuf::from("/Users/example/secret/project");
    let error = CommandError::from(ProjectProbeError::NotDirectory { path: secret.clone() });
    let json = serde_json::to_string(&error).unwrap();
    assert_eq!(error.category, ErrorCategory::Validation);
    assert!(!json.contains(secret.to_str().unwrap()));
    assert!(json.contains("userMessage"));
    assert!(json.contains("retryable"));
}

#[tokio::test]
async fn runtime_allows_exactly_one_active_desktop_session() {
    let runtime = fixture_runtime();
    runtime.open_project(fixture_root()).await.unwrap();
    assert_eq!(runtime.open_project(fixture_root()).await.unwrap_err().code, "project_already_open");
}
```

- [x] **Step 2: Run the desktop-runtime test and verify RED**

Run: `cargo test -p viewer-desktop --test m1_desktop_runtime -- --nocapture`

Expected: FAIL because the DTO, error, and runtime modules do not exist.

- [x] **Step 3: Implement the one safe boundary error**

```rust
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: String,
    pub category: ErrorCategory,
    pub user_message: String,
    pub retryable: bool,
    pub task_id: Option<String>,
    pub item_id: Option<String>,
}
```

Map errors exhaustively by type. Never format an internal error with `Debug` or expose source/cache/SQLite paths. Content failures identify only the project-relative item when one is available.

- [x] **Step 4: Implement `DesktopRuntime` and dependency-injected test constructor**

`DesktopRuntime` owns `Mutex<Option<DesktopSession>>`, the application project service, artifact registry, and cache base. `DesktopSession` owns canonical root only inside Rust, session/project IDs, access, generation, `Arc<SessionIndex>`, `Arc<dyn ImagePort>`, `Arc<SessionCache>`, and optional scan task. No DTO contains canonical root or cache path.

- [x] **Step 5: Run focused and boundary tests and verify GREEN**

Run: `cargo test -p viewer-desktop --test m1_desktop_runtime && cargo test -p viewer-desktop --test security_boundaries`

Expected: PASS with exact error serialization and no secret paths.

- [x] **Step 6: Commit**

```bash
git add src-tauri/src/error.rs src-tauri/src/dto.rs src-tauri/src/state.rs src-tauri/src/lib.rs src-tauri/Cargo.toml tests/m1_desktop_runtime.rs
git commit -m "feat: compose safe desktop runtime"
```

### Task 6: Project picker, drop, progressive scan, and close commands

**Files:**
- Create: `src-tauri/src/commands/mod.rs`
- Create: `src-tauri/src/commands/project.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/capabilities/main.json`
- Modify: `tests/security_boundaries.rs`
- Modify: `Cargo.lock`
- Modify: `ui/package.json`
- Modify: `pnpm-lock.yaml`
- Modify: `THIRD_PARTY_NOTICES.md`
- Test: `tests/m1_desktop_runtime.rs`

**Interfaces:**
- Consumes: `DesktopRuntime`, `ProjectWalker`, `CoordinatedScan`, official folder dialog.
- Produces: `open_project`, `close_project`, `project_snapshot`, `cancel_task`, and coalesced `viewer://scan-progress` events.

- [x] **Step 1: Write failing project-flow tests**

```rust
#[tokio::test]
async fn open_publishes_folders_before_files_and_close_cancels_then_deletes_cache() {
    let (runtime, events) = fixture_runtime_with_events(deep_project());
    let opened = runtime.open_project(deep_project().root()).await.unwrap();
    runtime.wait_for_scan(opened.session_id).await.unwrap();
    assert!(events.first_folder_index() < events.first_file_index());
    let cache = runtime.cache_path_for_test().await.unwrap();
    runtime.close_project().await.unwrap();
    assert!(!cache.exists());
    assert!(runtime.snapshot().await.is_none());
}

#[tokio::test]
async fn a_readable_non_writable_root_opens_read_only_without_project_metadata_writes() {
    let runtime = fixture_runtime_with_probe(ProjectAccess::ReadOnly);
    let project = deep_project();
    let opened = runtime.open_project(project.root()).await.unwrap();
    assert_eq!(opened.access, ProjectAccessDto::ReadOnly);
    assert!(!project.root().join(".viewer").exists());
}
```

- [x] **Step 2: Run focused test and verify RED**

Run: `cargo test -p viewer-desktop --test m1_desktop_runtime project_flow -- --nocapture`

Expected: FAIL because scanning/event/close orchestration is not implemented.

- [x] **Step 3: Add the official dialog plugin and exact narrow capability**

Run: `cargo add tauri-plugin-dialog@2 --package viewer-desktop && pnpm --dir ui add @tauri-apps/plugin-dialog@^2`

Set `permissions` to exactly `dialog:allow-open`, `core:event:allow-listen`, and `core:event:allow-unlisten`. Update the G4 security test to assert this reviewed allowlist and still reject fs/shell/http/process capabilities. Add exact locked dependency entries and licenses to `THIRD_PARTY_NOTICES.md`.

- [x] **Step 4: Implement progressive scan orchestration**

On open: create cache/index/image services, publish an immediate active-project DTO, then spawn `CoordinatedScan`. For each event, verify session/generation, commit the batch to `SessionIndex`, and emit a safe event only after commit. Coalesce progress to at most 20 Hz while never delaying the first folder batch. On close: take the session, cancel coordinator/image work, abort and await scan, clear artifact tokens, drop services, then delete the owned cache.

- [x] **Step 5: Wire commands and lifecycle hooks**

Register only Viewer commands plus `health`. Intercept window destroy/close and application exit to call the same close path; Dock reopen creates an empty window state. Do not restore a path from disk.

- [x] **Step 6: Run project flow, security, and dependency checks and verify GREEN**

Run: `cargo test -p viewer-desktop --test m1_desktop_runtime && ./scripts/check-tauri-security.sh && ./scripts/check-locked-dependencies.sh`

Expected: PASS; folders precede files, close removes cache, stale generations emit nothing, and only the reviewed dialog/event permissions exist.

- [x] **Step 7: Commit**

```bash
git add Cargo.lock pnpm-lock.yaml THIRD_PARTY_NOTICES.md src-tauri ui/package.json tests/security_boundaries.rs tests/m1_desktop_runtime.rs
git commit -m "feat: open and close progressive project sessions"
```

### Task 7: Folder query and cached image representation commands

**Files:**
- Create: `src-tauri/src/commands/browse.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/dto.rs`
- Modify: `src-tauri/src/state.rs`
- Modify: `src-tauri/src/image_protocol.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `tests/m1_desktop_runtime.rs`
- Test: `src-tauri/src/image_protocol.rs`

**Interfaces:**
- Consumes: `BrowseService`, `ImagePort`, `SessionCache`, `ImageArtifactRegistry`.
- Produces: `folder_tree`, `query_folder`, `request_image_representation`, and session-bound `viewer-image://localhost/{session}/{token}` URLs.

- [x] **Step 1: Write failing browse/image command tests**

```rust
#[tokio::test]
async fn representation_revalidates_entity_and_reuses_cached_artifact() {
    let runtime = opened_fixture_runtime().await;
    let first = runtime.request_image(image_id(), thumbnail(320, 2000)).await.unwrap();
    let second = runtime.request_image(image_id(), thumbnail(320, 2000)).await.unwrap();
    assert_eq!(first.cache_key, second.cache_key);
    assert_ne!(first.url, second.url); // fresh unguessable registry token
    assert_eq!(runtime.render_count(), 1);
}

#[test]
fn protocol_tracks_the_current_session_across_close_and_reopen() {
    let active = ActiveImageSession::default();
    let resolver = ImageProtocolResolver::new(active.clone(), registry());
    active.set(Some(session_a()));
    assert!(resolver.resolve(&url_for(session_a())).is_ok());
    active.set(Some(session_b()));
    assert_eq!(resolver.resolve(&url_for(session_a())), Err(ProtocolError::Forbidden));
}
```

- [x] **Step 2: Run focused tests and verify RED**

Run: `cargo test -p viewer-desktop request_image protocol_tracks -- --nocapture`

Expected: FAIL because the browse/image commands and active-session resolver are missing.

- [x] **Step 3: Implement query DTO mapping**

Return IDs as opaque strings, paths as validated relative strings, and file metadata only. Folder-card representative entries initially contain nullable image URLs; React requests visible thumbnails separately.

- [x] **Step 4: Implement the cached representation path**

Re-read the indexed node by entity ID, require JPEG/PNG, join against the private root, canonicalize the file and parent, reject any escape/link/reserved component, compute `ImageCacheKey`, render only on cache miss, register the artifact, and return width/height/backend/url. A fit preview uses Image I/O; thumbnails use Quick Look with the accepted fallback.

- [x] **Step 5: Make the protocol follow `ActiveImageSession`**

```rust
#[derive(Clone, Default)]
pub struct ActiveImageSession(Arc<RwLock<Option<SessionId>>>);
```

Set it only after project activation and clear it before removing registry/cache entries. Continue requiring exact GET, localhost authority, two normalized path segments, current session, registered random token, image MIME, and no-store headers.

- [x] **Step 6: Run M1 image and G1/security regression tests and verify GREEN**

Run: `cargo test -p viewer-desktop && ./scripts/run-g1-image-gate.sh && ./scripts/check-tauri-security.sh`

Expected: PASS with one render for identical source/representation and no cross-session access.

- [x] **Step 7: Commit**

```bash
git add src-tauri/src/commands src-tauri/src/dto.rs src-tauri/src/state.rs src-tauri/src/image_protocol.rs src-tauri/src/lib.rs tests/m1_desktop_runtime.rs
git commit -m "feat: query folders and deliver cached images"
```

### Task 8: Safe Markdown/TXT preview and explicit external links

**Files:**
- Create: `src-tauri/src/markdown.rs`
- Create: `src-tauri/src/commands/preview.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/dto.rs`
- Modify: `src-tauri/src/state.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `Cargo.toml`
- Modify: `src-tauri/Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `THIRD_PARTY_NOTICES.md`
- Modify: `crates/viewer-platform-macos/src/lib.rs`
- Test: `tests/m1_desktop_runtime.rs`
- Test: `tests/security_boundaries.rs`

**Interfaces:**
- Consumes: `TextPreviewPort`, indexed text entity, local image representation path.
- Produces: `preview_text`, `open_external_link`, sanitized `plainText` or `markdownHtml`, encoding/truncation metadata.

- [x] **Step 1: Write failing malicious-content and encoding-choice tests**

```rust
#[tokio::test]
async fn markdown_preview_strips_active_remote_and_escaping_resources() {
    let runtime = opened_markdown_fixture().await;
    let preview = runtime.preview_text(markdown_id(), None).await.unwrap();
    assert!(!preview.markdown_html.contains("<script"));
    assert!(!preview.markdown_html.contains("onerror"));
    assert!(!preview.markdown_html.contains("https://remote.example/image.png"));
    assert!(!preview.markdown_html.contains("../outside.png"));
    assert!(preview.markdown_html.contains("viewer-image://localhost/"));
}

#[test]
fn external_link_policy_allows_only_user_clicked_http_and_https() {
    assert!(ExternalUrl::parse("https://example.com/a").is_ok());
    assert!(ExternalUrl::parse("file:///etc/passwd").is_err());
    assert!(ExternalUrl::parse("javascript:alert(1)").is_err());
}
```

- [x] **Step 2: Run focused tests and verify RED**

Run: `cargo test -p viewer-desktop markdown_preview external_link_policy -- --nocapture`

Expected: FAIL because Markdown conversion and external URL policy are missing.

- [x] **Step 3: Add and review `pulldown-cmark`**

Run: `cargo add pulldown-cmark@0.13 --package viewer-desktop`

Record the exact locked version, MIT license, purpose, and upstream in `THIRD_PARTY_NOTICES.md`; verify with the locked dependency script.

- [x] **Step 4: Implement safe preview conversion**

Enable headings, paragraphs, lists, blockquotes, fenced code, tables, emphasis, and rules. Disable raw HTML events. Pre-resolve only relative JPG/JPEG/PNG image destinations against the Markdown file's parent, then pass them through the same root/entity/image representation validation and rewrite to registered `viewer-image` URLs. Strip remote, absolute, escaping, linked, alias, unsupported, and failed resources. Sanitize the final HTML through the existing explicit ammonia policy.

- [x] **Step 5: Implement explicit external-link handoff**

`open_external_link` accepts only normalized HTTP/HTTPS URLs, only after a UI click, and delegates to a macOS adapter that invokes the system default handler. It must never fetch the URL inside Viewer. Unit-test validation; make any real-launch smoke test opt-in.

- [x] **Step 6: Run text/security/dependency regressions and verify GREEN**

Run: `cargo test --test m1_text_preview && cargo test -p viewer-desktop && ./scripts/check-tauri-security.sh && ./scripts/check-locked-dependencies.sh`

Expected: PASS for syntax, encodings, truncation, sanitization, local-resource boundaries, and explicit URL policy.

- [x] **Step 7: Commit**

```bash
git add Cargo.toml Cargo.lock THIRD_PARTY_NOTICES.md crates/viewer-platform-macos src-tauri tests
git commit -m "feat: preview local text safely"
```

### Task 9: Typed frontend bridge and deterministic project controller

**Files:**
- Create: `ui/src/api/types.ts`
- Create: `ui/src/api/viewer.ts`
- Create: `ui/src/state/viewerReducer.ts`
- Create: `ui/src/state/useViewerController.ts`
- Create: `ui/src/components/EmptyProject.tsx`
- Modify: `ui/src/App.tsx`
- Modify: `ui/src/App.test.tsx`
- Test: `ui/src/state/viewerReducer.test.ts`
- Test: `ui/src/components/EmptyProject.test.tsx`

**Interfaces:**
- Consumes: exact Tasks 5–8 command/event DTOs and dialog API.
- Produces: injectable `ViewerBridge`, reducer states `empty|opening|active|closing|error`, and import/drop/close actions.

- [x] **Step 1: Write failing reducer and empty-entry tests**

```tsx
it('always starts empty and ignores stale scan generations', () => {
  const active = reduce(initialState, opened(project('s1', 7)))
  const next = reduce(active, scanFolders('s1', 6, [folder('stale')]))
  expect(next).toEqual(active)
})

it('opens only a dropped directory and reports a rejected file', async () => {
  render(<EmptyProject bridge={bridge} />)
  await dropPath('/fixture/not-a-directory.jpg')
  expect(await screen.findByRole('alert')).toHaveTextContent('请选择项目文件夹')
  expect(bridge.openProject).not.toHaveBeenCalled()
})
```

- [x] **Step 2: Run UI tests and verify RED**

Run: `pnpm --dir ui test -- App.test.tsx viewerReducer.test.ts EmptyProject.test.tsx`

Expected: FAIL because the bridge/controller/components do not exist.

- [x] **Step 3: Implement exact frontend types and injectable bridge**

```ts
export interface ViewerBridge {
  chooseProject(): Promise<string | null>
  openProject(path: string): Promise<ProjectSnapshot>
  closeProject(): Promise<void>
  folderTree(): Promise<FolderTreeItem[]>
  queryFolder(entityId: string): Promise<FolderWorkspace>
  requestImage(request: ImageRequest): Promise<ImageRepresentation>
  previewText(request: TextPreviewRequest): Promise<TextPreview>
  openExternalLink(url: string): Promise<void>
  listenScan(handler: (event: ScanEvent) => void): Promise<UnlistenFn>
}
```

The production bridge is the only module importing Tauri APIs. Tests inject a fake. Error display uses `userMessage`, never raw thrown objects.

- [x] **Step 4: Implement controller lifecycle**

Subscribe once, discard wrong session/generation, fetch a snapshot after event gaps, close on explicit project close, and remove listeners on unmount. Display a persistent `只读项目` banner whenever `ProjectSnapshot.access === 'read_only'`; do not render write actions in M1. Keep an unreadable-root failure on the entry surface with a retry/reselect action. Do not persist a root/path in localStorage, IndexedDB, URL, or settings.

- [x] **Step 5: Run UI tests/build and verify GREEN**

Run: `pnpm --dir ui test && pnpm --dir ui build`

Expected: PASS and production TypeScript compiles with no `any` DTO escape hatch.

- [x] **Step 6: Commit**

```bash
git add ui/src/api ui/src/state ui/src/components/EmptyProject.tsx ui/src/App.tsx ui/src/App.test.tsx
git commit -m "feat: control one frontend project session"
```

### Task 10: Virtual folder tree and progressive folder overview

**Files:**
- Create: `ui/src/components/FolderTree.tsx`
- Create: `ui/src/components/FolderOverview.tsx`
- Create: `ui/src/components/VirtualList.tsx`
- Create: `ui/src/components/FolderTree.test.tsx`
- Create: `ui/src/components/FolderOverview.test.tsx`
- Modify: `ui/src/App.tsx`
- Create: `ui/src/styles/app.css`
- Modify: `ui/src/main.tsx`

**Interfaces:**
- Consumes: progressive `FolderTreeItem[]`, `FolderWorkspace.Category`.
- Produces: resizable/collapsible left column, virtual flat rows, current-folder context, metadata-first 2×2 cards.

- [x] **Step 1: Write failing tree/card tests**

```tsx
it('renders only folders, including deep and empty folders, with full relative path labels', () => {
  render(<FolderTree folders={deepFolders} selectedId="id3" onSelect={fn} />)
  expect(screen.getByText('catalog/shoes/id-001')).toBeVisible()
  expect(screen.getByText('empty')).toBeVisible()
  expect(screen.queryByText('front.jpg')).not.toBeInTheDocument()
  expect(screen.queryByText('.viewer')).not.toBeInTheDocument()
})

it('shows card metadata before representative thumbnails resolve', () => {
  render(<FolderOverview folders={[cardWithFourPendingImages]} />)
  expect(screen.getByText('4 张图片')).toBeVisible()
  expect(screen.getByText('1 个文本')).toBeVisible()
  expect(screen.getAllByLabelText('缩略图加载中')).toHaveLength(4)
})

it('offers an explicit aggregate view without changing the selected category path', async () => {
  render(<FolderOverview folders={categoryCards} onShowAll={showAll} />)
  await user.click(screen.getByRole('button', { name: '显示全部后代文件' }))
  expect(showAll).toHaveBeenCalledOnce()
  expect(screen.getByText('catalog/shoes')).toBeVisible()
})
```

- [x] **Step 2: Run focused UI tests and verify RED**

Run: `pnpm --dir ui test -- FolderTree.test.tsx FolderOverview.test.tsx`

Expected: FAIL because these components do not exist.

- [x] **Step 3: Implement fixed-row virtualization and tree behavior**

Flatten only expanded nodes, compute visible `[start,end)` from `scrollTop`, viewport height, 28 px row height, and six-row overscan. Preserve selection when the column collapses/resizes. Use full relative path as accessible label and basename as primary visual label.

- [x] **Step 4: Implement metadata-first cards**

Render name/path/counts immediately; independently request up to four representative thumbnails while the card is visible. Keep stable cell positions for failures/empty slots. Selecting a category card queries that content folder without changing project context. An explicit `显示全部后代文件` action requests the backend aggregate projection while the breadcrumb/tree selection remains on the category folder; mixed descendant files are never the default.

- [x] **Step 5: Run tests/build and verify GREEN**

Run: `pnpm --dir ui test && pnpm --dir ui build`

Expected: PASS for deep/empty/duplicate-name trees, expand/collapse, resize, category cards, and progressive thumbnail states.

- [x] **Step 6: Commit**

```bash
git add ui/src/components/FolderTree* ui/src/components/FolderOverview* ui/src/components/VirtualList.tsx ui/src/styles/app.css ui/src/App.tsx ui/src/main.tsx
git commit -m "feat: browse progressive folder hierarchy"
```

### Task 11: Virtual image grid, text list, selection, and task feedback

**Files:**
- Create: `ui/src/components/VirtualGrid.tsx`
- Create: `ui/src/components/ContentBrowser.tsx`
- Create: `ui/src/components/TaskBar.tsx`
- Create: `ui/src/components/ContentBrowser.test.tsx`
- Create: `ui/src/components/TaskBar.test.tsx`
- Modify: `ui/src/App.tsx`
- Modify: `ui/src/styles/app.css`

**Interfaces:**
- Consumes: `FolderWorkspace.Content`, image representation requests, scan progress/failures.
- Produces: virtual responsive grid, independent Markdown/TXT list, standard selection, active item, thumbnail requests only for visible cells, compact expandable task bar.

- [ ] **Step 1: Write failing selection/virtualization/task tests**

```tsx
it('supports click command-toggle shift-range arrows and space without losing grid state', async () => {
  render(<ContentBrowser workspace={manyFiles} />)
  await click(file('1.jpg'))
  await commandClick(file('3.jpg'))
  await shiftClick(file('6.jpg'))
  expect(selectedNames()).toEqual(['1.jpg', '3.jpg', '4.jpg', '5.jpg', '6.jpg'])
  await press('ArrowRight')
  await press(' ')
  expect(onPreview).toHaveBeenCalledWith(activeImageId())
})

it('shows progress compactly and keeps failures until dismissed', () => {
  render(<TaskBar task={failedScanTask} />)
  expect(screen.getByText('扫描项目')).toBeVisible()
  expect(screen.getByText('2 项失败')).toBeVisible()
})
```

- [ ] **Step 2: Run focused UI tests and verify RED**

Run: `pnpm --dir ui test -- ContentBrowser.test.tsx TaskBar.test.tsx`

Expected: FAIL because the content browser/task bar do not exist.

- [ ] **Step 3: Implement grid windowing and visible work**

Compute column count from measured width and selected small/medium/large cell size; window rows with two-row overscan. The thumbnail effect keys by visible entity/size/device scale, aborts stale requests, never schedules all project images, and reports coalesced requested/completed/failed counts to the task model.

- [ ] **Step 4: Implement standard selection and keyboard rules**

Single click replaces selection, Command toggles, Shift selects the inclusive stable list range, arrows move the active item, double-click or Space previews an image/text, and Enter only emits a future rename intent (disabled until M3). Suppress global shortcuts while input/contenteditable owns focus.

- [ ] **Step 5: Implement independent text list and task bar**

Markdown/TXT appear below a labelled `文本文件` divider and never pair with images. Task bar shows scan and visible-image work counts/progress/failures, expands for safe relative-item details, offers cancel only for cancellable derived work, auto-collapses on clean completion, and retains failures. Text preview work is represented while active but never exposes file contents in task details.

- [ ] **Step 6: Run UI tests/build and verify GREEN**

Run: `pnpm --dir ui test && pnpm --dir ui build`

Expected: PASS with bounded rendered-cell counts for a 1,000-item fixture and all selection/task states.

- [ ] **Step 7: Commit**

```bash
git add ui/src/components/VirtualGrid.tsx ui/src/components/ContentBrowser* ui/src/components/TaskBar* ui/src/App.tsx ui/src/styles/app.css
git commit -m "feat: browse virtual image and text content"
```

### Task 12: Image preview, text preview, and on-demand information

**Files:**
- Create: `ui/src/components/ImagePreview.tsx`
- Create: `ui/src/components/TextPreview.tsx`
- Create: `ui/src/components/InfoOverlay.tsx`
- Create: `ui/src/components/ImagePreview.test.tsx`
- Create: `ui/src/components/TextPreview.test.tsx`
- Create: `ui/src/components/InfoOverlay.test.tsx`
- Modify: `ui/src/App.tsx`
- Modify: `ui/src/styles/app.css`

**Interfaces:**
- Consumes: image/text preview DTOs and current content order.
- Produces: fit/100%/free zoom, pan, display-only quarter-turn rotation, previous/next, Markdown/TXT preview, encoding retry, Command-I overlay, and grid restoration.

- [ ] **Step 1: Write failing preview-state tests**

```tsx
it('restores selection and scroll after fit zoom rotate and next-image preview', async () => {
  const browser = renderViewerAtGridState({ selected: ['b.jpg'], scrollTop: 640 })
  await press(' ')
  expect(screen.getByRole('img', { name: 'b.jpg' })).toHaveAttribute('data-mode', 'fit')
  await click(screen.getByRole('button', { name: '顺时针旋转' }))
  await press('ArrowRight')
  await press(' ')
  expect(browser.grid()).toHaveScrollTop(640)
  expect(file('b.jpg')).toHaveAttribute('aria-selected', 'true')
})

it('renders sanitized markdown and retries only the current preview encoding', async () => {
  render(<TextPreview preview={encodingRequired} />)
  await selectEncoding('GB18030')
  expect(bridge.previewText).toHaveBeenCalledWith(expect.objectContaining({ encoding: 'gb18030' }))
  expect(localStorage.length).toBe(0)
})
```

- [ ] **Step 2: Run focused UI tests and verify RED**

Run: `pnpm --dir ui test -- ImagePreview.test.tsx TextPreview.test.tsx InfoOverlay.test.tsx`

Expected: FAIL because preview components do not exist.

- [ ] **Step 3: Implement bounded image-preview state**

Default to fit. Request original only for explicit 100% and accept a typed `budget_exceeded` fallback. Clamp zoom to 10%–800%, pan only when content exceeds viewport, apply rotation after EXIF-correct proxy display, and never write rotation. Request at most previous/current/next fit representations and release URLs/state outside that window.

- [ ] **Step 4: Implement text preview and link handling**

TXT uses a selectable `<pre>` and Markdown uses only backend-sanitized HTML. Intercept anchor clicks, prevent WebView navigation, and call `openExternalLink`; never render backend text with a client Markdown parser. Show encoding selector and truncation note when supplied.

- [ ] **Step 5: Implement on-demand information overlay**

Command-I toggles an edge overlay rather than a third column. Single selection shows name, full relative path, kind, dimensions when known, size, and modified time. Multi-selection in M1 shows count, total size, and kind distribution; common review markers remain an M2 addition.

- [ ] **Step 6: Run UI tests/build and verify GREEN**

Run: `pnpm --dir ui test && pnpm --dir ui build`

Expected: PASS for preview navigation, zoom/rotation state, grid restoration, text selection, encoding, sanitization, explicit links, and overlay toggling.

- [ ] **Step 7: Commit**

```bash
git add ui/src/components/ImagePreview* ui/src/components/TextPreview* ui/src/components/InfoOverlay* ui/src/App.tsx ui/src/styles/app.css
git commit -m "feat: preview images and text in context"
```

### Task 13: M1 integrated gate, documentation, and review evidence

**Files:**
- Create: `scripts/run-m1-browsing-gate.sh`
- Modify: `scripts/repository-policy.test.mjs`
- Modify: `package.json`
- Modify: `docs/superpowers/plans/2026-07-16-viewer-0.1-roadmap.md`
- Create: `docs/reviews/2026-07-16-m1-browsing-core-review.md`
- Test: `tests/m1_desktop_runtime.rs`
- Test: all UI tests

**Interfaces:**
- Consumes: every M1 slice and G1–G4 gates.
- Produces: one reproducible `pnpm gate:m1` command and stage-review evidence suitable for the merge decision.

- [ ] **Step 1: Add a failing repository-policy assertion for the M1 gate**

```js
test('M1 browsing gate is exact and mandatory', () => {
  const pkg = JSON.parse(readFileSync('package.json', 'utf8'))
  assert.equal(pkg.scripts['gate:m1'], './scripts/run-m1-browsing-gate.sh')
  assert.match(readFileSync('scripts/run-m1-browsing-gate.sh', 'utf8'), /pnpm verify/)
  assert.match(readFileSync('scripts/run-m1-browsing-gate.sh', 'utf8'), /m1_desktop_runtime/)
})
```

- [ ] **Step 2: Run policy test and verify RED**

Run: `node --test scripts/repository-policy.test.mjs`

Expected: FAIL because the gate script/package command do not exist.

- [ ] **Step 3: Implement the aggregate M1 gate**

The executable script must run, with locked inputs:

```bash
pnpm verify
./scripts/check-locked-dependencies.sh
./scripts/check-tauri-security.sh
cargo test --locked --test m1_browse_queries
cargo test --locked --test m1_text_preview
cargo test --locked -p viewer-desktop --test m1_desktop_runtime
node scripts/check-scope-coverage.mjs
```

It must also fail if `.viewer`, an absolute project path, a cache path, or unsupported file kind appears in captured IPC fixtures.

- [ ] **Step 4: Exercise the real application with a disposable M1 fixture**

Run the Tauri dev app against a fixture containing deep folders, empty folders, duplicate names, JPG/PNG, corrupt image, Markdown/TXT encodings, unsupported/hidden/link entries, and at least 1,000 lightweight indexed items. Verify picker and drag-in, folder-first progressive tree, category cards, virtual grid, selection/keyboard preview, text rendering, close-to-empty, reopen-empty, and cache removal. Record observed results; do not claim final M4 performance acceptance.

- [ ] **Step 5: Run the complete M1 gate and Apple Silicon build**

Run: `pnpm gate:m1 && pnpm build:macos`

Expected: PASS; `.app` and DMG build for arm64/macOS 13+; no test, lint, dependency, scope, or security failure.

- [ ] **Step 6: Review the full M1 diff**

Review `git diff 8149d57...HEAD` for Critical/Important issues, architecture/API drift, stale publication, path disclosure, unbounded work, cache ownership, keyboard race, and scope creep. Fix every Critical/Important issue with a failing test and rerun the gate. Record findings, fixes, commands, accepted M4 deferrals, and product evidence in `docs/reviews/2026-07-16-m1-browsing-core-review.md`.

- [ ] **Step 7: Mark M1 complete only after evidence is green**

Update the roadmap M1 status and its link to the stage review. Do not start M2 or merge while any M1 gate is red.

- [ ] **Step 8: Commit**

```bash
git add package.json scripts/run-m1-browsing-gate.sh scripts/repository-policy.test.mjs docs/superpowers/plans/2026-07-16-viewer-0.1-roadmap.md docs/reviews/2026-07-16-m1-browsing-core-review.md
git commit -m "docs: approve M1 browsing core"
```

---

## M1 Exit Criteria

- Every launch/window starts at the empty import surface; one real directory can be picked or dropped and a second project cannot open concurrently.
- Folder-only tree appears progressively, includes deep/empty folders, excludes hidden/`.viewer`/link/alias entries, and remains interactive through virtual rows.
- Category folders show metadata-first descendant content-folder cards; content folders show a virtual image grid and independent Markdown/TXT list.
- Visible thumbnails and requested previews use the accepted native pipeline, are cached only for the session, and cannot be read across sessions.
- Single-image preview supports fit, explicit 100%, free zoom/pan, display-only rotation, navigation, and restoration of grid selection/scroll.
- TXT/Markdown preview is read-only, bounded to 10 MiB, supports UTF-8/UTF-16/GB18030 selection, strips active/remote/escaping content, and hands explicit external links to the system.
- Scan/image/text failures are isolated and mapped to safe DTOs; task progress is compact, expandable, cancel-aware, and generation-safe.
- Closing the project/window/app cancels derived work, invalidates tokens, deletes only the owned session cache, and returns future windows to empty.
- `pnpm gate:m1`, `pnpm build:macos`, full-diff review, and the M1 review document all pass before local fast-forward merge to `main`.
