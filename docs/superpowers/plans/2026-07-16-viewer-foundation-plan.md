# Viewer Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create a reproducible Viewer repository with a working Tauri 2 window, React UI, Rust workspace boundaries, typed IPC baseline, tests and CI.

**Architecture:** The repository is a modular monolith. `viewer-domain` is dependency-light, `viewer-application` defines use cases and ports, Infrastructure/macOS crates implement ports, and `src-tauri` is the composition root.

**Tech Stack:** Rust stable, Cargo workspace, Tauri 2, React, TypeScript, Vite, pnpm 10, Vitest, GitHub Actions.

## Global Constraints

- Build target is Apple Silicon (`aarch64-apple-darwin`) with minimum macOS 13.0.
- Runtime has no login, telemetry, updater, HTTP client or application-originated network request.
- Frontend has no broad filesystem, shell or SQL capability.
- Rust Core owns business state; React owns presentation state only.
- Dependency direction is `src-tauri → adapters → application → domain`.
- All resolved dependency versions are committed through `Cargo.lock` and `pnpm-lock.yaml`.
- Do not add Windows implementation code in Viewer 0.1.

---

## Target File Map

```text
Viewer/
├── .github/workflows/ci.yml
├── .gitignore
├── Cargo.toml
├── rust-toolchain.toml
├── package.json
├── pnpm-workspace.yaml
├── scripts/check-dev-env.sh
├── ui/
│   ├── package.json
│   ├── vite.config.ts
│   └── src/{App.tsx,App.test.tsx,main.tsx,setupTests.ts}
├── src-tauri/
│   ├── Cargo.toml
│   ├── build.rs
│   ├── capabilities/main.json
│   ├── src/{lib.rs,main.rs}
│   └── tauri.conf.json
└── crates/
    ├── viewer-domain/src/lib.rs
    ├── viewer-application/src/{lib.rs,ports.rs,session.rs}
    ├── viewer-infrastructure/src/lib.rs
    ├── viewer-platform-macos/src/lib.rs
    └── viewer-test-support/src/lib.rs
```

### Task 1: Initialize Git and establish a verified development environment

**Files:**
- Create: `.gitignore`
- Create: `rust-toolchain.toml`
- Create: `scripts/check-dev-env.sh`

**Interfaces:**
- Consumes: macOS Command Line Tools with an SDK discoverable through `xcrun`.
- Produces: a Git repository and a preflight command used by every later plan.

- [ ] **Step 1: Initialize the repository**

Run:

```bash
git init
git branch -M main
```

Expected: an empty repository on branch `main`.

- [ ] **Step 2: Install missing stable toolchains**

Rust is currently absent from the machine. Install it with the official rustup installer, then restart the shell:

```bash
curl --proto '=https' --tlsv1.2 https://sh.rustup.rs -sSf | sh -s -- -y
. "$HOME/.cargo/env"
rustup default stable
rustup target add aarch64-apple-darwin
```

Install the current Node.js LTS release from the official Node.js installer, then enable pnpm:

```bash
corepack enable
corepack prepare pnpm@10.0.0 --activate
```

Expected: `rustc`, `cargo`, `node`, `pnpm`, `clang` and `xcrun` are available. Full Xcode is optional for desktop development; install it before signing/notarization if the release toolchain requires it.

- [ ] **Step 3: Add reproducibility and ignore files**

Create `rust-toolchain.toml`:

```toml
[toolchain]
channel = "stable"
components = ["clippy", "rustfmt"]
targets = ["aarch64-apple-darwin"]
profile = "minimal"
```

Create `.gitignore`:

```gitignore
/target/
/node_modules/
/ui/node_modules/
/ui/dist/
/.superpowers/
/.DS_Store
*.log
src-tauri/gen/
```

Create `scripts/check-dev-env.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

required=(rustc cargo node pnpm clang xcrun)
for command_name in "${required[@]}"; do
  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "[MISSING] $command_name"
    exit 1
  fi
  echo "[OK] $command_name"
done

xcrun --show-sdk-path >/dev/null
rustup target list --installed | grep -qx aarch64-apple-darwin
echo "[OK] macOS SDK"
echo "[OK] aarch64-apple-darwin target"
```

- [ ] **Step 4: Run the environment check**

Run:

```bash
chmod +x scripts/check-dev-env.sh
./scripts/check-dev-env.sh
```

Expected: eight `[OK]` lines and exit code 0.

- [ ] **Step 5: Commit the environment baseline**

```bash
git add .gitignore rust-toolchain.toml scripts/check-dev-env.sh
git commit -m "chore: initialize Viewer development environment"
```

### Task 2: Scaffold the Cargo workspace and React/Tauri shell

**Files:**
- Create: `Cargo.toml`
- Create: `package.json`
- Create: `pnpm-workspace.yaml`
- Create: `ui/package.json`
- Create: `ui/vite.config.ts`
- Create: `ui/src/App.tsx`
- Create: `ui/src/main.tsx`
- Create: `src-tauri/Cargo.toml`
- Create: `src-tauri/build.rs`
- Create: `src-tauri/src/main.rs`
- Create: `src-tauri/src/lib.rs`
- Create: `src-tauri/tauri.conf.json`
- Create: `src-tauri/capabilities/main.json`

**Interfaces:**
- Consumes: verified toolchain from Task 1.
- Produces: `viewer-desktop::run()` and a React application served at `http://localhost:5173` in development.

- [ ] **Step 1: Create the frontend and root package workspace**

Run:

```bash
pnpm create vite ui --template react-ts
```

Remove the generated demonstration-only files (`ui/public/vite.svg`,
`ui/src/assets/react.svg`, `ui/src/App.css`, `ui/src/index.css`,
`ui/README.md` and `ui/eslint.config.js`). Keep Vite/TypeScript files required
to build the application. Replace the generated components with this minimal
shell so no deleted asset remains imported:

```tsx
// ui/src/App.tsx
export default function App() {
  return <main><h1>Viewer</h1></main>
}
```

```tsx
// ui/src/main.tsx
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import App from './App'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
```

Create root `package.json`:

```json
{
  "name": "viewer",
  "private": true,
  "packageManager": "pnpm@10.0.0",
  "scripts": {
    "dev": "pnpm --dir ui dev",
    "test:ui": "pnpm --dir ui test",
    "build:ui": "pnpm --dir ui build",
    "tauri": "tauri"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2.0.0"
  }
}
```

Create `pnpm-workspace.yaml`:

```yaml
packages:
  - ui
```

Run all installs only after the root workspace files exist, so the repository
has one root lockfile and no nested `ui/pnpm-lock.yaml`:

```bash
pnpm install
pnpm --dir ui add @tauri-apps/api@^2
pnpm --dir ui add -D vitest @testing-library/react @testing-library/jest-dom jsdom
```

Expected: root `pnpm-lock.yaml` is created and `ui/pnpm-lock.yaml` does not
exist.

- [ ] **Step 2: Create the Rust workspace**

Create root `Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = ["src-tauri"]

[workspace.package]
edition = "2024"
rust-version = "1.85"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
thiserror = "2"
uuid = { version = "1", features = ["v4", "serde"] }
```

Register only crates that exist. Tasks 3–5 append each crate to `members` in
the same commit that creates it; the completed Foundation workspace still has
all six planned members.

If the installed stable compiler cannot compile edition 2024 with `rust-version = "1.85"`, stop: the wrong Rust installation is active. Do not lower the project baseline silently.

- [ ] **Step 3: Write the failing Tauri smoke test**

Create the minimal package manifest first so RED exercises the requested
symbol rather than failing because the package is absent. Create
`src-tauri/Cargo.toml`:

```toml
[package]
name = "viewer-desktop"
version = "0.1.0"
edition.workspace = true

[lib]
name = "viewer_desktop"
crate-type = ["lib", "cdylib", "staticlib"]
```

Then create `src-tauri/src/lib.rs`:

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn app_name_is_viewer() {
        assert_eq!(super::APP_NAME, "Viewer");
    }
}
```

Run:

```bash
cargo test -p viewer-desktop app_name_is_viewer
```

Expected: FAIL with an unresolved `APP_NAME` error. The package must be found;
a missing-package or missing-manifest error is not valid RED evidence.

- [ ] **Step 4: Add the minimal Tauri package**

Replace `src-tauri/Cargo.toml` with the complete Tauri package manifest:

```toml
[package]
name = "viewer-desktop"
version = "0.1.0"
edition.workspace = true

[lib]
name = "viewer_desktop"
crate-type = ["lib", "cdylib", "staticlib"]

[build-dependencies]
tauri-build = { version = "2" }

[dependencies]
serde.workspace = true
tauri = { version = "2" }
```

Create `src-tauri/build.rs`:

```rust
fn main() {
    tauri_build::build();
}
```

Replace `src-tauri/src/lib.rs` with:

```rust
pub const APP_NAME: &str = "Viewer";

pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("failed to run Viewer");
}

#[cfg(test)]
mod tests {
    #[test]
    fn app_name_is_viewer() {
        assert_eq!(super::APP_NAME, "Viewer");
    }
}
```

Create `src-tauri/src/main.rs`:

```rust
fn main() {
    viewer_desktop::run();
}
```

Create `src-tauri/tauri.conf.json`:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "Viewer",
  "version": "0.1.0",
  "identifier": "com.viewer.app",
  "build": {
    "beforeDevCommand": "pnpm --dir ui dev",
    "devUrl": "http://localhost:5173",
    "beforeBuildCommand": "pnpm --dir ui build",
    "frontendDist": "../ui/dist"
  },
  "app": {
    "windows": [{ "title": "Viewer", "width": 1280, "height": 800 }],
    "security": {
      "csp": "default-src 'self'; connect-src ipc: http://ipc.localhost; img-src 'self' blob: data:; style-src 'self' 'unsafe-inline'; script-src 'self'"
    }
  },
  "bundle": {
    "active": true,
    "targets": ["app", "dmg"],
    "macOS": { "minimumSystemVersion": "13.0" }
  }
}
```

Create `src-tauri/capabilities/main.json`:

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "main",
  "description": "Minimal capability for Viewer main window",
  "windows": ["main"],
  "permissions": ["core:default"]
}
```

- [ ] **Step 5: Verify the shell**

Run:

```bash
cargo test -p viewer-desktop app_name_is_viewer
pnpm --dir ui build
```

Expected: Rust test PASS and Vite build succeeds.

- [ ] **Step 6: Commit the shell**

```bash
git add Cargo.toml Cargo.lock package.json pnpm-workspace.yaml pnpm-lock.yaml ui src-tauri
git commit -m "feat: scaffold Viewer Tauri shell"
```

### Task 3: Create domain identifiers and safe relative paths

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/viewer-domain/Cargo.toml`
- Create: `crates/viewer-domain/src/lib.rs`
- Test: `crates/viewer-domain/src/lib.rs`

**Interfaces:**
- Produces: `ProjectId`, `SessionId`, `EntityId`, `TaskId`, `OperationId`, `RelativePath::parse(&str)`.

- [ ] **Step 1: Write failing domain tests**

Append `"crates/viewer-domain"` to the root workspace `members`, then create
`crates/viewer-domain/Cargo.toml` before running RED:

```toml
[package]
name = "viewer-domain"
version = "0.1.0"
edition.workspace = true

[dependencies]
serde.workspace = true
thiserror.workspace = true
uuid.workspace = true
```

Create `crates/viewer-domain/src/lib.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_path_rejects_escape_and_absolute_paths() {
        assert!(RelativePath::parse("../outside").is_err());
        assert!(RelativePath::parse("/absolute").is_err());
        assert!(RelativePath::parse(".viewer/metadata.sqlite").is_err());
        assert!(RelativePath::parse("products/id-1/front.png").is_ok());
    }

    #[test]
    fn ids_are_distinct() {
        assert_ne!(ProjectId::new().to_string(), ProjectId::new().to_string());
    }
}
```

Run `cargo test -p viewer-domain`. Expected: FAIL with unresolved
`RelativePath`/`ProjectId` symbols. A missing-package or missing-manifest error
is not valid RED evidence.

- [ ] **Step 2: Implement the minimal domain primitives**

Replace `crates/viewer-domain/src/lib.rs` with:

```rust
use serde::{Deserialize, Serialize};
use std::{fmt, path::{Component, Path}};
use thiserror::Error;
use uuid::Uuid;

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
        pub struct $name(Uuid);

        impl $name {
            pub fn new() -> Self { Self(Uuid::new_v4()) }
        }

        impl Default for $name {
            fn default() -> Self { Self::new() }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.0.fmt(f) }
        }

        impl std::str::FromStr for $name {
            type Err = uuid::Error;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(value).map(Self)
            }
        }
    };
}

id_type!(ProjectId);
id_type!(SessionId);
id_type!(EntityId);
id_type!(TaskId);
id_type!(OperationId);

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct RelativePath(String);

#[derive(Debug, Error, Eq, PartialEq)]
pub enum RelativePathError {
    #[error("path must be a non-empty project-relative path")]
    Invalid,
}

impl RelativePath {
    pub fn parse(value: &str) -> Result<Self, RelativePathError> {
        let path = Path::new(value);
        let valid = !value.is_empty()
            && !path.is_absolute()
            && path.components().all(|part| {
                matches!(part, Component::Normal(name) if name != ".viewer")
            });
        valid.then(|| Self(value.to_owned())).ok_or(RelativePathError::Invalid)
    }

    pub fn as_str(&self) -> &str { &self.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_path_rejects_escape_and_absolute_paths() {
        assert!(RelativePath::parse("../outside").is_err());
        assert!(RelativePath::parse("/absolute").is_err());
        assert!(RelativePath::parse(".viewer/metadata.sqlite").is_err());
        assert!(RelativePath::parse("products/id-1/front.png").is_ok());
    }

    #[test]
    fn ids_are_distinct() {
        assert_ne!(ProjectId::new().to_string(), ProjectId::new().to_string());
    }
}
```

- [ ] **Step 3: Verify and commit**

Run `cargo test -p viewer-domain`. Expected: 2 PASS.

```bash
git add Cargo.toml Cargo.lock crates/viewer-domain
git commit -m "feat: add Viewer domain primitives"
```

### Task 4: Define the application session and adapter ports

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/viewer-application/Cargo.toml`
- Create: `crates/viewer-application/src/lib.rs`
- Create: `crates/viewer-application/src/session.rs`
- Create: `crates/viewer-application/src/ports.rs`
- Test: `crates/viewer-application/src/session.rs`

**Interfaces:**
- Consumes: `ProjectId`, `SessionId`.
- Produces: `SessionState`, `ProjectSession`, `ProjectAccess`, `ClockPort`, `ProjectProbePort`.

- [ ] **Step 1: Write the failing session transition test**

Add `async-trait = "0.1"` to root workspace dependencies, append
`"crates/viewer-application"` to workspace `members`, and create
`crates/viewer-application/Cargo.toml`:

```toml
[package]
name = "viewer-application"
version = "0.1.0"
edition.workspace = true

[dependencies]
async-trait.workspace = true
serde.workspace = true
thiserror.workspace = true
viewer-domain = { path = "../viewer-domain" }
```

Create `crates/viewer-application/src/lib.rs` containing `pub mod session;`,
then create `session.rs` with the tests below. This makes the crate resolvable
while leaving the requested session behavior unimplemented for RED.

Create `crates/viewer-application/src/session.rs` with a test asserting `Empty → Opening → ActiveReadWrite → Closing → Empty` and rejecting a second `open` while active.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_follows_the_only_valid_writeable_path() {
        let mut session = ProjectSession::default();
        session.begin_open().unwrap();
        session.activate(ProjectAccess::ReadWrite).unwrap();
        session.begin_close().unwrap();
        session.finish_close().unwrap();
        assert_eq!(session.state(), SessionState::Empty);
    }

    #[test]
    fn active_session_rejects_second_open() {
        let mut session = ProjectSession::default();
        session.begin_open().unwrap();
        session.activate(ProjectAccess::ReadOnly).unwrap();
        assert_eq!(session.begin_open(), Err(SessionTransitionError::Invalid));
    }
}
```

Run `cargo test -p viewer-application`. Expected: FAIL with unresolved session
types or methods. A missing-package or missing-manifest error is not valid RED
evidence.

- [ ] **Step 2: Implement session types and ports**

Implement the exact public surface in `session.rs`:

```rust
use crate::ports::ProjectAccess;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionState { Empty, Opening, ActiveReadWrite, ActiveReadOnly, Closing }

#[derive(Debug)]
pub struct ProjectSession { state: SessionState }

impl Default for ProjectSession {
    fn default() -> Self { Self { state: SessionState::Empty } }
}

impl ProjectSession {
    pub fn state(&self) -> SessionState;
    pub fn begin_open(&mut self) -> Result<(), SessionTransitionError>;
    pub fn activate(&mut self, access: ProjectAccess) -> Result<(), SessionTransitionError>;
    pub fn begin_close(&mut self) -> Result<(), SessionTransitionError>;
    pub fn finish_close(&mut self) -> Result<(), SessionTransitionError>;
}
```

Create `ports.rs`:

```rust
use async_trait::async_trait;
use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectAccess { ReadWrite, ReadOnly }

#[async_trait]
pub trait ProjectProbePort: Send + Sync {
    async fn probe(&self, root: &Path) -> Result<ProjectAccess, String>;
}

pub trait ClockPort: Send + Sync {
    fn unix_millis(&self) -> i64;
}
```

Re-export ports and session types from `lib.rs`. Keep `ProjectAccess` defined once in `ports.rs` and imported by `session.rs`.

- [ ] **Step 3: Verify and commit**

Run:

```bash
cargo fmt --check
cargo test -p viewer-application
```

Expected: 2 session tests PASS.

```bash
git add Cargo.toml Cargo.lock crates/viewer-application
git commit -m "feat: define application session boundaries"
```

### Task 5: Create Infrastructure, macOS and test-support adapter crates

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/viewer-infrastructure/{Cargo.toml,src/lib.rs}`
- Create: `crates/viewer-platform-macos/{Cargo.toml,src/lib.rs}`
- Create: `crates/viewer-test-support/{Cargo.toml,src/lib.rs}`

**Interfaces:**
- Consumes: Application ports from Task 4.
- Produces: `SystemClock`, `MacProjectProbe`, `FixedClock`.

- [ ] **Step 1: Write failing adapter contract tests**

Append the three adapter crates to workspace `members` in dependency order.
Add `futures = "0.3"` and `tempfile = "3"` to workspace dependencies for
adapter contract tests. Create each crate manifest before RED:

```toml
# crates/viewer-infrastructure/Cargo.toml
[package]
name = "viewer-infrastructure"
version = "0.1.0"
edition.workspace = true

[dependencies]
viewer-application = { path = "../viewer-application" }
```

```toml
# crates/viewer-platform-macos/Cargo.toml
[package]
name = "viewer-platform-macos"
version = "0.1.0"
edition.workspace = true

[dependencies]
async-trait.workspace = true
uuid.workspace = true
viewer-application = { path = "../viewer-application" }

[dev-dependencies]
futures.workspace = true
tempfile.workspace = true
```

```toml
# crates/viewer-test-support/Cargo.toml
[package]
name = "viewer-test-support"
version = "0.1.0"
edition.workspace = true

[dependencies]
viewer-application = { path = "../viewer-application" }
```

In `viewer-test-support`, define a test requiring
`FixedClock::new(42).unix_millis() == 42`. In `viewer-infrastructure`, define a
test proving `SystemClock::unix_millis()` falls between wall-clock readings
taken immediately before and after it. In `viewer-platform-macos`, use
`tempfile` plus `futures::executor::block_on` to verify that a readable
temporary directory returns either `ReadWrite` or `ReadOnly`, never an error.

Run:

```bash
cargo test -p viewer-infrastructure -p viewer-test-support -p viewer-platform-macos
```

Expected: FAIL with unresolved adapter types. Missing-package or
missing-manifest errors are not valid RED evidence.

- [ ] **Step 2: Implement the minimal adapters**

Implement:

```rust
pub struct SystemClock;
impl ClockPort for SystemClock {
    fn unix_millis(&self) -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before epoch")
            .as_millis() as i64
    }
}
```

```rust
pub struct FixedClock(i64);
impl FixedClock {
    pub fn new(value: i64) -> Self { Self(value) }
}
impl ClockPort for FixedClock {
    fn unix_millis(&self) -> i64 { self.0 }
}
```

Implement `MacProjectProbe` by checking directory metadata and attempting to create and remove a uniquely named zero-byte probe file. Convert `PermissionDenied` to `ProjectAccess::ReadOnly`; propagate unreadable-directory errors. Never probe inside `.viewer` at this stage.

- [ ] **Step 3: Verify and commit**

Run `cargo test --workspace`. Expected: all workspace tests PASS.

```bash
git add Cargo.toml Cargo.lock crates/viewer-infrastructure crates/viewer-platform-macos crates/viewer-test-support
git commit -m "feat: add Viewer adapter crates"
```

### Task 6: Add typed health IPC and an empty-state UI test

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/lib.rs`
- Modify: `ui/src/App.tsx`
- Create: `ui/src/App.test.tsx`
- Create: `ui/src/setupTests.ts`
- Modify: `ui/vite.config.ts`
- Modify: `ui/package.json`

**Interfaces:**
- Produces: Tauri command `health() -> HealthResponse` and UI heading `Viewer` with import guidance.

- [ ] **Step 1: Write the failing UI test**

Create `ui/src/setupTests.ts`:

```ts
import '@testing-library/jest-dom/vitest'
```

Create `ui/src/App.test.tsx`:

```tsx
import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import App from './App'

describe('Viewer empty state', () => {
  it('asks the user to import one project folder', () => {
    render(<App />)
    expect(screen.getByRole('heading', { name: 'Viewer' })).toBeVisible()
    expect(screen.getByText('拖入或选择一个项目文件夹')).toBeVisible()
  })
})
```

Configure Vitest in `vite.config.ts` with `environment: 'jsdom'` and `setupFiles: './src/setupTests.ts'`; add `"test": "vitest run"` to `ui/package.json`.

Run `pnpm --dir ui test`. Expected: FAIL because the minimal shell does not
yet contain the import guidance.

Add a Rust test to the existing test module in `src-tauri/src/lib.rs` before
the health command exists:

```rust
#[test]
fn health_returns_app_identity() {
    let response = super::health();
    assert_eq!(response.app_name, "Viewer");
    assert_eq!(response.version, env!("CARGO_PKG_VERSION"));
}
```

Run `cargo test -p viewer-desktop health_returns_app_identity`. Expected:
FAIL with an unresolved `health` function. A package/configuration failure is
not valid RED evidence.

- [ ] **Step 2: Implement the minimal empty state**

Replace `ui/src/App.tsx`:

```tsx
export default function App() {
  return (
    <main>
      <h1>Viewer</h1>
      <p>拖入或选择一个项目文件夹</p>
    </main>
  )
}
```

- [ ] **Step 3: Add typed Rust health command**

Add to `src-tauri/src/lib.rs`:

```rust
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    app_name: &'static str,
    version: &'static str,
}

#[tauri::command]
pub fn health() -> HealthResponse {
    HealthResponse { app_name: APP_NAME, version: env!("CARGO_PKG_VERSION") }
}
```

Register it with `.invoke_handler(tauri::generate_handler![health])` immediately before the existing `.run(tauri::generate_context!())` call.

- [ ] **Step 4: Verify and commit**

Run:

```bash
pnpm --dir ui test
pnpm --dir ui build
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: all commands PASS.

```bash
git add ui src-tauri Cargo.lock pnpm-lock.yaml
git commit -m "feat: add Viewer empty state and typed health IPC"
```

### Task 7: Add continuous verification and dependency policy

**Files:**
- Create: `.github/workflows/ci.yml`
- Create: `deny.toml`
- Modify: `package.json`
- Modify: `ACKNOWLEDGEMENTS.md`

**Interfaces:**
- Produces: one `pnpm verify` command and CI checks for format, lint, unit tests, UI build and licenses.

- [ ] **Step 1: Add the local verification script**

Add to root `package.json`:

```json
"verify": "pnpm --dir ui test && pnpm --dir ui build && cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace"
```

Run `pnpm verify`. Expected: PASS.

- [ ] **Step 2: Add Cargo deny policy**

Install `cargo-deny` with `cargo install cargo-deny --locked`. Create `deny.toml` permitting MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Unicode-3.0, CC0-1.0 and MPL-2.0, while denying unmaintained/yanked advisories and unknown git sources.

Run `cargo deny check`. Expected: PASS for the foundation dependency graph.

- [ ] **Step 3: Add CI**

Create `.github/workflows/ci.yml` with a `macos-14` job that checks out code, installs stable Rust with the Apple Silicon target, sets up current Node LTS and pnpm 10, runs `pnpm install --frozen-lockfile`, `pnpm verify`, and `cargo deny check`.

Do not add signing, release upload, updater or network runtime tests in this foundation task.

- [ ] **Step 4: Record direct foundation dependencies**

Update `ACKNOWLEDGEMENTS.md` to list Tauri, React, Vite, Rust, Vitest and the testing libraries only if they are now direct dependencies. Do not list planned dependencies that are not in lockfiles.

- [ ] **Step 5: Verify and commit**

Run:

```bash
pnpm verify
cargo deny check
```

Expected: PASS.

```bash
git add .github deny.toml package.json ACKNOWLEDGEMENTS.md
git commit -m "ci: enforce Viewer foundation checks"
```

## Foundation Completion Check

Run:

```bash
./scripts/check-dev-env.sh
pnpm verify
cargo deny check
pnpm tauri dev
```

Expected:

- All automated checks pass.
- A window titled Viewer opens on the empty import state.
- DevTools show no remote network requests.
- `git status --short` is empty.
