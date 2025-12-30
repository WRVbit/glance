//! System cleaner module - Enhanced
//! Handles cleanup of cache, logs, trash, etc. (async)
//! Uses distro-agnostic paths via DistroContext

use crate::error::{AppError, Result};
use crate::state::AppState;
use crate::utils::privileged;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use tauri::State;

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

/// Clear multiple directories and return total size/count
fn clear_directories(paths: &[String]) -> (u64, u32) {
    let mut total_size = 0u64;
    let mut total_count = 0u32;

    for path in paths {
        let p = Path::new(path);
        if p.exists() {
            if let Ok((s, c)) = clear_directory(p) {
                total_size += s;
                total_count += c;
            }
        }
    }

    (total_size, total_count)
}

/// Get size of multiple directories
fn get_dirs_size(paths: &[String]) -> (u64, u32) {
    let mut total_size = 0u64;
    let mut total_count = 0u32;

    for path in paths {
        let (s, c) = get_dir_size(Path::new(path));
        total_size += s;
        total_count += c;
    }

    (total_size, total_count)
}

// ============================================================================
// Cleanup Categories
// ============================================================================

/// Get all cleanup categories with their current sizes (async)
#[tauri::command]
pub async fn get_cleanup_categories(state: State<'_, AppState>) -> Result<Vec<CleanupCategory>> {
    let pkg_cache_path = state.context.paths.package_cache.clone();
    let pm_name = state.context.package_manager.name().to_string();
    
    let categories = tokio::task::spawn_blocking(move || {
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
            description: "Deleted files awaiting permanent removal. Safe to empty if you don't need to recover anything.".to_string(),
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
            description: "Cached preview images for file managers. Will be regenerated when browsing folders.".to_string(),
        });

        // 3. Browser Cache
        let browser_paths = vec![
            format!("{}/.cache/google-chrome", home),
            format!("{}/.cache/chromium", home),
            format!("{}/.cache/mozilla/firefox", home),
            format!("{}/.cache/BraveSoftware", home),
            format!("{}/.cache/vivaldi", home),
            format!("{}/.cache/opera", home),
        ];
        let (browser_size, browser_count) = get_dirs_size(&browser_paths);
        categories.push(CleanupCategory {
            id: "browser_cache".to_string(),
            name: "Browser Cache".to_string(),
            icon: "🌐".to_string(),
            size_bytes: browser_size,
            file_count: browser_count,
            requires_root: false,
            description: "Cached web pages, images, scripts from Chrome, Firefox, Brave, etc. Browsers will re-download when needed.".to_string(),
        });

        // 4. Pip/Python Cache
        let pip_paths = vec![
            format!("{}/.cache/pip", home),
            format!("{}/.cache/pipx", home),
        ];
        let (pip_size, pip_count) = get_dirs_size(&pip_paths);
        if pip_size > 0 {
            categories.push(CleanupCategory {
                id: "pip_cache".to_string(),
                name: "Python/Pip Cache".to_string(),
                icon: "🐍".to_string(),
                size_bytes: pip_size,
                file_count: pip_count,
                requires_root: false,
                description: "Downloaded Python packages and wheels. Pip will re-download packages when needed.".to_string(),
            });
        }

        // 5. npm/yarn Cache
        let npm_paths = vec![
            format!("{}/.npm/_cacache", home),
            format!("{}/.cache/yarn", home),
            format!("{}/.cache/pnpm", home),
        ];
        let (npm_size, npm_count) = get_dirs_size(&npm_paths);
        if npm_size > 0 {
            categories.push(CleanupCategory {
                id: "npm_cache".to_string(),
                name: "Node.js Cache".to_string(),
                icon: "📦".to_string(),
                size_bytes: npm_size,
                file_count: npm_count,
                requires_root: false,
                description: "npm, yarn, and pnpm package cache. Packages will be re-downloaded when installing.".to_string(),
            });
        }

        // 6. VSCode Cache
        let vscode_paths = vec![
            format!("{}/.config/Code/Cache", home),
            format!("{}/.config/Code/CachedData", home),
            format!("{}/.config/Code/CachedExtensions", home),
            format!("{}/.config/Code/CachedExtensionVSIXs", home),
            format!("{}/.config/Code - OSS/Cache", home),
        ];
        let (vscode_size, vscode_count) = get_dirs_size(&vscode_paths);
        if vscode_size > 0 {
            categories.push(CleanupCategory {
                id: "vscode_cache".to_string(),
                name: "VSCode Cache".to_string(),
                icon: "💻".to_string(),
                size_bytes: vscode_size,
                file_count: vscode_count,
                requires_root: false,
                description: "Visual Studio Code cached data and extensions. VSCode will recreate these on startup.".to_string(),
            });
        }

        // 7. Mesa Shader Cache (GPU)
        let mesa_path = format!("{}/.cache/mesa_shader_cache", home);
        let nvidia_path = format!("{}/.cache/nvidia", home);
        let (mesa_size, mesa_count) = get_dirs_size(&[mesa_path.clone(), nvidia_path]);
        if mesa_size > 0 {
            categories.push(CleanupCategory {
                id: "shader_cache".to_string(),
                name: "GPU Shader Cache".to_string(),
                icon: "🎮".to_string(),
                size_bytes: mesa_size,
                file_count: mesa_count,
                requires_root: false,
                description: "Compiled GPU shaders for games and apps. Will be recompiled on first use (may cause brief stutter).".to_string(),
            });
        }

        // 8. Font Cache
        let font_path = format!("{}/.cache/fontconfig", home);
        let (font_size, font_count) = get_dir_size(Path::new(&font_path));
        if font_size > 0 {
            categories.push(CleanupCategory {
                id: "font_cache".to_string(),
                name: "Font Cache".to_string(),
                icon: "🔤".to_string(),
                size_bytes: font_size,
                file_count: font_count,
                requires_root: false,
                description: "Cached font metadata and rendered glyphs. Run 'fc-cache' after cleaning to rebuild.".to_string(),
            });
        }

        // 9. Flatpak Cache (if exists)
        let flatpak_path = format!("{}/.cache/flatpak", home);
        let (flatpak_size, flatpak_count) = get_dir_size(Path::new(&flatpak_path));
        if flatpak_size > 0 {
            categories.push(CleanupCategory {
                id: "flatpak_cache".to_string(),
                name: "Flatpak Cache".to_string(),
                icon: "📤".to_string(),
                size_bytes: flatpak_size,
                file_count: flatpak_count,
                requires_root: false,
                description: "Flatpak application cache and temporary files. Apps may take longer to start after cleaning.".to_string(),
            });
        }

        // 10. Crash Reports
        let crash_paths = vec![
            format!("{}/.local/share/apport", home),
            "/var/crash".to_string(),
        ];
        let (crash_size, crash_count) = get_dirs_size(&crash_paths);
        if crash_size > 0 {
            categories.push(CleanupCategory {
                id: "crash_reports".to_string(),
                name: "Crash Reports".to_string(),
                icon: "💥".to_string(),
                size_bytes: crash_size,
                file_count: crash_count,
                requires_root: false,
                description: "Application crash dumps and reports. Safe to remove unless debugging an issue.".to_string(),
            });
        }

        // 11. Recent Documents
        let recent_path = format!("{}/.local/share/recently-used.xbel", home);
        let recent_size = Path::new(&recent_path).metadata().map(|m| m.len()).unwrap_or(0);
        if recent_size > 1024 * 10 { // Only show if > 10KB
            categories.push(CleanupCategory {
                id: "recent_docs".to_string(),
                name: "Recent Documents".to_string(),
                icon: "📋".to_string(),
                size_bytes: recent_size,
                file_count: 1,
                requires_root: false,
                description: "History of recently opened files. Clears the 'Recent' section in file manager.".to_string(),
            });
        }

        // 12. Package Cache (distro-agnostic)
        let (pkg_size, pkg_count) = get_dir_size(Path::new(&pkg_cache_path));
        categories.push(CleanupCategory {
            id: "pkg_cache".to_string(),
            name: format!("{} Package Cache", pm_name.to_uppercase()),
            icon: "📥".to_string(),
            size_bytes: pkg_size,
            file_count: pkg_count,
            requires_root: true,
            description: "Downloaded package files. Safe to remove - packages will be re-downloaded if needed.".to_string(),
        });

        // 13. Snap Cache (requires root)
        let snap_path = Path::new("/var/cache/snapd");
        let (snap_size, snap_count) = get_dir_size(snap_path);
        if snap_size > 0 {
            categories.push(CleanupCategory {
                id: "snap_cache".to_string(),
                name: "Snap Cache".to_string(),
                icon: "🔷".to_string(),
                size_bytes: snap_size,
                file_count: snap_count,
                requires_root: true,
                description: "Snap package cache and assertions. Safe to clean for package manager maintenance.".to_string(),
            });
        }

        // 14. Journal Logs (requires root)
        let journal_size = get_journal_size_sync();
        categories.push(CleanupCategory {
            id: "journal".to_string(),
            name: "System Logs".to_string(),
            icon: "📝".to_string(),
            size_bytes: journal_size,
            file_count: 0,
            requires_root: true,
            description: "Systemd journal logs. Keeps last 100MB after cleaning for troubleshooting.".to_string(),
        });

        // 15. Old Rotated Logs (requires root)
        let old_logs_size = get_old_logs_size_sync();
        if old_logs_size > 0 {
            categories.push(CleanupCategory {
                id: "old_logs".to_string(),
                name: "Old Log Files".to_string(),
                icon: "📄".to_string(),
                size_bytes: old_logs_size,
                file_count: 0,
                requires_root: true,
                description: "Compressed and rotated log files (*.gz, *.old). Old logs are no longer needed.".to_string(),
            });
        }

        // 16. Old Kernels (requires root)
        categories.push(CleanupCategory {
            id: "old_kernels".to_string(),
            name: "Old Kernels & Packages".to_string(),
            icon: "🐧".to_string(),
            size_bytes: 0, // Can't easily calculate
            file_count: 0,
            requires_root: true,
            description: "Unused kernel versions and orphaned packages. Runs 'apt autoremove' to clean.".to_string(),
        });

        categories
    }).await.unwrap();

    Ok(categories)
}

/// Get systemd journal disk usage (sync helper)
fn get_journal_size_sync() -> u64 {
    if let Ok(output) = std::process::Command::new("journalctl")
        .args(["--disk-usage"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // Parse output like "Archived and active journals take up 256.0M in the file system."
        for word in stdout.split_whitespace() {
            if word.ends_with('M') || word.ends_with('G') || word.ends_with('K') {
                let num_str = &word[..word.len()-1];
                if let Ok(size) = num_str.parse::<f64>() {
                    let multiplier = match word.chars().last() {
                        Some('G') => 1024.0 * 1024.0 * 1024.0,
                        Some('M') => 1024.0 * 1024.0,
                        Some('K') => 1024.0,
                        _ => 1.0,
                    };
                    return (size * multiplier) as u64;
                }
            }
        }
    }
    0
}

/// Get old rotated log files size
fn get_old_logs_size_sync() -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = fs::read_dir("/var/log") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".gz") || name.ends_with(".old") || name.ends_with(".1") {
                if let Ok(meta) = entry.metadata() {
                    total += meta.len();
                }
            }
        }
    }
    total
}

// ============================================================================
// Cleanup Actions (All async)
// ============================================================================

/// Preview cleanup (dry run) - shows what would be deleted without actually deleting
/// Returns the cleanup result with calculated size but no actual deletion
#[tauri::command]
pub async fn preview_cleanup(category_id: String, state: State<'_, AppState>) -> Result<CleanupResult> {
    // Just get the category info - this is already a "dry run" calculation
    let categories = get_cleanup_categories(state).await?;
    
    if let Some(cat) = categories.iter().find(|c| c.id == category_id) {
        Ok(CleanupResult {
            category: category_id,
            success: true,
            bytes_freed: cat.size_bytes,
            files_removed: cat.file_count,
            message: format!("Preview: Would free {} bytes from {} files", cat.size_bytes, cat.file_count),
        })
    } else {
        Err(AppError::System(format!("Unknown category: {}", category_id)))
    }
}

/// Clean a specific category (async with timeout for root ops)
#[tauri::command]
pub async fn clean_category(category_id: String, state: State<'_, AppState>) -> Result<CleanupResult> {
    let home = home_dir();

    match category_id.as_str() {
        "trash" => {
            let trash_files = format!("{}/.local/share/Trash/files", home);
            let trash_info = format!("{}/.local/share/Trash/info", home);

            let result = tokio::task::spawn_blocking(move || {
                let (size, count) = get_dir_size(Path::new(&trash_files));
                let _ = clear_directory(Path::new(&trash_files));
                let _ = clear_directory(Path::new(&trash_info));
                (size, count)
            }).await.unwrap();

            Ok(CleanupResult {
                category: "trash".to_string(),
                success: true,
                bytes_freed: result.0,
                files_removed: result.1,
                message: "Trash emptied successfully".to_string(),
            })
        }

        "thumbnails" => {
            let thumb_path = format!("{}/.cache/thumbnails", home);
            let result = tokio::task::spawn_blocking(move || {
                clear_directory(Path::new(&thumb_path))
            }).await.unwrap()?;

            Ok(CleanupResult {
                category: "thumbnails".to_string(),
                success: true,
                bytes_freed: result.0,
                files_removed: result.1,
                message: "Thumbnail cache cleared".to_string(),
            })
        }

        "browser_cache" => {
            let result = tokio::task::spawn_blocking(move || {
                let paths = vec![
                    format!("{}/.cache/google-chrome/Default/Cache", home),
                    format!("{}/.cache/google-chrome/Default/Code Cache", home),
                    format!("{}/.cache/chromium/Default/Cache", home),
                    format!("{}/.cache/BraveSoftware/Brave-Browser/Default/Cache", home),
                    format!("{}/.cache/vivaldi/Default/Cache", home),
                    format!("{}/.cache/opera/Cache", home),
                    // Firefox uses different structure
                    format!("{}/.cache/mozilla/firefox", home),
                ];
                clear_directories(&paths)
            }).await.unwrap();

            Ok(CleanupResult {
                category: "browser_cache".to_string(),
                success: true,
                bytes_freed: result.0,
                files_removed: result.1,
                message: "Browser cache cleared".to_string(),
            })
        }

        "pip_cache" => {
            let result = tokio::task::spawn_blocking(move || {
                let paths = vec![
                    format!("{}/.cache/pip", home),
                    format!("{}/.cache/pipx", home),
                ];
                clear_directories(&paths)
            }).await.unwrap();

            Ok(CleanupResult {
                category: "pip_cache".to_string(),
                success: true,
                bytes_freed: result.0,
                files_removed: result.1,
                message: "Python cache cleared".to_string(),
            })
        }

        "npm_cache" => {
            let result = tokio::task::spawn_blocking(move || {
                let paths = vec![
                    format!("{}/.npm/_cacache", home),
                    format!("{}/.cache/yarn", home),
                    format!("{}/.cache/pnpm", home),
                ];
                clear_directories(&paths)
            }).await.unwrap();

            Ok(CleanupResult {
                category: "npm_cache".to_string(),
                success: true,
                bytes_freed: result.0,
                files_removed: result.1,
                message: "Node.js cache cleared".to_string(),
            })
        }

        "vscode_cache" => {
            let result = tokio::task::spawn_blocking(move || {
                let paths = vec![
                    format!("{}/.config/Code/Cache", home),
                    format!("{}/.config/Code/CachedData", home),
                    format!("{}/.config/Code/CachedExtensions", home),
                    format!("{}/.config/Code/CachedExtensionVSIXs", home),
                    format!("{}/.config/Code - OSS/Cache", home),
                ];
                clear_directories(&paths)
            }).await.unwrap();

            Ok(CleanupResult {
                category: "vscode_cache".to_string(),
                success: true,
                bytes_freed: result.0,
                files_removed: result.1,
                message: "VSCode cache cleared".to_string(),
            })
        }

        "shader_cache" => {
            let result = tokio::task::spawn_blocking(move || {
                let paths = vec![
                    format!("{}/.cache/mesa_shader_cache", home),
                    format!("{}/.cache/nvidia", home),
                ];
                clear_directories(&paths)
            }).await.unwrap();

            Ok(CleanupResult {
                category: "shader_cache".to_string(),
                success: true,
                bytes_freed: result.0,
                files_removed: result.1,
                message: "GPU shader cache cleared".to_string(),
            })
        }

        "font_cache" => {
            let font_path = format!("{}/.cache/fontconfig", home);
            let result = tokio::task::spawn_blocking(move || {
                clear_directory(Path::new(&font_path))
            }).await.unwrap()?;

            // Rebuild font cache
            let _ = std::process::Command::new("fc-cache").arg("-f").output();

            Ok(CleanupResult {
                category: "font_cache".to_string(),
                success: true,
                bytes_freed: result.0,
                files_removed: result.1,
                message: "Font cache cleared and rebuilt".to_string(),
            })
        }

        "flatpak_cache" => {
            let flatpak_path = format!("{}/.cache/flatpak", home);
            let result = tokio::task::spawn_blocking(move || {
                clear_directory(Path::new(&flatpak_path))
            }).await.unwrap()?;

            Ok(CleanupResult {
                category: "flatpak_cache".to_string(),
                success: true,
                bytes_freed: result.0,
                files_removed: result.1,
                message: "Flatpak cache cleared".to_string(),
            })
        }

        "crash_reports" => {
            let result = tokio::task::spawn_blocking(move || {
                let paths = vec![
                    format!("{}/.local/share/apport", home),
                ];
                clear_directories(&paths)
            }).await.unwrap();

            // Also try to clear /var/crash (may need root)
            let _ = privileged::run_privileged_shell("rm -f /var/crash/*").await;

            Ok(CleanupResult {
                category: "crash_reports".to_string(),
                success: true,
                bytes_freed: result.0,
                files_removed: result.1,
                message: "Crash reports cleared".to_string(),
            })
        }

        "recent_docs" => {
            let recent_path = format!("{}/.local/share/recently-used.xbel", home);
            let size = Path::new(&recent_path).metadata().map(|m| m.len()).unwrap_or(0);
            
            // Write empty file instead of deleting (GNOME expects it to exist)
            let _ = fs::write(&recent_path, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<xbel version=\"1.0\"/>\n");

            Ok(CleanupResult {
                category: "recent_docs".to_string(),
                success: true,
                bytes_freed: size,
                files_removed: 1,
                message: "Recent documents history cleared".to_string(),
            })
        }

        "pkg_cache" | "apt_cache" => {
            let result = state.context.package_manager.clean_cache().await;
            
            match result {
                Ok(cleanup) => Ok(CleanupResult {
                    category: "pkg_cache".to_string(),
                    success: cleanup.success,
                    bytes_freed: cleanup.bytes_freed,
                    files_removed: cleanup.items_removed,
                    message: cleanup.message,
                }),
                Err(e) => Err(e),
            }
        }

        "snap_cache" => {
            // Clean old snap revisions
            let result = privileged::run_privileged_shell(
                "snap list --all | awk '/disabled/{print $1, $3}' | while read snapname revision; do snap remove \"$snapname\" --revision=\"$revision\"; done"
            ).await;

            match result {
                Ok(_) => Ok(CleanupResult {
                    category: "snap_cache".to_string(),
                    success: true,
                    bytes_freed: 0,
                    files_removed: 0,
                    message: "Old snap revisions removed".to_string(),
                }),
                Err(AppError::UserCancelled) => Ok(CleanupResult {
                    category: "snap_cache".to_string(),
                    success: false,
                    bytes_freed: 0,
                    files_removed: 0,
                    message: "Operation cancelled by user".to_string(),
                }),
                Err(_) => Ok(CleanupResult {
                    category: "snap_cache".to_string(),
                    success: false,
                    bytes_freed: 0,
                    files_removed: 0,
                    message: "No old snap revisions found or snap not installed".to_string(),
                }),
            }
        }

        "journal" => {
            let result = privileged::run_privileged("journalctl", &["--vacuum-size=100M"]).await;

            match result {
                Ok(output) => Ok(CleanupResult {
                    category: "journal".to_string(),
                    success: true,
                    bytes_freed: 0,
                    files_removed: 0,
                    message: if output.len() > 100 { "Journal vacuumed to 100MB".to_string() } else { output },
                }),
                Err(AppError::UserCancelled) => Ok(CleanupResult {
                    category: "journal".to_string(),
                    success: false,
                    bytes_freed: 0,
                    files_removed: 0,
                    message: "Operation cancelled by user".to_string(),
                }),
                Err(AppError::Timeout(msg)) => Ok(CleanupResult {
                    category: "journal".to_string(),
                    success: false,
                    bytes_freed: 0,
                    files_removed: 0,
                    message: msg,
                }),
                Err(e) => Err(e),
            }
        }

        "old_logs" => {
            let result = privileged::run_privileged_shell(
                "find /var/log -type f \\( -name '*.gz' -o -name '*.old' -o -name '*.1' \\) -delete"
            ).await;

            match result {
                Ok(_) => Ok(CleanupResult {
                    category: "old_logs".to_string(),
                    success: true,
                    bytes_freed: 0,
                    files_removed: 0,
                    message: "Old log files removed".to_string(),
                }),
                Err(AppError::UserCancelled) => Ok(CleanupResult {
                    category: "old_logs".to_string(),
                    success: false,
                    bytes_freed: 0,
                    files_removed: 0,
                    message: "Operation cancelled by user".to_string(),
                }),
                Err(_) => Ok(CleanupResult {
                    category: "old_logs".to_string(),
                    success: false,
                    bytes_freed: 0,
                    files_removed: 0,
                    message: "Failed to remove old logs".to_string(),
                }),
            }
        }

        "old_kernels" => {
            let result = state.context.package_manager.autoremove().await;
            
            match result {
                Ok(action) => Ok(CleanupResult {
                    category: "old_kernels".to_string(),
                    success: action.success,
                    bytes_freed: 0,
                    files_removed: 0,
                    message: action.message,
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

/// Get total reclaimable space (async)
#[tauri::command]
pub async fn get_total_reclaimable(state: State<'_, AppState>) -> Result<u64> {
    let categories = get_cleanup_categories(state).await?;
    Ok(categories.iter().map(|c| c.size_bytes).sum())
}

// ============================================================================
// Scheduled Cleaning (systemd user timers)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleConfig {
    pub enabled: bool,
    pub interval: String, // "daily", "weekly", "monthly"
    pub categories: Vec<String>, // which categories to auto-clean
    pub last_run: Option<String>,
}

const SERVICE_NAME: &str = "glance-autoclean";

/// Get current schedule configuration
#[tauri::command]
pub async fn get_autoclean_schedule() -> Result<ScheduleConfig> {
    let home = home_dir();
    let config_path = format!("{}/.config/glance/autoclean.json", home);
    
    if let Ok(content) = fs::read_to_string(&config_path) {
        if let Ok(config) = serde_json::from_str::<ScheduleConfig>(&content) {
            return Ok(config);
        }
    }
    
    // Default config
    Ok(ScheduleConfig {
        enabled: false,
        interval: "weekly".to_string(),
        categories: vec![
            "trash".to_string(),
            "thumbnails".to_string(),
            "browser_cache".to_string(),
        ],
        last_run: None,
    })
}

/// Enable scheduled cleaning
#[tauri::command]
pub async fn set_autoclean_schedule(config: ScheduleConfig) -> Result<String> {
    let home = home_dir();
    let config_dir = format!("{}/.config/glance", home);
    let systemd_dir = format!("{}/.config/systemd/user", home);
    
    // Create directories
    let _ = fs::create_dir_all(&config_dir);
    let _ = fs::create_dir_all(&systemd_dir);
    
    // Save config
    let config_path = format!("{}/autoclean.json", config_dir);
    let config_json = serde_json::to_string_pretty(&config)
        .map_err(|e| AppError::System(e.to_string()))?;
    fs::write(&config_path, &config_json)
        .map_err(|e| AppError::System(e.to_string()))?;
    
    if config.enabled {
        // Create the cleanup script
        let script_path = format!("{}/autoclean.sh", config_dir);
        let categories_arg = config.categories.join(",");
        let script_content = format!(r#"#!/bin/bash
# Glance Auto-Clean Script
# Generated automatically - do not edit

CATEGORIES="{}"
HOME_DIR="{}"

# Clean each category
for cat in $(echo $CATEGORIES | tr ',' ' '); do
    case $cat in
        trash)
            rm -rf "$HOME_DIR/.local/share/Trash/files/"* 2>/dev/null
            rm -rf "$HOME_DIR/.local/share/Trash/info/"* 2>/dev/null
            ;;
        thumbnails)
            rm -rf "$HOME_DIR/.cache/thumbnails/"* 2>/dev/null
            ;;
        browser_cache)
            rm -rf "$HOME_DIR/.cache/google-chrome/Default/Cache/"* 2>/dev/null
            rm -rf "$HOME_DIR/.cache/chromium/Default/Cache/"* 2>/dev/null
            rm -rf "$HOME_DIR/.cache/BraveSoftware/Brave-Browser/Default/Cache/"* 2>/dev/null
            rm -rf "$HOME_DIR/.cache/mozilla/firefox/"*.default*/cache2/* 2>/dev/null
            ;;
        pip_cache)
            rm -rf "$HOME_DIR/.cache/pip/"* 2>/dev/null
            ;;
        npm_cache)
            rm -rf "$HOME_DIR/.npm/_cacache/"* 2>/dev/null
            rm -rf "$HOME_DIR/.cache/yarn/"* 2>/dev/null
            ;;
        vscode_cache)
            rm -rf "$HOME_DIR/.config/Code/Cache/"* 2>/dev/null
            rm -rf "$HOME_DIR/.config/Code/CachedData/"* 2>/dev/null
            ;;
        shader_cache)
            rm -rf "$HOME_DIR/.cache/mesa_shader_cache/"* 2>/dev/null
            ;;
        font_cache)
            rm -rf "$HOME_DIR/.cache/fontconfig/"* 2>/dev/null
            fc-cache -f 2>/dev/null
            ;;
    esac
done

# Update last run time
echo "{{\\"last_run\\": \\"$(date -Iseconds)\\"}}" > "$HOME_DIR/.config/glance/last_autoclean.json"
"#, categories_arg, home);
        
        fs::write(&script_path, script_content)
            .map_err(|e| AppError::System(e.to_string()))?;
        
        // Make executable
        let _ = std::process::Command::new("chmod")
            .args(["+x", &script_path])
            .output();
        
        // Create systemd service
        let service_content = format!(r#"[Unit]
Description=Glance Auto-Clean Service
After=default.target

[Service]
Type=oneshot
ExecStart=/bin/bash {}
"#, script_path);
        
        let service_path = format!("{}/{}.service", systemd_dir, SERVICE_NAME);
        fs::write(&service_path, service_content)
            .map_err(|e| AppError::System(e.to_string()))?;
        
        // Create systemd timer
        let timer_schedule = match config.interval.as_str() {
            "daily" => "OnCalendar=daily",
            "weekly" => "OnCalendar=weekly",
            "monthly" => "OnCalendar=monthly",
            _ => "OnCalendar=weekly",
        };
        
        let timer_content = format!(r#"[Unit]
Description=Glance Auto-Clean Timer

[Timer]
{}
Persistent=true
RandomizedDelaySec=1h

[Install]
WantedBy=timers.target
"#, timer_schedule);
        
        let timer_path = format!("{}/{}.timer", systemd_dir, SERVICE_NAME);
        fs::write(&timer_path, timer_content)
            .map_err(|e| AppError::System(e.to_string()))?;
        
        // Reload and enable timer
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .output();
        
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "enable", "--now", &format!("{}.timer", SERVICE_NAME)])
            .output();
        
        Ok(format!("Auto-clean enabled ({})", config.interval))
    } else {
        // Disable timer
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "disable", "--now", &format!("{}.timer", SERVICE_NAME)])
            .output();
        
        // Remove files
        let _ = fs::remove_file(format!("{}/{}.timer", systemd_dir, SERVICE_NAME));
        let _ = fs::remove_file(format!("{}/{}.service", systemd_dir, SERVICE_NAME));
        let _ = fs::remove_file(format!("{}/autoclean.sh", config_dir));
        
        Ok("Auto-clean disabled".to_string())
    }
}

/// Get timer status
#[tauri::command]
pub async fn get_autoclean_status() -> Result<String> {
    let output = std::process::Command::new("systemctl")
        .args(["--user", "status", &format!("{}.timer", SERVICE_NAME)])
        .output();
    
    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if stdout.contains("Active: active") {
                // Extract next trigger time
                for line in stdout.lines() {
                    if line.contains("Trigger:") {
                        return Ok(line.trim().to_string());
                    }
                }
                Ok("Active".to_string())
            } else {
                Ok("Inactive".to_string())
            }
        }
        Err(_) => Ok("Not configured".to_string()),
    }
}

/// Run auto-clean now (manual trigger)
#[tauri::command]
pub async fn run_autoclean_now(state: State<'_, AppState>) -> Result<String> {
    let config = get_autoclean_schedule().await?;
    
    if config.categories.is_empty() {
        return Ok("No categories configured".to_string());
    }
    
    let mut cleaned = Vec::new();
    for cat in &config.categories {
        if let Ok(result) = clean_category(cat.clone(), state.clone()).await {
            if result.success {
                cleaned.push(cat.clone());
            }
        }
    }
    
    Ok(format!("Cleaned {} categories", cleaned.len()))
}
