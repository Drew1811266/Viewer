# Task 13 implementation report

Status: reviewed code committed. Non-release verification passed; the Developer
ID, notarization, stapling, and `spctl` release gate is deferred by explicit
user ruling. No signed application artifact is claimed.

## Scope implemented

- Added `release_layout.rs`, confirming that the application resource root
  resolves only `ViewerVideoRuntime/lib/libmpv.2.dylib`, `bin/ffmpeg`, and
  `bin/ffprobe`; no host fallback is constructed.
- Added exact Tauri resource packaging for
  `target/viewer-video-runtime/universal-apple-darwin/ViewerVideoRuntime`.
- Added a build-script schema gate that reads the reviewed runtime lock,
  rejects schemas other than version 1, exposes the schema version through
  `cargo:rustc-env`, allows an empty resource placeholder only for non-release
  Cargo profiles, and requires an inventoried staged runtime for release.
- Added `stage-tauri-resources.sh`. It verifies the source before inspecting or
  mutating the target, compares both source and target files against the
  inventory, rejects symlinks/unexpected files, preserves executable bits,
  signs every nested dylib/executable, regenerates signed hashes, and verifies
  the staged result.
- Added `verify-app-bundle.sh`. It verifies runtime hashes and exact inventory,
  requires architecture equality with the app executable, accepts only
  resolved `@loader_path`, bundle-resolved `@rpath`, resolved
  `@executable_path`, and Apple system dependencies, verifies every nested
  signature plus the deep app signature and Gatekeeper assessment, and runs
  the bundled ffprobe through a network-denying sandbox on a local fixture.
- Added runtime license auditing, the complete LGPL 2.1-or-later text, an exact
  reproducible source offer, notices for all ten locked components, explicit
  build options, security ownership, review/next-review dates, and an upgrade
  procedure. Existing advisory exceptions explicitly record no runtime impact.
- Added package commands and CI gates. Pull requests run lock/API/notices tests
  but do not download/build the runtime. Schedule, release-published, and
  manual events run an arm64/x86_64 matrix with locked runtime caching,
  build, stage, runtime/license verification, app build, and app-bundle audit.

## TDD evidence

- Initial brief gate:
  `cargo test -p viewer-video-mpv --test release_layout && bash scripts/video/verify-app-bundle.sh target/release/bundle/macos/Viewer.app`
  passed the layout test (1/1), then failed with exit 127 because the required
  app-bundle verifier did not yet exist.
- Initial packaging fixture run:
  `node --test scripts/video/release-packaging.test.mjs`
  produced five expected missing-script failures and one precondition pass.
- CI contract test failed on the pre-Task-13 workflow because schedule,
  release, and runtime matrix entries were absent.
- Exact source inventory test failed before the exact-file guard existed, then
  passed after the stage implementation rejected the injected unexpected file.
- Native verifier behavior tests initially failed on absent fixture binaries;
  after completing the fixture, the acceptance and host-link rejection paths
  both exercised the actual verifier script and passed.

## Fresh focused verification on frozen implementation

- `pnpm test:video:packaging`: 23 tests passed, 0 failed.
- `node --test scripts/repository-policy.test.mjs`: 25 tests passed, 0 failed.
- `cargo test -p viewer-video-mpv --test release_layout`: 1 passed, 0 failed.
- `cargo check -p viewer-desktop`: exit 0.
- `cargo fmt --check`: exit 0.
- `pnpm video:licenses:verify`: exit 0.
- `node scripts/check-dependency-exceptions.mjs`: 5 valid exceptions.
- `git diff --check`: exit 0.
- Ruby YAML parse of `.github/workflows/ci.yml`: exit 0.

## Pending release gate

Per the Task 13 execution contract, the one local staged runtime/app-bundle
build and audit plus final `pnpm verify` will run only after the single fresh
reviewer reports no open Critical/Important findings. No Task 14 work is
included.

## Review fix round 1

The unique reviewer reported 1 Critical and 4 Important findings. Each was
verified against the implementation and addressed:

- Release signing is now fail-closed: no ad-hoc Tauri identity is configured,
  staging requires a Developer ID Application identity and secure timestamp,
  the app verifier requires Developer ID authority plus timestamp before
  Gatekeeper assessment, and the runtime CI imports a secrets-backed identity,
  notarizes, staples, and then audits. This local host has zero valid signing
  identities and no notary credentials, so it cannot falsely pass this gate.
- FFmpeg now locks, builds, and verifies `--disable-devices` and
  `--disable-avdevice`, with the options synchronized to both notices and
  dependency health.
- `@rpath` now must resolve through an actual LC_RPATH to a canonical path
  inside the app; only a dylib's own install ID is accepted without a search
  path. Host-rpath behavior has a regression test.
- Staging preflights every target non-directory entry and all directories,
  rejects symlinks before `ditto`, and retains exact post-copy validation. A
  behavioral target-symlink regression test preserves the outside referent.
- The runtime crate exports the single schema constant used by manifest
  validation; `build.rs` embeds the reviewed schema, and app setup consumes and
  checks it against the loader constant.

Fresh covering verification after the fixes:

- `node --test scripts/video/release-packaging.test.mjs`: 12 passed, 0 failed.
- `node --test scripts/video/runtime-verifier.test.mjs scripts/video/runtime-lock.test.mjs`: 13 passed, 0 failed.
- `cargo test -p viewer-video-mpv --test runtime_manifest`: 8 passed, 0 failed.
- `cargo check -p viewer-desktop`: exit 0.
- `cargo fmt --check`: exit 0 after mechanical formatting.
- `node --test scripts/repository-policy.test.mjs`: 25 passed, 0 failed.
- `pnpm video:licenses:verify`, CI YAML parse, and `git diff --check`: exit 0.

### Fix round 2

The same reviewer found two remaining Important issues. The app verifier now
reads the inspected Mach-O's actual `LC_ID_DYLIB` with `otool -D` and grants a
self-install-name exception only for an exact match; a same-basename executable
with no install ID is rejected by a new behavioral test. Generic `pnpm tauri`
is again credential-free; release staging is scoped to Tauri's
`beforeBuildCommand`, while `beforeDevCommand` remains the ordinary UI dev
server. Fresh covering verification: packaging fixtures 14 passed / 0 failed,
`cargo check -p viewer-desktop`, `cargo fmt --check`, and `git diff --check`
all exited 0.

### Fix round 3

The remaining reviewer finding concerned mutation sensitivity in the
same-basename executable regression. The fixture now emits only an Apple system
dependency for every other runtime binary and exposes unresolved
`@rpath/ffprobe` only while inspecting `bin/ffprobe`. The old basename-only
implementation would therefore pass the entire fixture, while the corrected
`LC_ID_DYLIB` implementation rejects it. Fresh packaging fixtures: 14 passed,
0 failed.

## Local release gate — deferred by explicit user ruling

After the same reviewer approved the implementation with exactly 0 Critical
and 0 Important findings, the one authorized local gate was executed:

```text
pnpm tauri build && pnpm video:runtime:verify && pnpm video:licenses:verify &&
scripts/video/verify-app-bundle.sh target/release/bundle/macos/Viewer.app
```

It exited 1 in Tauri's `beforeBuildCommand` before any app bundle was built:

```text
> pnpm video:runtime:stage
staged runtime lock differs from the reviewed lock
beforeBuildCommand `pnpm video:runtime:stage && pnpm --dir ui build` failed
```

The only available arm64 runtime was the older Task 1 artifact and predates the
review-mandated `--disable-devices` / `--disable-avdevice` lock contract. The
verify-before-copy stage correctly rejected it. Rebuilding that source artifact
would not unblock this host's release gate: `security find-identity -v -p
codesigning` reports `0 valid identities found`, and `xcrun notarytool` reports
that credentials are required. The implementation intentionally requires a
Developer ID Application identity, secure timestamp, notarization, staple, and
successful `spctl`; no ad-hoc or bypass path was used.

The user explicitly ruled: "defer the release signing gate; commit only the
reviewed code." Accordingly, no `pnpm tauri build`, signed local app build,
notarization/stapling, or `spctl` assessment was run or claimed as passing.

Fresh non-release verification after that ruling:

- `pnpm verify`: exit 0. This includes policy tests (29/29), video packaging
  tests (27/27), UI tests (105 files; 935 passed, 1 skipped), UI build, Rust
  format/clippy/workspace tests, and security/license checks.

The release gate remains deferred. To resume it, provide/authorize a Developer
ID Application certificate plus Apple notarization credentials and rebuild the
reviewed runtime from an empty work directory before the release-gate retry.
