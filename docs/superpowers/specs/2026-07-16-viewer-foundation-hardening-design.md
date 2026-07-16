# Viewer Foundation Hardening Design

**Status:** Approved

**Date:** 2026-07-16

## 1. Purpose

The complete Foundation branch review found six cross-task defects that are not
visible when Tasks 2–7 are reviewed independently. This design corrects those
contracts before later Viewer milestones depend on them.

This document supersedes conflicting capability, navigation, session,
`RelativePath`, project-probe, and CI prescriptions in the original Foundation
implementation plan. It does not expand Viewer 0.1 product scope.

## 2. Goals

- Make the WebView-to-native boundary least-privileged and deny remote
  navigation by default.
- Make project opening recoverable after any failure.
- Preserve the `.viewer` metadata boundary on common case-insensitive macOS
  filesystems.
- Model project-probe failures without erasing their operation or path context.
- Keep blocking filesystem calls out of an async interface that suggests they
  are non-blocking.
- Make local and CI verification use reviewed, reproducible toolchain inputs.
- Add regression tests for every behavioral correction.

## 3. Non-goals

- No project import, indexing, thumbnail, preview, search, or file-operation
  feature is added here.
- No Windows implementation is added.
- No application license is selected or implied.
- No broad filesystem, shell, HTTP, SQL, updater, or telemetry capability is
  introduced.
- No general-purpose async runtime is added solely for the Foundation probe.

## 4. Security boundary

### 4.1 Tauri capability

The main window capability will contain an empty `permissions` array. The
Foundation frontend does not call any Tauri core or plugin API. Viewer-owned
commands registered through `invoke_handler` remain available under Tauri's
application-command model; future plugin access must be added as an explicit,
reviewed permission.

A regression test will parse the committed capability and require:

- the window selector is exactly `main`;
- no remote capability origin is present; and
- the permissions array is empty.

This prevents a future image feature from silently activating the broad
`core:image:allow-from-path` permission inherited by `core:default`.

### 4.2 Navigation policy

A small pure predicate will define the only permitted top-level origins:

- production: `tauri://localhost` with no credentials or port;
- debug development: `http://localhost:5173` with no credentials.

Paths, queries, and fragments within an allowed origin are permitted. Other
hosts, ports, schemes, credentials, `file:`, `data:`, and arbitrary HTTP(S)
origins are denied. A Tauri plugin navigation hook will call this predicate and
cancel rejected navigation.

The CSP will retain the IPC and local asset requirements while adding explicit
denials for base URL rewriting, form submission, embedded objects, and frames:
`base-uri 'none'`, `form-action 'none'`, `object-src 'none'`, and
`frame-src 'none'`.

Unit tests will cover both accepted origins and representative bypass attempts.

## 5. Session failure recovery

The session state machine will add an explicit `fail_open` operation:

```text
EMPTY -> OPENING -> ACTIVE_READ_WRITE -> CLOSING -> EMPTY
                 -> ACTIVE_READ_ONLY  -> CLOSING -> EMPTY
         OPENING --fail_open---------> CLOSING -> EMPTY
```

`fail_open` is legal only from `Opening`. It transitions to `Closing`, not
directly to `Empty`, so later opening stages can release partially acquired
resources through the same close path. `finish_close` remains the only
transition from `Closing` to `Empty`.

The operation matrix grows from five to six operations. Seven cells are legal
and all remaining 23 cells must return `Invalid` without changing state.

## 6. Relative path invariant

`RelativePath` will reject:

- `.viewer` in any ASCII case at any segment depth;
- embedded NUL characters;
- the existing absolute, empty, dot, dot-dot, repeated-separator, trailing-
  separator, and non-normal-component cases.

ASCII case-insensitive comparison is intentional because the reserved name is
ASCII and must alias correctly on the default case-insensitive APFS setup.
Custom deserialization continues to call the same parser. Parse and serde tests
will cover root and nested case variants plus NUL input.

## 7. Project probe contract

`ProjectProbePort::probe` becomes synchronous because its implementation uses
blocking filesystem APIs. The future bounded I/O scheduler will own dispatch of
this synchronous port; the Foundation will not pretend that blocking work is an
async operation or add a runtime only to wrap it.

The port returns a typed `ProjectProbeError` that distinguishes:

- root metadata read;
- non-directory root;
- root directory read;
- write-probe creation; and
- write-probe cleanup.

Each I/O error retains the operation, affected path, and a diagnostic message.
Only write-probe creation errors classified as `PermissionDenied` or
`ReadOnlyFilesystem` produce `ProjectAccess::ReadOnly`. Metadata/read failures,
unexpected creation failures, and cleanup failures remain typed errors.

Pure classification tests will cover both read-only kinds and a non-read-only
kind. Existing integration tests continue to prove ordering, cleanup, and the
absence of writes inside `.viewer`.

Removing the now-unused direct `async-trait` and `futures` dependencies is part
of this correction. Acknowledgements must reflect the final direct dependency
set.

## 8. Reproducible verification

The repository and CI will use these reviewed inputs:

- `actions/checkout` commit
  `9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0` (`v7`);
- `actions/setup-node` commit
  `820762786026740c76f36085b0efc47a31fe5020` (`v7`);
- Node.js `24.18.0`;
- pnpm `10.0.0`;
- Rust `1.97.0` with rustfmt, Clippy, and `aarch64-apple-darwin`;
- cargo-deny `0.20.2`; and
- GitHub's supported arm64 `macos-15` runner with an explicit `uname -m`
  assertion.

Cargo Clippy, test, and cargo-deny commands will use `--locked`. Cargo fmt has
no dependency resolution and therefore needs no lock flag. The Rust toolchain
file will be pinned to `1.97.0`, and CI will install that same release.

The RustSec advisory database remains intentionally fresh rather than frozen;
dependency versions and tool binaries are reproducible, while new security
advisories are expected to make the gate fail.

## 9. IPC wire-shape regression

The health response test will serialize the public response and assert the
exact JSON object:

```json
{"appName":"Viewer","version":"0.1.0"}
```

This protects the camel-case IPC contract rather than testing only private Rust
fields. `serde_json` may be added as a direct test dependency and must be listed
accurately in acknowledgements.

## 10. Test and verification strategy

Implementation follows strict red-green cycles:

1. Add capability and navigation policy tests; observe failures; remove
   `core:default`, add the predicate/hook, and harden CSP.
2. Add the opening-failure path and 23-invalid-cell expectations; observe
   failures; implement `fail_open`.
3. Add case-variant, nested, NUL, and serde path cases; observe failures;
   strengthen the parser.
4. Add typed probe and read-only classification tests; observe failures;
   convert the port and adapter to synchronous typed behavior.
5. Add the exact health JSON assertion; observe failure or compile error; add
   only the required test dependency and serialization assertion.
6. Validate current CI/toolchain policy against the exact contract; update the
   configuration; run locked verification.

Final evidence must include:

```text
./scripts/check-dev-env.sh
pnpm verify
cargo deny --offline --locked check
pnpm tauri dev
git diff --check
git status --short
```

The desktop runtime check must confirm the Viewer title and empty state, no
console errors, and no application-originated remote request. Any Tauri CLI
manifest normalization created by `tauri dev` must be inspected and must not be
left as an unexplained working-tree change.

## 11. Acceptance criteria

- No Critical or Important whole-branch review finding remains.
- Capability permissions are empty and regression-tested.
- Remote navigation is rejected by code, not only discouraged by CSP.
- Failed opening always has a legal route back to `Empty`.
- Reserved metadata paths cannot be reached through case variants or NUL input.
- Read-only filesystem roots produce a read-only access result.
- Probe errors are typed and blocking behavior is explicit.
- CI inputs and Cargo resolution are pinned as specified.
- All automated checks and the desktop runtime smoke test pass from a clean
  worktree.
