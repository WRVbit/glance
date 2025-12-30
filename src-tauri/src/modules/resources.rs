//! Resource Monitor module
//! Collects and stores resource usage history for graphing (Non-blocking)
//! Enhanced version with GPU, Disk I/O, and per-core CPU monitoring

use crate::error::{AppError, Result};
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs;
use std::sync::Mutex;
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, Networks};
use tauri::State;

// ============================================================================
// Data Structures
// ============================================================================

const HISTORY_SIZE: usize = 60; // 60 seconds of history

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResourceSnapshot {
    pub timestamp: u64,
    pub cpu_percent: f32,
    pub per_core_percent: Vec<f32>,
    pub ram_used_bytes: u64,
    pub ram_total_bytes: u64,
    pub ram_cached_bytes: u64,
    pub swap_used_bytes: u64,
    pub swap_total_bytes: u64,
    pub net_rx_bytes: u64,
    pub net_tx_bytes: u64,
    pub disk_read_bytes: u64,
    pub disk_write_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceHistory {
    pub snapshots: Vec<ResourceSnapshot>,
    pub net_rx_speed: Vec<u64>,
    pub net_tx_speed: Vec<u64>,
    pub disk_read_speed: Vec<u64>,
    pub disk_write_speed: Vec<u64>,
    pub ram_history: Vec<f32>, // RAM usage percent history
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    pub name: String,
    pub vendor: String,
    pub vram_total_mb: u64,
    pub vram_used_mb: u64,
    pub usage_percent: Option<f32>,
    pub temperature_c: Option<f32>,
    pub driver_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskIoStats {
    pub name: String,
    pub read_bytes: u64,
    pub write_bytes: u64,
}

/// Shared state for resource history
pub struct ResourceHistoryState {
    pub history: Mutex<VecDeque<ResourceSnapshot>>,
}

impl ResourceHistoryState {
    pub fn new() -> Self {
        Self {
            history: Mutex::new(VecDeque::with_capacity(HISTORY_SIZE)),
        }
    }
}

impl Default for ResourceHistoryState {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Read disk I/O stats from /proc/diskstats
fn read_disk_io() -> (u64, u64) {
    let content = fs::read_to_string("/proc/diskstats").unwrap_or_default();
    let mut total_read: u64 = 0;
    let mut total_write: u64 = 0;

    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 14 {
            let name = parts[2];
            // Only count main disks (sda, nvme0n1, etc), not partitions
            if (name.starts_with("sd") && name.len() == 3)
                || (name.starts_with("nvme") && name.contains("n") && !name.contains("p"))
                || (name.starts_with("vd") && name.len() == 3)
            {
                // Field 6 = sectors read, Field 10 = sectors written
                // Sector size is typically 512 bytes
                if let (Ok(read), Ok(write)) = (parts[5].parse::<u64>(), parts[9].parse::<u64>()) {
                    total_read += read * 512;
                    total_write += write * 512;
                }
            }
        }
    }

    (total_read, total_write)
}

/// Try to get GPU info using nvidia-smi or other tools
fn detect_gpu() -> Option<GpuInfo> {
    // Try NVIDIA first
    if let Ok(output) = std::process::Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,memory.total,memory.used,utilization.gpu,temperature.gpu,driver_version",
            "--format=csv,noheader,nounits",
        ])
        .output()
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let parts: Vec<&str> = stdout.trim().split(", ").collect();
            if parts.len() >= 6 {
                return Some(GpuInfo {
                    name: parts[0].to_string(),
                    vendor: "nvidia".to_string(),
                    vram_total_mb: parts[1].parse().unwrap_or(0),
                    vram_used_mb: parts[2].parse().unwrap_or(0),
                    usage_percent: parts[3].parse().ok(),
                    temperature_c: parts[4].parse().ok(),
                    driver_version: Some(parts[5].to_string()),
                });
            }
        }
    }

    // Try to detect AMD GPU via /sys
    if let Ok(entries) = fs::read_dir("/sys/class/drm") {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name.starts_with("card") && !name.contains("-") {
                let device_path = path.join("device");
                
                // Check if it's AMD
                if let Ok(vendor) = fs::read_to_string(device_path.join("vendor")) {
                    if vendor.trim() == "0x1002" {
                        // AMD vendor ID
                        let gpu_name = fs::read_to_string(device_path.join("product_name"))
                            .or_else(|_| fs::read_to_string(device_path.join("device")))
                            .unwrap_or_else(|_| "AMD GPU".to_string());
                        
                        // Try to get VRAM from mem_info_vram_total
                        let vram_total = fs::read_to_string(device_path.join("mem_info_vram_total"))
                            .ok()
                            .and_then(|s| s.trim().parse::<u64>().ok())
                            .map(|b| b / 1024 / 1024)
                            .unwrap_or(0);
                        
                        let vram_used = fs::read_to_string(device_path.join("mem_info_vram_used"))
                            .ok()
                            .and_then(|s| s.trim().parse::<u64>().ok())
                            .map(|b| b / 1024 / 1024)
                            .unwrap_or(0);
                        
                        // Try to get temperature
                        let temp = fs::read_to_string(device_path.join("hwmon/hwmon0/temp1_input"))
                            .or_else(|_| fs::read_to_string(device_path.join("hwmon/hwmon1/temp1_input")))
                            .ok()
                            .and_then(|s| s.trim().parse::<f32>().ok())
                            .map(|t| t / 1000.0);
                        
                        return Some(GpuInfo {
                            name: gpu_name.trim().to_string(),
                            vendor: "amd".to_string(),
                            vram_total_mb: vram_total,
                            vram_used_mb: vram_used,
                            usage_percent: None,
                            temperature_c: temp,
                            driver_version: None,
                        });
                    }
                }
            }
        }
    }

    // Try Intel GPU via /sys
    if let Ok(output) = std::process::Command::new("lspci")
        .args(["-nn"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if line.contains("VGA") && line.to_lowercase().contains("intel") {
                // Extract GPU name
                let name = line.split(":").nth(2).unwrap_or("Intel GPU").trim();
                return Some(GpuInfo {
                    name: name.to_string(),
                    vendor: "intel".to_string(),
                    vram_total_mb: 0, // Intel uses shared memory
                    vram_used_mb: 0,
                    usage_percent: None,
                    temperature_c: None,
                    driver_version: None,
                });
            }
        }
    }

    None
}

// ============================================================================
// Tauri Commands (All non-blocking)
// ============================================================================

/// Get current resource snapshot with enhanced data
#[tauri::command]
pub async fn get_resource_snapshot(app_state: State<'_, AppState>) -> Result<ResourceSnapshot> {
    let sys = app_state.sys.clone();

    let snapshot = tokio::task::spawn_blocking(move || {
        let mut sys = sys.lock().unwrap();

        sys.refresh_cpu_specifics(CpuRefreshKind::nothing().with_cpu_usage());
        sys.refresh_memory_specifics(MemoryRefreshKind::everything());

        let per_core: Vec<f32> = sys.cpus().iter().map(|cpu| cpu.cpu_usage()).collect();
        let cpu_percent: f32 = if per_core.is_empty() {
            0.0
        } else {
            per_core.iter().sum::<f32>() / per_core.len() as f32
        };

        let networks = Networks::new_with_refreshed_list();
        let (net_rx, net_tx) = networks
            .iter()
            .filter(|(name, _)| !name.starts_with("lo") && !name.starts_with("docker"))
            .fold((0u64, 0u64), |(rx, tx), (_, data)| {
                (rx + data.total_received(), tx + data.total_transmitted())
            });

        let (disk_read, disk_write) = read_disk_io();

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Calculate cached memory
        let total = sys.total_memory();
        let used = sys.used_memory();
        let available = sys.available_memory();
        let cached = if total > used + available {
            total - used - available
        } else {
            0
        };

        ResourceSnapshot {
            timestamp,
            cpu_percent,
            per_core_percent: per_core,
            ram_used_bytes: used,
            ram_total_bytes: total,
            ram_cached_bytes: cached,
            swap_used_bytes: sys.used_swap(),
            swap_total_bytes: sys.total_swap(),
            net_rx_bytes: net_rx,
            net_tx_bytes: net_tx,
            disk_read_bytes: disk_read,
            disk_write_bytes: disk_write,
        }
    })
    .await
    .unwrap();

    Ok(snapshot)
}

/// Get resource history with calculated speeds
#[tauri::command]
pub fn get_resource_history(history_state: State<ResourceHistoryState>) -> Result<ResourceHistory> {
    let history = history_state.history.lock().unwrap();

    let snapshots: Vec<ResourceSnapshot> = history.iter().cloned().collect();

    let mut net_rx_speed = Vec::new();
    let mut net_tx_speed = Vec::new();
    let mut disk_read_speed = Vec::new();
    let mut disk_write_speed = Vec::new();
    let mut ram_history = Vec::new();

    for i in 0..snapshots.len() {
        // RAM history
        let ram_percent = if snapshots[i].ram_total_bytes > 0 {
            (snapshots[i].ram_used_bytes as f32 / snapshots[i].ram_total_bytes as f32) * 100.0
        } else {
            0.0
        };
        ram_history.push(ram_percent);

        // Speed calculations
        if i > 0 {
            let prev = &snapshots[i - 1];
            let curr = &snapshots[i];
            let time_delta = curr.timestamp.saturating_sub(prev.timestamp).max(1);

            net_rx_speed.push(curr.net_rx_bytes.saturating_sub(prev.net_rx_bytes) / time_delta);
            net_tx_speed.push(curr.net_tx_bytes.saturating_sub(prev.net_tx_bytes) / time_delta);
            disk_read_speed.push(curr.disk_read_bytes.saturating_sub(prev.disk_read_bytes) / time_delta);
            disk_write_speed.push(curr.disk_write_bytes.saturating_sub(prev.disk_write_bytes) / time_delta);
        } else {
            net_rx_speed.push(0);
            net_tx_speed.push(0);
            disk_read_speed.push(0);
            disk_write_speed.push(0);
        }
    }

    Ok(ResourceHistory {
        snapshots,
        net_rx_speed,
        net_tx_speed,
        disk_read_speed,
        disk_write_speed,
        ram_history,
    })
}

/// Add a snapshot to history
#[tauri::command]
pub fn add_resource_snapshot(
    snapshot: ResourceSnapshot,
    history_state: State<ResourceHistoryState>,
) -> Result<()> {
    let mut history = history_state.history.lock().unwrap();

    while history.len() >= HISTORY_SIZE {
        history.pop_front();
    }

    history.push_back(snapshot);
    Ok(())
}

/// Clear resource history
#[tauri::command]
pub fn clear_resource_history(history_state: State<ResourceHistoryState>) -> Result<()> {
    let mut history = history_state.history.lock().unwrap();
    history.clear();
    Ok(())
}

/// Get per-core CPU usage
#[tauri::command]
pub async fn get_per_core_usage(app_state: State<'_, AppState>) -> Result<Vec<f32>> {
    let sys = app_state.sys.clone();
    let per_core = tokio::task::spawn_blocking(move || {
        let mut sys = sys.lock().unwrap();
        sys.refresh_cpu_specifics(CpuRefreshKind::nothing().with_cpu_usage());
        sys.cpus().iter().map(|cpu| cpu.cpu_usage()).collect::<Vec<f32>>()
    })
    .await
    .unwrap();

    Ok(per_core)
}

/// Get GPU information
#[tauri::command]
pub async fn get_gpu_info() -> Result<Option<GpuInfo>> {
    let gpu = tokio::task::spawn_blocking(detect_gpu).await.unwrap();
    Ok(gpu)
}

/// Get disk I/O statistics
#[tauri::command]
pub async fn get_disk_io_stats() -> Result<Vec<DiskIoStats>> {
    let stats = tokio::task::spawn_blocking(|| {
        let content = fs::read_to_string("/proc/diskstats").unwrap_or_default();
        let mut result = Vec::new();

        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 14 {
                let name = parts[2];
                if (name.starts_with("sd") && name.len() == 3)
                    || (name.starts_with("nvme") && name.contains("n") && !name.contains("p"))
                    || (name.starts_with("vd") && name.len() == 3)
                {
                    if let (Ok(read), Ok(write)) = (parts[5].parse::<u64>(), parts[9].parse::<u64>()) {
                        result.push(DiskIoStats {
                            name: name.to_string(),
                            read_bytes: read * 512,
                            write_bytes: write * 512,
                        });
                    }
                }
            }
        }

        result
    })
    .await
    .unwrap();

    Ok(stats)
}
