use std::{fs, path::PathBuf};

pub fn image_fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures/images")
        .join(name)
}

pub fn png_dimensions(path: impl AsRef<std::path::Path>) -> std::io::Result<(u32, u32)> {
    let bytes = fs::read(path)?;
    if bytes.len() < 24 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" || &bytes[12..16] != b"IHDR" {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "not a PNG with an IHDR chunk",
        ));
    }

    Ok((
        u32::from_be_bytes(bytes[16..20].try_into().expect("four width bytes")),
        u32::from_be_bytes(bytes[20..24].try_into().expect("four height bytes")),
    ))
}
