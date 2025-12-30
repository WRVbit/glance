//! Systemd services module
//! Lists and manages system services with categorization (async)

use crate::error::{AppError, Result};
use crate::utils::privileged;
use serde::{Deserialize, Serialize};
use tokio::process::Command;

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
    pub category: String,
    pub memory_mb: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceAction {
    pub name: String,
    pub action: String,
    pub success: bool,
    pub message: String,
}

// Category detection patterns
const SYSTEM_SERVICES: &[&str] = &[
    "systemd", "dbus", "polkit", "udev", "cron", "rsyslog", "syslog",
    "journal", "login", "resolved", "timesyncd", "fstrim", "snapd",
    "apparmor", "fail2ban", "irqbalance", "thermald", "init", "boot"
];

const NETWORK_SERVICES: &[&str] = &[
    "network", "NetworkManager", "wpa_supplicant", "dhcp", "avahi",
    "ssh", "openvpn", "wireguard", "ufw", "firewalld", "iptables",
    "bluetooth", "modem", "netfilter", "dns", "ntp"
];

const AUDIO_SERVICES: &[&str] = &[
    "pulse", "pipewire", "alsa", "jack", "sound", "audio",
    "wireplumber", "ofono"
];

const DESKTOP_SERVICES: &[&str] = &[
    "gdm", "sddm", "lightdm", "gnome", "kde", "plasma", "display",
    "xorg", "wayland", "colord", "accounts", "gvfs", "tracker",
    "evolution", "geoclue", "power", "upower", "screen"
];

const DATABASE_SERVICES: &[&str] = &[
    "mysql", "mariadb", "postgres", "mongodb", "redis", "sqlite",
    "memcached", "elasticsearch", "cassandra"
];

const WEB_SERVICES: &[&str] = &[
    "apache", "nginx", "httpd", "caddy", "php-fpm", "tomcat",
    "docker", "containerd", "podman", "kubelet"
];

const HARDWARE_SERVICES: &[&str] = &[
    "acpi", "battery", "lid", "suspend", "hibernate", "laptop",
    "nvidia", "amd", "intel", "gpu", "graphics", "touchpad",
    "keyboard", "mouse", "usb", "pci", "bolt"
];

const PRINT_SERVICES: &[&str] = &[
    "cups", "print", "scanner", "sane"
];

/// Detect service category from name and description
fn detect_category(name: &str, description: &str) -> String {
    let check = |patterns: &[&str]| {
        patterns.iter().any(|p| {
            name.to_lowercase().contains(*p) || description.to_lowercase().contains(*p)
        })
    };
    
    if check(SYSTEM_SERVICES) {
        "System".to_string()
    } else if check(NETWORK_SERVICES) {
        "Network".to_string()
    } else if check(AUDIO_SERVICES) {
        "Audio".to_string()
    } else if check(DESKTOP_SERVICES) {
        "Desktop".to_string()
    } else if check(DATABASE_SERVICES) {
        "Database".to_string()
    } else if check(WEB_SERVICES) {
        "Web/Container".to_string()
    } else if check(HARDWARE_SERVICES) {
        "Hardware".to_string()
    } else if check(PRINT_SERVICES) {
        "Printing".to_string()
    } else if name.contains("@") || name.starts_with("user") {
        "User Apps".to_string()
    } else {
        "Other".to_string()
    }
}

// ============================================================================
// Tauri Commands (All async)
// ============================================================================

/// List all systemd services (async)
#[tauri::command]
pub async fn get_services() -> Result<Vec<ServiceInfo>> {
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
        .await
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

        // Check if enabled (async)
        let is_enabled = check_enabled_async(&name).await;
        
        // Detect category
        let category = detect_category(&name, &description);

        services.push(ServiceInfo {
            name: name.clone(),
            description: description.clone(),
            load_state,
            active_state: active_state.clone(),
            sub_state,
            is_enabled,
            can_stop: active_state == "active",
            can_restart: active_state == "active",
            category,
            memory_mb: None, // Could be enhanced to query systemctl show MemoryCurrent
        });
    }

    // Sort by name
    services.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(services)
}

/// Check if a service is enabled (async helper)
async fn check_enabled_async(name: &str) -> bool {
    Command::new("systemctl")
        .args(["is-enabled", name])
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Start a service (requires auth, async with timeout)
#[tauri::command]
pub async fn start_service(name: String) -> Result<ServiceAction> {
    let result = privileged::run_privileged("systemctl", &["start", &name]).await;

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
        Err(AppError::Timeout(msg)) => Ok(ServiceAction {
            name,
            action: "start".to_string(),
            success: false,
            message: msg,
        }),
        Err(e) => Err(e),
    }
}

/// Stop a service (requires auth, async with timeout)
#[tauri::command]
pub async fn stop_service(name: String) -> Result<ServiceAction> {
    let result = privileged::run_privileged("systemctl", &["stop", &name]).await;

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
        Err(AppError::Timeout(msg)) => Ok(ServiceAction {
            name,
            action: "stop".to_string(),
            success: false,
            message: msg,
        }),
        Err(e) => Err(e),
    }
}

/// Restart a service (requires auth, async with timeout)
#[tauri::command]
pub async fn restart_service(name: String) -> Result<ServiceAction> {
    let result = privileged::run_privileged("systemctl", &["restart", &name]).await;

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
        Err(AppError::Timeout(msg)) => Ok(ServiceAction {
            name,
            action: "restart".to_string(),
            success: false,
            message: msg,
        }),
        Err(e) => Err(e),
    }
}

/// Enable a service (requires auth, async with timeout)
#[tauri::command]
pub async fn enable_service(name: String) -> Result<ServiceAction> {
    let result = privileged::run_privileged("systemctl", &["enable", &name]).await;

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
        Err(AppError::Timeout(msg)) => Ok(ServiceAction {
            name,
            action: "enable".to_string(),
            success: false,
            message: msg,
        }),
        Err(e) => Err(e),
    }
}

/// Disable a service (requires auth, async with timeout)
#[tauri::command]
pub async fn disable_service(name: String) -> Result<ServiceAction> {
    let result = privileged::run_privileged("systemctl", &["disable", &name]).await;

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
        Err(AppError::Timeout(msg)) => Ok(ServiceAction {
            name,
            action: "disable".to_string(),
            success: false,
            message: msg,
        }),
        Err(e) => Err(e),
    }
}

/// Search services by name (async)
#[tauri::command]
pub async fn search_services(query: String) -> Result<Vec<ServiceInfo>> {
    let all_services = get_services().await?;
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
