//! Startup applications module
//! Manages autostart entries from ~/.config/autostart and /etc/xdg/autostart

use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

// ============================================================================
// Data Structures
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartupApp {
    pub name: String,
    pub comment: String,
    pub exec: String,
    pub icon: String,
    pub file_path: String,
    pub is_enabled: bool,
    pub is_system: bool, // true if in /etc/xdg/autostart
}

// ============================================================================
// Helper Functions
// ============================================================================

fn home_dir() -> String {
    std::env::var("HOME").unwrap_or_else(|_| "/home".to_string())
}

fn user_autostart_dir() -> PathBuf {
    PathBuf::from(format!("{}/.config/autostart", home_dir()))
}

fn system_autostart_dir() -> PathBuf {
    PathBuf::from("/etc/xdg/autostart")
}

/// Parse a .desktop file
fn parse_desktop_file(path: &Path, is_system: bool) -> Option<StartupApp> {
    let content = fs::read_to_string(path).ok()?;

    let mut name = String::new();
    let mut comment = String::new();
    let mut exec = String::new();
    let mut icon = String::new();
    let mut hidden = false;
    let mut no_display = false;

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') && line != "[Desktop Entry]" {
            break; // Stop at other sections
        }

        if let Some((key, value)) = line.split_once('=') {
            match key {
                "Name" => name = value.to_string(),
                "Comment" => comment = value.to_string(),
                "Exec" => exec = value.to_string(),
                "Icon" => icon = value.to_string(),
                "Hidden" => hidden = value.to_lowercase() == "true",
                "NoDisplay" => no_display = value.to_lowercase() == "true",
                _ => {}
            }
        }
    }

    // Skip hidden entries
    if hidden || no_display {
        return None;
    }

    // Must have at least a name
    if name.is_empty() {
        name = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
    }

    Some(StartupApp {
        name,
        comment,
        exec,
        icon,
        file_path: path.to_string_lossy().to_string(),
        is_enabled: true,
        is_system,
    })
}

/// Check if a system app has a user override that disables it
fn check_user_override(app_name: &str) -> Option<bool> {
    let user_file = user_autostart_dir().join(format!("{}.desktop", app_name));
    if user_file.exists() {
        if let Ok(content) = fs::read_to_string(&user_file) {
            for line in content.lines() {
                if line.starts_with("Hidden=") || line.starts_with("X-GNOME-Autostart-enabled=") {
                    let value = line.split('=').nth(1).unwrap_or("");
                    return Some(value.to_lowercase() != "true" && value.to_lowercase() != "false");
                }
            }
        }
    }
    None
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Get all startup applications
#[tauri::command]
pub fn get_startup_apps() -> Result<Vec<StartupApp>> {
    let mut apps = Vec::new();

    // 1. User autostart apps
    let user_dir = user_autostart_dir();
    if user_dir.exists() {
        if let Ok(entries) = fs::read_dir(&user_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "desktop").unwrap_or(false) {
                    if let Some(mut app) = parse_desktop_file(&path, false) {
                        // Check for disabled flag
                        if let Ok(content) = fs::read_to_string(&path) {
                            if content.contains("Hidden=true")
                                || content.contains("X-GNOME-Autostart-enabled=false")
                            {
                                app.is_enabled = false;
                            }
                        }
                        apps.push(app);
                    }
                }
            }
        }
    }

    // 2. System autostart apps (that don't have user override)
    let system_dir = system_autostart_dir();
    if system_dir.exists() {
        if let Ok(entries) = fs::read_dir(&system_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "desktop").unwrap_or(false) {
                    // Check if there's a user override
                    let file_name = path.file_name().unwrap().to_string_lossy();
                    let user_override = user_autostart_dir().join(file_name.as_ref());

                    if !user_override.exists() {
                        if let Some(app) = parse_desktop_file(&path, true) {
                            apps.push(app);
                        }
                    }
                }
            }
        }
    }

    // Sort by name
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    Ok(apps)
}

/// Enable a startup app
#[tauri::command]
pub fn enable_startup_app(file_path: String) -> Result<()> {
    let path = Path::new(&file_path);

    if path.starts_with("/etc") {
        // System app - remove user override if exists
        let file_name = path
            .file_name()
            .ok_or_else(|| AppError::System("Invalid path".to_string()))?;
        let user_override = user_autostart_dir().join(file_name);
        if user_override.exists() {
            fs::remove_file(user_override)?;
        }
    } else {
        // User app - modify the file
        let content = fs::read_to_string(path)?;
        let new_content = content
            .replace("Hidden=true", "Hidden=false")
            .replace("X-GNOME-Autostart-enabled=false", "X-GNOME-Autostart-enabled=true");
        fs::write(path, new_content)?;
    }

    Ok(())
}

/// Disable a startup app
#[tauri::command]
pub fn disable_startup_app(file_path: String) -> Result<()> {
    let path = Path::new(&file_path);

    if path.starts_with("/etc") {
        // System app - create user override
        let file_name = path
            .file_name()
            .ok_or_else(|| AppError::System("Invalid path".to_string()))?;

        // Ensure user autostart dir exists
        let user_dir = user_autostart_dir();
        fs::create_dir_all(&user_dir)?;

        // Create override file
        let user_file = user_dir.join(file_name);
        let content = format!(
            "[Desktop Entry]\nHidden=true\nX-GNOME-Autostart-enabled=false\n"
        );
        fs::write(user_file, content)?;
    } else {
        // User app - modify the file
        let content = fs::read_to_string(path)?;

        // Check if Hidden or X-GNOME-Autostart-enabled exists
        let new_content = if content.contains("Hidden=") {
            content.replace("Hidden=false", "Hidden=true")
        } else if content.contains("X-GNOME-Autostart-enabled=") {
            content.replace("X-GNOME-Autostart-enabled=true", "X-GNOME-Autostart-enabled=false")
        } else {
            // Add Hidden=true after [Desktop Entry]
            content.replace("[Desktop Entry]", "[Desktop Entry]\nHidden=true")
        };

        fs::write(path, new_content)?;
    }

    Ok(())
}

/// Remove a user startup app (cannot remove system apps)
#[tauri::command]
pub fn remove_startup_app(file_path: String) -> Result<()> {
    let path = Path::new(&file_path);

    if path.starts_with("/etc") {
        return Err(AppError::PermissionDenied(
            "Cannot remove system autostart apps".to_string(),
        ));
    }

    fs::remove_file(path)?;
    Ok(())
}

/// Add a new startup app
#[tauri::command]
pub fn add_startup_app(name: String, command: String, comment: String) -> Result<String> {
    let user_dir = user_autostart_dir();
    fs::create_dir_all(&user_dir)?;

    // Generate safe filename
    let safe_name: String = name
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    let file_name = format!("{}.desktop", safe_name.to_lowercase());
    let file_path = user_dir.join(&file_name);

    let content = format!(
        "[Desktop Entry]\nType=Application\nName={}\nComment={}\nExec={}\nHidden=false\nX-GNOME-Autostart-enabled=true\n",
        name, comment, command
    );

    fs::write(&file_path, content)?;

    Ok(file_path.to_string_lossy().to_string())
}
