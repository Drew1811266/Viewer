# Viewer

Viewer is a local-first macOS image and text review application under active development. Viewer 0.1 is a development-stage baseline, not a release or delivery target.

## Current scope

Viewer supports local review of JPG, JPEG, PNG, Markdown, and TXT files. It does not add cloud, account, upload, update, shell, or broad filesystem capabilities.

## Architecture

The active architecture is governed by [ADR 0005](docs/adr/0005-continuous-development-governance.md), the [product specification](docs/PRODUCT_SPEC.md), and the [engineering optimization roadmap](docs/superpowers/plans/2026-07-23-viewer-engineering-optimization-roadmap.md). The implementation separates [Domain](crates/viewer-domain), [Application](crates/viewer-application), [Infrastructure](crates/viewer-infrastructure), [Platform/macOS](crates/viewer-platform-macos), [Tauri/Desktop](src-tauri), and [UI](ui) concerns.

## Requirements

The pinned development baseline is Apple Silicon macOS 13 or newer, Xcode Command Line Tools, Node.js 24.18.0, Corepack-managed pnpm 10.0.0, and Rust 1.97.0 with `rustfmt` and `clippy`.

## Install

Enable Corepack, then install the locked dependency set:

```bash
corepack enable
corepack prepare pnpm@10.0.0 --activate
pnpm install --frozen-lockfile
```

Install the pinned Rust coverage developer tool separately from the product dependency graph:

```bash
cargo install cargo-llvm-cov --version 0.8.7 --locked
```

## Develop

Run the desktop development workflow from the repository root:

```bash
pnpm tauri dev
```

## Verify

Run the repository verification entry command before requesting review:

```bash
pnpm verify:clean
```

The command snapshots the pre-existing porcelain entry set and fails if verification adds or removes an entry.

Continuous quality reports are opt-in trend evidence, not a release threshold. Run the UI and Rust coverage reports followed by the architecture health report:

```bash
pnpm quality:report
```

The separately installed `cargo-llvm-cov` tool is required only for
`pnpm quality:report`; ordinary `pnpm verify` remains deterministic and does
not require coverage tooling.

## Repository map

- [Domain](crates/viewer-domain), [Application](crates/viewer-application), [Infrastructure](crates/viewer-infrastructure), and [Platform/macOS](crates/viewer-platform-macos) contain the Rust layers.
- [Tauri/Desktop](src-tauri) contains the desktop bridge, and [UI](ui) contains the React interface.
- [Integration tests](tests) exercise repository-level behavior.
- [Architecture decision records](docs/adr), the [product specification](docs/PRODUCT_SPEC.md), and the [engineering optimization roadmap](docs/superpowers/plans/2026-07-23-viewer-engineering-optimization-roadmap.md) are the repository's active documentation sources.

## Active documentation

Start with [ADR 0005](docs/adr/0005-continuous-development-governance.md) for the active development-stage governance model. Use the [product specification](docs/PRODUCT_SPEC.md) and [engineering optimization roadmap](docs/superpowers/plans/2026-07-23-viewer-engineering-optimization-roadmap.md) as the active scope and planning sources; historical evidence remains documented separately.

## License

Viewer is licensed under [Apache-2.0](LICENSE).
