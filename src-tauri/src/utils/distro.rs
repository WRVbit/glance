//! Linux distribution detection
//! Parses /etc/os-release for distro information

use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::fs;

// ============================================================================
// Distro Family Enum
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DistroFamily {
    Debian,  // Ubuntu, Debian, Mint, Pop!_OS, Elementary, Zorin
    Arch,    // Arch, Manjaro, EndeavourOS, Artix, Garuda
    Fedora,  // Fedora, RHEL, CentOS Stream, Rocky, AlmaLinux
    Suse,    // OpenSUSE Tumbleweed/Leap, SUSE Linux Enterprise
    Unknown, // Any other distro
}

impl DistroFamily {
    /// Get display name for the family
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Debian => "Debian/Ubuntu",
            Self::Arch => "Arch Linux",
            Self::Fedora => "Fedora/RHEL",
            Self::Suse => "openSUSE",
            Self::Unknown => "Unknown",
        }
    }
    
    /// Get the package manager name
    pub fn package_manager_name(&self) -> &'static str {
        match self {
            Self::Debian => "apt",
            Self::Arch => "pacman",
            Self::Fedora => "dnf",
            Self::Suse => "zypper",
            Self::Unknown => "unknown",
        }
    }
}

impl Default for DistroFamily {
    fn default() -> Self {
        Self::Unknown
    }
}

// ============================================================================
// Distro Info Struct
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistroInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub version_codename: String,
    pub family: DistroFamily,
    pub is_supported: bool,
}

impl Default for DistroInfo {
    fn default() -> Self {
        Self {
            id: "unknown".to_string(),
            name: "Unknown Linux".to_string(),
            version: "0".to_string(),
            version_codename: String::new(),
            family: DistroFamily::Unknown,
            is_supported: false,
        }
    }
}

impl DistroInfo {
    /// Parse /etc/os-release to get distribution info
    /// Supports FORCE_DISTRO env var for simulation testing
    pub fn detect() -> Result<Self> {
        // Check for FORCE_DISTRO environment variable (for mock testing)
        if let Ok(forced) = std::env::var("FORCE_DISTRO") {
            log::info!("[MOCK MODE] FORCE_DISTRO={} - Using simulated distro", forced);
            return Ok(Self::mock_distro(&forced));
        }
        
        let content = fs::read_to_string("/etc/os-release")
            .map_err(|e| AppError::System(format!("Cannot read /etc/os-release: {}", e)))?;

        let mut id = String::new();
        let mut id_like = String::new();
        let mut name = String::new();
        let mut version = String::new();
        let mut version_codename = String::new();

        for line in content.lines() {
            let parts: Vec<&str> = line.splitn(2, '=').collect();
            if parts.len() != 2 {
                continue;
            }

            let key = parts[0];
            let value = parts[1].trim_matches('"');

            match key {
                "ID" => id = value.to_string(),
                "ID_LIKE" => id_like = value.to_string(),
                "NAME" => name = value.to_string(),
                "VERSION_ID" => version = value.to_string(),
                "VERSION_CODENAME" => version_codename = value.to_string(),
                _ => {}
            }
        }

        // Detect family based on ID and ID_LIKE
        let family = Self::detect_family(&id, &id_like);
        let is_supported = Self::check_supported(&id, &version, &family);

        Ok(Self {
            id,
            name,
            version,
            version_codename,
            family,
            is_supported,
        })
    }
    
    /// Create a mock DistroInfo for testing
    fn mock_distro(distro: &str) -> Self {
        let (id, name, family) = match distro.to_lowercase().as_str() {
            "arch" | "manjaro" | "endeavouros" => {
                ("arch".to_string(), "Arch Linux (Mock)".to_string(), DistroFamily::Arch)
            }
            "fedora" | "rhel" | "centos" => {
                ("fedora".to_string(), "Fedora (Mock)".to_string(), DistroFamily::Fedora)
            }
            "suse" | "opensuse" | "tumbleweed" => {
                ("opensuse".to_string(), "openSUSE (Mock)".to_string(), DistroFamily::Suse)
            }
            "debian" | "ubuntu" | "mint" => {
                ("ubuntu".to_string(), "Ubuntu (Mock)".to_string(), DistroFamily::Debian)
            }
            _ => {
                ("unknown".to_string(), "Unknown (Mock)".to_string(), DistroFamily::Unknown)
            }
        };
        
        Self {
            id,
            name,
            version: "0.0".to_string(),
            version_codename: "mock".to_string(),
            family,
            is_supported: family != DistroFamily::Unknown,
        }
    }
    
    /// Detect distro family from ID and ID_LIKE fields
    fn detect_family(id: &str, id_like: &str) -> DistroFamily {
        let id_lower = id.to_lowercase();
        let like_lower = id_like.to_lowercase();
        
        // Check ID first (more specific)
        match id_lower.as_str() {
            // Debian family
            "ubuntu" | "debian" | "linuxmint" | "pop" | "elementary" | "zorin" 
            | "kali" | "parrot" | "mx" | "lmde" | "devuan" | "raspbian" => {
                return DistroFamily::Debian;
            }
            
            // Arch family
            "arch" | "manjaro" | "endeavouros" | "artix" | "garuda" 
            | "arcolinux" | "blackarch" | "archcraft" => {
                return DistroFamily::Arch;
            }
            
            // Fedora family
            "fedora" | "rhel" | "centos" | "rocky" | "almalinux" 
            | "nobara" | "ultramarine" => {
                return DistroFamily::Fedora;
            }
            
            // SUSE family
            "opensuse" | "opensuse-tumbleweed" | "opensuse-leap" | "suse" | "sled" | "sles" => {
                return DistroFamily::Suse;
            }
            
            _ => {}
        }
        
        // Check ID_LIKE for derivatives
        if like_lower.contains("debian") || like_lower.contains("ubuntu") {
            DistroFamily::Debian
        } else if like_lower.contains("arch") {
            DistroFamily::Arch
        } else if like_lower.contains("fedora") || like_lower.contains("rhel") {
            DistroFamily::Fedora
        } else if like_lower.contains("suse") {
            DistroFamily::Suse
        } else {
            DistroFamily::Unknown
        }
    }

    fn check_supported(id: &str, version: &str, family: &DistroFamily) -> bool {
        match family {
            DistroFamily::Debian => {
                match id {
                    "ubuntu" => {
                        // Ubuntu 22.04+ (LTS and later)
                        if let Ok(ver) = version.parse::<f32>() {
                            ver >= 22.04
                        } else {
                            true // Unknown version, assume supported
                        }
                    }
                    "debian" => {
                        // Debian 11+
                        if let Ok(ver) = version.parse::<u32>() {
                            ver >= 11
                        } else {
                            true
                        }
                    }
                    // All derivatives are supported
                    _ => true,
                }
            }
            DistroFamily::Arch => true, // All Arch-based are rolling, always supported
            DistroFamily::Fedora => {
                // Fedora 38+
                if let Ok(ver) = version.parse::<u32>() {
                    ver >= 38
                } else {
                    true
                }
            }
            DistroFamily::Suse => true, // Tumbleweed is rolling, Leap versions supported
            DistroFamily::Unknown => false,
        }
    }
    
    /// Get the distro family
    pub fn family(&self) -> DistroFamily {
        self.family
    }
    
    /// Check if repositories feature is available
    pub fn has_repositories_feature(&self) -> bool {
        // Arch uses mirrorlist, not apt-style sources
        self.family != DistroFamily::Arch
    }
    
    /// Check if apt-fast is available (Debian-only)
    pub fn has_apt_fast(&self) -> bool {
        self.family == DistroFamily::Debian
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_distro() {
        let info = DistroInfo::detect();
        assert!(info.is_ok());
        let info = info.unwrap();
        println!("Detected: {} ({:?})", info.name, info.family);
    }
    
    #[test]
    fn test_family_detection() {
        assert_eq!(DistroInfo::detect_family("ubuntu", ""), DistroFamily::Debian);
        assert_eq!(DistroInfo::detect_family("arch", ""), DistroFamily::Arch);
        assert_eq!(DistroInfo::detect_family("fedora", ""), DistroFamily::Fedora);
        assert_eq!(DistroInfo::detect_family("opensuse-tumbleweed", ""), DistroFamily::Suse);
        assert_eq!(DistroInfo::detect_family("manjaro", "arch"), DistroFamily::Arch);
        assert_eq!(DistroInfo::detect_family("pop", "ubuntu debian"), DistroFamily::Debian);
    }
}
