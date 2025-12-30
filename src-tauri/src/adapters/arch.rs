//! Arch Linux Package Manager Adapter
//! Uses pacman for package management

use super::{PackageInfo, PackageAction, CleanupResult, PackageManager, detect_package_category};
use crate::error::{AppError, Result};
use crate::utils::privileged;
use async_trait::async_trait;
use tokio::process::Command;

pub struct ArchAdapter;

impl ArchAdapter {
    pub fn new() -> Self {
        Self
    }
    
    /// Check if paccache is available (from pacman-contrib)
    async fn has_paccache(&self) -> bool {
        Command::new("which")
            .arg("paccache")
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

impl Default for ArchAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PackageManager for ArchAdapter {
    fn name(&self) -> &'static str {
        "pacman"
    }
    
    fn cache_path(&self) -> &'static str {
        "/var/cache/pacman/pkg"
    }
    
    fn log_path(&self) -> &'static str {
        "/var/log/pacman.log"
    }
    
    async fn refresh_repositories(&self) -> Result<String> {
        let result = privileged::run_privileged(&["pacman", "-Sy"])
            .map_err(|e| AppError::CommandFailed(e))?;
        
        if result.success {
            Ok("Package database synchronized".to_string())
        } else {
            Err(AppError::CommandFailed(result.stderr))
        }
    }
    
    async fn get_installed_packages(&self) -> Result<Vec<PackageInfo>> {
        // Get explicitly installed packages
        let explicit_output = Command::new("pacman")
            .args(["-Qe", "-q"])
            .output()
            .await
            .map_err(|e| AppError::CommandFailed(e.to_string()))?;
        
        let explicit_packages: std::collections::HashSet<String> = 
            String::from_utf8_lossy(&explicit_output.stdout)
                .lines()
                .map(|s| s.to_string())
                .collect();
        
        // Get all installed packages with info
        let output = Command::new("pacman")
            .args(["-Qi"])
            .output()
            .await
            .map_err(|e| AppError::CommandFailed(e.to_string()))?;
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut packages = Vec::new();
        
        let mut name = String::new();
        let mut version = String::new();
        let mut size_bytes: u64 = 0;
        let mut description = String::new();
        
        for line in stdout.lines() {
            if line.starts_with("Name            :") {
                name = line.split(':').nth(1).unwrap_or("").trim().to_string();
            } else if line.starts_with("Version         :") {
                version = line.split(':').nth(1).unwrap_or("").trim().to_string();
            } else if line.starts_with("Installed Size  :") {
                let size_str = line.split(':').nth(1).unwrap_or("").trim();
                size_bytes = Self::parse_size(size_str);
            } else if line.starts_with("Description     :") {
                description = line.split(':').nth(1).unwrap_or("").trim().to_string();
            } else if line.is_empty() && !name.is_empty() {
                let category = detect_package_category(&name, &description);
                let is_auto = !explicit_packages.contains(&name);
                
                packages.push(PackageInfo {
                    name: name.clone(),
                    version: version.clone(),
                    size_bytes,
                    description: description.clone(),
                    is_auto,
                    category,
                });
                
                name.clear();
                version.clear();
                size_bytes = 0;
                description.clear();
            }
        }
        
        packages.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(packages)
    }
    
    async fn search_packages(&self, query: &str) -> Result<Vec<PackageInfo>> {
        let all_packages = self.get_installed_packages().await?;
        let query_lower = query.to_lowercase();
        
        Ok(all_packages
            .into_iter()
            .filter(|p| {
                p.name.to_lowercase().contains(&query_lower)
                    || p.description.to_lowercase().contains(&query_lower)
            })
            .collect())
    }
    
    async fn uninstall_package(&self, name: &str) -> Result<PackageAction> {
        let result = privileged::run_privileged(&["pacman", "-R", "--noconfirm", name])
            .map_err(|e| AppError::CommandFailed(e))?;
        
        Ok(PackageAction {
            name: name.to_string(),
            action: "uninstall".to_string(),
            success: result.success,
            message: if result.success {
                format!("Package {} removed", name)
            } else {
                result.stderr
            },
        })
    }
    
    async fn purge_package(&self, name: &str) -> Result<PackageAction> {
        // -Rns removes package, dependencies, and config files
        let result = privileged::run_privileged(&["pacman", "-Rns", "--noconfirm", name])
            .map_err(|e| AppError::CommandFailed(e))?;
        
        Ok(PackageAction {
            name: name.to_string(),
            action: "purge".to_string(),
            success: result.success,
            message: if result.success {
                format!("Package {} purged with dependencies", name)
            } else {
                result.stderr
            },
        })
    }
    
    async fn autoremove(&self) -> Result<PackageAction> {
        // Get orphan packages
        let orphans = Command::new("pacman")
            .args(["-Qdtq"])
            .output()
            .await
            .map_err(|e| AppError::CommandFailed(e.to_string()))?;
        
        let orphan_list = String::from_utf8_lossy(&orphans.stdout);
        
        if orphan_list.trim().is_empty() {
            return Ok(PackageAction {
                name: "autoremove".to_string(),
                action: "autoremove".to_string(),
                success: true,
                message: "No orphan packages to remove".to_string(),
            });
        }
        
        // Remove orphans
        let packages: Vec<&str> = orphan_list.lines().collect();
        let mut args = vec!["pacman", "-Rns", "--noconfirm"];
        args.extend(packages.iter());
        
        let result = privileged::run_privileged(&args)
            .map_err(|e| AppError::CommandFailed(e))?;
        
        Ok(PackageAction {
            name: "autoremove".to_string(),
            action: "autoremove".to_string(),
            success: result.success,
            message: if result.success {
                format!("Removed {} orphan packages", packages.len())
            } else {
                result.stderr
            },
        })
    }
    
    async fn clean_cache(&self) -> Result<CleanupResult> {
        // Use paccache if available, otherwise pacman -Sc
        let result = if self.has_paccache().await {
            privileged::run_privileged(&["paccache", "-r", "-k", "1"])
        } else {
            privileged::run_privileged(&["pacman", "-Sc", "--noconfirm"])
        }.map_err(|e| AppError::CommandFailed(e))?;
        
        Ok(CleanupResult {
            category: "pacman_cache".to_string(),
            items_removed: 0,
            bytes_freed: 0,
            success: result.success,
            message: if result.success {
                "Pacman cache cleaned".to_string()
            } else {
                result.stderr
            },
        })
    }
    
    async fn get_stats(&self) -> Result<(usize, usize, u64)> {
        let packages = self.get_installed_packages().await?;
        
        let total = packages.len();
        let auto = packages.iter().filter(|p| p.is_auto).count();
        let size: u64 = packages.iter().map(|p| p.size_bytes).sum();
        
        Ok((total, auto, size))
    }
}

impl ArchAdapter {
    /// Parse size string like "12.5 MiB" to bytes
    fn parse_size(size_str: &str) -> u64 {
        let parts: Vec<&str> = size_str.split_whitespace().collect();
        if parts.len() < 2 {
            return 0;
        }
        
        let num: f64 = parts[0].parse().unwrap_or(0.0);
        let unit = parts[1].to_lowercase();
        
        match unit.as_str() {
            "b" => num as u64,
            "kib" | "kb" => (num * 1024.0) as u64,
            "mib" | "mb" => (num * 1024.0 * 1024.0) as u64,
            "gib" | "gb" => (num * 1024.0 * 1024.0 * 1024.0) as u64,
            _ => 0,
        }
    }
}
