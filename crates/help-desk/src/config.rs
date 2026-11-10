//! Configuration file management (`config.toml`).

use crate::error::{Error, Result};
use crate::paths::Paths;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_include_system")]
    pub include_system: bool,

    #[serde(default)]
    pub scan_dirs: Vec<PathBuf>,

    #[serde(default)]
    pub allowed_function_dirs: Vec<PathBuf>,

    #[serde(default)]
    pub disabled_sources: Vec<String>,

    #[serde(default)]
    pub custom_redact_patterns: Vec<String>,
}

fn default_include_system() -> bool {
    false
}

impl Config {
    /// Load config from the path determined by `Paths`, or return default if missing.
    pub fn load(paths: &Paths) -> Result<Self> {
        let path = paths.config_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(&path)
            .map_err(|e| Error::io(format!("Failed to read config from {}", path.display()), e))?;
        toml::from_str(&content)
            .map_err(|e| Error::Other(format!("Failed to parse config at {}: {e}", path.display())))
    }

    /// Save config to the path determined by `Paths`.
    pub fn save(&self, paths: &Paths) -> Result<()> {
        let path = paths.config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                Error::io(
                    format!("Failed to create config dir {}", parent.display()),
                    e,
                )
            })?;
        }
        let content = toml::to_string_pretty(self)
            .map_err(|e| Error::Other(format!("Failed to serialize config: {e}")))?;
        fs::write(&path, content)
            .map_err(|e| Error::io(format!("Failed to write config to {}", path.display()), e))
    }

    /// Check if a source adapter is enabled.
    pub fn is_source_enabled(&self, source_name: &str) -> bool {
        !self
            .disabled_sources
            .iter()
            .any(|s| s.eq_ignore_ascii_case(source_name))
    }

    /// Get all effective scan directories (standard defaults + custom scan_dirs).
    pub fn effective_scan_dirs(&self, home: &Path) -> Vec<PathBuf> {
        let mut dirs = vec![
            home.join(".local/bin"),
            home.join(".cargo/bin"),
            PathBuf::from("/usr/local/bin"),
            PathBuf::from("/opt/homebrew/bin"),
            PathBuf::from("/home/linuxbrew/.linuxbrew/bin"),
        ];

        // Add custom scan dirs from config
        for d in &self.scan_dirs {
            let expanded = crate::paths::expand_tilde(d, home);
            if !dirs.contains(&expanded) {
                dirs.push(expanded);
            }
        }

        // Conditionally include system directories if enabled
        if self.include_system {
            dirs.push(PathBuf::from("/usr/bin"));
            dirs.push(PathBuf::from("/bin"));
        }

        dirs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = Config::default();
        assert!(!cfg.include_system);
        assert!(cfg.is_source_enabled("cargo"));
        assert!(cfg.is_source_enabled("npm"));
    }

    #[test]
    fn test_effective_scan_dirs() {
        let cfg = Config::default();
        let home = Path::new("/home/testuser");
        let dirs = cfg.effective_scan_dirs(home);
        assert!(dirs.contains(&PathBuf::from("/home/testuser/.local/bin")));
        assert!(dirs.contains(&PathBuf::from("/home/testuser/.cargo/bin")));
        assert!(dirs.contains(&PathBuf::from("/usr/local/bin")));
        assert!(!dirs.contains(&PathBuf::from("/usr/bin")));
    }
}
