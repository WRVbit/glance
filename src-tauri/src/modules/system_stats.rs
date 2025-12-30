//! System statistics module
//! Uses native sysinfo crate - NO shell commands, NO blocking sleep

use crate::error::Result;
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use sysinfo::{CpuRefreshKind, Disks, MemoryRefreshKind, Networks, System};
use tauri::State;

// ============================================================================
// Data Structures
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub hostname: String,
    pub os_name: String,
    pub os_version: String,
    pub kernel_version: String,
    pub uptime_seconds: u64,
    pub cpu_model: String,
    pub cpu_cores: usize,
    pub cpu_threads: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuStats {
    pub usage_percent: f32,
    pub per_core: Vec<f32>,
    pub frequency_mhz: u64,
    pub core_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub cached_bytes: u64,
    pub swap_total_bytes: u64,
    pub swap_used_bytes: u64,
    pub usage_percent: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskStats {
    pub name: String,
    pub mount_point: String,
    pub filesystem: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub usage_percent: f32,
    pub is_removable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStats {
    pub interface: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_packets: u64,
    pub tx_packets: u64,
}

// ============================================================================
// Tauri Commands (All async, non-blocking)
// ============================================================================

/// Get static system information (hostname, OS, CPU model, etc.)
#[tauri::command]
pub async fn get_system_info() -> Result<SystemInfo> {
    // Spawn blocking since System::new_all does disk I/O
    let info = tokio::task::spawn_blocking(|| {
        let sys = System::new_all();

        SystemInfo {
            hostname: System::host_name().unwrap_or_else(|| "Unknown".into()),
            os_name: System::name().unwrap_or_else(|| "Linux".into()),
            os_version: System::os_version().unwrap_or_else(|| "Unknown".into()),
            kernel_version: System::kernel_version().unwrap_or_else(|| "Unknown".into()),
            uptime_seconds: System::uptime(),
            cpu_model: sys
                .cpus()
                .first()
                .map(|c| c.brand().to_string())
                .unwrap_or_else(|| "Unknown".into()),
            cpu_cores: System::physical_core_count().unwrap_or(0),
            cpu_threads: sys.cpus().len(),
        }
    }).await.unwrap();

    Ok(info)
}

/// Get CPU statistics (usage, frequency) - NO SLEEP
/// For accurate readings, frontend should poll every 1-2 seconds
#[tauri::command]
pub async fn get_cpu_stats(state: State<'_, AppState>) -> Result<CpuStats> {
    // Clone Arc to pass to blocking thread
    let sys = state.sys.clone();
    
    let stats = tokio::task::spawn_blocking(move || {
        let mut sys = sys.lock().unwrap();

        // Refresh only CPU usage - no sleep needed
        sys.refresh_cpu_specifics(CpuRefreshKind::nothing().with_cpu_usage());

        let per_core: Vec<f32> = sys.cpus().iter().map(|cpu| cpu.cpu_usage()).collect();
        let usage_percent = if per_core.is_empty() {
            0.0
        } else {
            per_core.iter().sum::<f32>() / per_core.len() as f32
        };

        CpuStats {
            usage_percent,
            per_core,
            frequency_mhz: sys.cpus().first().map(|c| c.frequency()).unwrap_or(0),
            core_count: sys.cpus().len(),
        }
    }).await.unwrap();

    Ok(stats)
}

/// Get memory statistics
#[tauri::command]
pub async fn get_memory_stats(state: State<'_, AppState>) -> Result<MemoryStats> {
    // Clone Arc to pass to blocking thread
    let sys = state.sys.clone();
    
    let stats = tokio::task::spawn_blocking(move || {
        let mut sys = sys.lock().unwrap();

        // Selective refresh - only memory
        sys.refresh_memory_specifics(MemoryRefreshKind::everything());

        let total = sys.total_memory();
        let used = sys.used_memory();
        let available = sys.available_memory();

        // Calculate cached (total - used - available gives us cached/buffers)
        let cached = if total > used + available {
            total - used - available
        } else {
            0
        };

        let usage_percent = if total > 0 {
            (used as f32 / total as f32) * 100.0
        } else {
            0.0
        };

        MemoryStats {
            total_bytes: total,
            used_bytes: used,
            available_bytes: available,
            cached_bytes: cached,
            swap_total_bytes: sys.total_swap(),
            swap_used_bytes: sys.used_swap(),
            usage_percent,
        }
    }).await.unwrap();

    Ok(stats)
}

/// Get disk statistics
#[tauri::command]
pub async fn get_disk_stats() -> Result<Vec<DiskStats>> {
    let stats = tokio::task::spawn_blocking(|| {
        let disks = Disks::new_with_refreshed_list();

        disks
            .iter()
            .filter(|disk| {
                // Filter out small/virtual filesystems
                let mount = disk.mount_point().to_string_lossy();
                !mount.starts_with("/snap")
                    && !mount.starts_with("/sys")
                    && !mount.starts_with("/proc")
                    && !mount.starts_with("/run")
                    && !mount.starts_with("/dev")
            })
            .map(|disk| {
                let total = disk.total_space();
                let available = disk.available_space();
                let used = total.saturating_sub(available);

                DiskStats {
                    name: disk.name().to_string_lossy().to_string(),
                    mount_point: disk.mount_point().to_string_lossy().to_string(),
                    filesystem: disk.file_system().to_string_lossy().to_string(),
                    total_bytes: total,
                    used_bytes: used,
                    available_bytes: available,
                    usage_percent: if total > 0 {
                        (used as f32 / total as f32) * 100.0
                    } else {
                        0.0
                    },
                    is_removable: disk.is_removable(),
                }
            })
            .collect()
    }).await.unwrap();

    Ok(stats)
}

/// Get network interface statistics
#[tauri::command]
pub async fn get_network_stats() -> Result<Vec<NetworkStats>> {
    let stats = tokio::task::spawn_blocking(|| {
        let networks = Networks::new_with_refreshed_list();

        networks
            .iter()
            .filter(|(name, _)| {
                // Filter out virtual interfaces
                !name.starts_with("lo")
                    && !name.starts_with("docker")
                    && !name.starts_with("veth")
                    && !name.starts_with("br-")
            })
            .map(|(name, data)| NetworkStats {
                interface: name.clone(),
                rx_bytes: data.total_received(),
                tx_bytes: data.total_transmitted(),
                rx_packets: data.total_packets_received(),
                tx_packets: data.total_packets_transmitted(),
            })
            .collect()
    }).await.unwrap();

    Ok(stats)
}
