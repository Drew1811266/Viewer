# Dependency Health

> Status: Active

## Bundled video runtime ownership

Security owner: Viewer maintainers

Review date: 2026-08-12

Next review: 2026-11-12

Upgrade procedure: update one component at a time in
`scripts/video/runtime.lock.json`, verify the official archive digest and
license, reproduce both supported architecture builds from empty work
directories, review the complete `otool -L` closure and enabled options, update
`THIRD_PARTY_NOTICES.md` plus the source offer, run the packaging/license gates,
and accept the change only after both signed app bundles pass offline audit.

Every downloaded runtime source and every linked non-Apple dependency is owned
here. Components built statically remain listed because their code is
distributed inside libmpv or the FFmpeg tools.

| Component | Version | License | Source URL | Archive SHA-256 | Enabled build options | Security owner | Review date | Upgrade procedure |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| pkgconf | 2.5.1 | ISC | https://github.com/pkgconf/pkgconf/archive/refs/tags/pkgconf-2.5.1.tar.gz | `79721badcad1987dead9c3609eb4877ab9b58821c06bdacb824f2c8897c11f2a` | `-Dtests=disabled -Dwith-system-includedir=/usr/include -Dwith-system-libdir=/usr/lib` | Viewer maintainers | 2026-08-12 | Use the procedure above. |
| freetype | 2.13.3 | FTL | https://github.com/freetype/freetype/archive/refs/tags/VER-2-13-3.tar.gz | `bc5c898e4756d373e0d991bab053036c5eb2aa7c0d5c67e8662ddc6da40c4103` | `-Dbrotli=disabled -Dbzip2=disabled -Dharfbuzz=disabled -Dpng=disabled -Dtests=disabled -Dzlib=system` | Viewer maintainers | 2026-08-12 | Use the procedure above. |
| fribidi | 1.0.16 | LGPL-2.1-or-later | https://github.com/fribidi/fribidi/archive/refs/tags/v1.0.16.tar.gz | `5a1d187a33daa58fcee2ad77f0eb9d136dd6fa4096239199ba31e850d397e8a8` | `-Ddeprecated=false -Ddocs=false -Dbin=false -Dtests=false` | Viewer maintainers | 2026-08-12 | Use the procedure above. |
| harfbuzz | 10.4.0 | MIT | https://github.com/harfbuzz/harfbuzz/archive/refs/tags/10.4.0.tar.gz | `0d25a3f74af4e8744700ac19050af5a80ae330378a5802a5cd71e523bb6fda1f` | `-Dglib=disabled -Dgobject=disabled -Dcairo=disabled -Dchafa=disabled -Dicu=disabled -Dgraphite2=disabled -Dfreetype=disabled -Dcoretext=disabled -Dtests=disabled -Dintrospection=disabled -Ddocs=disabled -Dutilities=disabled` | Viewer maintainers | 2026-08-12 | Use the procedure above. |
| libass | 0.17.4 | ISC | https://github.com/libass/libass/archive/refs/tags/0.17.4.tar.gz | `c287d180d93dc9c9021872574b618ac49027e84cc90e1289318b1ee68bb42251` | `-Dtest=disabled -Dcompare=disabled -Dprofile=disabled -Dfuzz=disabled -Dcheckasm=disabled -Dfontconfig=disabled -Dcoretext=enabled -Dasm=disabled -Dlibunibreak=disabled` | Viewer maintainers | 2026-08-12 | Use the procedure above. |
| ffmpeg (FFmpeg) | 8.0 (`n8.0`) | LGPL-2.1-or-later | https://github.com/FFmpeg/FFmpeg/archive/refs/tags/n8.0.tar.gz | `dd4030dbfdc34d9ff255a116bdd1caade42500ac2981efa27f8b151cc54c7b9e` | `--disable-gpl --disable-network --disable-nonfree --disable-ffplay --disable-devices --disable-avdevice --enable-zlib --enable-encoder=png`; AudioToolbox and VideoToolbox enabled | Viewer maintainers | 2026-08-12 | Use the procedure above. |
| fast_float | 2b2395f9ac836ffca6404424bcc252bff7aa80e4 | Apache-2.0 | https://github.com/fastfloat/fast_float/archive/2b2395f9ac836ffca6404424bcc252bff7aa80e4.tar.gz | `230d20e4e4ac1f6a9df92c4d746c6ec536cdb0c085bc8635d4b88cead5dc22cb` | header-only exact libplacebo submodule source | Viewer maintainers | 2026-08-12 | Use the procedure above. |
| vulkan-headers | d732b2de303ce505169011d438178191136bfb00 | Apache-2.0 | https://github.com/KhronosGroup/Vulkan-Headers/archive/d732b2de303ce505169011d438178191136bfb00.tar.gz | `570f9ae1e65466dbaf5fcab667abd079dd0a61c4ab86cf535efd492bf70a5b74` | exact libplacebo submodule source; Vulkan runtime disabled | Viewer maintainers | 2026-08-12 | Use the procedure above. |
| libplacebo | 6.338.2 (`v6.338.2`) | LGPL-2.1-or-later | https://github.com/haasn/libplacebo/archive/refs/tags/v6.338.2.tar.gz | `2f1e624e09d72a8c9db70f910f7560e764a1c126dae42acc5b3bcef836a7aec6` | `-Dauto_features=disabled -Ddemos=false -Dtests=false -Dbench=false -Dfuzz=false -Dvulkan=disabled -Dopengl=disabled -Dd3d11=disabled -Dlcms=disabled -Ddovi=disabled -Dlibdovi=disabled -Dunwind=disabled -Dxxhash=disabled` | Viewer maintainers | 2026-08-12 | Use the procedure above. |
| mpv / libmpv | 0.41.0 (`v0.41.0`, commit `41f6a64`) | LGPL-2.1-or-later | https://github.com/mpv-player/mpv/archive/refs/tags/v0.41.0.tar.gz | `ee21092a5ee427353392360929dc64645c54479aefdb5babc5cfbb5fad626209` | `-Dbuild-date=false -Dcocoa=enabled -Dcplugins=disabled -Dcplayer=false -Dgl=enabled -Dgl-cocoa=enabled -Dgpl=false -Diconv=enabled -Djavascript=disabled -Dlibmpv=true -Dlua=disabled -Dmacos-cocoa-cb=disabled -Dplain-gl=enabled -Dswift-build=enabled -Dvideotoolbox-gl=enabled` | Viewer maintainers | 2026-08-12 | Use the procedure above. |

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
