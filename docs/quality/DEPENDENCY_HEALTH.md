# Dependency Health

> Status: Active

This register classifies the duplicate-version warnings emitted by
`cargo deny --offline --locked check`. Duplicate versions remain informational:
they are reviewed here, not suppressed with patches or version overrides.

## Tauri/platform transitive

| Crate | Locked versions | Classification |
| --- | --- | --- |
| `base64` | 0.21.7, 0.22.1 | Both branches enter through Tauri (`swift-rs` and `plist`/`tauri-codegen`). |
| `bitflags` | 1.3.2, 2.13.1 | The older branch enters through Tauri code generation and PNG/ICO; the newer branch is shared by macOS/runtime dependencies. |
| `getrandom` | 0.3.4, 0.4.3 | Tauri retains 0.3 while current `tempfile` and `uuid` users retain 0.4. |
| `hashbrown` | 0.12.3, 0.17.1 | Tauri's `schemars`/`indexmap` branch retains 0.12; current `rusqlite` and `plist` branches use 0.17. |
| `indexmap` | 1.9.3, 2.14.0 | Tauri's `schemars` branch retains 1.x while current Tauri/plist and workspace transitive branches use 2.x. |
| `png` | 0.17.16, 0.18.1 | Tauri code generation (`ico`) retains 0.17 and the Tauri menu stack (`muda`) uses 0.18. |
| `thiserror` | 1.0.69, 2.0.18 | Tauri's `json-patch` branch retains 1.x; Tauri runtime and Viewer crates use 2.x. |
| `thiserror-impl` | 1.0.69, 2.0.18 | Procedural-macro pair corresponding to the two `thiserror` branches above. |

These groups are owned by the Tauri/platform graph. Revisit them when updating
Tauri, its plugins, `tauri-utils`, or the macOS adapter dependencies.

## Build-only

| Crate | Locked versions | Classification |
| --- | --- | --- |
| `toml` | 0.9.12+spec-1.1.0, 1.1.3+spec-1.1.0 | The older branch exists only under `cargo_toml > tauri-build`; the current branch is also used by Tauri utilities. |
| `toml_datetime` | 0.7.5+spec-1.1.0, 1.1.1+spec-1.1.0 | Follows the two TOML branches; the removable older branch is build-only. |
| `winnow` | 0.7.15, 1.0.4 | Follows the two TOML parser branches; the removable older branch is build-only. |

These warnings are caused by Tauri build tooling and do not justify production
dependency overrides. Revisit them with `tauri-build` and `cargo_toml` updates.

## Potentially direct-upgrade-removable

| Crate | Locked versions | Classification |
| --- | --- | --- |
| `cssparser` | 0.36.0, 0.37.0 | Tauri's `dom_query` branch uses 0.36 while direct `ammonia` uses 0.37. |
| `html5ever` | 0.38.0, 0.39.0 | Tauri's HTML-query branch uses 0.38 while direct `ammonia` uses 0.39. |
| `markup5ever` | 0.38.0, 0.39.0 | Paired parser dependency of the `html5ever` branches above. |

Re-evaluate these three together when upgrading the direct `ammonia`
dependency. Do not force convergence unless both Tauri's parser graph and
Viewer's sanitizer remain on compatible releases.

## Advisory exceptions

The machine-readable register is
[`dependency-exceptions.json`](dependency-exceptions.json). Every exception
must have an owner, review date, and removal condition, and must match an
advisory ignore in `deny.toml` in both directions. `RUSTSEC-2026-0213` is not
excepted: the lockfile uses patched `ammonia` 4.1.4.

The narrow `foldhash@0.2.0` Zlib license exception remains unchanged and must
not be broadened to unrelated crates.
