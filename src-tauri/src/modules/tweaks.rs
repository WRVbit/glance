//! System tweaks module - Enhanced
//! Reads from /proc/sys and /sys, applies via pkexec sysctl (async)
//! Features: sliders with ranges, device tier detection, TCP algorithm selection

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
    // New fields for sliders and options
    pub min_value: Option<i32>,
    pub max_value: Option<i32>,
    pub options: Option<Vec<String>>, // For dropdown/selector
    pub tweak_type: String, // "slider", "selector", "preset"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TweakCategory {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub tweaks: Vec<Tweak>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub tier: String, // "low", "mid", "high"
    pub ram_gb: u64,
    pub disk_type: String, // "nvme", "ssd", "hdd"
    pub disk_device: String,
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

/// Get available TCP congestion control algorithms
fn get_available_tcp_algos() -> Vec<String> {
    read_sys_value("/proc/sys/net/ipv4/tcp_available_congestion_control")
        .split_whitespace()
        .map(|s| s.to_string())
        .collect()
}

/// Get current I/O scheduler for a block device
fn get_io_scheduler(device: &str) -> String {
    let path = format!("/sys/block/{}/queue/scheduler", device);
    let content = read_sys_value(&path);

    content
        .split('[')
        .nth(1)
        .and_then(|s| s.split(']').next())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Get available I/O schedulers for a device
fn get_available_schedulers(device: &str) -> Vec<String> {
    let path = format!("/sys/block/{}/queue/scheduler", device);
    let content = read_sys_value(&path);
    
    content
        .replace("[", "")
        .replace("]", "")
        .split_whitespace()
        .map(|s| s.to_string())
        .collect()
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

/// Detect disk type (NVMe, SSD, or HDD)
fn get_disk_type(device: &str) -> String {
    if device.starts_with("nvme") {
        return "nvme".to_string();
    }
    
    // Check if rotational (HDD = 1, SSD = 0)
    let path = format!("/sys/block/{}/queue/rotational", device);
    let rotational = read_sys_value(&path);
    
    if rotational == "0" {
        "ssd".to_string()
    } else if rotational == "1" {
        "hdd".to_string()
    } else {
        "unknown".to_string()
    }
}

/// Detect device tier based on RAM
fn get_device_tier() -> (String, u64) {
    let meminfo = read_sys_value("/proc/meminfo");
    let mut ram_kb = 0u64;
    
    for line in meminfo.lines() {
        if line.starts_with("MemTotal:") {
            if let Some(kb_str) = line.split_whitespace().nth(1) {
                ram_kb = kb_str.parse().unwrap_or(0);
            }
            break;
        }
    }
    
    let ram_gb = ram_kb / 1024 / 1024;
    
    let tier = if ram_gb < 4 {
        "low".to_string()
    } else if ram_gb <= 16 {
        "mid".to_string()
    } else {
        "high".to_string()
    };
    
    (tier, ram_gb)
}

/// Get recommended value based on device tier
fn get_recommended(tier: &str, low: &str, mid: &str, high: &str) -> String {
    match tier {
        "low" => low.to_string(),
        "high" => high.to_string(),
        _ => mid.to_string(),
    }
}

// ============================================================================
// Tauri Commands (All async)
// ============================================================================

/// Get device information
#[tauri::command]
pub async fn get_device_info() -> Result<DeviceInfo> {
    let info = tokio::task::spawn_blocking(|| {
        let (tier, ram_gb) = get_device_tier();
        let disk_device = get_main_block_device();
        let disk_type = get_disk_type(&disk_device);
        
        DeviceInfo {
            tier,
            ram_gb,
            disk_type,
            disk_device,
        }
    }).await.unwrap();
    
    Ok(info)
}

/// Get all tweaks organized by category (async)
#[tauri::command]
pub async fn get_tweaks() -> Result<Vec<TweakCategory>> {
    let categories = tokio::task::spawn_blocking(|| {
        let block_device = get_main_block_device();
        let disk_type = get_disk_type(&block_device);
        let (tier, _) = get_device_tier();
        let available_governors = get_available_governors();
        let available_tcp = get_available_tcp_algos();
        let available_io = get_available_schedulers(&block_device);
        
        let mut categories = Vec::new();

        // =========== Memory Tweaks (Sliders) ===========
        let swappiness = read_sys_value("/proc/sys/vm/swappiness");
        let vfs_cache = read_sys_value("/proc/sys/vm/vfs_cache_pressure");
        let dirty_ratio = read_sys_value("/proc/sys/vm/dirty_ratio");
        let dirty_bg_ratio = read_sys_value("/proc/sys/vm/dirty_background_ratio");
        
        // Check ZRAM status
        let zram_enabled = std::path::Path::new("/sys/block/zram0").exists();
        let zram_status = if zram_enabled { "enabled" } else { "disabled" };

        let swap_rec = get_recommended(&tier, "30", "10", "5");
        let vfs_rec = get_recommended(&tier, "100", "50", "30");
        let dirty_rec = get_recommended(&tier, "20", "10", "5");
        let dirty_bg_rec = get_recommended(&tier, "10", "5", "3");

        categories.push(TweakCategory {
            id: "memory".to_string(),
            name: "Memory".to_string(),
            icon: "🧠".to_string(),
            tweaks: vec![
                Tweak {
                    id: "swappiness".to_string(),
                    name: "Swappiness".to_string(),
                    category: "memory".to_string(),
                    description: "How aggressively to use swap. Lower = prefer RAM.".to_string(),
                    current_value: swappiness.clone(),
                    recommended_value: swap_rec.clone(),
                    is_applied: swappiness == swap_rec,
                    sysctl_key: Some("vm.swappiness".to_string()),
                    file_path: None,
                    min_value: Some(0),
                    max_value: Some(100),
                    options: None,
                    tweak_type: "slider".to_string(),
                },
                Tweak {
                    id: "vfs_cache_pressure".to_string(),
                    name: "VFS Cache Pressure".to_string(),
                    category: "memory".to_string(),
                    description: "How aggressively to reclaim directory/inode cache.".to_string(),
                    current_value: vfs_cache.clone(),
                    recommended_value: vfs_rec.clone(),
                    is_applied: vfs_cache == vfs_rec,
                    sysctl_key: Some("vm.vfs_cache_pressure".to_string()),
                    file_path: None,
                    min_value: Some(10),
                    max_value: Some(200),
                    options: None,
                    tweak_type: "slider".to_string(),
                },
                Tweak {
                    id: "dirty_ratio".to_string(),
                    name: "Dirty Ratio".to_string(),
                    category: "memory".to_string(),
                    description: "Max % of memory for dirty pages before sync.".to_string(),
                    current_value: dirty_ratio.clone(),
                    recommended_value: dirty_rec.clone(),
                    is_applied: dirty_ratio == dirty_rec,
                    sysctl_key: Some("vm.dirty_ratio".to_string()),
                    file_path: None,
                    min_value: Some(5),
                    max_value: Some(50),
                    options: None,
                    tweak_type: "slider".to_string(),
                },
                Tweak {
                    id: "dirty_background_ratio".to_string(),
                    name: "Dirty Background Ratio".to_string(),
                    category: "memory".to_string(),
                    description: "% of memory before background writeback starts.".to_string(),
                    current_value: dirty_bg_ratio.clone(),
                    recommended_value: dirty_bg_rec.clone(),
                    is_applied: dirty_bg_ratio == dirty_bg_rec,
                    sysctl_key: Some("vm.dirty_background_ratio".to_string()),
                    file_path: None,
                    min_value: Some(1),
                    max_value: Some(25),
                    options: None,
                    tweak_type: "slider".to_string(),
                },
                Tweak {
                    id: "zram".to_string(),
                    name: "ZRAM Compressed Swap".to_string(),
                    category: "memory".to_string(),
                    description: "Compress RAM for extra virtual memory. Great for low-RAM systems.".to_string(),
                    current_value: zram_status.to_string(),
                    recommended_value: "enabled".to_string(),
                    is_applied: zram_enabled,
                    sysctl_key: None,
                    file_path: None,
                    min_value: None,
                    max_value: None,
                    options: Some(vec!["disabled".to_string(), "enabled".to_string()]),
                    tweak_type: "toggle".to_string(),
                },
            ],
        });

        // =========== Network Tweaks ===========
        let tcp_cc = read_sys_value("/proc/sys/net/ipv4/tcp_congestion_control");
        let tcp_fastopen = read_sys_value("/proc/sys/net/ipv4/tcp_fastopen");
        let tcp_mtu_probing = read_sys_value("/proc/sys/net/ipv4/tcp_mtu_probing");
        let rmem_max = read_sys_value("/proc/sys/net/core/rmem_max");
        let wmem_max = read_sys_value("/proc/sys/net/core/wmem_max");

        categories.push(TweakCategory {
            id: "network".to_string(),
            name: "Network".to_string(),
            icon: "🌐".to_string(),
            tweaks: vec![
                Tweak {
                    id: "tcp_congestion".to_string(),
                    name: "TCP Congestion Control".to_string(),
                    category: "network".to_string(),
                    description: "BBR: Better throughput. CUBIC: Default, stable.".to_string(),
                    current_value: tcp_cc.clone(),
                    recommended_value: "bbr".to_string(),
                    is_applied: tcp_cc == "bbr",
                    sysctl_key: Some("net.ipv4.tcp_congestion_control".to_string()),
                    file_path: None,
                    min_value: None,
                    max_value: None,
                    options: Some(available_tcp),
                    tweak_type: "selector".to_string(),
                },
                Tweak {
                    id: "tcp_fastopen".to_string(),
                    name: "TCP Fast Open".to_string(),
                    category: "network".to_string(),
                    description: "0=Off, 1=Client, 2=Server, 3=Both".to_string(),
                    current_value: tcp_fastopen.clone(),
                    recommended_value: "3".to_string(),
                    is_applied: tcp_fastopen == "3",
                    sysctl_key: Some("net.ipv4.tcp_fastopen".to_string()),
                    file_path: None,
                    min_value: Some(0),
                    max_value: Some(3),
                    options: None,
                    tweak_type: "slider".to_string(),
                },
                Tweak {
                    id: "tcp_mtu_probing".to_string(),
                    name: "TCP MTU Probing".to_string(),
                    category: "network".to_string(),
                    description: "0=Off, 1=On disconnect, 2=Always".to_string(),
                    current_value: tcp_mtu_probing.clone(),
                    recommended_value: "1".to_string(),
                    is_applied: tcp_mtu_probing == "1",
                    sysctl_key: Some("net.ipv4.tcp_mtu_probing".to_string()),
                    file_path: None,
                    min_value: Some(0),
                    max_value: Some(2),
                    options: None,
                    tweak_type: "slider".to_string(),
                },
                Tweak {
                    id: "rmem_max".to_string(),
                    name: "Receive Buffer Max".to_string(),
                    category: "network".to_string(),
                    description: "Maximum socket receive buffer (bytes).".to_string(),
                    current_value: rmem_max.clone(),
                    recommended_value: "16777216".to_string(),
                    is_applied: rmem_max.parse::<u64>().unwrap_or(0) >= 16777216,
                    sysctl_key: Some("net.core.rmem_max".to_string()),
                    file_path: None,
                    min_value: None,
                    max_value: None,
                    options: Some(vec![
                        "212992".to_string(),    // Default
                        "4194304".to_string(),   // 4MB
                        "16777216".to_string(),  // 16MB (Recommended)
                        "33554432".to_string(),  // 32MB
                    ]),
                    tweak_type: "selector".to_string(),
                },
                Tweak {
                    id: "wmem_max".to_string(),
                    name: "Send Buffer Max".to_string(),
                    category: "network".to_string(),
                    description: "Maximum socket send buffer (bytes).".to_string(),
                    current_value: wmem_max.clone(),
                    recommended_value: "16777216".to_string(),
                    is_applied: wmem_max.parse::<u64>().unwrap_or(0) >= 16777216,
                    sysctl_key: Some("net.core.wmem_max".to_string()),
                    file_path: None,
                    min_value: None,
                    max_value: None,
                    options: Some(vec![
                        "212992".to_string(),
                        "4194304".to_string(),
                        "16777216".to_string(),
                        "33554432".to_string(),
                    ]),
                    tweak_type: "selector".to_string(),
                },
            ],
        });

        // =========== CPU Tweaks (Presets) ===========
        let governor = read_sys_value("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor");

        categories.push(TweakCategory {
            id: "cpu".to_string(),
            name: "CPU".to_string(),
            icon: "⚡".to_string(),
            tweaks: vec![Tweak {
                id: "cpu_governor".to_string(),
                name: "CPU Power Mode".to_string(),
                category: "cpu".to_string(),
                description: format!("Available: {}", available_governors.join(", ")),
                current_value: governor.clone(),
                recommended_value: "performance".to_string(),
                is_applied: governor == "performance",
                sysctl_key: None,
                file_path: Some("/sys/devices/system/cpu/cpu*/cpufreq/scaling_governor".to_string()),
                min_value: None,
                max_value: None,
                options: Some(available_governors),
                tweak_type: "preset".to_string(), // Special type for 3-button preset
            }],
        });

        // =========== Disk Tweaks ===========
        let scheduler = get_io_scheduler(&block_device);
        
        // Recommend based on disk type
        let io_rec = match disk_type.as_str() {
            "nvme" => "none".to_string(),
            "ssd" => "mq-deadline".to_string(),
            "hdd" => if available_io.contains(&"bfq".to_string()) { "bfq".to_string() } else { "mq-deadline".to_string() },
            _ => "mq-deadline".to_string(),
        };

        categories.push(TweakCategory {
            id: "disk".to_string(),
            name: format!("Disk I/O ({})", disk_type.to_uppercase()),
            icon: "💾".to_string(),
            tweaks: vec![Tweak {
                id: "io_scheduler".to_string(),
                name: "I/O Scheduler".to_string(),
                category: "disk".to_string(),
                description: format!("Device: {} | Type: {}", block_device, disk_type.to_uppercase()),
                current_value: scheduler.clone(),
                recommended_value: io_rec.clone(),
                is_applied: scheduler == io_rec,
                sysctl_key: None,
                file_path: Some(format!("/sys/block/{}/queue/scheduler", block_device)),
                min_value: None,
                max_value: None,
                options: Some(available_io),
                tweak_type: "selector".to_string(),
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
        // Sysctl tweaks (memory, network)
        "swappiness" | "vfs_cache_pressure" | "dirty_ratio" | "dirty_background_ratio"
        | "tcp_congestion" | "tcp_fastopen" | "tcp_mtu_probing" | "rmem_max" | "wmem_max" => {
            let key = match tweak_id.as_str() {
                "swappiness" => "vm.swappiness",
                "vfs_cache_pressure" => "vm.vfs_cache_pressure",
                "dirty_ratio" => "vm.dirty_ratio",
                "dirty_background_ratio" => "vm.dirty_background_ratio",
                "tcp_congestion" => "net.ipv4.tcp_congestion_control",
                "tcp_fastopen" => "net.ipv4.tcp_fastopen",
                "tcp_mtu_probing" => "net.ipv4.tcp_mtu_probing",
                "rmem_max" => "net.core.rmem_max",
                "wmem_max" => "net.core.wmem_max",
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

        // ZRAM Compressed Swap
        "zram" => {
            if value == "enabled" {
                // Enable ZRAM with 50% of RAM using zstd compression
                let script = r#"
                    # Load zram module
                    modprobe zram num_devices=1
                    
                    # Get RAM size and calculate 50%
                    RAM_KB=$(grep MemTotal /proc/meminfo | awk '{print $2}')
                    ZRAM_SIZE=$((RAM_KB * 1024 / 2))
                    
                    # Setup zram0 if not already active
                    if [ ! -e /sys/block/zram0/disksize ] || [ "$(cat /sys/block/zram0/disksize)" = "0" ]; then
                        echo zstd > /sys/block/zram0/comp_algorithm 2>/dev/null || echo lz4 > /sys/block/zram0/comp_algorithm
                        echo $ZRAM_SIZE > /sys/block/zram0/disksize
                        mkswap /dev/zram0
                        swapon -p 100 /dev/zram0
                    fi
                "#;
                privileged::run_privileged_shell(script).await?;
                Ok("ZRAM enabled with 50% of RAM using zstd compression".to_string())
            } else {
                // Disable ZRAM
                let script = r#"
                    swapoff /dev/zram0 2>/dev/null
                    echo 1 > /sys/block/zram0/reset 2>/dev/null
                    rmmod zram 2>/dev/null || true
                "#;
                privileged::run_privileged_shell(script).await?;
                Ok("ZRAM disabled".to_string())
            }
        }

        _ => Err(AppError::System(format!("Unknown tweak: {}", tweak_id))),
    }
}

/// Apply all recommended tweaks at once (async)
#[tauri::command]
pub async fn apply_all_recommended() -> Result<Vec<String>> {
    let mut results = Vec::new();
    let (tier, _) = get_device_tier();

    // Memory optimizations based on tier
    let swap_val = get_recommended(&tier, "30", "10", "5");
    let vfs_val = get_recommended(&tier, "100", "50", "30");
    let dirty_val = get_recommended(&tier, "20", "10", "5");
    let dirty_bg_val = get_recommended(&tier, "10", "5", "3");

    let memory_tweaks = [
        ("vm.swappiness", swap_val.as_str()),
        ("vm.vfs_cache_pressure", vfs_val.as_str()),
        ("vm.dirty_ratio", dirty_val.as_str()),
        ("vm.dirty_background_ratio", dirty_bg_val.as_str()),
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
        ("net.core.rmem_max", "16777216"),
        ("net.core.wmem_max", "16777216"),
    ];

    for (key, value) in network_tweaks {
        if privileged::run_privileged("sysctl", &["-w", &format!("{}={}", key, value)]).await.is_ok() {
            results.push(format!("✓ {}", key));
        }
    }

    // CPU Governor - performance
    let governor_script = "for gov in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do echo performance > \"$gov\"; done";
    if privileged::run_privileged_shell(governor_script).await.is_ok() {
        results.push("✓ CPU Governor".to_string());
    }

    // I/O Scheduler - auto-detect best
    let device = get_main_block_device();
    let disk_type = get_disk_type(&device);
    let io_val = match disk_type.as_str() {
        "nvme" => "none",
        "ssd" => "mq-deadline",
        _ => "mq-deadline",
    };
    let io_script = format!("echo {} > /sys/block/{}/queue/scheduler", io_val, device);
    if privileged::run_privileged_shell(&io_script).await.is_ok() {
        results.push(format!("✓ I/O Scheduler ({})", io_val));
    }

    Ok(results)
}
