const PRIVACY_AND_SECURITY_URL: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles";

/// Opens the one fixed macOS destination Viewer uses for permission recovery.
/// No caller-controlled URL or path crosses this adapter.
pub fn open_privacy_and_security() -> std::io::Result<()> {
    super::open_external_url(PRIVACY_AND_SECURITY_URL)
}

#[cfg(test)]
mod tests {
    #[test]
    fn destination_is_compile_time_fixed_to_privacy_and_security() {
        assert_eq!(
            super::PRIVACY_AND_SECURITY_URL,
            "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles"
        );
        let _opener: fn() -> std::io::Result<()> = super::open_privacy_and_security;
    }
}
