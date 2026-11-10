//! General utility functions: hashing, timestamps, string normalization.

use chrono::Utc;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

/// Compute SHA-256 of a byte slice and return hex string.
pub fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let result = hasher.finalize();
    format!("{result:x}")
}

/// Compute SHA-256 of a file without loading the entire file into memory.
pub fn sha256_file(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    let result = hasher.finalize();
    Ok(format!("{result:x}"))
}

/// Compute SHA-256 of a string.
pub fn sha256_str(s: &str) -> String {
    sha256_bytes(s.as_bytes())
}

/// Current UTC timestamp in ISO 8601 / RFC 3339 format.
pub fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

/// Generate a snapshot ID: `YYYYMMDDTHHMMSSZ-<os>-<arch>`.
pub fn generate_snapshot_id(os: &str, arch: &str) -> String {
    let timestamp = Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    format!("{timestamp}-{os}-{arch}")
}

/// Normalize command names (lowercase, trimmed).
pub fn normalize_command_name(name: &str) -> String {
    name.trim().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256_bytes() {
        let hash = sha256_bytes(b"hello world");
        assert_eq!(
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_normalize_command_name() {
        assert_eq!(normalize_command_name("  RipGrep  "), "ripgrep");
        assert_eq!(normalize_command_name("TAR"), "tar");
    }

    #[test]
    fn test_generate_snapshot_id() {
        let id = generate_snapshot_id("linux", "x86_64");
        assert!(id.ends_with("-linux-x86_64"));
        assert!(id.contains('T'));
        assert!(id.contains('Z'));
    }
}
