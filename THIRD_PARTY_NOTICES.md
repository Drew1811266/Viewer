# Viewer 0.1 third-party notices

This inventory records the direct third-party dependencies in the Viewer 0.1 locked build graph. Exact versions come from `Cargo.lock` and `pnpm-lock.yaml`; manifests remain the machine-readable source of truth. Viewer does not modify the listed packages. License expressions are copied from package metadata and were reviewed with `cargo-deny` plus `pnpm licenses list` on 2026-07-16; `pnpm audit` separately checks known vulnerabilities.

“Distributed” means code from the dependency is linked into the native application or bundled into the production frontend. Build and test tools are listed separately even when they do not enter the shipped application.

## Rust runtime dependencies

| Package | Version | License | Viewer purpose | Upstream | Distributed |
| --- | --- | --- | --- | --- | --- |
| `ammonia` | 4.1.4 | MIT OR Apache-2.0 | Sanitize rendered Markdown HTML at the Tauri boundary | [rust-ammonia/ammonia](https://github.com/rust-ammonia/ammonia) | Yes |
| `async-trait` | 0.1.89 | MIT OR Apache-2.0 | Async Application port traits and adapters | [dtolnay/async-trait](https://github.com/dtolnay/async-trait) | Yes, as expanded code |
| `blake3` | 1.8.5 | CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception | File-operation evidence and recovery fingerprints | [BLAKE3-team/BLAKE3](https://github.com/BLAKE3-team/BLAKE3) | Yes |
| `block2` | 0.6.2 | MIT | Objective-C completion blocks for Quick Look | [madsmtm/objc2](https://github.com/madsmtm/objc2) | Yes |
| `dispatch2` | 0.3.1 | Zlib OR Apache-2.0 OR MIT | Schedule native video feasibility rendering work on the macOS main dispatch queue | [madsmtm/objc2](https://github.com/madsmtm/objc2) | Yes, when the video-feasibility feature is enabled |
| `encoding_rs` | 0.8.35 | (Apache-2.0 OR MIT) AND BSD-3-Clause | Strict UTF-8, UTF-16 and GB18030 text-preview decoding | [hsivonen/encoding_rs](https://github.com/hsivonen/encoding_rs) | Yes |
| `getrandom` | 0.4.3 | MIT OR Apache-2.0 | Random transaction identifiers and protected temporary names | [rust-random/getrandom](https://github.com/rust-random/getrandom) | Yes |
| `libc` | 0.2.186 | MIT OR Apache-2.0 | Native filesystem primitives and memory measurements | [rust-lang/libc](https://github.com/rust-lang/libc) | Yes |
| `libloading` | 0.8.9 | ISC | Load the exact Viewer-bundled libmpv dynamic library and reviewed symbols | [nagisa/rust_libloading](https://github.com/nagisa/rust_libloading) | Yes |
| `nucleo-matcher` | 0.3.1 | MPL-2.0 | Unicode-aware fuzzy filename and path scoring; consumed unmodified | [helix-editor/nucleo](https://github.com/helix-editor/nucleo) | Yes |
| `notify` | 8.2.0 | CC0-1.0 | macOS filesystem event source | [notify-rs/notify](https://github.com/notify-rs/notify) | Yes |
| `notify-debouncer-full` | 0.7.0 | MIT OR Apache-2.0 | Coalesce filesystem hints before scoped rescans | [notify-rs/notify](https://github.com/notify-rs/notify) | Yes |
| `objc2` | 0.6.4 | MIT | Objective-C runtime bindings | [madsmtm/objc2](https://github.com/madsmtm/objc2) | Yes |
| `objc2-app-kit` | 0.3.2 | Zlib OR Apache-2.0 OR MIT | Native Finder drag sessions and macOS application lifecycle integration | [madsmtm/objc2](https://github.com/madsmtm/objc2) | Yes |
| `objc2-core-foundation` | 0.3.2 | Zlib OR Apache-2.0 OR MIT | Core Foundation types for image APIs | [madsmtm/objc2](https://github.com/madsmtm/objc2) | Yes |
| `objc2-core-graphics` | 0.3.2 | Zlib OR Apache-2.0 OR MIT | Core Graphics image representation | [madsmtm/objc2](https://github.com/madsmtm/objc2) | Yes |
| `objc2-foundation` | 0.3.2 | MIT | Foundation URLs, strings, objects and errors | [madsmtm/objc2](https://github.com/madsmtm/objc2) | Yes |
| `objc2-image-io` | 0.3.2 | Zlib OR Apache-2.0 OR MIT | JPG/PNG metadata, fallback thumbnails and full previews | [madsmtm/objc2](https://github.com/madsmtm/objc2) | Yes |
| `objc2-quick-look-thumbnailing` | 0.3.2 | Zlib OR Apache-2.0 OR MIT | Primary macOS thumbnail generation | [madsmtm/objc2](https://github.com/madsmtm/objc2) | Yes |
| `pulldown-cmark` | 0.13.4 | MIT | Parse Markdown into a filtered event stream before HTML sanitization | [raphlinus/pulldown-cmark](https://github.com/raphlinus/pulldown-cmark) | Yes |
| `rusqlite` | 0.40.1 | MIT | Portable metadata and rebuildable session/search indexes | [rusqlite/rusqlite](https://github.com/rusqlite/rusqlite) | Yes |
| `serde` | 1.0.228 | MIT OR Apache-2.0 | Typed serialization at IPC and persistence boundaries | [serde-rs/serde](https://github.com/serde-rs/serde) | Yes |
| `sha2` | 0.10.9 | MIT OR Apache-2.0 | Verify bundled ffprobe runtime integrity and immutable video-fixture identities | [RustCrypto/hashes](https://github.com/RustCrypto/hashes) | Yes |
| `thiserror` | 2.0.18 | MIT OR Apache-2.0 | Structured domain, application and adapter errors | [dtolnay/thiserror](https://github.com/dtolnay/thiserror) | Yes |
| `tokio` | 1.52.3 | MIT | Async tasks, cancellation, synchronization and timers | [tokio-rs/tokio](https://github.com/tokio-rs/tokio) | Yes |
| `tokio-util` | 0.7.18 | MIT | Cancellation tokens for bounded video-probe process and session lifecycles | [tokio-rs/tokio](https://github.com/tokio-rs/tokio) | Yes |
| `trash` | 5.2.6 | MIT | Move user-selected files to the macOS Trash | [ArturKovacs/trash](https://github.com/ArturKovacs/trash) | Yes |
| `uuid` | 1.24.0 | Apache-2.0 OR MIT | Stable session, file, task and operation identifiers | [uuid-rs/uuid](https://github.com/uuid-rs/uuid) | Yes |
| `walkdir` | 2.5.0 | Unlicense OR MIT | Bounded recursive project traversal | [BurntSushi/walkdir](https://github.com/BurntSushi/walkdir) | Yes |
| `tauri` | 2.11.5 | Apache-2.0 OR MIT | Native desktop shell, WebView/IPC boundary and application runtime | [tauri-apps/tauri](https://github.com/tauri-apps/tauri) | Yes |
| `tauri-plugin-dialog` | 2.7.1 | Apache-2.0 OR MIT | Native project-folder chooser with open-only capability | [tauri-apps/plugins-workspace](https://github.com/tauri-apps/plugins-workspace) | Yes |

## Frontend runtime dependencies

| Package | Version | License | Viewer purpose | Upstream | Distributed |
| --- | --- | --- | --- | --- | --- |
| `@tauri-apps/api` | 2.11.1 | Apache-2.0 OR MIT | Typed frontend access to the restricted Tauri API | [tauri-apps/tauri](https://github.com/tauri-apps/tauri) | Yes |
| `@tauri-apps/plugin-dialog` | 2.7.1 | MIT OR Apache-2.0 | Frontend binding for the native project-folder chooser | [tauri-apps/plugins-workspace](https://github.com/tauri-apps/plugins-workspace) | Yes |
| `react` | 19.2.7 | MIT | User-interface rendering and state composition | [facebook/react](https://github.com/facebook/react) | Yes |
| `react-dom` | 19.2.7 | MIT | React DOM/WebView renderer | [facebook/react](https://github.com/facebook/react) | Yes |

## Build dependencies and tools

| Package | Version | License | Viewer purpose | Upstream | Distributed |
| --- | --- | --- | --- | --- | --- |
| `tauri-build` | 2.6.3 | Apache-2.0 OR MIT | Generate Tauri build metadata and resources | [tauri-apps/tauri](https://github.com/tauri-apps/tauri) | No, build only |
| `@tauri-apps/cli` | 2.11.4 | Apache-2.0 OR MIT | Development and macOS packaging CLI | [tauri-apps/tauri](https://github.com/tauri-apps/tauri) | No, build only |
| `@biomejs/biome` | 2.5.5 | MIT | Strict frontend linting and deterministic formatting | [biomejs/biome](https://github.com/biomejs/biome) | No, build only |
| `@types/node` | 24.13.3 | MIT | Node.js type declarations | [DefinitelyTyped/DefinitelyTyped](https://github.com/DefinitelyTyped/DefinitelyTyped) | No, types only |
| `@types/react` | 19.2.17 | MIT | React type declarations | [DefinitelyTyped/DefinitelyTyped](https://github.com/DefinitelyTyped/DefinitelyTyped) | No, types only |
| `@types/react-dom` | 19.2.3 | MIT | React DOM type declarations | [DefinitelyTyped/DefinitelyTyped](https://github.com/DefinitelyTyped/DefinitelyTyped) | No, types only |
| `@vitejs/plugin-react` | 6.0.3 | MIT | React transform for Vite production builds | [vitejs/vite-plugin-react](https://github.com/vitejs/vite-plugin-react) | No, build only |
| `typescript` | 6.0.3 | Apache-2.0 | Static checking and JavaScript compilation | [microsoft/TypeScript](https://github.com/microsoft/TypeScript) | No, build only |
| `vite` | 8.1.5 | MIT | Frontend development and production bundling | [vitejs/vite](https://github.com/vitejs/vite) | No, build only |

## Test-only dependencies

| Package | Version | License | Viewer purpose | Upstream | Distributed |
| --- | --- | --- | --- | --- | --- |
| `serde_json` | 1.0.150 | MIT OR Apache-2.0 | Test fixtures, benchmark reports and security configuration tests | [serde-rs/json](https://github.com/serde-rs/json) | No, test only |
| `tempfile` | 3.27.0 | MIT OR Apache-2.0 | Isolated filesystem and crash-recovery tests | [Stebalien/tempfile](https://github.com/Stebalien/tempfile) | No, test only |
| `@testing-library/jest-dom` | 6.9.1 | MIT | DOM assertions | [testing-library/jest-dom](https://github.com/testing-library/jest-dom) | No, test only |
| `@testing-library/react` | 16.3.2 | MIT | React component tests | [testing-library/react-testing-library](https://github.com/testing-library/react-testing-library) | No, test only |
| `@vitest/coverage-v8` | 4.1.10 | MIT | Frontend V8 coverage collection and reporting | [vitest-dev/vitest](https://github.com/vitest-dev/vitest) | No, test only |
| `jsdom` | 29.1.1 | MIT | Test DOM runtime | [jsdom/jsdom](https://github.com/jsdom/jsdom) | No, test only |
| `playwright` | 1.62.1 | Apache-2.0 | Drive the non-shipping exhaustive browser visual-acceptance harness | [microsoft/playwright](https://github.com/microsoft/playwright) | No, test only |
| `vitest` | 4.1.10 | MIT | Frontend test runner | [vitest-dev/vitest](https://github.com/vitest-dev/vitest) | No, test only |

`viewer-domain`, `viewer-application`, `viewer-infrastructure`, `viewer-platform-macos`, `viewer-test-support` and `viewer-desktop` are internal Viewer workspace packages licensed under Apache-2.0, not third-party dependencies.

## Bundled and operating-system components

### ViewerVideoRuntime

Viewer ships the following unmodified source-built video components inside
`ViewerVideoRuntime`. All non-Apple runtime dependencies are compiled from the
reviewed archives below. Static dependencies are incorporated into libmpv or
the bundled FFmpeg tools; libmpv itself remains dynamically replaceable.

| Component | Version | License | Official source archive | SHA-256 | Enabled/reviewed build options | Security owner | Review date | Upgrade procedure |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `pkgconf` | 2.5.1 | ISC | https://github.com/pkgconf/pkgconf/archive/refs/tags/pkgconf-2.5.1.tar.gz | `79721badcad1987dead9c3609eb4877ab9b58821c06bdacb824f2c8897c11f2a` | static library metadata tool; tests disabled | Viewer maintainers | 2026-08-12 | Follow the runtime upgrade procedure in `docs/quality/DEPENDENCY_HEALTH.md`. |
| `freetype` | 2.13.3 | FTL | https://github.com/freetype/freetype/archive/refs/tags/VER-2-13-3.tar.gz | `bc5c898e4756d373e0d991bab053036c5eb2aa7c0d5c67e8662ddc6da40c4103` | static; system zlib; optional Brotli, bzip2, HarfBuzz and PNG disabled | Viewer maintainers | 2026-08-12 | Follow the runtime upgrade procedure in `docs/quality/DEPENDENCY_HEALTH.md`. |
| `fribidi` | 1.0.16 | LGPL-2.1-or-later | https://github.com/fribidi/fribidi/archive/refs/tags/v1.0.16.tar.gz | `5a1d187a33daa58fcee2ad77f0eb9d136dd6fa4096239199ba31e850d397e8a8` | static; deprecated API, docs, binaries and tests disabled | Viewer maintainers | 2026-08-12 | Follow the runtime upgrade procedure in `docs/quality/DEPENDENCY_HEALTH.md`. |
| `harfbuzz` | 10.4.0 | MIT | https://github.com/harfbuzz/harfbuzz/archive/refs/tags/10.4.0.tar.gz | `0d25a3f74af4e8744700ac19050af5a80ae330378a5802a5cd71e523bb6fda1f` | static minimal shaping build; GLib, Cairo, ICU, CoreText and utilities disabled | Viewer maintainers | 2026-08-12 | Follow the runtime upgrade procedure in `docs/quality/DEPENDENCY_HEALTH.md`. |
| `libass` | 0.17.4 | ISC | https://github.com/libass/libass/archive/refs/tags/0.17.4.tar.gz | `c287d180d93dc9c9021872574b618ac49027e84cc90e1289318b1ee68bb42251` | static; CoreText enabled; fontconfig, ASM, tests and profiling disabled | Viewer maintainers | 2026-08-12 | Follow the runtime upgrade procedure in `docs/quality/DEPENDENCY_HEALTH.md`. |
| `ffmpeg` (FFmpeg) | 8.0 (`n8.0`) | LGPL-2.1-or-later | https://github.com/FFmpeg/FFmpeg/archive/refs/tags/n8.0.tar.gz | `dd4030dbfdc34d9ff255a116bdd1caade42500ac2981efa27f8b151cc54c7b9e` | `--disable-gpl --disable-network --disable-nonfree --disable-ffplay --disable-devices --disable-avdevice --enable-zlib --enable-encoder=png`; VideoToolbox and AudioToolbox enabled | Viewer maintainers | 2026-08-12 | Follow the runtime upgrade procedure in `docs/quality/DEPENDENCY_HEALTH.md`. |
| `fast_float` | 2b2395f9ac836ffca6404424bcc252bff7aa80e4 | Apache-2.0 | https://github.com/fastfloat/fast_float/archive/2b2395f9ac836ffca6404424bcc252bff7aa80e4.tar.gz | `230d20e4e4ac1f6a9df92c4d746c6ec536cdb0c085bc8635d4b88cead5dc22cb` | exact libplacebo submodule source; header-only | Viewer maintainers | 2026-08-12 | Follow the runtime upgrade procedure in `docs/quality/DEPENDENCY_HEALTH.md`. |
| `vulkan-headers` | d732b2de303ce505169011d438178191136bfb00 | Apache-2.0 | https://github.com/KhronosGroup/Vulkan-Headers/archive/d732b2de303ce505169011d438178191136bfb00.tar.gz | `570f9ae1e65466dbaf5fcab667abd079dd0a61c4ab86cf535efd492bf70a5b74` | exact libplacebo submodule source; Vulkan runtime disabled | Viewer maintainers | 2026-08-12 | Follow the runtime upgrade procedure in `docs/quality/DEPENDENCY_HEALTH.md`. |
| `libplacebo` | 6.338.2 (`v6.338.2`) | LGPL-2.1-or-later | https://github.com/haasn/libplacebo/archive/refs/tags/v6.338.2.tar.gz | `2f1e624e09d72a8c9db70f910f7560e764a1c126dae42acc5b3bcef836a7aec6` | static minimal build; demos, tests, Vulkan, OpenGL, D3D11 and optional libraries disabled | Viewer maintainers | 2026-08-12 | Follow the runtime upgrade procedure in `docs/quality/DEPENDENCY_HEALTH.md`. |
| `mpv` / `libmpv` | 0.41.0 (`v0.41.0`, commit `41f6a64`) | LGPL-2.1-or-later | https://github.com/mpv-player/mpv/archive/refs/tags/v0.41.0.tar.gz | `ee21092a5ee427353392360929dc64645c54479aefdb5babc5cfbb5fad626209` | `-Dgpl=false -Dcplayer=false -Dlibmpv=true`; isolated Cocoa/OpenGL/VideoToolbox library build | Viewer maintainers | 2026-08-12 | Follow the runtime upgrade procedure in `docs/quality/DEPENDENCY_HEALTH.md`. |

The complete GNU Lesser General Public License version 2.1-or-later text is
distributed at `scripts/video/LGPL-2.1-or-later.txt` and inside the runtime
license directory. The exact reproducible source offer is
`scripts/video/source-offer.txt`.

| Component | Source/license status | Viewer use | Distribution status |
| --- | --- | --- | --- |
| Lucide Icons 1.27.0 | Static SVG subset from [lucide-icons/lucide](https://github.com/lucide-icons/lucide/tree/1.27.0), licensed under ISC with MIT notices retained for Feather-derived icons; full text is bundled at `ui/src/assets/icons/lucide/LICENSE.txt` | Cross-platform Viewer action and state icons | Copied into the frontend bundle |
| SQLite | [SQLite is in the public domain](https://www.sqlite.org/copyright.html); bundled through `rusqlite`/`libsqlite3-sys` | Metadata, operation journal, session index and FTS5 | Compiled into the native binary |
| Quick Look Thumbnailing, Image I/O, Core Graphics, Core Foundation, Foundation and ColorSync | Apple macOS system frameworks governed by the macOS SDK and operating-system terms | Thumbnailing, decode, color, metadata and native types | Dynamically supplied by macOS; not copied into Viewer |
| WKWebView | Apple macOS system framework | Tauri frontend host | Dynamically supplied by macOS; not copied into Viewer |

## Distribution notes

- Viewer itself is licensed under Apache License 2.0; see `LICENSE`.
- The npm transitive graph is mechanically restricted to the reviewed MIT, MIT-0, Apache-2.0, Apache-2.0 OR MIT, MIT OR Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, CC0-1.0, MPL-2.0 and BlueOak-1.0.0 expressions. An unknown or unreviewed expression fails `scripts/check-npm-licenses.mjs`.
- The npm MPL-2.0 entries are unmodified `lightningcss` build packages and the BlueOak-1.0.0 entry is the permissively licensed `lru-cache` build/test transitive dependency; none is bundled into Viewer's React production chunk.
- Source distributions must retain the upstream license and attribution files shipped by dependencies. Binary distributions must include notices required by the selected upstream license terms.
- `nucleo-matcher` remains an unmodified MPL-2.0 library. Its source and license remain available at the exact upstream/version listed above; Viewer source files are not derived from it.
- Architecture and product references that do not enter the build are intentionally documented in `ACKNOWLEDGEMENTS.md`, not in this dependency inventory.
