# Viewer Tauri Runtime Decomposition Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Decompose the Tauri DTO, DesktopRuntime, and OperationRuntime modules into focused files without changing any IPC, lifecycle, safety, or persistence contract.

**Architecture:** Convert each oversized Rust file into a module directory while retaining the same public module path and facade type. Move existing implementations by responsibility, keep helper visibility at `pub(super)` or narrower, and use existing desktop/security tests as characterization gates.

**Tech Stack:** Rust 1.97.0 edition 2024, Tauri 2, Tokio, serde, viewer-domain/application/infrastructure/platform-macos.

## Global Constraints

- Plan 2 must be complete and `pnpm verify:clean` must pass before this plan starts.
- Preserve every `#[tauri::command]` name, event string, serde `rename_all`, DTO field, error code, and user-safe message.
- Preserve `viewer_desktop::state::DesktopRuntime`, `viewer_desktop::operation_runtime::OperationRuntime`, and `viewer_desktop::dto::*` paths.
- Do not move Tauri, SQLite, filesystem, or macOS types into Domain or Application.
- Do not change session/generation/revision validation order.
- Do not change operation terminal publication, close blocking, watcher drain, cache cleanup, or recovery ordering.
- Module extraction commits contain no behavioral edits.

---

### Task 1: Freeze Desktop Public and IPC Contracts

**Files:**
- Modify: `tests/m1_desktop_runtime.rs`
- Modify: `tests/m3_desktop_runtime.rs`
- Modify: `scripts/repository-policy.test.mjs`

**Interfaces:**
- Produces: compile-time proof for public facade paths.
- Produces: source policy for the target module layout.

- [ ] **Step 1: Add a public facade compile test**

Add to `tests/m1_desktop_runtime.rs`:

```rust
#[test]
fn desktop_facades_remain_available_at_the_frozen_paths() {
    fn assert_runtime(_: Option<&viewer_desktop::state::DesktopRuntime>) {}
    fn assert_operation(_: Option<&viewer_desktop::operation_runtime::OperationRuntime>) {}
    fn assert_snapshot(_: Option<viewer_desktop::dto::ProjectSnapshot>) {}

    assert_runtime(None);
    assert_operation(None);
    assert_snapshot(None);
}
```

- [ ] **Step 2: Add a source-layout policy that initially fails**

Append to `scripts/repository-policy.test.mjs`:

```js
test('desktop runtime responsibilities live in focused modules', async () => {
  const required = [
    'src-tauri/src/dto/mod.rs',
    'src-tauri/src/state/mod.rs',
    'src-tauri/src/state/session.rs',
    'src-tauri/src/state/scan_index.rs',
    'src-tauri/src/state/preview.rs',
    'src-tauri/src/state/markers.rs',
    'src-tauri/src/state/organization.rs',
    'src-tauri/src/operation_runtime/mod.rs',
    'src-tauri/src/operation_runtime/runtime.rs',
    'src-tauri/src/operation_runtime/commit.rs',
  ]
  await Promise.all(required.map((path) => stat(new URL(`../${path}`, import.meta.url))))
})
```

- [ ] **Step 3: Run the tests**

```bash
cargo test --locked -p viewer-desktop --test m1_desktop_runtime desktop_facades_remain_available_at_the_frozen_paths
node --test scripts/repository-policy.test.mjs
```

Expected: Rust compile test passes; repository policy fails on missing module files.

- [ ] **Step 4: Commit the contract and failing architecture policy**

```bash
git add tests/m1_desktop_runtime.rs scripts/repository-policy.test.mjs
git commit -m "test: freeze desktop runtime facade paths"
```

### Task 2: Split DTOs by IPC Domain

**Files:**
- Create: `src-tauri/src/dto/mod.rs`
- Create: `src-tauri/src/dto/project.rs`
- Create: `src-tauri/src/dto/browse.rs`
- Create: `src-tauri/src/dto/search.rs`
- Create: `src-tauri/src/dto/preview.rs`
- Create: `src-tauri/src/dto/markers.rs`
- Create: `src-tauri/src/dto/operations.rs`
- Delete: `src-tauri/src/dto.rs`

**Interfaces:**
- `crate::dto::*` remains available through public re-exports.
- JSON and serde shapes remain byte-for-byte compatible.

- [ ] **Step 1: Run DTO and IPC characterization**

```bash
cargo test --locked -p viewer-desktop dto::
cargo test --locked -p viewer-desktop --test m3_desktop_runtime m3_requests_are_exact_camel_case_and_never_accept_raw_paths
cargo test --locked -p viewer-desktop --test security_boundaries
```

Expected: PASS before extraction.

- [ ] **Step 2: Create the DTO module facade**

Create `dto/mod.rs`:

```rust
mod browse;
mod markers;
mod operations;
mod preview;
mod project;
mod search;

pub use browse::*;
pub use markers::*;
pub use operations::*;
pub use preview::*;
pub use project::*;
pub use search::*;
```

- [ ] **Step 3: Move DTOs without changing definitions**

Move exact types and `From` implementations:

- `project.rs`: close choice/outcome/target, project access/snapshot, recovery, project change, close blocked.
- `browse.rs`: scan DTOs, folder tree, browser file, content folder card, folder workspace, selection agreement/counts/info.
- `search.rs`: match range, hit/page/progress, filter/sort/request/snippet DTOs.
- `preview.rs`: image metadata/representation/backend and text preview DTOs.
- `markers.rs`: marker, review/favorite/selection request and marker batch/change DTOs.
- `operations.rs`: rename rules/preview, file command request/preflight/progress/results, undo and Finder drag DTOs.

Move shared sanitization helpers to the domain file that owns their only callers. If a helper has callers in two DTO modules, keep it in `dto/mod.rs` as:

```rust
use viewer_domain::file::RelativePath;

pub(super) fn safe_relative_display(value: &str) -> String {
    RelativePath::parse(value)
        .map(|path| path.as_str().to_owned())
        .unwrap_or_else(|_| "unavailable".to_owned())
}
```

Move DTO unit tests into `dto/mod.rs` under `#[cfg(test)] mod tests` and update imports to `use super::*;`.

- [ ] **Step 4: Run DTO and desktop tests**

```bash
cargo fmt --check
cargo test --locked -p viewer-desktop dto::
cargo test --locked -p viewer-desktop --test m1_desktop_runtime
cargo test --locked -p viewer-desktop --test m3_desktop_runtime
```

Expected: PASS with unchanged serialized shapes.

- [ ] **Step 5: Commit DTO split**

```bash
git add src-tauri/src/dto src-tauri/src/dto.rs src-tauri/src/lib.rs
git commit -m "refactor(desktop): split IPC DTO domains"
```

### Task 3: Convert DesktopRuntime to a Module Facade

**Files:**
- Create: `src-tauri/src/state/mod.rs`
- Create: `src-tauri/src/state/common.rs`
- Create: `src-tauri/src/state/session.rs`
- Create: `src-tauri/src/state/scan_index.rs`
- Create: `src-tauri/src/state/preview.rs`
- Create: `src-tauri/src/state/markers.rs`
- Create: `src-tauri/src/state/organization.rs`
- Delete: `src-tauri/src/state.rs`

**Interfaces:**
- `DesktopRuntime` remains defined in `state/mod.rs`.
- Child modules add `impl DesktopRuntime` blocks.
- `DesktopEventSink`, close enums, and factory traits remain at `crate::state::*`.

- [ ] **Step 1: Run lifecycle characterization**

```bash
cargo test --locked -p viewer-desktop --test m1_desktop_runtime
cargo test --locked -p viewer-desktop --test m3_readonly_lifecycle
```

Expected: PASS before extraction.

- [ ] **Step 2: Create `state/mod.rs`**

Move these definitions unchanged into `state/mod.rs`:

- close enums and `CloseRequestOutcome::completion_action`;
- `DesktopEventSink`, `DesktopImageFactory`, `DesktopMarkerProjectionFactory`;
- `DesktopSession`, `ScanServices`, `OrganizationServices`;
- `DesktopRuntime` fields;
- constructors from `new` through `new_with_runtime_services`.

Add:

```rust
mod common;
mod markers;
mod organization;
mod preview;
mod scan_index;
mod session;

pub(crate) use common::{
    internal_command_error, is_stale_derived_write_error, project_not_open,
    stale_project_session,
};
```

Keep items private unless a sibling module requires `pub(super)`.

- [ ] **Step 3: Move session lifecycle methods**

Move to `state/session.rs`:

- `prepare_organization_services`;
- `open_project`;
- `close_project`;
- `finalize_process_exit`;
- `cleanup_session_caches_for_process_exit`;
- `request_close`;
- `request_close_for`;
- `wait_for_scan`;
- `cancel_task`;
- `snapshot`;
- `ensure_project_current`;
- `ensure_session_active`;
- `register_if_session_active`;
- `resources_ready`.

Start the file with:

```rust
use super::*;
```

- [ ] **Step 4: Move common guards and errors**

Move validation and safe-error helpers into `state/common.rs`, keeping platform-specific `#[cfg]` pairs together:

```rust
pub(super) fn stale_project_session() -> CommandError {
    CommandError::new(
        "stale_project_session",
        ErrorCategory::Conflict,
        "该请求不属于当前项目会话。",
        false,
    )
}
```

Move every other helper with its current error code, `ErrorCategory`, message, and retryability flag unchanged.

- [ ] **Step 5: Compile and run lifecycle tests**

```bash
cargo fmt
cargo check --locked -p viewer-desktop --all-targets
cargo test --locked -p viewer-desktop --test m1_desktop_runtime
cargo test --locked -p viewer-desktop --test m3_readonly_lifecycle
```

Expected: PASS.

- [ ] **Step 6: Commit the runtime facade and lifecycle**

```bash
git add src-tauri/src/state src-tauri/src/state.rs
git commit -m "refactor(desktop): extract session runtime"
```

### Task 4: Extract Scan, Search, Marker, Preview and Organization Runtime Methods

**Files:**
- Modify: `src-tauri/src/state/scan_index.rs`
- Modify: `src-tauri/src/state/markers.rs`
- Modify: `src-tauri/src/state/preview.rs`
- Modify: `src-tauri/src/state/organization.rs`
- Modify: `src-tauri/src/state/mod.rs`
- Test: existing desktop integration tests.

**Interfaces:**
- Public methods stay inherent methods on `DesktopRuntime`.
- Cross-module helpers use `pub(super)` and typed arguments, not shared mutable globals.

- [ ] **Step 1: Move scan and search methods**

Move to `scan_index.rs`:

- `index_progress`;
- `search_project`;
- `search_text_snippet`;
- `ensure_search_current`;
- `active_index`;
- `run_scan`;
- `hydrate_portable_markers`;
- `run_derived_indexing`;
- `rebuild_derived_nodes`;
- derived-progress helpers.

Keep `(SessionId, Generation)` guards adjacent to publication. Do not move validation after a database read or event emission.

- [ ] **Step 2: Run scan/search tests**

```bash
cargo test --locked -p viewer-desktop --test m2_desktop_runtime
cargo test --locked -p viewer-desktop state::derived_error_tests
```

Expected: PASS.

- [ ] **Step 3: Move marker methods**

Move to `markers.rs`:

- `set_review_state`;
- `toggle_favorite`;
- `selection_info`;
- `apply_marker_patch`;
- marker-target validation and marker error helpers.

Run:

```bash
cargo test --locked -p viewer-desktop --test m2_desktop_runtime marker
```

Expected: PASS.

- [ ] **Step 4: Move browse and preview methods**

Move to `preview.rs`:

- `folder_tree`;
- `query_folder`;
- `query_folder_projection`;
- `request_image`;
- `request_image_for_session`;
- `preview_text`;
- `open_external_link`;
- `validated_indexed_source`;
- `resolve_markdown_image_path`.

Run:

```bash
cargo test --locked -p viewer-desktop --test m1_desktop_runtime
cargo test --locked -p viewer-desktop --test security_boundaries
```

Expected: PASS.

- [ ] **Step 5: Move organization methods**

Move to `organization.rs`:

- `preview_rename`;
- `prepare_finder_drag`;
- `run_if_project_current`;
- `execute_file_command`;
- `preflight_file_command`;
- `operation_status`;
- `operation_results`;
- `cancel_operation`;
- `wait_for_operation`;
- `undo_last_operation`;
- `active_operations`;
- `undo_last`.

Run:

```bash
cargo test --locked -p viewer-desktop --test m3_desktop_runtime
cargo test --locked -p viewer-desktop --test m3_drag_export
```

Expected: PASS.

- [ ] **Step 6: Commit service method extraction**

```bash
git add src-tauri/src/state
git commit -m "refactor(desktop): split runtime service domains"
```

### Task 5: Split OperationRuntime State and Publication

**Files:**
- Create: `src-tauri/src/operation_runtime/mod.rs`
- Create: `src-tauri/src/operation_runtime/runtime.rs`
- Create: `src-tauri/src/operation_runtime/publication.rs`
- Create: `src-tauri/src/operation_runtime/commit.rs`
- Create: `src-tauri/src/operation_runtime/undo.rs`
- Delete: `src-tauri/src/operation_runtime.rs`

**Interfaces:**
- `OperationRuntime`, `OperationStarted`, `OperationRuntimeError`, `DesktopOperationCommitPort`, and `adapter_as_undo_port` keep current paths through re-exports.

- [ ] **Step 1: Run operation ordering characterization**

```bash
cargo test --locked -p viewer-desktop operation_runtime::
cargo test --locked -p viewer-desktop --test m3_desktop_runtime runtime_rename_exposes_progress_results_and_a_safe_session_undo
```

Expected: PASS.

- [ ] **Step 2: Create the facade**

Create `operation_runtime/mod.rs`:

```rust
mod commit;
mod publication;
mod runtime;
mod undo;

pub use commit::DesktopOperationCommitPort;
pub use runtime::{OperationRuntime, OperationRuntimeError, OperationStarted};
pub use undo::adapter_as_undo_port;
```

- [ ] **Step 3: Move runtime state**

Move to `runtime.rs`:

- `OperationRuntimeState`;
- `OperationRuntime`;
- preflight/start/status/results/cancel/active/wait/cancel-and-wait methods;
- conflict-resolution validation.

Move completion latch and record publication mechanics to `publication.rs`. Expose only:

```rust
pub(super) struct OperationRecord {
    pub(super) progress: Mutex<BatchProgress>,
    pub(super) summary: Mutex<Option<BatchSummary>>,
    pub(super) failure: Mutex<Option<FileCommandServiceError>>,
    pub(super) complete: watch::Sender<bool>,
}

impl OperationRecord {
    pub(super) fn new(progress: BatchProgress) -> Self {
        let (complete, _) = watch::channel(false);
        Self {
            progress: Mutex::new(progress),
            summary: Mutex::new(None),
            failure: Mutex::new(None),
            complete,
        }
    }

    pub(super) fn progress(&self) -> BatchProgress {
        self.progress
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub(super) fn publish(&self, progress: BatchProgress) {
        *self
            .progress
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = progress;
    }

    pub(super) fn store_result(
        &self,
        result: Result<BatchSummary, FileCommandServiceError>,
    ) {
        match result {
            Ok(summary) => {
                *self
                    .summary
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(summary);
            }
            Err(error) => {
                *self
                    .failure
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(error);
            }
        }
    }

    pub(super) fn mark_complete(&self) {
        self.complete.send_replace(true);
    }

    pub(super) fn is_complete(&self) -> bool {
        *self.complete.borrow()
    }

    pub(super) fn subscribe_completion(&self) -> watch::Receiver<bool> {
        self.complete.subscribe()
    }
}
```

Do not change the order of `publish`, result storage, active-batch removal, and completion notification.

- [ ] **Step 4: Move commit and undo adapters**

Move `DesktopOperationCommitPort`, its constructors/helpers and `OperationCommitPort` implementation to `commit.rs`.

Move `adapter_as_undo_port` and undo-only adapter glue to `undo.rs`.

- [ ] **Step 5: Move unit tests with their owner**

- runtime cancellation/wait tests → `runtime.rs`;
- completion latch tests → `publication.rs`;
- commit mapping tests → `commit.rs`.

Keep test names unchanged.

- [ ] **Step 6: Run operation tests**

```bash
cargo fmt
cargo test --locked -p viewer-desktop operation_runtime::
cargo test --locked -p viewer-desktop --test m3_desktop_runtime
cargo test --locked -p viewer-desktop --test m3_readonly_lifecycle
```

Expected: PASS.

- [ ] **Step 7: Commit operation runtime split**

```bash
git add src-tauri/src/operation_runtime src-tauri/src/operation_runtime.rs
git commit -m "refactor(desktop): split operation runtime responsibilities"
```

### Task 6: Run Desktop Architecture and Security Exit Gate

**Files:**
- Verify only.

- [ ] **Step 1: Run architecture policy**

```bash
node --test scripts/repository-policy.test.mjs
```

Expected: PASS, including focused desktop module files.

- [ ] **Step 2: Run desktop and security suites**

```bash
cargo test --locked -p viewer-desktop
./scripts/check-tauri-security.sh
```

Expected: all desktop tests and 8 security-boundary tests pass.

- [ ] **Step 3: Run full clean verification**

```bash
pnpm verify:clean
```

Expected: PASS and no new worktree entries.

- [ ] **Step 4: Inspect boundaries**

```bash
git diff --check
rg -n 'pub struct DesktopRuntime|pub struct OperationRuntime|pub use .*ProjectSnapshot' src-tauri/src/state src-tauri/src/operation_runtime src-tauri/src/dto
```

Expected: one definition for each facade and public re-exports for DTO compatibility.
