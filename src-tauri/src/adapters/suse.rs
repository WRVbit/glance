//! OpenSUSE Package Manager Adapter
//! Uses zypper/rpm for package management

use super::{PackageInfo, PackageAction, CleanupResult, PackageManager, detect_package_category};
use crate::error::{AppError, Result};
use crate::utils::privileged;
use async_trait::async_trait;
use tokio::process::Command;

pub struct SuseAdapter;

impl SuseAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SuseAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PackageManager for SuseAdapter {
    fn name(&self) -> &'static str {
        "zypper"
    }
    
    fn cache_path(&self) -> &'static str {
        "/var/cache/zypp"
    }
    
    fn log_path(&self) -> &'static str {
        "/var/log/zypper.log"
    }
    
    async fn refresh_repositories(&self) -> Result<String> {
        privileged::run_privileged("zypper", &["refresh"]).await
    }
    
    async fn get_installed_packages(&self) -> Result<Vec<PackageInfo>> {
        // Return mock data in simulation mode
        if super::is_mock_mode() {
            log::info!("[MOCK] Returning mock package data for openSUSE");
            return Ok(super::generate_mock_packages("zypper"));
        }
        
        // Get all installed packages using rpm (shared with Fedora)
        let output = Command::new("rpm")
            .args(["-qa", "--queryformat", "%{NAME}\t%{VERSION}-%{RELEASE}\t%{SIZE}\t%{SUMMARY}\n"])
            .output()
            .await
            .map_err(|e| AppError::CommandFailed(e.to_string()))?;
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut packages = Vec::new();
        
        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() < 4 {
                continue;
            }
            
            let name = parts[0].to_string();
            let version = parts[1].to_string();
            let size_bytes: u64 = parts[2].parse().unwrap_or(0);
            let description = parts[3].to_string();
            let category = detect_package_category(&name, &description);
            
            packages.push(PackageInfo {
                name,
                version,
                size_bytes,
                description,
                is_auto: false, // zypper doesn't track this easily
                category,
            });
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
        let result = privileged::run_privileged("zypper", &["remove", "-y", name]).await;
        
        Ok(PackageAction {
            name: name.to_string(),
            action: "uninstall".to_string(),
            success: result.is_ok(),
            message: result.unwrap_or_else(|e| e.to_string()),
        })
    }
    
    async fn purge_package(&self, name: &str) -> Result<PackageAction> {
        // zypper doesn't distinguish between remove and purge
        self.uninstall_package(name).await
    }
    
    async fn autoremove(&self) -> Result<PackageAction> {
        // zypper packages --unneeded then remove
        let result = privileged::run_privileged("zypper", &["remove", "-y", "--clean-deps"]).await;
        
        Ok(PackageAction {
            name: "autoremove".to_string(),
            action: "autoremove".to_string(),
            success: result.is_ok(),
            message: result.unwrap_or_else(|e| e.to_string()),
        })
    }
    
    async fn clean_cache(&self) -> Result<CleanupResult> {
        let result = privileged::run_privileged("zypper", &["clean", "--all"]).await;
        
        Ok(CleanupResult {
            category: "zypper_cache".to_string(),
            items_removed: 0,
            bytes_freed: 0,
            success: result.is_ok(),
            message: result.unwrap_or_else(|e| e.to_string()),
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
