//! Build and release metadata.
//!
//! Note: This module only exposes static metadata and version information;
//! it does not perform any network operations or runtime releases.

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const NAME: &str = env!("CARGO_PKG_NAME");
pub const TARGET: &str = env!("TARGET");

pub fn version_info() -> String {
    format!("{NAME} {VERSION} ({TARGET})")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_info() {
        let v = version_info();
        assert!(v.contains(VERSION));
        assert!(v.contains(NAME));
    }
}
