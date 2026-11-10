//! Platform data/config locations.
//!
//! - Linux data:   `${XDG_DATA_HOME:-~/.local/share}/hdesk/catalog.db`
//! - Linux config: `${XDG_CONFIG_HOME:-~/.config}/hdesk/config.toml`
//! - macOS data and config: `~/Library/Application Support/hdesk/`
//!
//! `HDESK_DATA_DIR` and `HDESK_CONFIG_DIR` override both locations so tests
//! never touch the real home directory. The legacy `~/.help_desk.db` is only
//! ever checked for existence by `doctor`; it is never opened.

use crate::error::{Error, Result};
use std::env;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Paths {
    pub home: PathBuf,
    pub data_dir: PathBuf,
    pub config_dir: PathBuf,
}

fn env_path(name: &str) -> Option<PathBuf> {
    env::var_os(name)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
}

impl Paths {
    pub fn from_env() -> Result<Self> {
        let home = env_path("HOME")
            .ok_or_else(|| Error::Other("HOME is not set to an absolute path".into()))?;
        let data_dir = env_path("HDESK_DATA_DIR").unwrap_or_else(|| default_data_dir(&home));
        let config_dir = env_path("HDESK_CONFIG_DIR").unwrap_or_else(|| default_config_dir(&home));
        Ok(Paths {
            home,
            data_dir,
            config_dir,
        })
    }

    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join("catalog.db")
    }

    pub fn config_path(&self) -> PathBuf {
        self.config_dir.join("config.toml")
    }

    pub fn legacy_db_path(&self) -> PathBuf {
        self.home.join(".help_desk.db")
    }
}

fn default_data_dir(home: &Path) -> PathBuf {
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/hdesk")
    } else {
        env_path("XDG_DATA_HOME")
            .unwrap_or_else(|| home.join(".local/share"))
            .join("hdesk")
    }
}

fn default_config_dir(home: &Path) -> PathBuf {
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/hdesk")
    } else {
        env_path("XDG_CONFIG_HOME")
            .unwrap_or_else(|| home.join(".config"))
            .join("hdesk")
    }
}

/// Render a path with the home prefix replaced by `~`.
pub fn tilde(path: &Path, home: &Path) -> String {
    match path.strip_prefix(home) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".to_string(),
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}

/// Expand a leading `~/` using the given home directory.
pub fn expand_tilde(raw: &Path, home: &Path) -> PathBuf {
    match raw.strip_prefix("~") {
        Ok(rest) => home.join(rest),
        Err(_) => raw.to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tilde_replaces_home_prefix_only() {
        let home = Path::new("/home/u");
        assert_eq!(tilde(Path::new("/home/u/.zshrc"), home), "~/.zshrc");
        assert_eq!(tilde(Path::new("/home/u"), home), "~");
        assert_eq!(tilde(Path::new("/home/user2/x"), home), "/home/user2/x");
        assert_eq!(tilde(Path::new("/usr/bin/ls"), home), "/usr/bin/ls");
    }

    #[test]
    fn expand_tilde_round_trip() {
        let home = Path::new("/home/u");
        assert_eq!(
            expand_tilde(Path::new("~/dotfiles"), home),
            PathBuf::from("/home/u/dotfiles")
        );
        assert_eq!(
            expand_tilde(Path::new("/opt/x"), home),
            PathBuf::from("/opt/x")
        );
    }
}
