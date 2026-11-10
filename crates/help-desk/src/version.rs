//! Ecosystem version comparison and diff classification.
//!
//! Guidelines from spec:
//! - Do not blindly apply SemVer to every ecosystem.
//! - When versions can be reliably ordered (e.g. numeric component comparison),
//!   return `Upgraded` or `Downgraded`.
//! - If order cannot be reliably determined, fall back to `VersionChanged`.
//! - Never crash or invent upgrades/downgrades on arbitrary strings.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionDiff {
    Unchanged,
    Upgraded,
    Downgraded,
    VersionChanged,
}

/// Compare two versions in the context of an ecosystem.
pub fn compare_versions(ecosystem: &str, old_v: Option<&str>, new_v: Option<&str>) -> VersionDiff {
    let (old, new) = match (old_v, new_v) {
        (None, None) => return VersionDiff::Unchanged,
        (Some(_), None) | (None, Some(_)) => return VersionDiff::VersionChanged,
        (Some(o), Some(n)) => (o.trim(), n.trim()),
    };

    if old == new {
        return VersionDiff::Unchanged;
    }

    match ecosystem.to_lowercase().as_str() {
        "cargo" | "npm" | "pipx" | "python" | "composer" | "homebrew" => {
            if let Some(order) = compare_numeric_components(old, new) {
                return match order {
                    std::cmp::Ordering::Greater => VersionDiff::Downgraded,
                    std::cmp::Ordering::Less => VersionDiff::Upgraded,
                    std::cmp::Ordering::Equal => VersionDiff::Unchanged,
                };
            }
            VersionDiff::VersionChanged
        }
        _ => VersionDiff::VersionChanged,
    }
}

/// Split numeric version into components (e.g. "1.14.2" -> [1, 14, 2]).
/// Returns None if either string does not start with digits or cannot be cleanly compared.
fn compare_numeric_components(v1: &str, v2: &str) -> Option<std::cmp::Ordering> {
    // Strip leading 'v' or 'V' if present
    let s1 = v1
        .strip_prefix('v')
        .or_else(|| v1.strip_prefix('V'))
        .unwrap_or(v1);
    let s2 = v2
        .strip_prefix('v')
        .or_else(|| v2.strip_prefix('V'))
        .unwrap_or(v2);

    // Extract dot-separated chunks
    let c1: Vec<&str> = s1.split('.').collect();
    let c2: Vec<&str> = s2.split('.').collect();

    let max_len = c1.len().max(c2.len());
    let mut different = false;

    for i in 0..max_len {
        let p1 = c1.get(i).copied().unwrap_or("0");
        let p2 = c2.get(i).copied().unwrap_or("0");

        // Try parsing pure integers
        let n1: Result<u64, _> = p1.parse();
        let n2: Result<u64, _> = p2.parse();

        match (n1, n2) {
            (Ok(num1), Ok(num2)) => {
                if num1 != num2 {
                    return Some(num1.cmp(&num2));
                }
            }
            // If there are pre-release tags like "1.0.0-beta.1", fall back to string compare or None
            _ => {
                // If parts differ and can't be parsed as numbers, fallback to None (VersionChanged)
                different = true;
            }
        }
    }

    if different {
        None
    } else {
        Some(std::cmp::Ordering::Equal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_compare_semver() {
        assert_eq!(
            compare_versions("cargo", Some("1.2.3"), Some("1.2.4")),
            VersionDiff::Upgraded
        );
        assert_eq!(
            compare_versions("cargo", Some("2.0.0"), Some("1.9.9")),
            VersionDiff::Downgraded
        );
        assert_eq!(
            compare_versions("cargo", Some("1.0.0"), Some("1.0.0")),
            VersionDiff::Unchanged
        );
        assert_eq!(
            compare_versions("cargo", Some("v1.5.0"), Some("1.6.0")),
            VersionDiff::Upgraded
        );
    }

    #[test]
    fn test_unknown_or_complex_version_fallback() {
        assert_eq!(
            compare_versions("cargo", Some("1.0.0-alpha"), Some("1.0.0-beta")),
            VersionDiff::VersionChanged
        );
        assert_eq!(
            compare_versions("system", Some("2026-patch-a"), Some("2026-patch-b")),
            VersionDiff::VersionChanged
        );
    }
}
