# Acknowledgements

Viewer is independently designed and implemented. The project learns from the engineering experience and product patterns of the open-source community without copying complete application implementations.

This file is a draft. An entry becomes final only after the corresponding architecture is implemented and verified.

## Technical foundations

- [Tauri](https://github.com/tauri-apps/tauri) — desktop shell, Rust core, webview boundary, and macOS packaging.
- [rusqlite](https://github.com/rusqlite/rusqlite) and SQLite — project metadata, session indexes, and full-text search.
- [notify](https://github.com/notify-rs/notify) — filesystem notification model and event debouncing.
- [TanStack Virtual](https://github.com/TanStack/virtual) — virtualized folder, thumbnail, and search-result views.
- [trash-rs](https://github.com/Byron/trash-rs) — integration with the operating system trash.
- [nucleo](https://github.com/helix-editor/nucleo) — Unicode-aware fuzzy-matching research and candidate implementation.

## Architecture and product inspiration

- [Yazi](https://github.com/sxyazi/yazi) — asynchronous task scheduling, priorities, cancellation, preloading, and file-operation feedback.
- [Spacedrive](https://github.com/spacedriveapp/spacedrive) — content identity, transactional action previews, and treating file operations as durable tasks. Viewer does not copy Spacedrive source code.
- [digiKam](https://github.com/KDE/digikam) — separation of durable metadata, search data, fingerprints, and rebuildable thumbnails.
- [Tiefsee4](https://github.com/hbl917070/Tiefsee4), [Oculante](https://github.com/woelper/oculante), [qView](https://github.com/jurplel/qView), and [nomacs](https://github.com/nomacs/nomacs) — image-viewer interaction patterns and edge-case research.

Actual third-party dependencies and their license notices will be maintained separately in `THIRD_PARTY_NOTICES.md` once implementation versions are locked.

