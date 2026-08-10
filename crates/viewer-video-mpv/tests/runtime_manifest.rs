use viewer_video_mpv::runtime_manifest::RuntimeManifest;

#[test]
fn checked_in_manifest_loads_from_an_explicit_path() {
    let manifest_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../scripts/video/runtime.lock.json");
    let manifest = RuntimeManifest::load(&manifest_path)
        .expect("the checked-in video runtime manifest must load from disk");

    assert_eq!(manifest.schema_version, 1);
    assert_eq!(manifest.target, "universal-apple-darwin");
}

#[test]
fn checked_in_manifest_is_release_compliant() {
    let manifest =
        RuntimeManifest::from_json(include_str!("../../../scripts/video/runtime.lock.json"))
            .expect("the checked-in video runtime manifest must be valid");

    assert_eq!(manifest.schema_version, 1);
    assert_eq!(manifest.target, "universal-apple-darwin");
    assert_eq!(
        manifest.mpv.meson_options.get("gpl").map(String::as_str),
        Some("false")
    );
    assert!(
        manifest
            .components
            .iter()
            .all(|component| component.sha256.len() == 64)
    );
    assert!(
        manifest
            .components
            .iter()
            .all(|component| component.license != "GPL")
    );
}

#[test]
fn release_manifest_rejects_gpl_components() {
    let invalid = include_str!("../../../scripts/video/runtime.lock.json")
        .replace("LGPL-2.1-or-later", "GPL");
    assert!(RuntimeManifest::from_json(&invalid).is_err());
}

#[test]
fn release_manifest_rejects_non_sha256_digests() {
    let invalid = include_str!("../../../scripts/video/runtime.lock.json").replace(
        "ee21092a5ee427353392360929dc64645c54479aefdb5babc5cfbb5fad626209",
        "missing",
    );
    assert!(RuntimeManifest::from_json(&invalid).is_err());
}

#[test]
fn release_manifest_contains_the_reviewed_engine_sources() {
    let manifest =
        RuntimeManifest::from_json(include_str!("../../../scripts/video/runtime.lock.json"))
            .expect("the checked-in video runtime manifest must be valid");

    assert_eq!(
        manifest
            .component("mpv")
            .map(|component| component.sha256.as_str()),
        Some("ee21092a5ee427353392360929dc64645c54479aefdb5babc5cfbb5fad626209")
    );
    assert_eq!(
        manifest
            .component("ffmpeg")
            .map(|component| component.sha256.as_str()),
        Some("dd4030dbfdc34d9ff255a116bdd1caade42500ac2981efa27f8b151cc54c7b9e")
    );
    assert_eq!(
        manifest
            .component("libplacebo")
            .map(|component| component.sha256.as_str()),
        Some("2f1e624e09d72a8c9db70f910f7560e764a1c126dae42acc5b3bcef836a7aec6")
    );
}

#[test]
fn release_manifest_locks_the_macos_render_backend_contract() {
    let manifest =
        RuntimeManifest::from_json(include_str!("../../../scripts/video/runtime.lock.json"))
            .expect("the checked-in video runtime manifest must be valid");

    for (option, expected) in [
        ("cplayer", "false"),
        ("libmpv", "true"),
        ("cocoa", "enabled"),
        ("gl", "enabled"),
        ("gl-cocoa", "enabled"),
        ("iconv", "enabled"),
        ("plain-gl", "enabled"),
        ("swift-build", "enabled"),
        ("videotoolbox-gl", "enabled"),
        ("macos-cocoa-cb", "disabled"),
        ("javascript", "disabled"),
        ("lua", "disabled"),
        ("cplugins", "disabled"),
    ] {
        assert_eq!(
            manifest.mpv.meson_options.get(option).map(String::as_str),
            Some(expected),
            "unexpected locked mpv option {option}"
        );
    }
}

#[test]
fn release_manifest_locks_the_static_text_rendering_dependency_closure() {
    let manifest =
        RuntimeManifest::from_json(include_str!("../../../scripts/video/runtime.lock.json"))
            .expect("the checked-in video runtime manifest must be valid");
    let names = manifest
        .components
        .iter()
        .map(|component| component.name.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        names,
        [
            "pkgconf",
            "freetype",
            "fribidi",
            "harfbuzz",
            "libass",
            "ffmpeg",
            "fast_float",
            "vulkan-headers",
            "libplacebo",
            "mpv",
        ]
    );
}
