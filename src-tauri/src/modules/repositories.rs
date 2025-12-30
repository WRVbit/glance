//! APT Repository Manager module
//! Manages sources.list and PPAs with PARALLEL mirror speed testing

use crate::error::{AppError, Result};
use crate::utils::privileged;
use futures::future::join_all;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::time::Instant;
use tokio::time::{timeout, Duration};

// ============================================================================
// Data Structures
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    pub file_path: String,
    pub line_number: usize,
    pub repo_type: String,      // deb or deb-src
    pub uri: String,
    pub suite: String,          // noble, noble-updates, etc.
    pub components: Vec<String>, // main, restricted, universe, multiverse
    pub is_enabled: bool,
    pub is_ppa: bool,
    pub raw_line: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MirrorInfo {
    pub name: String,
    pub uri: String,
    pub country: String,
    pub latency_ms: Option<u64>,
}

// ============================================================================
// Ubuntu/Debian Mirrors
// ============================================================================

const UBUNTU_MIRRORS: &[(&str, &str, &str)] = &[
    ("Main", "http://archive.ubuntu.com/ubuntu", "US"),
    ("Indonesia", "http://id.archive.ubuntu.com/ubuntu", "ID"),
    ("Singapore", "http://sg.archive.ubuntu.com/ubuntu", "SG"),
    ("Japan", "http://jp.archive.ubuntu.com/ubuntu", "JP"),
    ("Korea", "http://kr.archive.ubuntu.com/ubuntu", "KR"),
    ("Australia", "http://au.archive.ubuntu.com/ubuntu", "AU"),
    ("Poliwangi", "http://mirror.poliwangi.ac.id/ubuntu", "ID"),
    ("Biznet", "http://mirror.biznetgio.com/ubuntu", "ID"),
    ("Telkom", "http://kartolo.sby.datautama.net.id/ubuntu", "ID"),
];

const DEBIAN_MIRRORS: &[(&str, &str, &str)] = &[
    ("Main", "http://deb.debian.org/debian", "Global"),
    ("Indonesia", "http://mirror.poliwangi.ac.id/debian", "ID"),
    ("Singapore", "http://mirror.sg.gs/debian", "SG"),
];

// ============================================================================
// Helper Functions
// ============================================================================

/// Parse a single line from sources.list
fn parse_repo_line(line: &str, file_path: &str, line_number: usize) -> Option<Repository> {
    let trimmed = line.trim();
    
    // Skip empty lines and pure comments
    if trimmed.is_empty() || (trimmed.starts_with('#') && !trimmed.contains("deb")) {
        return None;
    }
    
    let is_enabled = !trimmed.starts_with('#');
    let clean_line = trimmed.trim_start_matches('#').trim();
    
    // Parse: deb [options] uri suite component1 component2 ...
    let parts: Vec<&str> = clean_line.split_whitespace().collect();
    if parts.len() < 4 {
        return None;
    }
    
    let repo_type = parts[0].to_string();
    if repo_type != "deb" && repo_type != "deb-src" {
        return None;
    }
    
    // Handle [arch=amd64] style options
    let (uri_idx, uri) = if parts[1].starts_with('[') {
        // Find closing bracket
        let mut idx = 1;
        while idx < parts.len() && !parts[idx].contains(']') {
            idx += 1;
        }
        (idx + 1, parts.get(idx + 1).unwrap_or(&"").to_string())
    } else {
        (1, parts[1].to_string())
    };
    
    if uri_idx + 2 > parts.len() {
        return None;
    }
    
    let suite = parts[uri_idx + 1].to_string();
    let components: Vec<String> = parts[uri_idx + 2..].iter().map(|s| s.to_string()).collect();
    
    let is_ppa = uri.contains("ppa.launchpad.net") || uri.contains("ppa:");
    
    Some(Repository {
        file_path: file_path.to_string(),
        line_number,
        repo_type,
        uri,
        suite,
        components,
        is_enabled,
        is_ppa,
        raw_line: line.to_string(),
    })
}

/// Parse all repositories from a file
fn parse_sources_file(path: &Path) -> Vec<Repository> {
    let mut repos = Vec::new();
    
    if let Ok(content) = fs::read_to_string(path) {
        for (idx, line) in content.lines().enumerate() {
            if let Some(repo) = parse_repo_line(line, &path.to_string_lossy(), idx + 1) {
                repos.push(repo);
            }
        }
    }
    
    repos
}

// ============================================================================
// Tauri Commands (All async)
// ============================================================================

/// Get all APT repositories
#[tauri::command]
pub async fn get_repositories() -> Result<Vec<Repository>> {
    // Spawn blocking since this does file I/O
    let repos = tokio::task::spawn_blocking(|| {
        let mut all_repos = Vec::new();
        
        // Parse main sources.list
        let main_sources = Path::new("/etc/apt/sources.list");
        if main_sources.exists() {
            all_repos.extend(parse_sources_file(main_sources));
        }
        
        // Parse sources.list.d/*.list
        let sources_d = Path::new("/etc/apt/sources.list.d");
        if sources_d.exists() {
            if let Ok(entries) = fs::read_dir(sources_d) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().map(|e| e == "list").unwrap_or(false) {
                        all_repos.extend(parse_sources_file(&path));
                    }
                }
            }
        }
        
        all_repos
    }).await.unwrap();
    
    Ok(repos)
}

/// Toggle repository enabled/disabled (async with timeout)
#[tauri::command]
pub async fn toggle_repository(file_path: String, line_number: usize) -> Result<()> {
    let content = fs::read_to_string(&file_path)?;
    let lines: Vec<&str> = content.lines().collect();
    
    if line_number == 0 || line_number > lines.len() {
        return Err(AppError::System("Invalid line number".to_string()));
    }
    
    let mut new_lines: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
    let line = &new_lines[line_number - 1];
    
    // Toggle comment
    if line.trim().starts_with('#') {
        // Enable: remove leading #
        new_lines[line_number - 1] = line.trim_start_matches('#').trim_start().to_string();
    } else {
        // Disable: add # at start
        new_lines[line_number - 1] = format!("# {}", line);
    }
    
    let new_content = new_lines.join("\n") + "\n";
    
    // Write with sudo
    let script = format!(
        "echo '{}' | tee '{}' > /dev/null",
        new_content.replace("'", "'\\''"),
        file_path
    );
    privileged::run_privileged_shell(&script).await?;
    
    Ok(())
}

/// Add a PPA (async with timeout)
#[tauri::command]
pub async fn add_ppa(ppa: String) -> Result<String> {
    // Validate PPA format: ppa:user/repo
    if !ppa.starts_with("ppa:") {
        return Err(AppError::System("Invalid PPA format. Use ppa:user/repo".to_string()));
    }
    
    privileged::run_privileged("add-apt-repository", &["-y", &ppa]).await
}

/// Remove a PPA (async with timeout)
#[tauri::command]
pub async fn remove_ppa(ppa: String) -> Result<String> {
    if !ppa.starts_with("ppa:") {
        return Err(AppError::System("Invalid PPA format".to_string()));
    }
    
    privileged::run_privileged("add-apt-repository", &["-r", "-y", &ppa]).await
}

/// Get available mirrors
#[tauri::command]
pub fn get_mirrors() -> Vec<MirrorInfo> {
    // Detect if Ubuntu or Debian
    let os_release = fs::read_to_string("/etc/os-release").unwrap_or_default();
    let is_ubuntu = os_release.contains("ubuntu") || os_release.contains("Ubuntu");
    
    let mirrors = if is_ubuntu { UBUNTU_MIRRORS } else { DEBIAN_MIRRORS };
    
    mirrors
        .iter()
        .map(|(name, uri, country)| MirrorInfo {
            name: name.to_string(),
            uri: uri.to_string(),
            country: country.to_string(),
            latency_ms: None,
        })
        .collect()
}

/// Test a single mirror speed (async with 5s timeout)
#[tauri::command]
pub async fn test_mirror_speed(uri: String) -> Result<u64> {
    let start = Instant::now();
    
    // Use reqwest with timeout for async HTTP
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| AppError::Network(format!("Failed to create HTTP client: {}", e)))?;
    
    // Just do a HEAD request to measure latency
    let result = timeout(
        Duration::from_secs(5),
        client.head(&uri).send()
    ).await;
    
    match result {
        Ok(Ok(_)) => Ok(start.elapsed().as_millis() as u64),
        Ok(Err(e)) => Err(AppError::Network(format!("Mirror unreachable: {}", e))),
        Err(_) => Err(AppError::Timeout("Mirror test timed out".to_string())),
    }
}

/// Test ALL mirrors in PARALLEL and return sorted by speed
#[tauri::command]
pub async fn test_all_mirrors() -> Vec<MirrorInfo> {
    let mut mirrors = get_mirrors();
    
    // Create futures for all mirror tests
    let test_futures: Vec<_> = mirrors.iter().map(|m| {
        let uri = m.uri.clone();
        async move {
            test_mirror_speed(uri).await.ok()
        }
    }).collect();
    
    // Run ALL tests in parallel with join_all
    let results = join_all(test_futures).await;
    
    // Assign results to mirrors
    for (mirror, latency) in mirrors.iter_mut().zip(results) {
        mirror.latency_ms = latency;
    }
    
    // Sort by latency (None values at end)
    mirrors.sort_by(|a, b| {
        match (a.latency_ms, b.latency_ms) {
            (Some(a_ms), Some(b_ms)) => a_ms.cmp(&b_ms),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
    });
    
    mirrors
}

/// Set the fastest mirror as primary (async with timeout)
#[tauri::command]
pub async fn set_mirror(new_uri: String) -> Result<String> {
    let sources_path = "/etc/apt/sources.list";
    let content = fs::read_to_string(sources_path)?;
    
    // Replace common mirror patterns
    let patterns = [
        "http://archive.ubuntu.com/ubuntu",
        "http://id.archive.ubuntu.com/ubuntu",
        "http://sg.archive.ubuntu.com/ubuntu",
        "http://deb.debian.org/debian",
    ];
    
    let mut new_content = content.clone();
    for pattern in patterns {
        new_content = new_content.replace(pattern, &new_uri);
    }
    
    if new_content == content {
        return Ok("No changes needed".to_string());
    }
    
    // Write with sudo
    let script = format!(
        "echo '{}' | tee '{}' > /dev/null",
        new_content.replace("'", "'\\''"),
        sources_path
    );
    privileged::run_privileged_shell(&script).await?;
    
    Ok(format!("Mirror changed to {}", new_uri))
}

/// Run apt update (async with timeout)
#[tauri::command]
pub async fn apt_update() -> Result<String> {
    privileged::run_privileged("apt-get", &["update"]).await
}
