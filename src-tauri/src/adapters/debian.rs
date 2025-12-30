//! Debian/Ubuntu Package Manager Adapter
//! Uses apt/dpkg for package management

use super::{PackageInfo, PackageAction, CleanupResult, PackageManager, detect_package_category};
use crate::error::{AppError, Result};
use crate::utils::privileged;
use async_trait::async_trait;
use std::collections::HashSet;
use tokio::process::Command;

pub struct DebianAdapter;

impl DebianAdapter {
    pub fn new() -> Self {
        Self
    }
    
    /// Check if apt-fast is available
    async fn has_apt_fast(&self) -> bool {
        Command::new("which")
            .arg("apt-fast")
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
    
    /// Get the apt command (apt-fast if available, otherwise apt-get)
    async fn apt_cmd(&self) -> &'static str {
        if self.has_apt_fast().await {
            "apt-fast"
        } else {
            "apt-get"
        }
    }
}

impl Default for DebianAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PackageManager for DebianAdapter {
    fn name(&self) -> &'static str {
        "apt"
    }
    
    fn cache_path(&self) -> &'static str {
        "/var/cache/apt/archives"
    }
    
    fn log_path(&self) -> &'static str {
        "/var/log/apt"
    }
    
    async fn refresh_repositories(&self) -> Result<String> {
        let apt = self.apt_cmd().await;
        
        let result = privileged::run_privileged(&[apt, "update"])
            .map_err(|e| AppError::CommandFailed(e))?;
        
        if result.success {
            Ok("Package database updated successfully".to_string())
        } else {
            Err(AppError::CommandFailed(result.stderr))
        }
    }
    
    async fn get_installed_packages(&self) -> Result<Vec<PackageInfo>> {
        // Get auto-installed packages first
        let auto_output = Command::new("apt-mark")
            .arg("showauto")
            .output()
            .await
            .map_err(|e| AppError::CommandFailed(e.to_string()))?;
        
        let auto_packages: HashSet<String> = String::from_utf8_lossy(&auto_output.stdout)
            .lines()
            .map(|s| s.to_string())
            .collect();
        
        // Get all installed packages
        let output = Command::new("dpkg-query")
            .args([
                "-W",
                "-f=${Package}\t${Version}\t${Installed-Size}\t${Description}\n",
            ])
            .output()
            .await
            .map_err(|e| AppError::CommandFailed(e.to_string()))?;
        
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
        
        packages.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(packages)
    }
    
    async fn search_packages(&self, query: &str) -> Result<Vec<PackageInfo>> {
        let all_packages = self.get_installed_packages().await?;
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
    
    async fn uninstall_package(&self, name: &str) -> Result<PackageAction> {
        let result = privileged::run_privileged(&["apt-get", "remove", "-y", name])
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
        let result = privileged::run_privileged(&["apt-get", "purge", "-y", name])
            .map_err(|e| AppError::CommandFailed(e))?;
        
        Ok(PackageAction {
            name: name.to_string(),
            action: "purge".to_string(),
            success: result.success,
            message: if result.success {
                format!("Package {} purged", name)
            } else {
                result.stderr
            },
        })
    }
    
    async fn autoremove(&self) -> Result<PackageAction> {
        let result = privileged::run_privileged(&["apt-get", "autoremove", "-y"])
            .map_err(|e| AppError::CommandFailed(e))?;
        
        Ok(PackageAction {
            name: "autoremove".to_string(),
            action: "autoremove".to_string(),
            success: result.success,
            message: if result.success {
                "Unused packages removed".to_string()
            } else {
                result.stderr
            },
        })
    }
    
    async fn clean_cache(&self) -> Result<CleanupResult> {
        let result = privileged::run_privileged(&["apt-get", "clean"])
            .map_err(|e| AppError::CommandFailed(e))?;
        
        Ok(CleanupResult {
            category: "apt_cache".to_string(),
            items_removed: 0, // apt clean doesn't report count
            bytes_freed: 0,   // Would need to calculate before/after
            success: result.success,
            message: if result.success {
                "APT cache cleaned".to_string()
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
    
    async fn check_fast_download(&self) -> Result<bool> {
        Ok(self.has_apt_fast().await)
    }
}
