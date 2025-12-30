//! Resource Monitor module
//! Collects and stores resource usage history for graphing (Non-blocking)

use crate::error::Result;
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Mutex;
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, Networks};
use tauri::State;

// ============================================================================
// Data Structures
// ============================================================================

const HISTORY_SIZE: usize = 60; // 60 seconds of history

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResourceSnapshot {
    pub timestamp: u64,      // Unix timestamp in seconds
    pub cpu_percent: f32,
    pub ram_used_bytes: u64,
    pub ram_total_bytes: u64,
    pub net_rx_bytes: u64,   // Total received
    pub net_tx_bytes: u64,   // Total transmitted
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceHistory {
    pub snapshots: Vec<ResourceSnapshot>,
    pub net_rx_speed: Vec<u64>,  // Bytes per second
    pub net_tx_speed: Vec<u64>,  // Bytes per second
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
// Tauri Commands (All non-blocking)
// ============================================================================

/// Get current resource snapshot - NO SLEEP, returns immediately
/// CPU usage accuracy depends on frontend polling interval
#[tauri::command]
pub async fn get_resource_snapshot(app_state: State<'_, AppState>) -> Result<ResourceSnapshot> {
    // Use tokio::task::spawn_blocking for mutex lock since it might block briefly
    let snapshot = tokio::task::spawn_blocking(move || {
        let mut sys = app_state.sys.lock().unwrap();
        
        // Refresh CPU - usage will be calculated based on delta from last refresh
        sys.refresh_cpu_specifics(CpuRefreshKind::nothing().with_cpu_usage());
        sys.refresh_memory_specifics(MemoryRefreshKind::everything());
        
        // Calculate CPU average
        let cpu_percent: f32 = if sys.cpus().is_empty() {
            0.0
        } else {
            sys.cpus().iter().map(|cpu| cpu.cpu_usage()).sum::<f32>() / sys.cpus().len() as f32
        };
        
        // Get network stats
        let networks = Networks::new_with_refreshed_list();
        let (net_rx, net_tx) = networks
            .iter()
            .filter(|(name, _)| !name.starts_with("lo") && !name.starts_with("docker"))
            .fold((0u64, 0u64), |(rx, tx), (_, data)| {
                (rx + data.total_received(), tx + data.total_transmitted())
            });
        
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        
        ResourceSnapshot {
            timestamp,
            cpu_percent,
            ram_used_bytes: sys.used_memory(),
            ram_total_bytes: sys.total_memory(),
            net_rx_bytes: net_rx,
            net_tx_bytes: net_tx,
        }
    }).await.unwrap();
    
    Ok(snapshot)
}

/// Get resource history with calculated network speeds
#[tauri::command]
pub fn get_resource_history(history_state: State<ResourceHistoryState>) -> Result<ResourceHistory> {
    let history = history_state.history.lock().unwrap();
    
    let snapshots: Vec<ResourceSnapshot> = history.iter().cloned().collect();
    
    // Calculate network speeds (delta between snapshots)
    let mut net_rx_speed = Vec::new();
    let mut net_tx_speed = Vec::new();
    
    for i in 1..snapshots.len() {
        let prev = &snapshots[i - 1];
        let curr = &snapshots[i];
        let time_delta = curr.timestamp.saturating_sub(prev.timestamp).max(1);
        
        net_rx_speed.push(curr.net_rx_bytes.saturating_sub(prev.net_rx_bytes) / time_delta);
        net_tx_speed.push(curr.net_tx_bytes.saturating_sub(prev.net_tx_bytes) / time_delta);
    }
    
    // Add a 0 at the start to match snapshot count
    if !snapshots.is_empty() {
        net_rx_speed.insert(0, 0);
        net_tx_speed.insert(0, 0);
    }
    
    Ok(ResourceHistory {
        snapshots,
        net_rx_speed,
        net_tx_speed,
    })
}

/// Add a snapshot to history (called from frontend every second)
#[tauri::command]
pub fn add_resource_snapshot(
    snapshot: ResourceSnapshot,
    history_state: State<ResourceHistoryState>,
) -> Result<()> {
    let mut history = history_state.history.lock().unwrap();
    
    // Remove oldest if at capacity
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

/// Get per-core CPU usage for detailed view - NO SLEEP
#[tauri::command]
pub async fn get_per_core_usage(app_state: State<'_, AppState>) -> Result<Vec<f32>> {
    let per_core = tokio::task::spawn_blocking(move || {
        let mut sys = app_state.sys.lock().unwrap();
        sys.refresh_cpu_specifics(CpuRefreshKind::nothing().with_cpu_usage());
        sys.cpus().iter().map(|cpu| cpu.cpu_usage()).collect::<Vec<f32>>()
    }).await.unwrap();
    
    Ok(per_core)
}
