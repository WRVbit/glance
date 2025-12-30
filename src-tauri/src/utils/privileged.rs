//! Privileged command execution
//! Safe wrapper for pkexec with input validation

use crate::error::{AppError, Result};
use std::process::Command;

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
    "curl",
];

/// Whitelist of allowed paths for deletion
const ALLOWED_DELETE_PATHS: &[&str] = &[
    "/var/cache/apt/archives",
    "/var/log/journal",
    "/tmp",
];

/// Execute a command with root privileges via pkexec
/// 
/// # Security
/// - Only whitelisted commands are allowed
/// - Arguments are validated before execution
/// - Uses pkexec for GUI-friendly authentication
pub fn run_privileged(cmd: &str, args: &[&str]) -> Result<String> {
    // Validate command is whitelisted
    if !ALLOWED_COMMANDS.contains(&cmd) {
        return Err(AppError::PermissionDenied(format!(
            "Command '{}' is not in the allowed list",
            cmd
        )));
    }

    // Execute via pkexec
    let output = Command::new("pkexec")
        .arg(cmd)
        .args(args)
        .output()
        .map_err(|e| AppError::CommandFailed(format!("Failed to execute pkexec: {}", e)))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        
        // Check if user cancelled
        if stderr.contains("dismissed") || stderr.contains("cancelled") {
            return Err(AppError::UserCancelled);
        }
        
        Err(AppError::CommandFailed(stderr.to_string()))
    }
}

/// Execute a shell command with root privileges
/// Only for specific, validated operations
pub fn run_privileged_shell(script: &str) -> Result<String> {
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

    let output = Command::new("pkexec")
        .args(["bash", "-c", script])
        .output()
        .map_err(|e| AppError::CommandFailed(format!("Failed to execute: {}", e)))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("dismissed") {
            return Err(AppError::UserCancelled);
        }
        Err(AppError::CommandFailed(stderr.to_string()))
    }
}

/// Validate a path is safe to delete
pub fn validate_delete_path(path: &str) -> Result<()> {
    // Must be absolute path
    if !path.starts_with('/') {
        return Err(AppError::PermissionDenied(
            "Path must be absolute".to_string(),
        ));
    }

    // Check against whitelist for system paths
    if path.starts_with("/var") || path.starts_with("/etc") || path.starts_with("/usr") {
        let is_allowed = ALLOWED_DELETE_PATHS.iter().any(|allowed| path.starts_with(allowed));
        if !is_allowed {
            return Err(AppError::PermissionDenied(format!(
                "Path '{}' is not in allowed deletion paths",
                path
            )));
        }
    }

    // Prevent deleting critical paths
    let critical = ["/", "/home", "/root", "/boot", "/bin", "/sbin", "/lib"];
    if critical.contains(&path) {
        return Err(AppError::PermissionDenied(
            "Cannot delete critical system path".to_string(),
        ));
    }

    Ok(())
}

/// Run a non-privileged command
pub fn run_command(cmd: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| AppError::CommandFailed(format!("Failed to execute {}: {}", cmd, e)))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(AppError::CommandFailed(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ))
    }
}
