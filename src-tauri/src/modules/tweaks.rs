//! System tweaks module
//! Reads from /proc/sys and /sys, applies via pkexec sysctl (async)

use crate::error::{AppError, Result};
use crate::utils::privileged;
use serde::{Deserialize, Serialize};
use std::fs;

// ============================================================================
// Data Structures
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tweak {
    pub id: String,
    pub name: String,
    pub category: String,
    pub description: String,
    pub current_value: String,
    pub recommended_value: String,
    pub is_applied: bool,
    pub sysctl_key: Option<String>,
    pub file_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TweakCategory {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub tweaks: Vec<Tweak>,
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Read a value from /proc/sys or /sys
fn read_sys_value(path: &str) -> String {
    fs::read_to_string(path)
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

/// Get available CPU governors
fn get_available_governors() -> Vec<String> {
    read_sys_value("/sys/devices/system/cpu/cpu0/cpufreq/scaling_available_governors")
        .split_whitespace()
        .map(|s| s.to_string())
        .collect()
}

/// Get current I/O scheduler for a block device
fn get_io_scheduler(device: &str) -> String {
    let path = format!("/sys/block/{}/queue/scheduler", device);
    let content = read_sys_value(&path);

    // Extract active scheduler (inside brackets)
    content
        .split('[')
        .nth(1)
        .and_then(|s| s.split(']').next())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Get main block device (nvme or sda)
fn get_main_block_device() -> String {
    if fs::metadata("/sys/block/nvme0n1").is_ok() {
        "nvme0n1".to_string()
    } else if fs::metadata("/sys/block/sda").is_ok() {
        "sda".to_string()
    } else {
        "unknown".to_string()
    }
}

// ============================================================================
// Tauri Commands (All async)
// ============================================================================

/// Get all tweaks organized by category (async)
#[tauri::command]
pub async fn get_tweaks() -> Result<Vec<TweakCategory>> {
    // Spawn blocking since we read from /proc and /sys
    let categories = tokio::task::spawn_blocking(|| {
        let block_device = get_main_block_device();
        let mut categories = Vec::new();

        // =========== Memory Tweaks ===========
        let swappiness = read_sys_value("/proc/sys/vm/swappiness");
        let vfs_cache = read_sys_value("/proc/sys/vm/vfs_cache_pressure");
        let dirty_ratio = read_sys_value("/proc/sys/vm/dirty_ratio");
        let dirty_bg_ratio = read_sys_value("/proc/sys/vm/dirty_background_ratio");

        categories.push(TweakCategory {
            id: "memory".to_string(),
            name: "Memory".to_string(),
            icon: "🧠".to_string(),
            tweaks: vec![
                Tweak {
                    id: "swappiness".to_string(),
                    name: "Swappiness".to_string(),
                    category: "memory".to_string(),
                    description: "Lower values reduce swap usage, keeping more data in RAM".to_string(),
                    current_value: swappiness.clone(),
                    recommended_value: "10".to_string(),
                    is_applied: swappiness == "10",
                    sysctl_key: Some("vm.swappiness".to_string()),
                    file_path: None,
                },
                Tweak {
                    id: "vfs_cache_pressure".to_string(),
                    name: "VFS Cache Pressure".to_string(),
                    category: "memory".to_string(),
                    description: "Lower values keep filesystem cache in memory longer".to_string(),
                    current_value: vfs_cache.clone(),
                    recommended_value: "50".to_string(),
                    is_applied: vfs_cache == "50",
                    sysctl_key: Some("vm.vfs_cache_pressure".to_string()),
                    file_path: None,
                },
                Tweak {
                    id: "dirty_ratio".to_string(),
                    name: "Dirty Ratio".to_string(),
                    category: "memory".to_string(),
                    description: "Max % of memory for dirty pages before sync".to_string(),
                    current_value: dirty_ratio.clone(),
                    recommended_value: "10".to_string(),
                    is_applied: dirty_ratio == "10",
                    sysctl_key: Some("vm.dirty_ratio".to_string()),
                    file_path: None,
                },
                Tweak {
                    id: "dirty_background_ratio".to_string(),
                    name: "Dirty Background Ratio".to_string(),
                    category: "memory".to_string(),
                    description: "% of memory before background writeback starts".to_string(),
                    current_value: dirty_bg_ratio.clone(),
                    recommended_value: "5".to_string(),
                    is_applied: dirty_bg_ratio == "5",
                    sysctl_key: Some("vm.dirty_background_ratio".to_string()),
                    file_path: None,
                },
            ],
        });

        // =========== Network Tweaks ===========
        let tcp_cc = read_sys_value("/proc/sys/net/ipv4/tcp_congestion_control");
        let tcp_fastopen = read_sys_value("/proc/sys/net/ipv4/tcp_fastopen");
        let tcp_mtu_probing = read_sys_value("/proc/sys/net/ipv4/tcp_mtu_probing");

        categories.push(TweakCategory {
            id: "network".to_string(),
            name: "Network".to_string(),
            icon: "🌐".to_string(),
            tweaks: vec![
                Tweak {
                    id: "tcp_congestion".to_string(),
                    name: "TCP Congestion Control".to_string(),
                    category: "network".to_string(),
                    description: "BBR provides better throughput and lower latency".to_string(),
                    current_value: tcp_cc.clone(),
                    recommended_value: "bbr".to_string(),
                    is_applied: tcp_cc == "bbr",
                    sysctl_key: Some("net.ipv4.tcp_congestion_control".to_string()),
                    file_path: None,
                },
                Tweak {
                    id: "tcp_fastopen".to_string(),
                    name: "TCP Fast Open".to_string(),
                    category: "network".to_string(),
                    description: "Reduces latency for repeated connections".to_string(),
                    current_value: tcp_fastopen.clone(),
                    recommended_value: "3".to_string(),
                    is_applied: tcp_fastopen == "3",
                    sysctl_key: Some("net.ipv4.tcp_fastopen".to_string()),
                    file_path: None,
                },
                Tweak {
                    id: "tcp_mtu_probing".to_string(),
                    name: "TCP MTU Probing".to_string(),
                    category: "network".to_string(),
                    description: "Automatically find optimal packet size".to_string(),
                    current_value: tcp_mtu_probing.clone(),
                    recommended_value: "1".to_string(),
                    is_applied: tcp_mtu_probing == "1",
                    sysctl_key: Some("net.ipv4.tcp_mtu_probing".to_string()),
                    file_path: None,
                },
            ],
        });

        // =========== CPU Tweaks ===========
        let governor = read_sys_value("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor");
        let available = get_available_governors();

        categories.push(TweakCategory {
            id: "cpu".to_string(),
            name: "CPU".to_string(),
            icon: "⚡".to_string(),
            tweaks: vec![Tweak {
                id: "cpu_governor".to_string(),
                name: "CPU Governor".to_string(),
                category: "cpu".to_string(),
                description: format!("Available: {}", available.join(", ")),
                current_value: governor.clone(),
                recommended_value: "performance".to_string(),
                is_applied: governor == "performance",
                sysctl_key: None,
                file_path: Some("/sys/devices/system/cpu/cpu*/cpufreq/scaling_governor".to_string()),
            }],
        });

        // =========== Disk Tweaks ===========
        let scheduler = get_io_scheduler(&block_device);

        categories.push(TweakCategory {
            id: "disk".to_string(),
            name: "Disk I/O".to_string(),
            icon: "💾".to_string(),
            tweaks: vec![Tweak {
                id: "io_scheduler".to_string(),
                name: "I/O Scheduler".to_string(),
                category: "disk".to_string(),
                description: "Best for SSD/NVMe: none or mq-deadline".to_string(),
                current_value: scheduler.clone(),
                recommended_value: if block_device.starts_with("nvme") {
                    "none".to_string()
                } else {
                    "mq-deadline".to_string()
                },
                is_applied: scheduler == "none" || scheduler == "mq-deadline",
                sysctl_key: None,
                file_path: Some(format!("/sys/block/{}/queue/scheduler", block_device)),
            }],
        });

        categories
    }).await.unwrap();

    Ok(categories)
}

/// Apply a specific tweak (async with timeout)
#[tauri::command]
pub async fn apply_tweak(tweak_id: String, value: String) -> Result<String> {
    match tweak_id.as_str() {
        // Sysctl tweaks
        "swappiness" | "vfs_cache_pressure" | "dirty_ratio" | "dirty_background_ratio"
        | "tcp_congestion" | "tcp_fastopen" | "tcp_mtu_probing" => {
            let key = match tweak_id.as_str() {
                "swappiness" => "vm.swappiness",
                "vfs_cache_pressure" => "vm.vfs_cache_pressure",
                "dirty_ratio" => "vm.dirty_ratio",
                "dirty_background_ratio" => "vm.dirty_background_ratio",
                "tcp_congestion" => "net.ipv4.tcp_congestion_control",
                "tcp_fastopen" => "net.ipv4.tcp_fastopen",
                "tcp_mtu_probing" => "net.ipv4.tcp_mtu_probing",
                _ => return Err(AppError::System("Unknown sysctl key".to_string())),
            };

            // For BBR, we need to load the module first
            if tweak_id == "tcp_congestion" && value == "bbr" {
                let _ = privileged::run_privileged_shell("modprobe tcp_bbr").await;
            }

            privileged::run_privileged("sysctl", &["-w", &format!("{}={}", key, value)]).await?;
            Ok(format!("{} set to {}", key, value))
        }

        // CPU Governor
        "cpu_governor" => {
            let script = format!(
                "for gov in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do echo {} > \"$gov\"; done",
                value
            );
            privileged::run_privileged_shell(&script).await?;
            Ok(format!("CPU governor set to {}", value))
        }

        // I/O Scheduler
        "io_scheduler" => {
            let device = get_main_block_device();
            let script = format!(
                "echo {} > /sys/block/{}/queue/scheduler",
                value, device
            );
            privileged::run_privileged_shell(&script).await?;
            Ok(format!("I/O scheduler set to {}", value))
        }

        _ => Err(AppError::System(format!("Unknown tweak: {}", tweak_id))),
    }
}

/// Apply all recommended tweaks at once (async)
#[tauri::command]
pub async fn apply_all_recommended() -> Result<Vec<String>> {
    let mut results = Vec::new();

    // Memory optimizations
    let memory_tweaks = [
        ("vm.swappiness", "10"),
        ("vm.vfs_cache_pressure", "50"),
        ("vm.dirty_ratio", "10"),
        ("vm.dirty_background_ratio", "5"),
    ];

    for (key, value) in memory_tweaks {
        if privileged::run_privileged("sysctl", &["-w", &format!("{}={}", key, value)]).await.is_ok() {
            results.push(format!("✓ {}", key));
        }
    }

    // Network optimizations
    let _ = privileged::run_privileged_shell("modprobe tcp_bbr").await;
    let network_tweaks = [
        ("net.ipv4.tcp_congestion_control", "bbr"),
        ("net.ipv4.tcp_fastopen", "3"),
        ("net.ipv4.tcp_mtu_probing", "1"),
    ];

    for (key, value) in network_tweaks {
        if privileged::run_privileged("sysctl", &["-w", &format!("{}={}", key, value)]).await.is_ok() {
            results.push(format!("✓ {}", key));
        }
    }

    Ok(results)
}
