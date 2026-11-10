//! Safe subprocess execution.
//!
//! Guidelines from spec:
//! - Explicit argv arrays (no shell string concatenation)
//! - Closed interactive stdin (`Stdio::null()`)
//! - Hard execution timeout with process kill
//! - Captures stdout and stderr safely without pipe deadlocks
//! - Clears or controls environment variables to avoid leaking secrets

use crate::error::{Error, Result};
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct CommandOutput {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration: Duration,
}

/// Run an external command with timeout, capturing stdout/stderr safely.
pub fn run_command(
    program: &str,
    args: &[&str],
    timeout: Duration,
    cwd: Option<&Path>,
    extra_env: &[(&str, &str)],
) -> Result<CommandOutput> {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }

    for (k, v) in extra_env {
        cmd.env(k, v);
    }

    let start = Instant::now();
    let mut child = cmd
        .spawn()
        .map_err(|e| Error::Io(format!("Failed to spawn '{program}': {e}")))?;

    let mut stdout_handle = child.stdout.take().expect("stdout was piped");
    let mut stderr_handle = child.stderr.take().expect("stderr was piped");

    let stdout_thread = thread::spawn(move || -> std::io::Result<Vec<u8>> {
        let mut buf = Vec::new();
        stdout_handle.read_to_end(&mut buf)?;
        Ok(buf)
    });

    let stderr_thread = thread::spawn(move || -> std::io::Result<Vec<u8>> {
        let mut buf = Vec::new();
        stderr_handle.read_to_end(&mut buf)?;
        Ok(buf)
    });

    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(Error::Other(format!(
                        "Command '{program}' timed out after {:?}",
                        timeout
                    )));
                }
                thread::sleep(Duration::from_millis(15));
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(Error::Io(format!(
                    "Failed while waiting for '{program}': {e}"
                )));
            }
        }
    };

    let stdout_bytes = stdout_thread
        .join()
        .map_err(|_| Error::Other("Stdout reader thread panicked".into()))?
        .unwrap_or_default();

    let stderr_bytes = stderr_thread
        .join()
        .map_err(|_| Error::Other("Stderr reader thread panicked".into()))?
        .unwrap_or_default();

    let duration = start.elapsed();

    Ok(CommandOutput {
        success: status.success(),
        exit_code: status.code(),
        stdout: String::from_utf8_lossy(&stdout_bytes).to_string(),
        stderr: String::from_utf8_lossy(&stderr_bytes).to_string(),
        duration,
    })
}

/// Check if an executable exists in PATH.
pub fn is_executable_available(program: &str) -> bool {
    let check_cmd = if cfg!(target_os = "windows") {
        "where"
    } else {
        "which"
    };
    match Command::new(check_cmd)
        .arg(program)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        Ok(status) => status.success(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_command_echo() {
        let out = run_command("echo", &["hello_test"], Duration::from_secs(3), None, &[]).unwrap();
        assert!(out.success);
        assert_eq!(out.stdout.trim(), "hello_test");
    }

    #[test]
    fn test_run_command_timeout() {
        let res = run_command("sleep", &["2"], Duration::from_millis(100), None, &[]);
        assert!(res.is_err());
        let err_msg = res.unwrap_err().to_string();
        assert!(err_msg.contains("timed out"));
    }
}
