//! Privileged command execution
//! Safe async wrapper for pkexec with timeout

use crate::error::{AppError, Result};
use std::process::Stdio;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

/// Timeout for privileged operations (30 seconds)
const PKEXEC_TIMEOUT_SECS: u64 = 30;

/// Whitelist of allowed commands for privileged execution
const ALLOWED_COMMANDS: &[&str] = &[
    "sysctl",
    "journalctl",
    "apt",
    "apt-get",
    "systemctl",
    "rm",
    "bash",
    "add-apt-repository",
    "cp",
    "tee",
];

/// Execute a command with root privileges via pkexec (async with timeout)
/// 
/// # Security
/// - Only whitelisted commands are allowed
/// - Uses pkexec for GUI-friendly authentication
/// - 30 second timeout to prevent app freeze if user ignores dialog
pub async fn run_privileged(cmd: &str, args: &[&str]) -> Result<String> {
    // Validate command is whitelisted
    if !ALLOWED_COMMANDS.contains(&cmd) {
        return Err(AppError::PermissionDenied(format!(
            "Command '{}' is not in the allowed list",
            cmd
        )));
    }

    // Spawn the pkexec process
    let child = Command::new("pkexec")
        .arg(cmd)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| AppError::CommandFailed(format!("Failed to spawn pkexec: {}", e)))?;

    // Wait with timeout
    let output = timeout(Duration::from_secs(PKEXEC_TIMEOUT_SECS), child.wait_with_output())
        .await
        .map_err(|_| AppError::Timeout("Authentication dialog timed out after 30 seconds".to_string()))?
        .map_err(|e| AppError::CommandFailed(format!("Command execution failed: {}", e)))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        
        // Check if user cancelled
        if stderr.contains("dismissed") || stderr.contains("cancelled") || stderr.contains("Not authorized") {
            return Err(AppError::UserCancelled);
        }
        
        Err(AppError::CommandFailed(stderr.to_string()))
    }
}

/// Execute a shell command with root privileges (async with timeout)
/// Only for specific, validated operations
pub async fn run_privileged_shell(script: &str) -> Result<String> {
    // Basic validation - no dangerous patterns
    let dangerous_patterns = ["rm -rf /", "dd if=", "mkfs", "> /dev/"];
    for pattern in dangerous_patterns {
        if script.contains(pattern) {
            return Err(AppError::PermissionDenied(format!(
                "Script contains dangerous pattern: {}",
                pattern
            )));
        }
    }

    // Spawn the pkexec bash process
    let child = Command::new("pkexec")
        .args(["bash", "-c", script])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| AppError::CommandFailed(format!("Failed to spawn pkexec: {}", e)))?;

    // Wait with timeout
    let output = timeout(Duration::from_secs(PKEXEC_TIMEOUT_SECS), child.wait_with_output())
        .await
        .map_err(|_| AppError::Timeout("Authentication dialog timed out after 30 seconds".to_string()))?
        .map_err(|e| AppError::CommandFailed(format!("Command execution failed: {}", e)))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("dismissed") || stderr.contains("Not authorized") {
            return Err(AppError::UserCancelled);
        }
        Err(AppError::CommandFailed(stderr.to_string()))
    }
}

/// Run a non-privileged async command
pub async fn run_async_command(cmd: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(cmd)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| AppError::CommandFailed(format!("Failed to execute {}: {}", cmd, e)))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(AppError::CommandFailed(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ))
    }
}
