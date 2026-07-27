# Viewer

Viewer is a local-first macOS image and text review application under active development. Viewer 0.1 is a development-stage baseline, not a release or delivery target.

## Current scope

Viewer supports local review of JPG, JPEG, PNG, Markdown, and TXT files. It does not add cloud, account, upload, update, shell, or broad filesystem capabilities.

## Architecture

The current documentation entry point is the [documentation index](docs/README.md). Architecture governance comes from [ADR 0005](docs/adr/0005-continuous-development-governance.md) and the [engineering optimization governance design](docs/superpowers/specs/2026-07-23-viewer-engineering-optimization-governance-design.md), while product behavior and scope come from the [product specification](docs/PRODUCT_SPEC.md). The implementation separates [Domain](crates/viewer-domain), [Application](crates/viewer-application), [Infrastructure](crates/viewer-infrastructure), [Platform/macOS](crates/viewer-platform-macos), [Tauri/Desktop](src-tauri), and [UI](ui) concerns.

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

The separately installed `cargo-llvm-cov` is required for Rust coverage
generation both directly through `pnpm coverage:rust` and transitively through
`pnpm quality:report`. It is not required for ordinary `pnpm verify` or
`pnpm verify:clean`.

## Repository map

- [Domain](crates/viewer-domain), [Application](crates/viewer-application), [Infrastructure](crates/viewer-infrastructure), and [Platform/macOS](crates/viewer-platform-macos) contain the Rust layers.
- [Tauri/Desktop](src-tauri) contains the desktop bridge, and [UI](ui) contains the React interface.
- [Integration tests](tests) exercise repository-level behavior.
- The [documentation index](docs/README.md) identifies the current architecture decisions, product specification, engineering governance design, and dependency-governance sources.

## Active documentation

Start with the [documentation index](docs/README.md), [ADR 0005](docs/adr/0005-continuous-development-governance.md), and the [engineering optimization governance design](docs/superpowers/specs/2026-07-23-viewer-engineering-optimization-governance-design.md) for the active development-stage governance model. Use the [product specification](docs/PRODUCT_SPEC.md) for current product behavior and scope.

The [engineering optimization roadmap](docs/superpowers/plans/2026-07-23-viewer-engineering-optimization-roadmap.md) is a historical implementation record, not a current source of governance or product scope.

## License

Viewer is licensed under [Apache-2.0](LICENSE).
