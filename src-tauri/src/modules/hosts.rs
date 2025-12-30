//! Ad-Block Manager module
//! Manage /etc/hosts blocklists from various GitHub sources (async)
//! Optimized for large files using temp files and streaming

use crate::error::{AppError, Result};
use crate::utils::privileged;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use tokio::time::Duration;

// ============================================================================
// Data Structures
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlocklistSource {
    pub id: String,
    pub name: String,
    pub url: String,
    pub description: String,
    pub domain_count: Option<usize>,
    pub is_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdBlockStats {
    pub total_blocked_domains: usize,
    pub active_blocklists: Vec<String>,
    pub hosts_file_size: u64,
}

// ============================================================================
// Blocklist Sources
// ============================================================================

pub const BLOCKLIST_SOURCES: &[(&str, &str, &str, &str)] = &[
    // (id, name, url, description)
    (
        "stevenblack_unified",
        "StevenBlack Unified",
        "https://raw.githubusercontent.com/StevenBlack/hosts/master/hosts",
        "Comprehensive list (~130k domains) - ads, malware, fakenews",
    ),
    (
        "adaway",
        "AdAway Default",
        "https://adaway.org/hosts.txt",
        "Mobile-focused blocking list (~6k domains)",
    ),
    (
        "dan_pollock",
        "Dan Pollock's Hosts",
        "https://someonewhocares.org/hosts/hosts",
        "Well-maintained list (~15k domains)",
    ),
    (
        "mvps",
        "MVPS Hosts",
        "https://winhelp2002.mvps.org/hosts.txt",
        "Long-running Windows-focused list (~10k domains)",
    ),
    (
        "energized_basic",
        "Energized Basic",
        "https://energized.pro/basic/formats/hosts.txt",
        "Balanced protection (~50k domains)",
    ),
    (
        "urlhaus",
        "URLHaus Malicious",
        "https://urlhaus.abuse.ch/downloads/hostfile/",
        "Malware/ransomware URLs from abuse.ch",
    ),
    (
        "1hosts_lite",
        "1Hosts Lite",
        "https://o0.pages.dev/Lite/hosts.txt",
        "Lightweight protection (~30k domains)",
    ),
    (
        "pgl_yoyo",
        "Peter Lowe's List",
        "https://pgl.yoyo.org/adservers/serverlist.php?hostformat=hosts&showintro=0",
        "Ad servers list (~3k domains)",
    ),
    (
        "oisd_small",
        "OISD Small",
        "https://small.oisd.nl/hosts",
        "Minimal false positives (~70k domains)",
    ),
];

const HOSTS_PATH: &str = "/etc/hosts";
const BLOCKLIST_MARKER_START: &str = "# === GLANCE ADBLOCK START ===";
const BLOCKLIST_MARKER_END: &str = "# === GLANCE ADBLOCK END ===";

// ============================================================================
// Helper Functions
// ============================================================================

/// Create a temporary file with content
fn create_temp_file(content: &str) -> Result<PathBuf> {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let temp_path = std::env::temp_dir().join(format!("glance_hosts_{}.tmp", timestamp));

    let mut file = fs::File::create(&temp_path)
        .map_err(|e| AppError::System(format!("Failed to create temp file: {}", e)))?;

    file.write_all(content.as_bytes())
        .map_err(|e| AppError::System(format!("Failed to write temp file: {}", e)))?;

    Ok(temp_path)
}

/// Get the base hosts content (without blocklist section)
fn get_base_hosts_content() -> Result<String> {
    let content = fs::read_to_string(HOSTS_PATH)
        .map_err(|e| AppError::System(format!("Failed to read hosts file: {}", e)))?;

    let mut result = Vec::new();
    let mut in_blocklist_section = false;

    for line in content.lines() {
        if line.trim() == BLOCKLIST_MARKER_START {
            in_blocklist_section = true;
            continue;
        }
        if line.trim() == BLOCKLIST_MARKER_END {
            in_blocklist_section = false;
            continue;
        }
        if !in_blocklist_section {
            result.push(line.to_string());
        }
    }

    Ok(result.join("\n"))
}

/// Parse valid block entries from blocklist content
fn parse_blocklist_entries(content: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut entries = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();

        // Skip comments and empty lines
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Must start with 0.0.0.0 or 127.0.0.1
        if !trimmed.starts_with("0.0.0.0") && !trimmed.starts_with("127.0.0.1") {
            continue;
        }

        // Extract hostname (second part)
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }

        let hostname = parts[1];

        // Skip localhost entries
        if hostname == "localhost"
            || hostname == "localhost.localdomain"
            || hostname == "local"
            || hostname.starts_with("broadcasthost")
        {
            continue;
        }

        // Normalize to 0.0.0.0 format and deduplicate
        if seen.insert(hostname.to_string()) {
            entries.push(format!("0.0.0.0 {}", hostname));
        }
    }

    entries
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Get available blocklist sources with their status
#[tauri::command]
pub async fn get_blocklist_sources() -> Result<Vec<BlocklistSource>> {
    let content = tokio::task::spawn_blocking(|| {
        fs::read_to_string(HOSTS_PATH).unwrap_or_default()
    })
    .await
    .unwrap();

    let sources: Vec<BlocklistSource> = BLOCKLIST_SOURCES
        .iter()
        .map(|(id, name, url, desc)| {
            // Check if this blocklist is already applied by looking for its marker
            let marker = format!("# Source: {}", url);
            let is_enabled = content.contains(&marker);

            BlocklistSource {
                id: id.to_string(),
                name: name.to_string(),
                url: url.to_string(),
                description: desc.to_string(),
                domain_count: None, // Will be calculated after download
                is_enabled,
            }
        })
        .collect();

    Ok(sources)
}

/// Get current ad-block statistics
#[tauri::command]
pub async fn get_adblock_stats() -> Result<AdBlockStats> {
    let stats = tokio::task::spawn_blocking(|| {
        let content = fs::read_to_string(HOSTS_PATH).unwrap_or_default();
        let metadata = fs::metadata(HOSTS_PATH).ok();
        let file_size = metadata.map(|m| m.len()).unwrap_or(0);

        let mut total_blocked = 0;
        let mut active_lists = Vec::new();
        let mut in_blocklist = false;

        for line in content.lines() {
            if line.trim() == BLOCKLIST_MARKER_START {
                in_blocklist = true;
                continue;
            }
            if line.trim() == BLOCKLIST_MARKER_END {
                in_blocklist = false;
                continue;
            }

            if in_blocklist {
                if line.starts_with("# Source: ") {
                    let url = line.trim_start_matches("# Source: ").trim();
                    // Find the name for this URL
                    for (_, name, u, _) in BLOCKLIST_SOURCES {
                        if *u == url {
                            active_lists.push(name.to_string());
                            break;
                        }
                    }
                } else if !line.trim().is_empty() && !line.trim().starts_with('#') {
                    total_blocked += 1;
                }
            }
        }

        AdBlockStats {
            total_blocked_domains: total_blocked,
            active_blocklists: active_lists,
            hosts_file_size: file_size,
        }
    })
    .await
    .unwrap();

    Ok(stats)
}

/// Apply selected blocklists
#[tauri::command]
pub async fn apply_blocklists(source_ids: Vec<String>) -> Result<usize> {
    if source_ids.is_empty() {
        return Err(AppError::System("No blocklists selected".to_string()));
    }

    // Get URLs for selected sources
    let selected_sources: Vec<(&str, &str, &str)> = BLOCKLIST_SOURCES
        .iter()
        .filter(|(id, _, _, _)| source_ids.contains(&id.to_string()))
        .map(|(id, name, url, _)| (*id, *name, *url))
        .collect();

    if selected_sources.is_empty() {
        return Err(AppError::System("No valid blocklists found".to_string()));
    }

    // Download all blocklists
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| AppError::Network(format!("Failed to create HTTP client: {}", e)))?;

    let mut all_entries: Vec<String> = Vec::new();
    let mut source_markers: Vec<String> = Vec::new();

    for (_id, name, url) in &selected_sources {
        let response = client
            .get(*url)
            .send()
            .await
            .map_err(|e| AppError::Network(format!("Failed to download {}: {}", name, e)))?;

        if !response.status().is_success() {
            continue; // Skip failed downloads
        }

        let content = response
            .text()
            .await
            .map_err(|e| AppError::Network(format!("Failed to read {}: {}", name, e)))?;

        let entries = parse_blocklist_entries(&content);
        source_markers.push(format!("# Source: {} ({} entries)", url, entries.len()));
        all_entries.extend(entries);
    }

    if all_entries.is_empty() {
        return Err(AppError::System("No valid entries found in blocklists".to_string()));
    }

    // Deduplicate entries
    let mut seen = HashSet::new();
    let unique_entries: Vec<String> = all_entries
        .into_iter()
        .filter(|e| {
            let hostname = e.split_whitespace().nth(1).unwrap_or("");
            seen.insert(hostname.to_string())
        })
        .collect();

    let total_count = unique_entries.len();

    // Build the blocklist section
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    
    let blocklist_section = format!(
        "{}\n# Applied: {} (unix timestamp)\n# Total blocked: {} domains\n{}\n{}\n{}\n",
        BLOCKLIST_MARKER_START,
        timestamp,
        total_count,
        source_markers.join("\n"),
        unique_entries.join("\n"),
        BLOCKLIST_MARKER_END
    );

    // Get base hosts and append blocklist
    let base_content = tokio::task::spawn_blocking(get_base_hosts_content)
        .await
        .unwrap()?;

    let new_content = format!("{}\n\n{}", base_content.trim(), blocklist_section);

    // Write via temp file
    let temp_path = tokio::task::spawn_blocking(move || create_temp_file(&new_content))
        .await
        .unwrap()?;

    let script = format!(
        "cat '{}' > '{}' && rm '{}'",
        temp_path.to_string_lossy(),
        HOSTS_PATH,
        temp_path.to_string_lossy()
    );

    privileged::run_privileged_shell(&script).await?;

    Ok(total_count)
}

/// Clear all blocklists from hosts file
#[tauri::command]
pub async fn clear_blocklists() -> Result<()> {
    let base_content = tokio::task::spawn_blocking(get_base_hosts_content)
        .await
        .unwrap()?;

    let new_content = format!("{}\n", base_content.trim());

    let temp_path = tokio::task::spawn_blocking(move || create_temp_file(&new_content))
        .await
        .unwrap()?;

    let script = format!(
        "cat '{}' > '{}' && rm '{}'",
        temp_path.to_string_lossy(),
        HOSTS_PATH,
        temp_path.to_string_lossy()
    );

    privileged::run_privileged_shell(&script).await?;

    Ok(())
}

/// Backup hosts file
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
        backups.sort();
        backups.reverse(); // Newest first
        backups
    })
    .await
    .unwrap();

    Ok(backups)
}

/// Restore hosts from backup
#[tauri::command]
pub async fn restore_hosts(backup_path: String) -> Result<()> {
    // Validate path
    if !backup_path.starts_with("/etc/hosts.backup.") {
        return Err(AppError::PermissionDenied("Invalid backup path".to_string()));
    }

    privileged::run_privileged("cp", &[&backup_path, HOSTS_PATH]).await?;

    Ok(())
}
