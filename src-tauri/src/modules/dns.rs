//! DNS Manager module
//! Configure system DNS via /etc/systemd/resolved.conf

use crate::error::{AppError, Result};
use crate::utils::privileged;
use serde::{Deserialize, Serialize};
use std::fs;

// ============================================================================
// Data Structures
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsProvider {
    pub id: String,
    pub name: String,
    pub description: String,
    pub primary_dns: String,
    pub secondary_dns: String,
    pub category: String, // "general", "adblock", "security", "family"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsStatus {
    pub current_dns: Vec<String>,
    pub active_provider: Option<String>,
}

// ============================================================================
// DNS Providers
// ============================================================================

pub const DNS_PROVIDERS: &[(&str, &str, &str, &str, &str, &str)] = &[
    // (id, name, description, primary, secondary, category)
    // General
    ("cloudflare", "Cloudflare", "Fast and privacy-focused DNS", "1.1.1.1", "1.0.0.1", "general"),
    ("google", "Google DNS", "Reliable public DNS by Google", "8.8.8.8", "8.8.4.4", "general"),
    ("opendns", "OpenDNS", "Cisco's public DNS service", "208.67.222.222", "208.67.220.220", "general"),
    
    // Ad-blocking
    ("adguard", "AdGuard Default", "DNS with ad & tracker blocking", "94.140.14.14", "94.140.15.15", "adblock"),
    ("adguard_nonfilter", "AdGuard Non-filtering", "AdGuard without filtering", "94.140.14.140", "94.140.14.141", "general"),
    
    // Security
    ("cloudflare_malware", "Cloudflare Malware", "Blocks malware domains", "1.1.1.2", "1.0.0.2", "security"),
    ("quad9", "Quad9", "Security-focused, blocks malware", "9.9.9.9", "149.112.112.112", "security"),
    ("comodo", "Comodo Secure", "Security-focused DNS", "8.26.56.26", "8.20.247.20", "security"),
    
    // Family
    ("cloudflare_family", "Cloudflare Family", "Blocks malware + adult content", "1.1.1.3", "1.0.0.3", "family"),
    ("adguard_family", "AdGuard Family", "AdGuard + family protection", "94.140.14.15", "94.140.15.16", "family"),
    ("opendns_family", "OpenDNS FamilyShield", "Pre-configured family protection", "208.67.222.123", "208.67.220.123", "family"),
    ("cleanbrowsing_family", "CleanBrowsing Family", "Family-friendly filtering", "185.228.168.168", "185.228.169.168", "family"),
];

const RESOLVED_CONF_PATH: &str = "/etc/systemd/resolved.conf";

// ============================================================================
// Tauri Commands
// ============================================================================

/// Get all available DNS providers
#[tauri::command]
pub fn get_dns_providers() -> Vec<DnsProvider> {
    DNS_PROVIDERS
        .iter()
        .map(|(id, name, desc, primary, secondary, category)| DnsProvider {
            id: id.to_string(),
            name: name.to_string(),
            description: desc.to_string(),
            primary_dns: primary.to_string(),
            secondary_dns: secondary.to_string(),
            category: category.to_string(),
        })
        .collect()
}

/// Get current DNS configuration
#[tauri::command]
pub async fn get_current_dns() -> Result<DnsStatus> {
    let status = tokio::task::spawn_blocking(|| {
        let mut current_dns: Vec<String> = Vec::new();
        let mut active_provider: Option<String> = None;

        // Try to read from resolved.conf first
        if let Ok(content) = fs::read_to_string(RESOLVED_CONF_PATH) {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("DNS=") && !trimmed.starts_with('#') {
                    let dns_value = trimmed.trim_start_matches("DNS=").trim();
                    current_dns = dns_value
                        .split_whitespace()
                        .map(|s| s.to_string())
                        .collect();
                    break;
                }
            }
        }

        // If no DNS found in resolved.conf, try resolv.conf
        if current_dns.is_empty() {
            if let Ok(content) = fs::read_to_string("/etc/resolv.conf") {
                for line in content.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("nameserver") {
                        if let Some(dns) = trimmed.split_whitespace().nth(1) {
                            if !dns.starts_with("127.0.0.") {
                                current_dns.push(dns.to_string());
                            }
                        }
                    }
                }
            }
        }

        // Try to match with a known provider
        if !current_dns.is_empty() {
            let primary = &current_dns[0];
            for (id, _, _, p, _, _) in DNS_PROVIDERS {
                if primary == *p {
                    active_provider = Some(id.to_string());
                    break;
                }
            }
        }

        Ok::<_, AppError>(DnsStatus {
            current_dns,
            active_provider,
        })
    })
    .await
    .unwrap()?;

    Ok(status)
}

/// Set DNS using a provider ID
#[tauri::command]
pub async fn set_dns_provider(provider_id: String) -> Result<()> {
    // Find the provider
    let provider = DNS_PROVIDERS
        .iter()
        .find(|(id, _, _, _, _, _)| *id == provider_id)
        .ok_or_else(|| AppError::System("Unknown DNS provider".to_string()))?;

    let (_, _, _, primary, secondary, _) = provider;

    apply_dns(primary, secondary).await
}

/// Set custom DNS servers
#[tauri::command]
pub async fn set_custom_dns(primary: String, secondary: String) -> Result<()> {
    // Validate IP addresses (basic check)
    if !is_valid_ip(&primary) {
        return Err(AppError::System("Invalid primary DNS address".to_string()));
    }
    if !secondary.is_empty() && !is_valid_ip(&secondary) {
        return Err(AppError::System("Invalid secondary DNS address".to_string()));
    }

    let sec = if secondary.is_empty() { "" } else { &secondary };
    apply_dns(&primary, sec).await
}

/// Reset DNS to DHCP (automatic)
#[tauri::command]
pub async fn reset_dns() -> Result<()> {
    // Read current config
    let content = fs::read_to_string(RESOLVED_CONF_PATH).unwrap_or_default();

    // Comment out DNS line or remove it
    let new_content: String = content
        .lines()
        .map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with("DNS=") && !trimmed.starts_with('#') {
                format!("#{}", line)
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    let temp_path = std::env::temp_dir().join("glance_resolved.tmp");
    fs::write(&temp_path, &new_content)
        .map_err(|e| AppError::System(format!("Failed to write temp file: {}", e)))?;

    let script = format!(
        "cp '{}' '{}' && rm '{}' && systemctl restart systemd-resolved",
        temp_path.to_string_lossy(),
        RESOLVED_CONF_PATH,
        temp_path.to_string_lossy()
    );

    privileged::run_privileged_shell(&script).await?;

    Ok(())
}

// ============================================================================
// Helper Functions
// ============================================================================

fn is_valid_ip(ip: &str) -> bool {
    ip.parse::<std::net::IpAddr>().is_ok()
}

async fn apply_dns(primary: &str, secondary: &str) -> Result<()> {
    // Read current config
    let content = fs::read_to_string(RESOLVED_CONF_PATH).unwrap_or_else(|_| {
        "[Resolve]\n".to_string()
    });

    let dns_line = if secondary.is_empty() {
        format!("DNS={}", primary)
    } else {
        format!("DNS={} {}", primary, secondary)
    };

    // Update or add DNS line
    let mut found = false;
    let mut new_lines: Vec<String> = content
        .lines()
        .map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with("DNS=") || trimmed.starts_with("#DNS=") {
                found = true;
                dns_line.clone()
            } else {
                line.to_string()
            }
        })
        .collect();

    // If DNS line not found, add it after [Resolve]
    if !found {
        let mut result = Vec::new();
        let mut added = false;
        for line in new_lines {
            result.push(line.clone());
            if line.trim() == "[Resolve]" && !added {
                result.push(dns_line.clone());
                added = true;
            }
        }
        if !added {
            // If no [Resolve] section, create one
            result.insert(0, "[Resolve]".to_string());
            result.insert(1, dns_line);
        }
        new_lines = result;
    }

    // Add FallbackDNS for safety
    let has_fallback = new_lines.iter().any(|l| l.trim().starts_with("FallbackDNS="));
    if !has_fallback {
        new_lines.push("FallbackDNS=1.1.1.1 8.8.8.8 9.9.9.9".to_string());
    }

    let new_content = new_lines.join("\n") + "\n";

    let temp_path = std::env::temp_dir().join("glance_resolved.tmp");
    fs::write(&temp_path, &new_content)
        .map_err(|e| AppError::System(format!("Failed to write temp file: {}", e)))?;

    let script = format!(
        "cp '{}' '{}' && rm '{}' && systemctl restart systemd-resolved",
        temp_path.to_string_lossy(),
        RESOLVED_CONF_PATH,
        temp_path.to_string_lossy()
    );

    privileged::run_privileged_shell(&script).await?;

    Ok(())
}
