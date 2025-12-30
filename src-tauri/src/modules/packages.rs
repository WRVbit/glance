//! Package management module
//! Lists and uninstalls packages with categorization (async)

use crate::error::{AppError, Result};
use crate::utils::privileged;
use serde::{Deserialize, Serialize};
use tokio::process::Command;

// ============================================================================
// Data Structures
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub size_bytes: u64,
    pub description: String,
    pub is_auto: bool,
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageAction {
    pub name: String,
    pub action: String,
    pub success: bool,
    pub message: String,
}

// Category detection patterns
const GNOME_PACKAGES: &[&str] = &[
    "gnome", "gtk", "glib", "nautilus", "gedit", "evince", "eog",
    "totem", "mutter", "gdm", "gvfs", "gio", "gsettings"
];

const KDE_PACKAGES: &[&str] = &[
    "kde", "plasma", "qt5", "qt6", "kwin", "dolphin", "konsole",
    "kate", "okular", "kio", "kf5", "kf6"
];

const AUDIO_PACKAGES: &[&str] = &[
    "pulse", "pipewire", "alsa", "jack", "sound", "audio",
    "spotify", "rhythmbox", "vlc", "mpv", "audacity", "lame", "mp3"
];

const VIDEO_PACKAGES: &[&str] = &[
    "video", "ffmpeg", "gstreamer", "x264", "x265", "codec",
    "obs", "kdenlive", "handbrake", "mpv", "vlc"
];

const DEV_PACKAGES: &[&str] = &[
    "gcc", "clang", "llvm", "python", "node", "npm", "cargo", "rust",
    "golang", "java", "jdk", "jre", "maven", "gradle", "cmake", "make",
    "git", "mercurial", "subversion", "dev", "devel", "-dbg"
];

const GAMES_PACKAGES: &[&str] = &[
    "game", "steam", "wine", "proton", "lutris", "play",
    "minecraft", "supertux", "frozen"
];

const OFFICE_PACKAGES: &[&str] = &[
    "libreoffice", "openoffice", "office", "calc", "writer", "impress",
    "pdf", "document", "spreadsheet"
];

const INTERNET_PACKAGES: &[&str] = &[
    "firefox", "chrome", "chromium", "browser", "thunderbird", "mail",
    "telegram", "discord", "slack", "zoom", "teams", "skype"
];

const GRAPHICS_PACKAGES: &[&str] = &[
    "gimp", "inkscape", "krita", "blender", "image", "photo",
    "drawing", "paint", "svg", "png", "jpeg"
];

const FONT_PACKAGES: &[&str] = &[
    "font", "ttf", "otf", "noto", "dejavu", "liberation", "ubuntu-font"
];

const LIB_PACKAGES: &[&str] = &[
    "lib", "libc", "libx", "libgl", "libstdc"
];

/// Detect package category from name and description
fn detect_package_category(name: &str, description: &str) -> String {
    let check = |patterns: &[&str]| {
        patterns.iter().any(|p| {
            name.to_lowercase().contains(*p) || description.to_lowercase().contains(*p)
        })
    };
    
    if check(GNOME_PACKAGES) {
        "GNOME".to_string()
    } else if check(KDE_PACKAGES) {
        "KDE/Qt".to_string()
    } else if check(AUDIO_PACKAGES) {
        "Audio".to_string()
    } else if check(VIDEO_PACKAGES) {
        "Video".to_string()
    } else if check(DEV_PACKAGES) {
        "Development".to_string()
    } else if check(GAMES_PACKAGES) {
        "Games".to_string()
    } else if check(OFFICE_PACKAGES) {
        "Office".to_string()
    } else if check(INTERNET_PACKAGES) {
        "Internet".to_string()
    } else if check(GRAPHICS_PACKAGES) {
        "Graphics".to_string()
    } else if check(FONT_PACKAGES) {
        "Fonts".to_string()
    } else if check(LIB_PACKAGES) {
        "Libraries".to_string()
    } else if name.ends_with("-doc") || name.ends_with("-docs") {
        "Documentation".to_string()
    } else {
        "System".to_string()
    }
}

// ============================================================================
// Tauri Commands (All async)
// ============================================================================

/// Get all installed packages (async)
#[tauri::command]
pub async fn get_packages() -> Result<Vec<PackageInfo>> {
    // Use dpkg-query for comprehensive info
    let output = Command::new("dpkg-query")
        .args([
            "-W",
            "-f",
            "${Package}\t${Version}\t${Installed-Size}\t${binary:Summary}\n",
        ])
        .output()
        .await
        .map_err(|e| AppError::CommandFailed(format!("Failed to run dpkg-query: {}", e)))?;

    if !output.status.success() {
        return Err(AppError::CommandFailed(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    // Get list of auto-installed packages
    let auto_output = Command::new("apt-mark")
        .args(["showauto"])
        .output()
        .await
        .ok();

    let auto_packages: Vec<String> = auto_output
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut packages = Vec::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 3 {
            continue;
        }

        let name = parts[0].to_string();
        let version = parts[1].to_string();
        let size_kb: u64 = parts[2].parse().unwrap_or(0);
        let description = parts.get(3).unwrap_or(&"").to_string();
        
        // Detect category
        let category = detect_package_category(&name, &description);

        packages.push(PackageInfo {
            name: name.clone(),
            version,
            size_bytes: size_kb * 1024,
            description,
            is_auto: auto_packages.contains(&name),
            category,
        });
    }

    // Sort by name
    packages.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(packages)
}

/// Search packages by name (async)
#[tauri::command]
pub async fn search_packages(query: String) -> Result<Vec<PackageInfo>> {
    let all_packages = get_packages().await?;
    let query_lower = query.to_lowercase();

    let filtered: Vec<PackageInfo> = all_packages
        .into_iter()
        .filter(|p| {
            p.name.to_lowercase().contains(&query_lower)
                || p.description.to_lowercase().contains(&query_lower)
        })
        .collect();

    Ok(filtered)
}

/// Uninstall a package (requires auth, async with timeout)
#[tauri::command]
pub async fn uninstall_package(name: String) -> Result<PackageAction> {
    // Validate package name (prevent injection)
    if !name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '.' || c == '+' || c == ':') {
        return Err(AppError::PermissionDenied(
            "Invalid package name".to_string(),
        ));
    }

    let result = privileged::run_privileged("apt-get", &["remove", "-y", &name]).await;

    match result {
        Ok(output) => Ok(PackageAction {
            name,
            action: "remove".to_string(),
            success: true,
            message: output,
        }),
        Err(AppError::UserCancelled) => Ok(PackageAction {
            name,
            action: "remove".to_string(),
            success: false,
            message: "Operation cancelled by user".to_string(),
        }),
        Err(AppError::Timeout(msg)) => Ok(PackageAction {
            name,
            action: "remove".to_string(),
            success: false,
            message: msg,
        }),
        Err(e) => Err(e),
    }
}

/// Purge a package (remove with config files, async with timeout)
#[tauri::command]
pub async fn purge_package(name: String) -> Result<PackageAction> {
    // Validate package name
    if !name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '.' || c == '+' || c == ':') {
        return Err(AppError::PermissionDenied(
            "Invalid package name".to_string(),
        ));
    }

    let result = privileged::run_privileged("apt-get", &["purge", "-y", &name]).await;

    match result {
        Ok(output) => Ok(PackageAction {
            name,
            action: "purge".to_string(),
            success: true,
            message: output,
        }),
        Err(AppError::UserCancelled) => Ok(PackageAction {
            name,
            action: "purge".to_string(),
            success: false,
            message: "Operation cancelled by user".to_string(),
        }),
        Err(AppError::Timeout(msg)) => Ok(PackageAction {
            name,
            action: "purge".to_string(),
            success: false,
            message: msg,
        }),
        Err(e) => Err(e),
    }
}

/// Remove unused dependencies (async with timeout)
#[tauri::command]
pub async fn autoremove_packages() -> Result<PackageAction> {
    let result = privileged::run_privileged("apt-get", &["autoremove", "-y"]).await;

    match result {
        Ok(output) => Ok(PackageAction {
            name: "*".to_string(),
            action: "autoremove".to_string(),
            success: true,
            message: output,
        }),
        Err(AppError::UserCancelled) => Ok(PackageAction {
            name: "*".to_string(),
            action: "autoremove".to_string(),
            success: false,
            message: "Operation cancelled by user".to_string(),
        }),
        Err(AppError::Timeout(msg)) => Ok(PackageAction {
            name: "*".to_string(),
            action: "autoremove".to_string(),
            success: false,
            message: msg,
        }),
        Err(e) => Err(e),
    }
}

/// Get package count statistics (async)
#[tauri::command]
pub async fn get_package_stats() -> Result<(usize, usize, u64)> {
    let packages = get_packages().await?;

    let total_count = packages.len();
    let auto_count = packages.iter().filter(|p| p.is_auto).count();
    let total_size: u64 = packages.iter().map(|p| p.size_bytes).sum();

    Ok((total_count, auto_count, total_size))
}
