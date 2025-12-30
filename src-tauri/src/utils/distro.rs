//! Linux distribution detection
//! Parses /etc/os-release for distro information

use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistroInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub version_codename: String,
    pub is_supported: bool,
}

impl DistroInfo {
    /// Parse /etc/os-release to get distribution info
    pub fn detect() -> Result<Self> {
        let content = fs::read_to_string("/etc/os-release")
            .map_err(|e| AppError::System(format!("Cannot read /etc/os-release: {}", e)))?;

        let mut id = String::new();
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
                "NAME" => name = value.to_string(),
                "VERSION_ID" => version = value.to_string(),
                "VERSION_CODENAME" => version_codename = value.to_string(),
                _ => {}
            }
        }

        // Check if supported (Ubuntu 24.04+ or Debian 12+)
        let is_supported = Self::check_supported(&id, &version);

        Ok(Self {
            id,
            name,
            version,
            version_codename,
            is_supported,
        })
    }

    fn check_supported(id: &str, version: &str) -> bool {
        match id {
            "ubuntu" => {
                // Ubuntu 24.04+
                if let Ok(ver) = version.parse::<f32>() {
                    ver >= 24.04
                } else {
                    false
                }
            }
            "debian" => {
                // Debian 12+
                if let Ok(ver) = version.parse::<u32>() {
                    ver >= 12
                } else {
                    false
                }
            }
            // Also support derivatives
            "linuxmint" | "pop" | "elementary" | "zorin" => true,
            _ => false,
        }
    }

    /// Check if this distro uses APT package manager
    pub fn uses_apt(&self) -> bool {
        matches!(
            self.id.as_str(),
            "ubuntu" | "debian" | "linuxmint" | "pop" | "elementary" | "zorin"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_distro() {
        let info = DistroInfo::detect();
        assert!(info.is_ok());
    }
}
