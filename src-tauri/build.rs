use std::{env, fs, path::PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR").expect("Cargo must provide CARGO_MANIFEST_DIR"),
    );
    let manifest_path = manifest_dir.join("../scripts/video/runtime.lock.json");
    let manifest = fs::read_to_string(&manifest_path)
        .unwrap_or_else(|error| panic!("could not read {}: {error}", manifest_path.display()));
    let schema_version = manifest
        .lines()
        .find_map(|line| line.trim().strip_prefix("\"schemaVersion\": "))
        .and_then(|value| value.trim_end_matches(',').parse::<u32>().ok())
        .expect("runtime lock must contain an integer schemaVersion");
    assert_eq!(schema_version, 1, "unsupported video runtime schema");

    println!("cargo:rerun-if-changed={}", manifest_path.display());
    println!("cargo:rustc-env=VIEWER_VIDEO_RUNTIME_SCHEMA_VERSION={schema_version}");

    let runtime_resource = manifest_dir
        .join("../target/viewer-video-runtime/universal-apple-darwin/ViewerVideoRuntime");
    if env::var("PROFILE").as_deref() == Ok("release") {
        assert!(
            runtime_resource.join("runtime.inventory.sha256").is_file(),
            "release builds require a staged, inventoried ViewerVideoRuntime"
        );
    } else {
        fs::create_dir_all(&runtime_resource).unwrap_or_else(|error| {
            panic!(
                "could not create non-release runtime resource placeholder {}: {error}",
                runtime_resource.display()
            )
        });
    }
    tauri_build::build();
}
