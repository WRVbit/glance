//! Distro Context - Runtime configuration based on detected distro
//! Provides dynamic paths and feature availability

use super::distro::{DistroFamily, DistroInfo};
use crate::adapters::{PackageManager, DebianAdapter, ArchAdapter, FedoraAdapter, SuseAdapter};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// ============================================================================
// Dynamic Paths based on Distro
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistroPaths {
    /// Package manager cache directory
    pub package_cache: String,
    /// Package manager log file/directory
    pub package_logs: String,
    /// System log directory
    pub system_logs: String,
    /// Systemd journal directory
    pub journal_dir: String,
    /// User trash directory
    pub trash_dir: String,
    /// User cache directory
    pub user_cache: String,
    /// Repository sources directory (None for Arch)
    pub sources_dir: Option<String>,
    /// Thumbnail cache directory
    pub thumbnail_cache: String,
}

impl DistroPaths {
    /// Create paths for a specific distro family
    pub fn for_family(family: DistroFamily, home: &str) -> Self {
        match family {
            DistroFamily::Debian => Self {
                package_cache: "/var/cache/apt/archives".into(),
                package_logs: "/var/log/apt".into(),
                system_logs: "/var/log".into(),
                journal_dir: "/var/log/journal".into(),
                trash_dir: format!("{}/.local/share/Trash", home),
                user_cache: format!("{}/.cache", home),
                sources_dir: Some("/etc/apt/sources.list.d".into()),
                thumbnail_cache: format!("{}/.cache/thumbnails", home),
            },
            DistroFamily::Arch => Self {
                package_cache: "/var/cache/pacman/pkg".into(),
                package_logs: "/var/log/pacman.log".into(),
                system_logs: "/var/log".into(),
                journal_dir: "/var/log/journal".into(),
                trash_dir: format!("{}/.local/share/Trash", home),
                user_cache: format!("{}/.cache", home),
                sources_dir: None, // Arch uses mirrorlist, no sources.list.d
                thumbnail_cache: format!("{}/.cache/thumbnails", home),
            },
            DistroFamily::Fedora => Self {
                package_cache: "/var/cache/dnf".into(),
                package_logs: "/var/log/dnf.log".into(),
                system_logs: "/var/log".into(),
                journal_dir: "/var/log/journal".into(),
                trash_dir: format!("{}/.local/share/Trash", home),
                user_cache: format!("{}/.cache", home),
                sources_dir: Some("/etc/yum.repos.d".into()),
                thumbnail_cache: format!("{}/.cache/thumbnails", home),
            },
            DistroFamily::Suse => Self {
                package_cache: "/var/cache/zypp".into(),
                package_logs: "/var/log/zypper.log".into(),
                system_logs: "/var/log".into(),
                journal_dir: "/var/log/journal".into(),
                trash_dir: format!("{}/.local/share/Trash", home),
                user_cache: format!("{}/.cache", home),
                sources_dir: Some("/etc/zypp/repos.d".into()),
                thumbnail_cache: format!("{}/.cache/thumbnails", home),
            },
            DistroFamily::Unknown => Self {
                // Fallback to common Linux paths
                package_cache: "/var/cache".into(),
                package_logs: "/var/log".into(),
                system_logs: "/var/log".into(),
                journal_dir: "/var/log/journal".into(),
                trash_dir: format!("{}/.local/share/Trash", home),
                user_cache: format!("{}/.cache", home),
                sources_dir: None,
                thumbnail_cache: format!("{}/.cache/thumbnails", home),
            },
        }
    }
}

// ============================================================================
// Feature Availability
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureAvailability {
    /// APT-style repositories (sources.list)
    pub repositories: bool,
    /// apt-fast integration
    pub apt_fast: bool,
    /// pacman cache cleaning (paccache)
    pub pacman_cache: bool,
    /// dnf automatic updates
    pub dnf_automatic: bool,
    /// zypper patterns
    pub zypper_patterns: bool,
    /// Flatpak support
    pub flatpak: bool,
    /// Snap support
    pub snap: bool,
}

impl FeatureAvailability {
    pub fn for_family(family: DistroFamily) -> Self {
        match family {
            DistroFamily::Debian => Self {
                repositories: true,
                apt_fast: true,
                pacman_cache: false,
                dnf_automatic: false,
                zypper_patterns: false,
                flatpak: true,
                snap: true,
            },
            DistroFamily::Arch => Self {
                repositories: false, // Uses mirrorlist
                apt_fast: false,
                pacman_cache: true,
                dnf_automatic: false,
                zypper_patterns: false,
                flatpak: true,
                snap: false, // Snap in AUR but not common
            },
            DistroFamily::Fedora => Self {
                repositories: true, // yum.repos.d
                apt_fast: false,
                pacman_cache: false,
                dnf_automatic: true,
                zypper_patterns: false,
                flatpak: true,
                snap: false,
            },
            DistroFamily::Suse => Self {
                repositories: true, // zypper repos
                apt_fast: false,
                pacman_cache: false,
                dnf_automatic: false,
                zypper_patterns: true,
                flatpak: true,
                snap: false,
            },
            DistroFamily::Unknown => Self {
                repositories: false,
                apt_fast: false,
                pacman_cache: false,
                dnf_automatic: false,
                zypper_patterns: false,
                flatpak: false,
                snap: false,
            },
        }
    }
}

// ============================================================================
// Distro Context (Runtime Configuration)
// ============================================================================

/// Runtime context containing distro-specific configuration
pub struct DistroContext {
    /// Detected distro information
    pub distro: DistroInfo,
    /// Distro family (Debian, Arch, Fedora, SUSE)
    pub family: DistroFamily,
    /// Package manager adapter
    pub package_manager: Arc<dyn PackageManager>,
    /// Dynamic paths
    pub paths: DistroPaths,
    /// Feature availability
    pub features: FeatureAvailability,
}

impl DistroContext {
    /// Create a new context by detecting the current distro
    pub fn new() -> Self {
        let distro = DistroInfo::detect().unwrap_or_default();
        let family = distro.family();
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        
        let package_manager: Arc<dyn PackageManager> = match family {
            DistroFamily::Debian => Arc::new(DebianAdapter::new()),
            DistroFamily::Arch => Arc::new(ArchAdapter::new()),
            DistroFamily::Fedora => Arc::new(FedoraAdapter::new()),
            DistroFamily::Suse => Arc::new(SuseAdapter::new()),
            DistroFamily::Unknown => Arc::new(DebianAdapter::new()), // Fallback
        };
        
        Self {
            distro,
            family,
            package_manager,
            paths: DistroPaths::for_family(family, &home),
            features: FeatureAvailability::for_family(family),
        }
    }
    
    /// Get package manager name (for display)
    pub fn pm_name(&self) -> &'static str {
        self.family.package_manager_name()
    }
    
    /// Check if a feature is available
    pub fn has_feature(&self, feature: &str) -> bool {
        match feature {
            "repositories" => self.features.repositories,
            "apt_fast" => self.features.apt_fast,
            "pacman_cache" => self.features.pacman_cache,
            "dnf_automatic" => self.features.dnf_automatic,
            "zypper_patterns" => self.features.zypper_patterns,
            "flatpak" => self.features.flatpak,
            "snap" => self.features.snap,
            _ => false,
        }
    }
}

impl Default for DistroContext {
    fn default() -> Self {
        Self::new()
    }
}
