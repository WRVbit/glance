//! System cleaner module
//! Handles cleanup of cache, logs, trash, etc.
//! Uses hardcoded safe paths - NO arbitrary path deletion

use crate::error::{AppError, Result};
use crate::utils::privileged;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

// ============================================================================
// Data Structures
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupCategory {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub size_bytes: u64,
    pub file_count: u32,
    pub requires_root: bool,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupResult {
    pub category: String,
    pub success: bool,
    pub bytes_freed: u64,
    pub files_removed: u32,
    pub message: String,
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Calculate directory size recursively
fn get_dir_size(path: &Path) -> (u64, u32) {
    let mut total_size = 0u64;
    let mut file_count = 0u32;

    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata() {
                if metadata.is_file() {
                    total_size += metadata.len();
                    file_count += 1;
                } else if metadata.is_dir() {
                    let (sub_size, sub_count) = get_dir_size(&entry.path());
                    total_size += sub_size;
                    file_count += sub_count;
                }
            }
        }
    }

    (total_size, file_count)
}

/// Get home directory
fn home_dir() -> String {
    std::env::var("HOME").unwrap_or_else(|_| "/home".to_string())
}

/// Safely remove directory contents (not the directory itself)
fn clear_directory(path: &Path) -> Result<(u64, u32)> {
    let (size, count) = get_dir_size(path);

    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                let _ = fs::remove_dir_all(&entry_path);
            } else {
                let _ = fs::remove_file(&entry_path);
            }
        }
    }

    Ok((size, count))
}

// ============================================================================
// Cleanup Categories
// ============================================================================

/// Get all cleanup categories with their current sizes
#[tauri::command]
pub fn get_cleanup_categories() -> Result<Vec<CleanupCategory>> {
    let home = home_dir();
    let mut categories = Vec::new();

    // 1. User Trash
    let trash_path = format!("{}/.local/share/Trash/files", home);
    let (trash_size, trash_count) = get_dir_size(Path::new(&trash_path));
    categories.push(CleanupCategory {
        id: "trash".to_string(),
        name: "Trash".to_string(),
        icon: "🗑️".to_string(),
        size_bytes: trash_size,
        file_count: trash_count,
        requires_root: false,
        description: "Files in your trash folder".to_string(),
    });

    // 2. Thumbnail Cache
    let thumb_path = format!("{}/.cache/thumbnails", home);
    let (thumb_size, thumb_count) = get_dir_size(Path::new(&thumb_path));
    categories.push(CleanupCategory {
        id: "thumbnails".to_string(),
        name: "Thumbnail Cache".to_string(),
        icon: "🖼️".to_string(),
        size_bytes: thumb_size,
        file_count: thumb_count,
        requires_root: false,
        description: "Cached image thumbnails".to_string(),
    });

    // 3. Browser Cache (common locations)
    let mut browser_size = 0u64;
    let mut browser_count = 0u32;
    let browser_paths = [
        format!("{}/.cache/google-chrome", home),
        format!("{}/.cache/chromium", home),
        format!("{}/.cache/mozilla/firefox", home),
        format!("{}/.cache/BraveSoftware", home),
    ];
    for path in &browser_paths {
        let (s, c) = get_dir_size(Path::new(path));
        browser_size += s;
        browser_count += c;
    }
    categories.push(CleanupCategory {
        id: "browser_cache".to_string(),
        name: "Browser Cache".to_string(),
        icon: "🌐".to_string(),
        size_bytes: browser_size,
        file_count: browser_count,
        requires_root: false,
        description: "Chrome, Firefox, Brave cache files".to_string(),
    });

    // 4. Application Cache
    let app_cache_path = format!("{}/.cache", home);
    let (cache_size, cache_count) = get_dir_size(Path::new(&app_cache_path));
    // Subtract browser cache and thumbnails to avoid double counting
    let app_cache_size = cache_size.saturating_sub(browser_size + thumb_size);
    categories.push(CleanupCategory {
        id: "app_cache".to_string(),
        name: "Application Cache".to_string(),
        icon: "📦".to_string(),
        size_bytes: app_cache_size,
        file_count: cache_count.saturating_sub(browser_count + thumb_count),
        requires_root: false,
        description: "Other application cache files".to_string(),
    });

    // 5. APT Cache (requires root to clean)
    let apt_path = Path::new("/var/cache/apt/archives");
    let (apt_size, apt_count) = get_dir_size(apt_path);
    categories.push(CleanupCategory {
        id: "apt_cache".to_string(),
        name: "APT Package Cache".to_string(),
        icon: "📥".to_string(),
        size_bytes: apt_size,
        file_count: apt_count,
        requires_root: true,
        description: "Downloaded package files".to_string(),
    });

    // 6. Journal Logs (requires root)
    let journal_size = get_journal_size();
    categories.push(CleanupCategory {
        id: "journal".to_string(),
        name: "System Logs".to_string(),
        icon: "📝".to_string(),
        size_bytes: journal_size,
        file_count: 0,
        requires_root: true,
        description: "Systemd journal logs".to_string(),
    });

    // 7. Old Kernels (requires root)
    // This is just informational - actual removal via apt
    categories.push(CleanupCategory {
        id: "old_kernels".to_string(),
        name: "Old Kernels".to_string(),
        icon: "🐧".to_string(),
        size_bytes: 0, // Calculated separately
        file_count: 0,
        requires_root: true,
        description: "Unused kernel versions".to_string(),
    });

    Ok(categories)
}

/// Get systemd journal disk usage
fn get_journal_size() -> u64 {
    if let Ok(output) = std::process::Command::new("journalctl")
        .args(["--disk-usage"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // Parse "Archived and active journals take up X.X M/G"
        if let Some(line) = stdout.lines().next() {
            // Extract number and unit
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 7 {
                if let Ok(size) = parts[6].parse::<f64>() {
                    let multiplier = if line.contains('G') {
                        1024 * 1024 * 1024
                    } else if line.contains('M') {
                        1024 * 1024
                    } else {
                        1024
                    };
                    return (size * multiplier as f64) as u64;
                }
            }
        }
    }
    0
}

// ============================================================================
// Cleanup Actions
// ============================================================================

/// Clean a specific category
#[tauri::command]
pub fn clean_category(category_id: String) -> Result<CleanupResult> {
    let home = home_dir();

    match category_id.as_str() {
        "trash" => {
            let trash_files = format!("{}/.local/share/Trash/files", home);
            let trash_info = format!("{}/.local/share/Trash/info", home);

            let (size, count) = get_dir_size(Path::new(&trash_files));

            // Clear contents
            let _ = clear_directory(Path::new(&trash_files));
            let _ = clear_directory(Path::new(&trash_info));

            Ok(CleanupResult {
                category: "trash".to_string(),
                success: true,
                bytes_freed: size,
                files_removed: count,
                message: "Trash emptied successfully".to_string(),
            })
        }

        "thumbnails" => {
            let thumb_path = format!("{}/.cache/thumbnails", home);
            let (size, count) = clear_directory(Path::new(&thumb_path))?;

            Ok(CleanupResult {
                category: "thumbnails".to_string(),
                success: true,
                bytes_freed: size,
                files_removed: count,
                message: "Thumbnail cache cleared".to_string(),
            })
        }

        "browser_cache" => {
            let browser_paths = [
                format!("{}/.cache/google-chrome/Default/Cache", home),
                format!("{}/.cache/chromium/Default/Cache", home),
                format!("{}/.cache/BraveSoftware/Brave-Browser/Default/Cache", home),
            ];

            let mut total_size = 0u64;
            let mut total_count = 0u32;

            for path in &browser_paths {
                if let Ok((s, c)) = clear_directory(Path::new(path)) {
                    total_size += s;
                    total_count += c;
                }
            }

            Ok(CleanupResult {
                category: "browser_cache".to_string(),
                success: true,
                bytes_freed: total_size,
                files_removed: total_count,
                message: "Browser cache cleared".to_string(),
            })
        }

        "apt_cache" => {
            // Requires root
            let result = privileged::run_privileged("apt-get", &["clean"]);

            match result {
                Ok(_) => Ok(CleanupResult {
                    category: "apt_cache".to_string(),
                    success: true,
                    bytes_freed: 0,
                    files_removed: 0,
                    message: "APT cache cleaned".to_string(),
                }),
                Err(AppError::UserCancelled) => Ok(CleanupResult {
                    category: "apt_cache".to_string(),
                    success: false,
                    bytes_freed: 0,
                    files_removed: 0,
                    message: "Operation cancelled by user".to_string(),
                }),
                Err(e) => Err(e),
            }
        }

        "journal" => {
            // Vacuum to 100MB
            let result = privileged::run_privileged("journalctl", &["--vacuum-size=100M"]);

            match result {
                Ok(output) => Ok(CleanupResult {
                    category: "journal".to_string(),
                    success: true,
                    bytes_freed: 0,
                    files_removed: 0,
                    message: output,
                }),
                Err(AppError::UserCancelled) => Ok(CleanupResult {
                    category: "journal".to_string(),
                    success: false,
                    bytes_freed: 0,
                    files_removed: 0,
                    message: "Operation cancelled by user".to_string(),
                }),
                Err(e) => Err(e),
            }
        }

        "old_kernels" => {
            // Use apt autoremove
            let result = privileged::run_privileged("apt-get", &["autoremove", "-y"]);

            match result {
                Ok(_) => Ok(CleanupResult {
                    category: "old_kernels".to_string(),
                    success: true,
                    bytes_freed: 0,
                    files_removed: 0,
                    message: "Old packages removed".to_string(),
                }),
                Err(AppError::UserCancelled) => Ok(CleanupResult {
                    category: "old_kernels".to_string(),
                    success: false,
                    bytes_freed: 0,
                    files_removed: 0,
                    message: "Operation cancelled by user".to_string(),
                }),
                Err(e) => Err(e),
            }
        }

        _ => Err(AppError::System(format!(
            "Unknown cleanup category: {}",
            category_id
        ))),
    }
}

/// Get total reclaimable space
#[tauri::command]
pub fn get_total_reclaimable() -> Result<u64> {
    let categories = get_cleanup_categories()?;
    Ok(categories.iter().map(|c| c.size_bytes).sum())
}
