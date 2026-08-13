# Acknowledgements

Viewer is independently designed and implemented. The project learns from the engineering experience and product patterns of the open-source community without copying complete application implementations.

Viewer is licensed under Apache License 2.0. This acknowledgement distinguishes the tools and packages used by the implementation from projects consulted only for engineering or interaction patterns; the manifests, lockfiles and `THIRD_PARTY_NOTICES.md` remain the source of truth for exact build dependencies.

## Direct tools and dependencies

- [Tauri](https://github.com/tauri-apps/tauri) — desktop shell, JavaScript API, Rust build integration, CLI, webview boundary, and macOS packaging.
- [React](https://github.com/facebook/react) and React DOM — user-interface rendering.
- [Vite](https://github.com/vitejs/vite) and `@vitejs/plugin-react` — frontend development and production builds.
- [TypeScript](https://github.com/microsoft/TypeScript), Node.js, pnpm, and the React/Node type declarations — frontend language and toolchain.
- [Rust](https://github.com/rust-lang/rust) and Cargo — native application language and build toolchain. The direct Rust crates currently include `async-trait`, `blake3`, `block2`, `getrandom`, `libc` (native filesystem operations and benchmark RSS sampling), `nucleo-matcher` (Unicode fuzzy filename/path scoring), `notify` and `notify-debouncer-full` (debounced filesystem hints), the `objc2` Foundation/Core Foundation/Core Graphics/Image I/O/Quick Look Thumbnailing bindings, `rusqlite` with bundled SQLite, `serde`, `serde_json` (tests and benchmark reports), `tempfile` (tests), `thiserror`, `tokio`, `trash` (system Trash adapter), `uuid`, and `walkdir` (bounded project traversal).
- [Vitest](https://github.com/vitest-dev/vitest), [Testing Library](https://github.com/testing-library/react-testing-library), `jest-dom`, and jsdom — frontend unit and DOM testing.
- ViewerVideoRuntime — the Viewer-bundled, source-built LGPL video stack based on
  libmpv/mpv 0.41.0, FFmpeg 8.0, libplacebo 6.338.2, libass 0.17.4, and the
  exact dependency closure recorded in `scripts/video/runtime.lock.json`.
  Complete notices, license texts, build options, digests, and reproducible
  source acquisition commands ship with Viewer.

## Research and product inspiration (not code dependencies)

The projects in this section were reviewed for engineering and interaction patterns. Viewer does not depend on or incorporate their source code.

- [Yazi](https://github.com/sxyazi/yazi) — asynchronous task scheduling, priorities, cancellation, preloading, and file-operation feedback.
- [Spacedrive](https://github.com/spacedriveapp/spacedrive) — content identity, transactional action previews, and treating file operations as durable tasks.
- [digiKam](https://github.com/KDE/digikam) — separation of durable metadata, search data, fingerprints, and rebuildable thumbnails.
- [Tiefsee4](https://github.com/hbl917070/Tiefsee4), [Oculante](https://github.com/woelper/oculante), [qView](https://github.com/jurplel/qView), and [nomacs](https://github.com/nomacs/nomacs) — image-viewer interaction patterns and edge-case research.
- [CrabNebula drag-rs](https://github.com/crabnebula-dev/drag-rs) — reference for the public AppKit pattern of starting a native file drag from the window content view with a synthesized mouse-drag event. Viewer does not depend on or copy the crate; its adapter keeps Viewer-specific entity validation, identity checks, error boundaries and pasteboard construction.

Exact versions, upstream links, license expressions and distribution status for direct build dependencies are maintained in `THIRD_PARTY_NOTICES.md`. The research projects above are not linked code dependencies and their inclusion does not imply endorsement of Viewer.
