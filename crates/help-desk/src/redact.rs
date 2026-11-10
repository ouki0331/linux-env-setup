//! Secret redaction and sanitization for external AI payloads.
//!
//! Guidelines from spec:
//! - Redacts API keys, tokens, passwords, private URLs, SSH keys, registry credentials.
//! - Obfuscates private absolute home directories with tildes.
//! - Replaces suspicious secrets with `[REDACTED_SECRET]` and logs a warning.

use regex::Regex;
use std::path::Path;
use std::sync::OnceLock;

static SECRET_PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();

fn get_secret_patterns() -> &'static [Regex] {
    SECRET_PATTERNS.get_or_init(|| {
        vec![
            // Generic bearer tokens and authorization headers
            Regex::new(r"(?i)(bearer\s+)[A-Za-z0-9_\-\.]{12,}").unwrap(),
            // Basic auth credentials in URLs: https://user:pass@host
            Regex::new(r"://([^:]+):([^@]+)@").unwrap(),
            // Common token prefixes (GitHub, OpenAI, AWS, Slack, etc.)
            Regex::new(r"\b(ghp_[A-Za-z0-9_]{36,})\b").unwrap(),
            Regex::new(r"\b(sk-[A-Za-z0-9]{20,})\b").unwrap(),
            Regex::new(r"\b(xox[baprs]-[A-Za-z0-9_\-]{10,})\b").unwrap(),
            Regex::new(r"\b(AKIA[0-9A-Z]{16})\b").unwrap(),
            // Private SSH keys
            Regex::new(r"-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z ]*PRIVATE KEY-----").unwrap(),
            // npm/pip/composer tokens: _authToken=..., password=...
            Regex::new(r"(?i)(_authToken|auth_token|api_key|apikey|password|passwd|secret)\s*[:=]\s*([^\s;,&]{6,})").unwrap(),
            // Query parameters with secrets: ?token=..., &key=...
            Regex::new(r"(?i)([?&](token|key|secret|password|auth)=)[^&\s]+").unwrap(),
        ]
    })
}

/// Redact strings, replacing sensitive tokens with [REDACTED_SECRET].
pub fn redact_text(text: &str, home: Option<&Path>, warnings: &mut Vec<String>) -> String {
    let mut sanitized = text.to_string();

    // 1. Tilde-expand/replace home paths
    if let Some(h) = home {
        let home_str = h.to_string_lossy().to_string();
        if !home_str.is_empty() && sanitized.contains(&home_str) {
            sanitized = sanitized.replace(&home_str, "~");
        }
    }

    // 2. Scan and redact secret patterns
    for re in get_secret_patterns() {
        if re.is_match(&sanitized) {
            warnings.push(format!(
                "Redacted potential sensitive credential matching rule {:?}",
                re.as_str()
            ));
            sanitized = re.replace_all(&sanitized, "[REDACTED_SECRET]").to_string();
        }
    }

    sanitized
}

/// Safely summarize an alias definition for AI export.
pub fn redact_alias_for_ai(expansion: &str, home: &Path, warnings: &mut Vec<String>) -> String {
    redact_text(expansion, Some(home), warnings)
}

/// Safely summarize a Zsh function for AI export.
/// Functions should NEVER send raw function bodies to external AI unless strictly necessary.
pub fn redact_function_for_ai(definition: &str, home: &Path, warnings: &mut Vec<String>) -> String {
    let sanitized = redact_text(definition, Some(home), warnings);

    // If the body is lengthy, only provide a safe one-line summary
    if sanitized.lines().count() > 3 {
        format!(
            "Custom shell function ({} lines)",
            sanitized.lines().count()
        )
    } else {
        sanitized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_url_credentials() {
        let mut warnings = Vec::new();
        let raw = "git clone https://bot:ghp_1234567890abcdef1234567890abcdef1234@github.com/repo";
        let home = Path::new("/home/user");
        let redacted = redact_text(raw, Some(home), &mut warnings);

        assert!(!redacted.contains("ghp_"));
        assert!(!warnings.is_empty());
    }

    #[test]
    fn test_redact_home_path() {
        let mut warnings = Vec::new();
        let raw = "source /home/testuser/.config/env.sh";
        let home = Path::new("/home/testuser");
        let redacted = redact_text(raw, Some(home), &mut warnings);

        assert_eq!(redacted, "source ~/.config/env.sh");
    }

    #[test]
    fn test_redact_api_keys() {
        let mut warnings = Vec::new();
        let raw = "export OPENAI_API_KEY=sk-abcdefghijklmnopqrstuvwxyz123456";
        let redacted = redact_text(raw, None, &mut warnings);

        assert!(!redacted.contains("sk-"));
        assert!(redacted.contains("[REDACTED_SECRET]"));
    }
}
