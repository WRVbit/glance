//! PackageManager trait definition
//! Abstract interface for package management operations across distros

use crate::error::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

// ============================================================================
// Data Structures (Shared across all adapters)
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupResult {
    pub category: String,
    pub items_removed: u32,
    pub bytes_freed: u64,
    pub success: bool,
    pub message: String,
}

// ============================================================================
// PackageManager Trait
// ============================================================================

#[async_trait]
pub trait PackageManager: Send + Sync {
    /// Get the name of this package manager (apt, pacman, dnf, zypper)
    fn name(&self) -> &'static str;
    
    /// Refresh package database
    async fn refresh_repositories(&self) -> Result<String>;
    
    /// Get all installed packages
    async fn get_installed_packages(&self) -> Result<Vec<PackageInfo>>;
    
    /// Search packages by name
    async fn search_packages(&self, query: &str) -> Result<Vec<PackageInfo>>;
    
    /// Uninstall a package
    async fn uninstall_package(&self, name: &str) -> Result<PackageAction>;
    
    /// Purge a package (remove with config files)
    async fn purge_package(&self, name: &str) -> Result<PackageAction>;
    
    /// Remove unused dependencies
    async fn autoremove(&self) -> Result<PackageAction>;
    
    /// Clean package cache
    async fn clean_cache(&self) -> Result<CleanupResult>;
    
    /// Get package statistics (total, auto-installed, total size)
    async fn get_stats(&self) -> Result<(usize, usize, u64)>;
    
    /// Get cache directory path
    fn cache_path(&self) -> &'static str;
    
    /// Get log directory path
    fn log_path(&self) -> &'static str;
    
    /// Check if fast download tool is available (apt-fast, aria2c for pacman, etc.)
    async fn check_fast_download(&self) -> Result<bool> {
        Ok(false) // Default: not available
    }
}

// ============================================================================
// Category Detection (Shared logic)
// ============================================================================

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
    "minecraft", "supertux"
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
    "font", "ttf", "otf", "noto", "dejavu", "liberation"
];

const LIB_PACKAGES: &[&str] = &[
    "lib", "libc", "libx", "libgl", "libstdc"
];

/// Detect package category from name and description
pub fn detect_package_category(name: &str, description: &str) -> String {
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
