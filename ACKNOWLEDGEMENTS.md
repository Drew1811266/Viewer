# Acknowledgements

Viewer is independently designed and implemented. The project learns from the engineering experience and product patterns of the open-source community without copying complete application implementations.

Viewer's project license has not yet been selected. This acknowledgement records the direct tools and dependencies used by the current implementation; the manifests and lockfiles remain the source of truth for exact versions.

## Direct tools and dependencies

- [Tauri](https://github.com/tauri-apps/tauri) — desktop shell, JavaScript API, Rust build integration, CLI, webview boundary, and macOS packaging.
- [React](https://github.com/facebook/react) and React DOM — user-interface rendering.
- [Vite](https://github.com/vitejs/vite) and `@vitejs/plugin-react` — frontend development and production builds.
- [TypeScript](https://github.com/microsoft/TypeScript), Node.js, pnpm, and the React/Node type declarations — frontend language and toolchain.
- [Rust](https://github.com/rust-lang/rust) and Cargo — native application language and build toolchain. The direct Rust crates currently include `async-trait`, `blake3`, `block2`, `getrandom`, `libc` (native filesystem operations and benchmark RSS sampling), the `objc2` Foundation/Core Foundation/Core Graphics/Image I/O/Quick Look Thumbnailing bindings, `rusqlite` with bundled SQLite, `serde`, `serde_json` (tests and benchmark reports), `tempfile` (tests), `thiserror`, `tokio`, `trash` (system Trash adapter), and `uuid`.
- [Vitest](https://github.com/vitest-dev/vitest), [Testing Library](https://github.com/testing-library/react-testing-library), `jest-dom`, and jsdom — frontend unit and DOM testing.

## Research and product inspiration (not code dependencies)

The projects in this section were reviewed for engineering and interaction patterns. Viewer does not depend on or incorporate their source code.

- [Yazi](https://github.com/sxyazi/yazi) — asynchronous task scheduling, priorities, cancellation, preloading, and file-operation feedback.
- [Spacedrive](https://github.com/spacedriveapp/spacedrive) — content identity, transactional action previews, and treating file operations as durable tasks.
- [digiKam](https://github.com/KDE/digikam) — separation of durable metadata, search data, fingerprints, and rebuildable thumbnails.
- [Tiefsee4](https://github.com/hbl917070/Tiefsee4), [Oculante](https://github.com/woelper/oculante), [qView](https://github.com/jurplel/qView), and [nomacs](https://github.com/nomacs/nomacs) — image-viewer interaction patterns and edge-case research.

Third-party license notices will be maintained separately in `THIRD_PARTY_NOTICES.md` when that artifact is introduced. No project license is implied by this acknowledgement.
