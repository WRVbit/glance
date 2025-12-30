//! Hosts File Editor module
//! Parse and edit /etc/hosts with blocklist support (async)

use crate::error::{AppError, Result};
use crate::utils::privileged;
use serde::{Deserialize, Serialize};
use std::fs;
use tokio::time::Duration;

// ============================================================================
// Data Structures
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostEntry {
    pub line_number: usize,
    pub ip: String,
    pub hostnames: Vec<String>,
    pub comment: Option<String>,
    pub is_enabled: bool,
    pub raw_line: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostsStats {
    pub total_entries: usize,
    pub enabled_entries: usize,
    pub blocked_domains: usize, // Entries pointing to 0.0.0.0 or 127.0.0.1
}

// ============================================================================
// Common Blocklists
// ============================================================================

pub const BLOCKLISTS: &[(&str, &str)] = &[
    ("StevenBlack (Unified)", "https://raw.githubusercontent.com/StevenBlack/hosts/master/hosts"),
    ("AdAway Default", "https://adaway.org/hosts.txt"),
    ("MVPS Hosts", "https://winhelp2002.mvps.org/hosts.txt"),
];

// ============================================================================
// Helper Functions
// ============================================================================

const HOSTS_PATH: &str = "/etc/hosts";

/// Parse a single line from hosts file
fn parse_host_line(line: &str, line_number: usize) -> Option<HostEntry> {
    let trimmed = line.trim();
    
    // Empty line
    if trimmed.is_empty() {
        return None;
    }
    
    // Determine if enabled (not commented)
    let is_enabled = !trimmed.starts_with('#');
    let clean_line = trimmed.trim_start_matches('#').trim();
    
    // Skip pure comments without IP
    if clean_line.is_empty() {
        return None;
    }
    
    // Split into parts (IP, hostnames, optional comment)
    let (content, comment) = if let Some(idx) = clean_line.find('#') {
        (clean_line[..idx].trim(), Some(clean_line[idx + 1..].trim().to_string()))
    } else {
        (clean_line, None)
    };
    
    let parts: Vec<&str> = content.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }
    
    let ip = parts[0].to_string();
    
    // Validate IP format (basic check)
    if !ip.contains('.') && !ip.contains(':') {
        return None;
    }
    
    let hostnames: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();
    
    // Skip if no hostnames
    if hostnames.is_empty() {
        return None;
    }
    
    Some(HostEntry {
        line_number,
        ip,
        hostnames,
        comment,
        is_enabled,
        raw_line: line.to_string(),
    })
}

// ============================================================================
// Tauri Commands (All async)
// ============================================================================

/// Get all hosts entries
#[tauri::command]
pub async fn get_hosts() -> Result<Vec<HostEntry>> {
    let entries = tokio::task::spawn_blocking(|| {
        let content = fs::read_to_string(HOSTS_PATH)?;
        
        let entries: Vec<HostEntry> = content
            .lines()
            .enumerate()
            .filter_map(|(idx, line)| parse_host_line(line, idx + 1))
            .collect();
        
        Ok::<_, AppError>(entries)
    }).await.unwrap()?;
    
    Ok(entries)
}

/// Get hosts file statistics
#[tauri::command]
pub async fn get_hosts_stats() -> Result<HostsStats> {
    let entries = get_hosts().await?;
    
    let enabled_entries = entries.iter().filter(|e| e.is_enabled).count();
    let blocked_domains = entries
        .iter()
        .filter(|e| e.is_enabled && (e.ip == "0.0.0.0" || e.ip == "127.0.0.1") && !e.hostnames.contains(&"localhost".to_string()))
        .map(|e| e.hostnames.len())
        .sum();
    
    Ok(HostsStats {
        total_entries: entries.len(),
        enabled_entries,
        blocked_domains,
    })
}

/// Add a new host entry (async with timeout)
#[tauri::command]
pub async fn add_host(ip: String, hostname: String, comment: Option<String>) -> Result<()> {
    // Validate IP
    if !ip.contains('.') && !ip.contains(':') {
        return Err(AppError::System("Invalid IP address".to_string()));
    }
    
    // Validate hostname
    if hostname.is_empty() || hostname.contains(' ') {
        return Err(AppError::System("Invalid hostname".to_string()));
    }
    
    let entry = if let Some(c) = comment {
        format!("{} {} # {}", ip, hostname, c)
    } else {
        format!("{} {}", ip, hostname)
    };
    
    let script = format!(
        "echo '{}' >> '{}'",
        entry.replace("'", "'\\''"),
        HOSTS_PATH
    );
    privileged::run_privileged_shell(&script).await?;
    
    Ok(())
}

/// Remove a host entry by line number (async with timeout)
#[tauri::command]
pub async fn remove_host(line_number: usize) -> Result<()> {
    let content = fs::read_to_string(HOSTS_PATH)?;
    let lines: Vec<&str> = content.lines().collect();
    
    if line_number < 1 || line_number > lines.len() {
        return Err(AppError::System("Invalid line number".to_string()));
    }
    
    let new_lines: Vec<&str> = lines
        .iter()
        .enumerate()
        .filter_map(|(idx, line)| {
            if idx + 1 == line_number {
                None
            } else {
                Some(*line)
            }
        })
        .collect();
    
    let new_content = new_lines.join("\n") + "\n";
    
    let script = format!(
        "echo '{}' > '{}'",
        new_content.replace("'", "'\\''"),
        HOSTS_PATH
    );
    privileged::run_privileged_shell(&script).await?;
    
    Ok(())
}

/// Toggle host entry enabled/disabled (async with timeout)
#[tauri::command]
pub async fn toggle_host(line_number: usize) -> Result<()> {
    let content = fs::read_to_string(HOSTS_PATH)?;
    let lines: Vec<&str> = content.lines().collect();
    
    if line_number < 1 || line_number > lines.len() {
        return Err(AppError::System("Invalid line number".to_string()));
    }
    
    let mut new_lines: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
    let line = &new_lines[line_number - 1];
    
    // Toggle comment
    if line.trim().starts_with('#') {
        new_lines[line_number - 1] = line.trim_start_matches('#').trim_start().to_string();
    } else {
        new_lines[line_number - 1] = format!("# {}", line);
    }
    
    let new_content = new_lines.join("\n") + "\n";
    
    let script = format!(
        "echo '{}' > '{}'",
        new_content.replace("'", "'\\''"),
        HOSTS_PATH
    );
    privileged::run_privileged_shell(&script).await?;
    
    Ok(())
}

/// Get available blocklists
#[tauri::command]
pub fn get_blocklists() -> Vec<(String, String)> {
    BLOCKLISTS
        .iter()
        .map(|(name, url)| (name.to_string(), url.to_string()))
        .collect()
}

/// Import entries from a blocklist URL (async with progress indication)
#[tauri::command]
pub async fn import_blocklist(url: String) -> Result<usize> {
    // Download blocklist using reqwest
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| AppError::Network(format!("Failed to create HTTP client: {}", e)))?;
    
    let response = client.get(&url)
        .send()
        .await
        .map_err(|e| AppError::Network(format!("Failed to download blocklist: {}", e)))?;
    
    if !response.status().is_success() {
        return Err(AppError::Network("Failed to download blocklist".to_string()));
    }
    
    let blocklist = response.text()
        .await
        .map_err(|e| AppError::Network(format!("Failed to read blocklist: {}", e)))?;
    
    // Parse and count valid entries
    let valid_entries: Vec<String> = blocklist
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() 
                && !trimmed.starts_with('#')
                && (trimmed.starts_with("0.0.0.0") || trimmed.starts_with("127.0.0.1"))
        })
        .take(10000) // Limit to prevent huge imports
        .map(|s| s.to_string())
        .collect();
    
    let count = valid_entries.len();
    
    if count == 0 {
        return Err(AppError::System("No valid entries found in blocklist".to_string()));
    }
    
    // Append to hosts file
    let entries_text = valid_entries.join("\n");
    let marker = format!("\n# --- Imported from {} ---\n", url);
    
    let script = format!(
        "echo '{}{}' >> '{}'",
        marker.replace("'", "'\\''"),
        entries_text.replace("'", "'\\''"),
        HOSTS_PATH
    );
    privileged::run_privileged_shell(&script).await?;
    
    Ok(count)
}

/// Backup hosts file (async with timeout)
#[tauri::command]
pub async fn backup_hosts() -> Result<String> {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    
    let backup_path = format!("/etc/hosts.backup.{}", timestamp);
    
    privileged::run_privileged("cp", &[HOSTS_PATH, &backup_path]).await?;
    
    Ok(backup_path)
}

/// List available backups
#[tauri::command]
pub async fn list_hosts_backups() -> Result<Vec<String>> {
    let backups = tokio::task::spawn_blocking(|| {
        let mut backups = Vec::new();
        if let Ok(entries) = fs::read_dir("/etc/") {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("hosts.backup.") {
                    backups.push(format!("/etc/{}", name));
                }
            }
        }
        backups
    }).await.unwrap();
    
    Ok(backups)
}

/// Restore hosts from backup (async with timeout)
#[tauri::command]
pub async fn restore_hosts(backup_path: String) -> Result<()> {
    // Validate path
    if !backup_path.starts_with("/etc/hosts.backup.") {
        return Err(AppError::PermissionDenied("Invalid backup path".to_string()));
    }
    
    privileged::run_privileged("cp", &[&backup_path, HOSTS_PATH]).await?;
    
    Ok(())
}
