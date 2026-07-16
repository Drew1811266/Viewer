# Viewer G3 Scan and Search Prototype Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prove that Viewer can publish a usable folder tree quickly, build disposable indexes in the background, search Chinese names/text and reconcile external filesystem changes without stale UI publication.

**Architecture:** Scanner emits bounded batches into a session-index writer. Application tags every request/result with session and generation. Search combines SQLite filters/FTS5 with Unicode fuzzy scoring; Watcher events only trigger scoped reconciliation scans.

**Tech Stack:** Rust, Tokio, walkdir, rusqlite/SQLite FTS5, nucleo-matcher, notify, notify-debouncer-full, encoding_rs.

## Global Constraints

- One active project and one active session index.
- Left tree contains folders only; `.viewer`, hidden directories, symlinks and macOS aliases are excluded.
- Folder/tree publication precedes thumbnails, fingerprints and full-text extraction.
- Markdown/TXT above 10 MB is not indexed and previews at most its first 10 MB.
- Search supports all-project and current-subtree scope, file type, review/favorite filters and stable sorting.
- FTS5 trigram handles general substring search; one/two-character queries use a bounded fallback.
- Watcher events are hints, not truth; reconciliation reads the filesystem.
- Old session/generation results are discarded even if their worker finishes.

---

## Target File Map

```text
crates/viewer-domain/src/{file.rs,search.rs}
crates/viewer-application/src/{scheduler.rs,scan.rs,search.rs}
crates/viewer-application/src/ports.rs
crates/viewer-infrastructure/migrations/session/0001_initial.sql
crates/viewer-infrastructure/src/scan/{mod.rs,walker.rs,reconcile.rs}
crates/viewer-infrastructure/src/search/{mod.rs,index.rs,query.rs,text.rs}
crates/viewer-platform-macos/src/watcher.rs
crates/viewer-test-support/src/project_fixture.rs
tests/{progressive_scan.rs,search.rs,watcher_reconcile.rs}
scripts/{generate-g3-project.sh,run-g3-scan-search-gate.sh}
docs/adr/0003-scan-search-and-generation.md
```

### Task 1: Define file, search and generation contracts

**Files:**
- Create: `crates/viewer-domain/src/file.rs`
- Create: `crates/viewer-domain/src/search.rs`
- Modify: `crates/viewer-domain/src/lib.rs`
- Create: `crates/viewer-application/src/scheduler.rs`
- Modify: `crates/viewer-application/src/lib.rs`

**Interfaces:**
- Produces: `FileKind`, `FileNode`, `SearchQuery`, `SearchScope`, `SearchHit`, `Generation`, `GenerationGuard`.

- [ ] **Step 1: Write failing generation tests**

```rust
#[test]
fn old_generation_cannot_publish_after_bump() {
    let guard = GenerationGuard::default();
    let old = guard.current();
    let current = guard.bump();
    assert!(!guard.is_current(old));
    assert!(guard.is_current(current));
}
```

Run `cargo test -p viewer-application generation`. Expected: FAIL.

- [ ] **Step 2: Implement exact shared types**

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum FileKind { Directory, Jpeg, Png, Markdown, Text }

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ReviewState { Keep, Pending, Reject }

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FileNode {
    pub entity_id: EntityId,
    pub relative_path: RelativePath,
    pub kind: FileKind,
    pub size: u64,
    pub modified_ns: i128,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SearchScope { Project, Subtree(EntityId) }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchQuery {
    pub text: String,
    pub scope: SearchScope,
    pub kinds: Vec<FileKind>,
    pub review_states: Vec<ReviewState>,
    pub favorite_only: bool,
    pub offset: u32,
    pub limit: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum MatchedField { ExactFilename, Filename, Path, Body }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchHit {
    pub node: FileNode,
    pub matched_field: MatchedField,
    pub score: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchPage {
    pub total: u32,
    pub hits: Vec<SearchHit>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Generation(u64);
```

`GenerationGuard` stores an `AtomicU64`; `bump` increments with `SeqCst`, and `is_current` compares the supplied value to the current atomic value.

- [ ] **Step 3: Verify and commit**

Run `cargo test -p viewer-domain -p viewer-application generation`. Expected: PASS.

```bash
git add crates/viewer-domain crates/viewer-application Cargo.lock
git commit -m "feat: define scan and search contracts"
```

### Task 2: Implement safe progressive walking and folder-first publication

**Files:**
- Create: `crates/viewer-application/src/scan.rs`
- Modify: `crates/viewer-application/src/ports.rs`
- Create: `crates/viewer-infrastructure/src/scan/mod.rs`
- Create: `crates/viewer-infrastructure/src/scan/walker.rs`
- Modify: `crates/viewer-infrastructure/src/lib.rs`
- Test: `tests/progressive_scan.rs`

**Interfaces:**
- Produces: `ScanPort::scan`, `ScanBatch`, `ScanEvent::{Folders,Files,Finished,FailedItem}`.

- [ ] **Step 1: Define the scan port and failing order test**

```rust
#[derive(Clone, Debug)]
pub enum ScanEvent {
    Folders { generation: Generation, nodes: Vec<FileNode> },
    Files { generation: Generation, nodes: Vec<FileNode> },
    FailedItem { generation: Generation, relative_display: String, code: String },
    Finished { generation: Generation, totals: ScanTotals },
}

#[async_trait]
pub trait ScanPort: Send + Sync {
    async fn scan(&self, request: ScanRequest, sink: ScanSink) -> Result<(), ScanError>;
}

#[derive(Clone, Debug)]
pub struct ScanRequest {
    pub session_id: SessionId,
    pub generation: Generation,
    pub root: PathBuf,
}

pub type ScanSink = tokio::sync::mpsc::Sender<ScanEvent>;

#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error("project root cannot be read: {0}")]
    RootUnreadable(String),
    #[error("scan request was cancelled")]
    Cancelled,
}
```

Create a fixture with three directory levels, supported files, `.viewer`, `.hidden`, a symlink and unsupported files. Assert the first non-error event is `Folders`, and excluded nodes never appear.

- [ ] **Step 2: Implement bounded walker**

Use `walkdir` without following links. For every entry:

1. Reject `.viewer` and names beginning with `.` before descending.
2. Reject symlinks using `symlink_metadata`.
3. Canonicalize the containing directory and verify it starts with canonical project root.
4. Classify only directory, jpg/jpeg, png, md/markdown and txt.
5. Buffer folders separately from files and flush at 128 nodes or 20 ms.
6. Emit folders before the first file batch.

Run blocking filesystem work in a bounded Tokio blocking task, and send through a bounded channel of eight batches to enforce backpressure.

- [ ] **Step 3: Verify and commit**

Run `cargo test --test progressive_scan -- --nocapture`. Expected: folder-first, exclusion and error-isolation cases PASS.

```bash
git add crates/viewer-application crates/viewer-infrastructure tests/progressive_scan.rs Cargo.lock
git commit -m "feat: add progressive project scanner"
```

### Task 3: Create the disposable session index and batch writer

**Files:**
- Create: `crates/viewer-infrastructure/migrations/session/0001_initial.sql`
- Create: `crates/viewer-infrastructure/src/search/index.rs`
- Create: `crates/viewer-infrastructure/src/search/mod.rs`
- Test: `tests/progressive_scan.rs`

**Interfaces:**
- Produces: `SessionIndex::{open,upsert_batch,remove_subtree,directory_children,close}`.

- [ ] **Step 1: Add the session schema**

```sql
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA foreign_keys = ON;

CREATE TABLE nodes (
  entity_id TEXT PRIMARY KEY,
  parent_entity_id TEXT,
  relative_path TEXT NOT NULL UNIQUE,
  name TEXT NOT NULL,
  kind INTEGER NOT NULL,
  size INTEGER NOT NULL,
  modified_ns TEXT NOT NULL,
  review_state INTEGER,
  favorite INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX nodes_parent ON nodes(parent_entity_id, name);
CREATE INDEX nodes_kind ON nodes(kind);

CREATE VIRTUAL TABLE text_fts USING fts5(
  entity_id UNINDEXED,
  relative_path,
  body,
  tokenize = 'trigram'
);
```

- [ ] **Step 2: Write failing atomic-batch tests**

Insert a batch containing two folders and two files, reopen the database and assert all four appear. Inject an error on item three and assert the transaction exposes none of the four.

- [ ] **Step 3: Implement a single-writer batch repository**

Prepare statements once per connection. Each `upsert_batch` uses one SQLite transaction and one `INSERT ... ON CONFLICT(entity_id) DO UPDATE`. Store nanoseconds as decimal text to avoid SQLite signed-64 overflow assumptions across platforms.

- [ ] **Step 4: Verify and commit**

Run `cargo test --test progressive_scan session_index`. Expected: atomic batch and reopen tests PASS.

```bash
git add crates/viewer-infrastructure tests/progressive_scan.rs Cargo.lock
git commit -m "feat: add disposable session index"
```

### Task 4: Add text extraction and FTS5 trigram indexing

**Files:**
- Create: `crates/viewer-infrastructure/src/search/text.rs`
- Modify: `crates/viewer-infrastructure/src/search/index.rs`
- Test: `tests/search.rs`

**Interfaces:**
- Produces: `TextExtractor::extract`, `SessionIndex::replace_text`.

- [ ] **Step 1: Write failing extraction tests**

Fixtures cover UTF-8, UTF-8 BOM, empty text, invalid UTF-8 and an 11 MB file. Assert:

- UTF-8 and BOM text are normalized without the BOM;
- empty file indexes an empty body;
- invalid input returns `TextStatus::UnsupportedEncoding` without stopping other files;
- 11 MB returns `TextStatus::TooLarge` and no FTS row.

- [ ] **Step 2: Implement bounded extraction**

Open the file, read at most `10 * 1024 * 1024 + 1` bytes, reject indexing when the extra byte exists, strip UTF-8 BOM, validate UTF-8, normalize CRLF to LF and return owned text. Do not attempt lossy replacement for the index.

- [ ] **Step 3: Implement transactional FTS replacement**

Delete the entity's previous FTS row and insert the new row in one session-index transaction. Store only entity ID, relative path and normalized body.

- [ ] **Step 4: Verify and commit**

Run `cargo test --test search text_index`. Expected: all extraction/index tests PASS.

```bash
git add crates/viewer-infrastructure tests/search.rs
git commit -m "feat: index bounded Markdown and text content"
```

### Task 5: Combine fuzzy filename/path search, FTS and filters

**Files:**
- Create: `crates/viewer-application/src/search.rs`
- Create: `crates/viewer-infrastructure/src/search/query.rs`
- Modify: `crates/viewer-infrastructure/Cargo.toml`
- Test: `tests/search.rs`

**Interfaces:**
- Produces: `SearchPort::search(SearchQuery, Generation) -> SearchPage`.

The port signature is:

```rust
#[async_trait]
pub trait SearchPort: Send + Sync {
    async fn search(
        &self,
        session_id: SessionId,
        generation: Generation,
        query: SearchQuery,
    ) -> Result<SearchPage, SearchError>;
}
```

- [ ] **Step 1: Write failing ranking and CJK tests**

Create nodes/text containing:

```text
产品-A/front.png
产品-A/side.png
产品说明.md: “白色陶瓷杯，正面产品图”
notes.txt: “普通备注”
```

Assert `front` ranks the filename first, `产品图` finds the Markdown body, `陶瓷` works through trigram, `白色` works through the two-character fallback, and a subtree scope excludes sibling folders.

- [ ] **Step 2: Implement candidate generation and scoring**

1. SQL first applies project/subtree scope, kind, review and favorite filters.
2. `nucleo-matcher` scores candidate names and relative paths.
3. FTS5 `MATCH` returns body/path hits for queries of at least three Unicode scalar values.
4. One/two-character body queries scan normalized text only within the already filtered candidate entity IDs and enforce a maximum of 2,000 text files per query.
5. Merge by entity ID using rank bands: exact filename, filename fuzzy, path fuzzy, body.
6. Stable tie break uses normalized relative path then entity ID.

Return `total`, requested page and a `matched_field` enum; never return full indexed body over IPC.

- [ ] **Step 3: Verify and commit**

Run `cargo test --test search ranking cjk scope -- --nocapture`. Expected: all cases PASS.

```bash
git add crates/viewer-application crates/viewer-infrastructure tests/search.rs Cargo.lock
git commit -m "feat: add scoped Unicode search"
```

### Task 6: Enforce cancellation and stale-result rejection

**Files:**
- Modify: `crates/viewer-application/src/scheduler.rs`
- Modify: `crates/viewer-application/src/scan.rs`
- Modify: `crates/viewer-application/src/search.rs`
- Test: `tests/progressive_scan.rs`
- Test: `tests/search.rs`

**Interfaces:**
- Produces: `TaskCoordinator::{begin_session,bump_generation,is_publishable,cancel_session}`.

- [ ] **Step 1: Write racing-request tests**

Use a barrier-controlled fake scanner/search port. Start generation 1, block it, bump to generation 2, complete generation 2, then release generation 1. Assert only generation 2 reaches the publication sink.

- [ ] **Step 2: Implement coordinator checks at both boundaries**

Check `(SessionId, Generation)` before starting work and again immediately before every publication/index commit. Cancellation closes the session token, drains queued work and allows already-running workers only to release resources.

Use bounded queues per architecture priority: current query P1, folder publication P1, visible derived tasks P2, text index P3, fingerprints P4.

- [ ] **Step 3: Verify and commit**

Run `cargo test --test progressive_scan stale --test search stale -- --nocapture`. Expected: delayed generation 1 never publishes.

```bash
git add crates/viewer-application tests/progressive_scan.rs tests/search.rs
git commit -m "feat: reject stale scan and search results"
```

### Task 7: Add Watcher event coalescing and scoped reconciliation

**Files:**
- Create: `crates/viewer-platform-macos/src/watcher.rs`
- Create: `crates/viewer-infrastructure/src/scan/reconcile.rs`
- Modify: `crates/viewer-platform-macos/src/lib.rs`
- Test: `tests/watcher_reconcile.rs`

**Interfaces:**
- Produces: `WatcherPort`, `ExpectedChangeLedger`, `ReconcileRequest`.

- [ ] **Step 1: Write failing event-burst tests**

Feed create/write/rename events for the same path within 250 ms and assert one parent-directory reconciliation. Register an expected Viewer rename and assert its OS events do not produce duplicate entity mutations. Feed an overflow event and assert the smallest known common ancestor is rescanned.

- [ ] **Step 2: Implement Watcher adapter**

Use `notify` with `notify-debouncer-full`. Normalize each event into added/removed/modified/renamed/overflow categories, but never update indexes directly. Emit only `ReconcileRequest { session_id, generation, roots, reason }`.

`ExpectedChangeLedger` stores operation ID, old/new canonical paths, expected file identity and expiry. Matching suppresses duplicate interpretation, not the final reconciliation scan.

- [ ] **Step 3: Verify and commit**

Run `cargo test --test watcher_reconcile -- --nocapture`. Expected: burst, expected-change and overflow cases PASS.

```bash
git add crates/viewer-platform-macos crates/viewer-infrastructure tests/watcher_reconcile.rs Cargo.lock
git commit -m "feat: reconcile external filesystem changes"
```

### Task 8: Produce G3 performance evidence

**Files:**
- Create: `scripts/generate-g3-project.sh`
- Create: `scripts/run-g3-scan-search-gate.sh`
- Create: `docs/adr/0003-scan-search-and-generation.md`
- Modify: `docs/TECHNICAL_FOUNDATIONS.md`

**Interfaces:**
- Produces: reproducible M4 scan/search benchmark and selected queue/batch sizes.

- [ ] **Step 1: Generate a deterministic local corpus**

The generator creates 1,000 image placeholder entries distributed across three directory levels plus 100 Markdown/TXT files with Chinese and English content. It also creates excluded hidden/link cases. It records a manifest and deletes only the output directory supplied as its first argument.

- [ ] **Step 2: Run cold scan and indexed search measurements**

`run-g3-scan-search-gate.sh` creates a fresh session directory for every cold run and measures 20 runs. Record p50/p95 for first folder-tree event, base scan completion and indexed queries; verify stale-publication tests and peak memory.

Expected on the standard M4 device:

- folder tree first usable event ≤ 1.5 seconds p95;
- base metadata scan ≤ 3 seconds p95;
- indexed search page ≤ 100 ms p95;
- no old generation publication;
- session index can be deleted and rebuilt without portable metadata loss.

- [ ] **Step 3: Record decisions and commit**

ADR 0003 records batch size, queue capacities, debounce window, short-query cap and measured results. If a target fails, tune only these documented parameters or revise the architecture; do not hide failed runs.

```bash
git add scripts/generate-g3-project.sh scripts/run-g3-scan-search-gate.sh docs/adr/0003-scan-search-and-generation.md docs/TECHNICAL_FOUNDATIONS.md
git commit -m "docs: record G3 scan and search decisions"
```

## G3 Completion Check

Run:

```bash
./scripts/run-g3-scan-search-gate.sh
git status --short
```

Expected: gate exits 0, performance evidence and queue parameters are recorded, and the working tree is clean.
