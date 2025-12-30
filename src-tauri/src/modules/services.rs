//! Systemd services module
//! Lists and manages system services

use crate::error::{AppError, Result};
use crate::utils::privileged;
use serde::{Deserialize, Serialize};
use std::process::Command;

// ============================================================================
// Data Structures
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInfo {
    pub name: String,
    pub description: String,
    pub load_state: String,
    pub active_state: String,
    pub sub_state: String,
    pub is_enabled: bool,
    pub can_stop: bool,
    pub can_restart: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceAction {
    pub name: String,
    pub action: String,
    pub success: bool,
    pub message: String,
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// List all systemd services
#[tauri::command]
pub fn get_services() -> Result<Vec<ServiceInfo>> {
    // Get list of all services
    let output = Command::new("systemctl")
        .args([
            "list-units",
            "--type=service",
            "--all",
            "--no-pager",
            "--no-legend",
        ])
        .output()
        .map_err(|e| AppError::CommandFailed(format!("Failed to run systemctl: {}", e)))?;

    if !output.status.success() {
        return Err(AppError::CommandFailed(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut services = Vec::new();

    for line in stdout.lines() {
        // Parse: UNIT LOAD ACTIVE SUB DESCRIPTION
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 5 {
            continue;
        }

        let name = parts[0].trim_end_matches(".service").to_string();
        let load_state = parts[1].to_string();
        let active_state = parts[2].to_string();
        let sub_state = parts[3].to_string();
        let description = parts[4..].join(" ");

        // Check if enabled
        let is_enabled = check_enabled(&name);

        services.push(ServiceInfo {
            name: name.clone(),
            description,
            load_state,
            active_state: active_state.clone(),
            sub_state,
            is_enabled,
            can_stop: active_state == "active",
            can_restart: active_state == "active",
        });
    }

    // Sort by name
    services.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(services)
}

/// Check if a service is enabled
fn check_enabled(name: &str) -> bool {
    Command::new("systemctl")
        .args(["is-enabled", name])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Start a service (requires auth)
#[tauri::command]
pub fn start_service(name: String) -> Result<ServiceAction> {
    let result = privileged::run_privileged("systemctl", &["start", &name]);

    match result {
        Ok(_) => Ok(ServiceAction {
            name,
            action: "start".to_string(),
            success: true,
            message: "Service started successfully".to_string(),
        }),
        Err(AppError::UserCancelled) => Ok(ServiceAction {
            name,
            action: "start".to_string(),
            success: false,
            message: "Operation cancelled by user".to_string(),
        }),
        Err(e) => Err(e),
    }
}

/// Stop a service (requires auth)
#[tauri::command]
pub fn stop_service(name: String) -> Result<ServiceAction> {
    let result = privileged::run_privileged("systemctl", &["stop", &name]);

    match result {
        Ok(_) => Ok(ServiceAction {
            name,
            action: "stop".to_string(),
            success: true,
            message: "Service stopped successfully".to_string(),
        }),
        Err(AppError::UserCancelled) => Ok(ServiceAction {
            name,
            action: "stop".to_string(),
            success: false,
            message: "Operation cancelled by user".to_string(),
        }),
        Err(e) => Err(e),
    }
}

/// Restart a service (requires auth)
#[tauri::command]
pub fn restart_service(name: String) -> Result<ServiceAction> {
    let result = privileged::run_privileged("systemctl", &["restart", &name]);

    match result {
        Ok(_) => Ok(ServiceAction {
            name,
            action: "restart".to_string(),
            success: true,
            message: "Service restarted successfully".to_string(),
        }),
        Err(AppError::UserCancelled) => Ok(ServiceAction {
            name,
            action: "restart".to_string(),
            success: false,
            message: "Operation cancelled by user".to_string(),
        }),
        Err(e) => Err(e),
    }
}

/// Enable a service (requires auth)
#[tauri::command]
pub fn enable_service(name: String) -> Result<ServiceAction> {
    let result = privileged::run_privileged("systemctl", &["enable", &name]);

    match result {
        Ok(_) => Ok(ServiceAction {
            name,
            action: "enable".to_string(),
            success: true,
            message: "Service enabled successfully".to_string(),
        }),
        Err(AppError::UserCancelled) => Ok(ServiceAction {
            name,
            action: "enable".to_string(),
            success: false,
            message: "Operation cancelled by user".to_string(),
        }),
        Err(e) => Err(e),
    }
}

/// Disable a service (requires auth)
#[tauri::command]
pub fn disable_service(name: String) -> Result<ServiceAction> {
    let result = privileged::run_privileged("systemctl", &["disable", &name]);

    match result {
        Ok(_) => Ok(ServiceAction {
            name,
            action: "disable".to_string(),
            success: true,
            message: "Service disabled successfully".to_string(),
        }),
        Err(AppError::UserCancelled) => Ok(ServiceAction {
            name,
            action: "disable".to_string(),
            success: false,
            message: "Operation cancelled by user".to_string(),
        }),
        Err(e) => Err(e),
    }
}

/// Search services by name
#[tauri::command]
pub fn search_services(query: String) -> Result<Vec<ServiceInfo>> {
    let all_services = get_services()?;
    let query_lower = query.to_lowercase();

    let filtered: Vec<ServiceInfo> = all_services
        .into_iter()
        .filter(|s| {
            s.name.to_lowercase().contains(&query_lower)
                || s.description.to_lowercase().contains(&query_lower)
        })
        .collect();

    Ok(filtered)
}
