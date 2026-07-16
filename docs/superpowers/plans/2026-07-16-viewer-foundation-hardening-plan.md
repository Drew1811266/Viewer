# Viewer Foundation Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Correct the six Important whole-branch review findings and the IPC regression-test gap without expanding Viewer 0.1 product scope.

**Architecture:** Keep the existing modular-monolith boundaries. Tighten the Tauri shell around an explicit local-origin policy, extend the application state machine with failure cleanup, strengthen the domain path invariant, make the filesystem probe a synchronous typed port, and turn exact toolchain inputs into an executable repository policy.

**Tech Stack:** Rust 1.97.0, Tauri 2.11.5, React, TypeScript, Node.js 24.18.0, pnpm 10.0.0, Vitest, Node test runner, cargo-deny 0.20.2, GitHub Actions.

## Global Constraints

- Build target remains Apple Silicon `aarch64-apple-darwin` with minimum macOS 13.0.
- Runtime has no login, telemetry, updater, HTTP client, or application-originated remote request.
- React receives no broad filesystem, shell, SQL, HTTP, updater, or unused Tauri core permission.
- The only permitted top-level origins are production `tauri://localhost` and, in debug builds, `http://localhost:5173`.
- Dependency direction remains `src-tauri -> adapters -> application -> domain`.
- Viewer project license remains unselected; do not add a `LICENSE` file or package license metadata.
- Every behavioral fix uses a witnessed red-green cycle. Configuration-only corrections use a failing repository-policy test before edits.
- This plan implements the approved design at `docs/superpowers/specs/2026-07-16-viewer-foundation-hardening-design.md` and supersedes conflicting snippets in the original Foundation plan.

---

## Target File Map

```text
Viewer/
├── .github/workflows/ci.yml                       # pinned arm64 CI
├── ACKNOWLEDGEMENTS.md                            # actual direct dependencies
├── Cargo.toml                                     # final workspace dependencies
├── package.json                                   # locked verification command
├── rust-toolchain.toml                            # Rust 1.97.0
├── scripts/
│   ├── check-dev-env.sh                           # exact local toolchain check
│   └── repository-policy.test.mjs                 # configuration fitness test
├── src-tauri/
│   ├── Cargo.toml                                 # serde_json test dependency
│   ├── capabilities/main.json                     # empty permission set
│   ├── src/lib.rs                                 # navigation guard and IPC tests
│   └── tauri.conf.json                            # hardened CSP
└── crates/
    ├── viewer-domain/src/lib.rs                   # RelativePath invariant
    ├── viewer-application/
    │   ├── Cargo.toml                             # remove async-trait
    │   └── src/{lib.rs,ports.rs,session.rs}       # typed sync probe and fail_open
    └── viewer-platform-macos/
        ├── Cargo.toml                             # remove async/futures
        └── src/lib.rs                             # typed synchronous probe
```

### Task 1: Enforce the Tauri capability and navigation boundary

**Files:**
- Modify: `ACKNOWLEDGEMENTS.md`
- Modify: `Cargo.toml`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/capabilities/main.json`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/tauri.conf.json`

**Interfaces:**
- Produces: `is_allowed_navigation(url: &tauri::Url) -> bool` and a main-window capability with zero core/plugin permissions.
- Preserves: public `viewer_desktop::health()` and `viewer_desktop::run()`.

- [ ] **Step 1: Add the test-only JSON dependency**

Add to `[workspace.dependencies]` in root `Cargo.toml`:

```toml
serde_json = "1"
```

Add to `src-tauri/Cargo.toml`:

```toml
[dev-dependencies]
serde_json.workspace = true
```

Replace the Rust foundation bullet in `ACKNOWLEDGEMENTS.md` with:

```markdown
- [Rust](https://github.com/rust-lang/rust) and Cargo — native application language and build toolchain. The direct Rust crates in the foundation are `async-trait`, `futures`, `serde`, `serde_json` (tests), `tempfile`, `thiserror`, and `uuid`.
```

Retain `async-trait` and `futures` until Task 4 removes their manifests.

Run `cargo check --workspace` once so `Cargo.lock` records the workspace package dependency. This is test setup, not a production behavior change.

- [ ] **Step 2: Write the failing capability regression test**

Append inside `src-tauri/src/lib.rs`'s existing `tests` module:

```rust
#[test]
fn main_capability_grants_no_core_or_plugin_permissions() {
    let capability: serde_json::Value = serde_json::from_str(include_str!(
        "../capabilities/main.json"
    ))
    .expect("main capability must be valid JSON");

    assert_eq!(capability["windows"], serde_json::json!(["main"]));
    assert_eq!(capability["permissions"], serde_json::json!([]));
    assert!(capability.get("remote").is_none());
}
```

- [ ] **Step 3: Run the capability test and witness RED**

Run:

```bash
cargo test -p viewer-desktop --lib main_capability_grants_no_core_or_plugin_permissions
```

Expected: FAIL because the committed capability contains `"core:default"`.

- [ ] **Step 4: Remove the unused Tauri permissions**

Replace `src-tauri/capabilities/main.json` with:

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "main",
  "description": "Viewer main window with no Tauri core or plugin permissions",
  "windows": ["main"],
  "permissions": []
}
```

Run the focused test again. Expected: PASS.

- [ ] **Step 5: Write failing navigation-policy tests**

Add these tests to `src-tauri/src/lib.rs`:

```rust
#[test]
fn navigation_policy_allows_only_viewer_origins() {
    for allowed in [
        "tauri://localhost/",
        "tauri://localhost/folder?id=1#preview",
    ] {
        let url = tauri::Url::parse(allowed).expect("valid test URL");
        assert!(super::is_allowed_navigation(&url), "rejected {allowed}");
    }

    let development = tauri::Url::parse("http://localhost:5173/")
        .expect("valid development URL");
    assert_eq!(
        super::is_allowed_navigation(&development),
        cfg!(debug_assertions)
    );
}

#[test]
fn navigation_policy_rejects_remote_and_origin_bypasses() {
    for rejected in [
        "https://example.com/",
        "http://localhost:5174/",
        "http://127.0.0.1:5173/",
        "http://user@localhost:5173/",
        "file:///tmp/project.html",
        "data:text/html,viewer",
    ] {
        let url = tauri::Url::parse(rejected).expect("valid test URL");
        assert!(!super::is_allowed_navigation(&url), "accepted {rejected}");
    }
}
```

- [ ] **Step 6: Run the navigation tests and witness RED**

Run:

```bash
cargo test -p viewer-desktop --lib navigation_policy
```

Expected: compilation FAIL because `is_allowed_navigation` does not exist.

- [ ] **Step 7: Implement the pure navigation predicate and hook**

Add above `run()` in `src-tauri/src/lib.rs`:

```rust
fn is_allowed_navigation(url: &tauri::Url) -> bool {
    if !url.username().is_empty() || url.password().is_some() {
        return false;
    }

    match (url.scheme(), url.host_str(), url.port()) {
        ("tauri", Some("localhost"), None) => true,
        ("http", Some("localhost"), Some(5173)) if cfg!(debug_assertions) => true,
        _ => false,
    }
}

fn navigation_guard<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("viewer-navigation-guard")
        .on_navigation(|_, url| is_allowed_navigation(url))
        .build()
}
```

Register it before the invoke handler:

```rust
tauri::Builder::default()
    .plugin(navigation_guard())
    .invoke_handler(tauri::generate_handler![health])
```

- [ ] **Step 8: Harden the CSP**

Set `app.security.csp` in `src-tauri/tauri.conf.json` to this one-line policy:

```json
"default-src 'self'; script-src 'self'; connect-src 'self' ipc: http://ipc.localhost; img-src 'self' asset: data:; style-src 'self' 'unsafe-inline'; base-uri 'none'; form-action 'none'; object-src 'none'; frame-src 'none'"
```

- [ ] **Step 9: Verify Task 1 and commit**

Run:

```bash
cargo test -p viewer-desktop --lib
cargo clippy --locked -p viewer-desktop --all-targets -- -D warnings
git diff --check
```

Expected: all Viewer desktop tests PASS, Clippy PASS, diff check PASS.

Commit:

```bash
git add ACKNOWLEDGEMENTS.md Cargo.toml Cargo.lock src-tauri/Cargo.toml src-tauri/capabilities/main.json src-tauri/src/lib.rs src-tauri/tauri.conf.json
git commit -m "fix: harden Viewer webview boundary"
```

### Task 2: Add session opening-failure recovery

**Files:**
- Modify: `crates/viewer-application/src/session.rs`

**Interfaces:**
- Produces: `ProjectSession::fail_open(&mut self) -> Result<(), SessionTransitionError>`.
- Preserves: all existing valid session paths and state-preserving invalid transitions.

- [ ] **Step 1: Write the failing legal failure-path test**

Add to `session.rs` tests:

```rust
#[test]
fn failed_open_uses_the_close_cleanup_path() {
    let mut session = ProjectSession::default();
    session.begin_open().unwrap();
    session.fail_open().unwrap();
    assert_eq!(session.state(), SessionState::Closing);
    session.finish_close().unwrap();
    assert_eq!(session.state(), SessionState::Empty);
}
```

- [ ] **Step 2: Extend the exhaustive test before production code**

Add `FailOpen` to the local `Operation` enum and `operations` array. Add this
legal cell:

```rust
(SessionState::Opening, Operation::FailOpen)
```

Dispatch it with:

```rust
Operation::FailOpen => session.fail_open(),
```

Change the final count assertion to:

```rust
assert_eq!(invalid_cells, 23);
```

- [ ] **Step 3: Run the focused tests and witness RED**

Run:

```bash
cargo test -p viewer-application session::tests::failed_open_uses_the_close_cleanup_path
```

Expected: compilation FAIL because `fail_open` does not exist.

- [ ] **Step 4: Implement the minimal transition**

Add to `impl ProjectSession` after `activate`:

```rust
pub fn fail_open(&mut self) -> Result<(), SessionTransitionError> {
    match self.state {
        SessionState::Opening => {
            self.state = SessionState::Closing;
            Ok(())
        }
        _ => Err(SessionTransitionError::Invalid),
    }
}
```

- [ ] **Step 5: Verify Task 2 and commit**

Run:

```bash
cargo test -p viewer-application
cargo clippy --locked -p viewer-application --all-targets -- -D warnings
```

Expected: six tests PASS, including 23 invalid cells.

Commit:

```bash
git add crates/viewer-application/src/session.rs
git commit -m "fix: recover failed project opening"
```

### Task 3: Strengthen the RelativePath metadata boundary

**Files:**
- Modify: `crates/viewer-domain/src/lib.rs`

**Interfaces:**
- Preserves: `RelativePath::parse(&str) -> Result<RelativePath, RelativePathError>` and custom `Deserialize`.
- Strengthens: `.viewer` is reserved in every ASCII case and NUL is always invalid.

- [ ] **Step 1: Add failing parser cases**

Extend `relative_path_rejects_non_canonical_or_reserved_paths` with:

```rust
".VIEWER/metadata.sqlite",
".Viewer/metadata.sqlite",
"products/.vIeWeR/metadata.sqlite",
"products/id-1/front\0.png",
```

- [ ] **Step 2: Add failing serde cases**

Extend `relative_path_deserialization_preserves_validation`:

```rust
for value in [".VIEWER/metadata.sqlite", "products/.Viewer/file", "a\0b"] {
    let invalid = StrDeserializer::<ValueError>::new(value);
    assert!(RelativePath::deserialize(invalid).is_err(), "accepted {value:?}");
}
```

- [ ] **Step 3: Run the focused tests and witness RED**

Run:

```bash
cargo test -p viewer-domain relative_path
```

Expected: FAIL because case variants and NUL are currently accepted.

- [ ] **Step 4: Strengthen the single parser invariant**

Replace `segments_are_canonical` with:

```rust
let segments_are_canonical = !value.contains('\0')
    && value.split('/').all(|segment| {
        !matches!(segment, "" | "." | "..")
            && !segment.eq_ignore_ascii_case(".viewer")
    });
```

Keep absolute-path and `Component::Normal` checks unchanged.

- [ ] **Step 5: Verify Task 3 and commit**

Run:

```bash
cargo test -p viewer-domain
cargo clippy --locked -p viewer-domain --all-targets -- -D warnings
```

Expected: all domain tests PASS.

Commit:

```bash
git add crates/viewer-domain/src/lib.rs
git commit -m "fix: reserve Viewer metadata paths safely"
```

### Task 4: Make the project probe synchronous and typed

**Files:**
- Modify: `ACKNOWLEDGEMENTS.md`
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `crates/viewer-application/Cargo.toml`
- Modify: `crates/viewer-application/src/lib.rs`
- Modify: `crates/viewer-application/src/ports.rs`
- Modify: `crates/viewer-platform-macos/Cargo.toml`
- Modify: `crates/viewer-platform-macos/src/lib.rs`

**Interfaces:**
- Produces: synchronous `ProjectProbePort::probe(&self, root: &Path) -> Result<ProjectAccess, ProjectProbeError>`.
- Produces: `ProjectProbeOperation` and typed `ProjectProbeError` in `viewer-application`.
- Removes: direct workspace use of `async-trait` and `futures`.

- [ ] **Step 1: Write the failing typed-port contract test**

Append to `crates/viewer-application/src/ports.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    struct TypedProbe;

    impl ProjectProbePort for TypedProbe {
        fn probe(&self, root: &Path) -> Result<ProjectAccess, ProjectProbeError> {
            Err(ProjectProbeError::NotDirectory {
                path: root.to_path_buf(),
            })
        }
    }

    #[test]
    fn project_probe_port_exposes_a_synchronous_typed_error() {
        let result: Result<ProjectAccess, ProjectProbeError> =
            TypedProbe.probe(Path::new("not-a-directory"));
        assert_eq!(
            result,
            Err(ProjectProbeError::NotDirectory {
                path: PathBuf::from("not-a-directory"),
            })
        );
    }
}
```

- [ ] **Step 2: Run the typed-port test and witness RED**

Run:

```bash
cargo test -p viewer-application ports::tests::project_probe_port_exposes_a_synchronous_typed_error
```

Expected: compilation FAIL because the trait is async/String-based and `ProjectProbeError` is absent.

- [ ] **Step 3: Implement the typed application contract**

Replace `ports.rs` with:

```rust
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectAccess {
    ReadWrite,
    ReadOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectProbeOperation {
    ReadMetadata,
    ReadDirectory,
    CreateWriteProbe,
    RemoveWriteProbe,
}

impl fmt::Display for ProjectProbeOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::ReadMetadata => "read project root metadata",
            Self::ReadDirectory => "read project root directory",
            Self::CreateWriteProbe => "create project write probe",
            Self::RemoveWriteProbe => "remove project write probe",
        };
        formatter.write_str(label)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ProjectProbeError {
    #[error("project root is not a directory: {path}")]
    NotDirectory { path: PathBuf },
    #[error("failed to {operation} at {path}: {message}")]
    Io {
        operation: ProjectProbeOperation,
        path: PathBuf,
        message: String,
    },
}

impl ProjectProbeError {
    pub fn io(
        operation: ProjectProbeOperation,
        path: impl Into<PathBuf>,
        error: &std::io::Error,
    ) -> Self {
        Self::Io {
            operation,
            path: path.into(),
            message: error.to_string(),
        }
    }
}

pub trait ProjectProbePort: Send + Sync {
    fn probe(&self, root: &Path) -> Result<ProjectAccess, ProjectProbeError>;
}

pub trait ClockPort: Send + Sync {
    fn unix_millis(&self) -> i64;
}
```

Append the exact Step 1 test module below this production code.

Update `crates/viewer-application/src/lib.rs` to re-export the complete public
contract:

```rust
pub mod ports;
pub mod session;

pub use ports::{
    ClockPort, ProjectAccess, ProjectProbeError, ProjectProbeOperation, ProjectProbePort,
};
pub use session::{ProjectSession, SessionState, SessionTransitionError};
```

- [ ] **Step 4: Remove obsolete application dependency**

Remove `async-trait.workspace = true` from
`crates/viewer-application/Cargo.toml`.

Run the focused application test. Expected: PASS.

- [ ] **Step 5: Write failing macOS classification tests**

Inside the existing macOS test module, add:

```rust
#[test]
fn write_probe_classifies_permission_and_read_only_filesystems() {
    for kind in [ErrorKind::PermissionDenied, ErrorKind::ReadOnlyFilesystem] {
        let error = std::io::Error::from(kind);
        assert!(super::is_read_only_write_error(&error));
    }

    let other = std::io::Error::from(ErrorKind::Other);
    assert!(!super::is_read_only_write_error(&other));
}
```

Import `std::io::ErrorKind` in the test module.

- [ ] **Step 6: Run the macOS test and witness RED**

Run:

```bash
cargo test -p viewer-platform-macos write_probe_classifies_permission_and_read_only_filesystems
```

Expected: compilation FAIL because the helper is absent and the adapter still implements the old async contract.

- [ ] **Step 7: Convert the macOS adapter**

Replace the production portion of `crates/viewer-platform-macos/src/lib.rs`
above its test module with:

```rust
use std::fs::{self, OpenOptions};
use std::io::ErrorKind;
use std::path::Path;
use uuid::Uuid;
use viewer_application::{
    ProjectAccess, ProjectProbeError, ProjectProbeOperation, ProjectProbePort,
};

pub struct MacProjectProbe;

fn is_read_only_write_error(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        ErrorKind::PermissionDenied | ErrorKind::ReadOnlyFilesystem
    )
}

impl ProjectProbePort for MacProjectProbe {
    fn probe(&self, root: &Path) -> Result<ProjectAccess, ProjectProbeError> {
        let metadata = fs::metadata(root).map_err(|error| {
            ProjectProbeError::io(ProjectProbeOperation::ReadMetadata, root, &error)
        })?;
        if !metadata.is_dir() {
            return Err(ProjectProbeError::NotDirectory {
                path: root.to_path_buf(),
            });
        }

        let entries = fs::read_dir(root).map_err(|error| {
            ProjectProbeError::io(ProjectProbeOperation::ReadDirectory, root, &error)
        })?;
        drop(entries);

        let probe_path = root.join(format!(".viewer-write-probe-{}", Uuid::new_v4()));
        let probe_file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&probe_path)
        {
            Ok(file) => file,
            Err(error) if is_read_only_write_error(&error) => {
                return Ok(ProjectAccess::ReadOnly);
            }
            Err(error) => {
                return Err(ProjectProbeError::io(
                    ProjectProbeOperation::CreateWriteProbe,
                    &probe_path,
                    &error,
                ));
            }
        };
        drop(probe_file);

        fs::remove_file(&probe_path).map_err(|error| {
            ProjectProbeError::io(
                ProjectProbeOperation::RemoveWriteProbe,
                &probe_path,
                &error,
            )
        })?;

        Ok(ProjectAccess::ReadWrite)
    }
}
```

Return `ProjectProbeError::NotDirectory { path: root.to_path_buf() }` for a
non-directory. Preserve metadata-before-read-before-create ordering,
`create_new(true)`, zero-byte probing, close-before-remove, and direct-root
probe placement.

Update the three existing tests to call:

```rust
let result = MacProjectProbe.probe(root.path());
```

with no `block_on`.

- [ ] **Step 8: Remove obsolete adapter dependencies**

Remove `async-trait.workspace = true` from
`crates/viewer-platform-macos/Cargo.toml` and remove its entire
`[dev-dependencies] futures.workspace = true` entry. Remove `async-trait` and
`futures` from root `[workspace.dependencies]` only after confirming with:

```bash
rg -n "async_trait|futures" crates src-tauri --glob '*.rs' --glob 'Cargo.toml'
```

Expected: no remaining direct source/manifest use.

Replace the direct Rust-crate sentence in `ACKNOWLEDGEMENTS.md` with:

```markdown
The direct Rust crates in the foundation are `serde`, `serde_json` (tests),
`tempfile` (tests), `thiserror`, and `uuid`.
```

Run `cargo check --workspace` to refresh `Cargo.lock`.

- [ ] **Step 9: Verify Task 4 and commit**

Run:

```bash
cargo test --locked -p viewer-application -p viewer-platform-macos
cargo clippy --locked -p viewer-application -p viewer-platform-macos --all-targets -- -D warnings
git diff --check
```

Expected: typed-port and classification tests PASS; existing permission,
cleanup, and no-`.viewer` tests PASS.

Commit:

```bash
git add ACKNOWLEDGEMENTS.md Cargo.toml Cargo.lock crates/viewer-application/Cargo.toml crates/viewer-application/src/lib.rs crates/viewer-application/src/ports.rs crates/viewer-platform-macos/Cargo.toml crates/viewer-platform-macos/src/lib.rs
git commit -m "fix: type the macOS project probe contract"
```

### Task 5: Protect the health IPC wire shape

**Files:**
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Preserves: `health() -> HealthResponse` and camel-case serialization.
- Produces: an exact JSON contract test.

- [ ] **Step 1: Add the serialization regression test**

Add to `src-tauri/src/lib.rs` tests:

```rust
#[test]
fn health_serializes_the_exact_ipc_shape() {
    let value = serde_json::to_value(super::health()).expect("serialize health response");
    assert_eq!(
        value,
        serde_json::json!({
            "appName": "Viewer",
            "version": env!("CARGO_PKG_VERSION"),
        })
    );
}
```

- [ ] **Step 2: Prove the test detects a wire regression**

Run the test once; it should PASS because the production behavior already
exists. Temporarily change the existing production annotation to:

```rust
#[serde(rename_all = "snake_case")]
```

Run:

```bash
cargo test -p viewer-desktop --lib health_serializes_the_exact_ipc_shape
```

Expected: FAIL because the response contains `app_name` instead of `appName`.
Immediately restore `#[serde(rename_all = "camelCase")]` and rerun. Expected:
PASS. Do not commit the deliberate mutation.

- [ ] **Step 3: Verify Task 5 and commit**

Run:

```bash
cargo test --locked -p viewer-desktop --lib
git diff --check
```

Commit only the regression test:

```bash
git add src-tauri/src/lib.rs
git commit -m "test: protect Viewer health IPC shape"
```

### Task 6: Pin repository and CI verification inputs

**Files:**
- Create: `scripts/repository-policy.test.mjs`
- Modify: `.github/workflows/ci.yml`
- Modify: `package.json`
- Modify: `rust-toolchain.toml`
- Modify: `scripts/check-dev-env.sh`
- Modify: `docs/superpowers/plans/2026-07-16-viewer-foundation-plan.md`

**Interfaces:**
- Produces: an executable repository-policy test included in `pnpm verify`.
- Pins: exact action SHAs, Node 24.18.0, Rust 1.97.0, pnpm 10.0.0, cargo-deny 0.20.2, and arm64 macOS 15 CI.

- [ ] **Step 1: Write the failing repository-policy test**

Create `scripts/repository-policy.test.mjs`:

```javascript
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const read = (path) => readFile(new URL(`../${path}`, import.meta.url), 'utf8')

test('repository verification inputs are exact and locked', async () => {
  const [workflow, toolchain, packageText] = await Promise.all([
    read('.github/workflows/ci.yml'),
    read('rust-toolchain.toml'),
    read('package.json'),
  ])
  const packageJson = JSON.parse(packageText)

  assert.match(workflow, /runs-on: macos-15/)
  assert.match(workflow, /actions\/checkout@9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0/)
  assert.match(workflow, /actions\/setup-node@820762786026740c76f36085b0efc47a31fe5020/)
  assert.match(workflow, /node-version: "24\.18\.0"/)
  assert.match(workflow, /rustup toolchain install 1\.97\.0/)
  assert.match(workflow, /cargo deny --locked check/)
  assert.doesNotMatch(workflow, /actions\/(checkout|setup-node)@v\d/)

  assert.match(toolchain, /channel = "1\.97\.0"/)
  assert.match(packageJson.scripts.verify, /^node --test scripts\/repository-policy\.test\.mjs && /)
  assert.match(packageJson.scripts.verify, /cargo clippy --locked /)
  assert.match(packageJson.scripts.verify, /cargo test --locked --workspace$/)
})
```

- [ ] **Step 2: Run the policy test and witness RED**

Run:

```bash
node --test scripts/repository-policy.test.mjs
```

Expected: FAIL on `macos-14`, movable action tags, major-only Node, stable Rust,
and missing Cargo lock flags.

- [ ] **Step 3: Pin the Rust toolchain and local verification**

Set `rust-toolchain.toml`:

```toml
[toolchain]
channel = "1.97.0"
components = ["clippy", "rustfmt"]
targets = ["aarch64-apple-darwin"]
profile = "minimal"
```

Set root `package.json`'s `verify` script exactly to:

```json
"verify": "node --test scripts/repository-policy.test.mjs && pnpm --dir ui test && pnpm --dir ui build && cargo fmt --check && cargo clippy --locked --workspace --all-targets -- -D warnings && cargo test --locked --workspace"
```

After the toolchain file changes, let rustup install/use 1.97.0 before running
the Rust gates.

- [ ] **Step 4: Make the environment preflight exact**

After the existing command and SDK checks in `scripts/check-dev-env.sh`, add:

```bash
[[ "$(rustc --version)" == rustc\ 1.97.0\ * ]]
[[ "$(node --version)" == "v24.18.0" ]]
[[ "$(pnpm --version)" == "10.0.0" ]]
echo "[OK] Rust 1.97.0"
echo "[OK] Node.js 24.18.0"
echo "[OK] pnpm 10.0.0"
```

- [ ] **Step 5: Pin the GitHub workflow**

Update `.github/workflows/ci.yml` to use:

```yaml
runs-on: macos-15
```

Pin official actions with explanatory version comments:

```yaml
uses: actions/checkout@9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0 # v7
```

```yaml
uses: actions/setup-node@820762786026740c76f36085b0efc47a31fe5020 # v7
with:
  node-version: "24.18.0"
```

Install exact Rust:

```bash
rustup toolchain install 1.97.0 --profile minimal --component clippy,rustfmt --target aarch64-apple-darwin
rustc --version --verbose
```

Change the final audit command to:

```yaml
run: cargo deny --locked check
```

Keep the arm64 assertion, frozen pnpm install, exact pnpm activation, exact
cargo-deny installation, read-only token permissions, and timeout.

- [ ] **Step 6: Verify dependency acknowledgements**

Confirm `ACKNOWLEDGEMENTS.md` still names only the direct final Rust crates
`serde`, `serde_json` (tests), `tempfile` (tests), `thiserror`, and `uuid`, and
retains the explicit "project license has not yet been selected" wording. Do
not edit it if Task 4 already left that exact state.

- [ ] **Step 7: Mark the original plan as superseded where defective**

Add immediately below the original Foundation plan title:

```markdown
> **Hardening supersession:** The approved
> `2026-07-16-viewer-foundation-hardening-design.md` and
> `2026-07-16-viewer-foundation-hardening-plan.md` supersede this plan's
> capability, navigation, session-failure, reserved-path, project-probe, and
> reproducibility snippets. Those original snippets are retained only as task
> history and must not be copied into new work.
```

- [ ] **Step 8: Verify Task 6 and commit**

Run:

```bash
node --test scripts/repository-policy.test.mjs
./scripts/check-dev-env.sh
pnpm verify
cargo deny --offline --locked check
git diff --check
```

Expected: policy test PASS, environment reports exact versions, all UI/Rust
checks PASS, all cargo-deny categories PASS.

Commit:

```bash
git add .github/workflows/ci.yml package.json rust-toolchain.toml scripts/check-dev-env.sh scripts/repository-policy.test.mjs docs/superpowers/plans/2026-07-16-viewer-foundation-plan.md
git commit -m "ci: pin Viewer foundation verification"
```

### Task 7: Run the hardening integration gate

**Files:**
- No tracked file change is planned for this verification-only task.
- Update ignored ledger: `.superpowers/sdd/progress.md`.

**Interfaces:**
- Consumes: Tasks 1–6.
- Produces: fresh automated, architecture, dependency-policy, and desktop runtime evidence from a clean branch.

- [ ] **Step 1: Verify forbidden dependencies and capability content**

Run:

```bash
rg -n "tauri-plugin-(fs|shell|http|sql|updater)|reqwest|telemetry|sentry" Cargo.toml src-tauri crates ui package.json pnpm-lock.yaml
```

Expected: no application dependency match.

Run:

```bash
node --test scripts/repository-policy.test.mjs
cargo tree --workspace --edges normal
```

Expected: policy PASS; dependency direction remains inward.

- [ ] **Step 2: Run the complete automated gate**

Run:

```bash
./scripts/check-dev-env.sh
pnpm verify
cargo deny --offline --locked check
git diff --check
git status --short
```

Expected: all commands exit 0 and `git status --short` is empty.

- [ ] **Step 3: Run the desktop smoke test**

Run:

```bash
pnpm tauri dev
```

Expected:

- Tauri compiles and launches `viewer-desktop`.
- Window title is `Viewer`.
- Empty state contains `拖入或选择一个项目文件夹`.
- No console error or application-originated remote resource request is seen.
- The registered navigation plugin does not prevent startup; rejection behavior
  remains covered by the focused origin-policy unit tests without contacting an
  arbitrary remote site.

Terminate the dev command. If Tauri CLI normalizes empty `features = []` fields
in `src-tauri/Cargo.toml`, inspect that exact diff and restore the committed
form with `apply_patch`; do not use destructive Git commands.

- [ ] **Step 4: Confirm the final tree is clean**

Run:

```bash
git diff --check
git status --short
```

Expected: no output.

- [ ] **Step 5: Record execution evidence**

Update `.superpowers/sdd/progress.md` with each hardening task commit, review
result, exact test counts, cargo-deny result, runtime result, and remaining
Minor findings. This file is intentionally ignored and is not committed.

## Hardening Completion Criteria

- Tasks 1–6 each have a witnessed red-green or mutation-test cycle and an
  independent review.
- The complete Foundation diff receives a final whole-branch re-review with no
  Critical or Important issue.
- `pnpm verify` and `cargo deny --offline --locked check` pass under exact local
  toolchains.
- The Viewer desktop process launches with the expected title and empty state.
- No unexplained working-tree change or running development process remains.
