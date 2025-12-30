//! Process management module
//! Uses native sysinfo crate for process listing with categorization (async)

use crate::error::{AppError, Result};
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use sysinfo::{ProcessStatus, ProcessesToUpdate, Signal};
use tauri::State;

// ============================================================================
// Data Structures
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cpu_usage: f32,
    pub memory_bytes: u64,
    pub status: String,
    pub user: String,
    pub command: String,
    pub category: String,
    pub is_killable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessAction {
    pub pid: u32,
    pub action: String,
    pub success: bool,
    pub message: String,
}

// Category detection patterns
const SYSTEM_PROCESSES: &[&str] = &[
    "systemd", "init", "kthreadd", "kworker", "ksoftirqd", "migration",
    "rcu_", "watchdog", "cpuhp", "idle_inject", "netns", "xfsaio",
    "scsi", "ata_", "md_", "raid", "dm-", "loop", "nfsd", "blk",
    "irq/", "devfreq", "mmc", "ext4", "jbd2", "xfs", "btrfs",
    "dbus", "polkit", "udev", "cron", "rsyslog", "journal",
    "snap", "apparmor", "audit", "acpi", "thermal"
];

const DESKTOP_PROCESSES: &[&str] = &[
    "gnome", "gdm", "kde", "plasma", "xorg", "wayland", "mutter",
    "kwin", "nautilus", "dolphin", "tracker", "gvfs", "pipewire",
    "pulseaudio", "wireplumber", "dconf", "gsd-", "xdg-", "at-spi"
];

const BROWSER_PROCESSES: &[&str] = &[
    "firefox", "chrome", "chromium", "brave", "opera", "vivaldi",
    "edge", "webkit", "electron"
];

const MEDIA_PROCESSES: &[&str] = &[
    "vlc", "mpv", "totem", "spotify", "rhythmbox", "audacity",
    "obs", "kdenlive", "gimp", "inkscape", "blender", "steam"
];

/// Detect process category from name and command
fn detect_process_category(name: &str, command: &str, user: &str) -> (String, bool) {
    let check = |patterns: &[&str]| {
        patterns.iter().any(|p| {
            name.to_lowercase().contains(*p) || command.to_lowercase().contains(*p)
        })
    };
    
    // System processes run as root and have kernel-like names
    let is_system = user == "0" || user == "root" || 
        name.starts_with("[") || // Kernel threads
        check(SYSTEM_PROCESSES);
    
    if name.starts_with("[") {
        ("Kernel".to_string(), false)
    } else if check(SYSTEM_PROCESSES) {
        ("System".to_string(), false)
    } else if check(DESKTOP_PROCESSES) {
        ("Desktop".to_string(), true)
    } else if check(BROWSER_PROCESSES) {
        ("Browser".to_string(), true)
    } else if check(MEDIA_PROCESSES) {
        ("Media".to_string(), true)
    } else if is_system {
        ("System".to_string(), false)
    } else {
        ("Apps".to_string(), true)
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

fn status_to_string(status: ProcessStatus) -> String {
    match status {
        ProcessStatus::Run => "Running".to_string(),
        ProcessStatus::Sleep => "Sleeping".to_string(),
        ProcessStatus::Stop => "Stopped".to_string(),
        ProcessStatus::Zombie => "Zombie".to_string(),
        ProcessStatus::Idle => "Idle".to_string(),
        _ => "Unknown".to_string(),
    }
}

// ============================================================================
// Tauri Commands (All async)
// ============================================================================

/// Get all running processes (async)
#[tauri::command]
pub async fn get_processes(state: State<'_, AppState>) -> Result<Vec<ProcessInfo>> {
    let sys = state.sys.clone();
    
    let processes = tokio::task::spawn_blocking(move || {
        let mut sys = sys.lock().unwrap();

        // Refresh process list
        sys.refresh_processes(ProcessesToUpdate::All, true);

        let mut processes: Vec<ProcessInfo> = sys
            .processes()
            .iter()
            .map(|(pid, process)| {
                let name = process.name().to_string_lossy().to_string();
                let command = process.cmd().iter().map(|s| s.to_string_lossy().to_string()).collect::<Vec<_>>().join(" ");
                let user = process
                    .user_id()
                    .map(|uid| uid.to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                
                let (category, is_killable) = detect_process_category(&name, &command, &user);
                
                ProcessInfo {
                    pid: pid.as_u32(),
                    name,
                    cpu_usage: process.cpu_usage(),
                    memory_bytes: process.memory(),
                    status: status_to_string(process.status()),
                    user,
                    command,
                    category,
                    is_killable,
                }
            })
            .collect();

        // Sort by CPU usage (descending)
        processes.sort_by(|a, b| b.cpu_usage.partial_cmp(&a.cpu_usage).unwrap_or(std::cmp::Ordering::Equal));

        processes
    }).await.unwrap();

    Ok(processes)
}

/// Get top processes by CPU usage (async)
#[tauri::command]
pub async fn get_top_processes(state: State<'_, AppState>, limit: usize) -> Result<Vec<ProcessInfo>> {
    let mut processes = get_processes(state).await?;
    processes.truncate(limit);
    Ok(processes)
}

/// Search processes by name (async)
#[tauri::command]
pub async fn search_processes(state: State<'_, AppState>, query: String) -> Result<Vec<ProcessInfo>> {
    let all_processes = get_processes(state).await?;
    let query_lower = query.to_lowercase();

    let filtered: Vec<ProcessInfo> = all_processes
        .into_iter()
        .filter(|p| {
            p.name.to_lowercase().contains(&query_lower)
                || p.command.to_lowercase().contains(&query_lower)
        })
        .collect();

    Ok(filtered)
}

/// Kill a process by PID (async)
#[tauri::command]
pub async fn kill_process(state: State<'_, AppState>, pid: u32) -> Result<ProcessAction> {
    let sys = state.sys.clone();
    
    let result = tokio::task::spawn_blocking(move || {
        let sys = sys.lock().unwrap();
        let pid_obj = sysinfo::Pid::from_u32(pid);

        if let Some(process) = sys.process(pid_obj) {
            // Try SIGTERM first
            if process.kill_with(Signal::Term).is_some() {
                return Ok(ProcessAction {
                    pid,
                    action: "kill".to_string(),
                    success: true,
                    message: "Process terminated".to_string(),
                });
            }
        }

        // Process doesn't exist or couldn't be killed
        Err(AppError::CommandFailed(format!(
            "Failed to kill process {}",
            pid
        )))
    }).await.unwrap();

    result
}

/// Force kill a process by PID (SIGKILL) - async
#[tauri::command]
pub async fn force_kill_process(state: State<'_, AppState>, pid: u32) -> Result<ProcessAction> {
    let sys = state.sys.clone();
    
    let result = tokio::task::spawn_blocking(move || {
        let sys = sys.lock().unwrap();
        let pid_obj = sysinfo::Pid::from_u32(pid);

        if let Some(process) = sys.process(pid_obj) {
            if process.kill_with(Signal::Kill).is_some() {
                return Ok(ProcessAction {
                    pid,
                    action: "force_kill".to_string(),
                    success: true,
                    message: "Process killed".to_string(),
                });
            }
        }

        Err(AppError::CommandFailed(format!(
            "Failed to force kill process {}",
            pid
        )))
    }).await.unwrap();

    result
}

/// Get process count (async)
#[tauri::command]
pub async fn get_process_count(state: State<'_, AppState>) -> Result<usize> {
    let sys = state.sys.clone();
    
    let count = tokio::task::spawn_blocking(move || {
        let sys = sys.lock().unwrap();
        sys.processes().len()
    }).await.unwrap();

    Ok(count)
}

/// Bulk terminate all killable app processes to free RAM (async)
#[tauri::command]
pub async fn bulk_terminate_apps(state: State<'_, AppState>) -> Result<ProcessAction> {
    let sys = state.sys.clone();
    
    let result = tokio::task::spawn_blocking(move || {
        let mut sys = sys.lock().unwrap();
        sys.refresh_processes(ProcessesToUpdate::All, true);
        
        let mut killed_count = 0;
        let mut failed_count = 0;
        let mut total_memory_freed: u64 = 0;
        
        // Get all killable processes
        let killable_pids: Vec<(sysinfo::Pid, u64)> = sys
            .processes()
            .iter()
            .filter_map(|(pid, process)| {
                let name = process.name().to_string_lossy().to_string();
                let command = process.cmd().iter().map(|s| s.to_string_lossy().to_string()).collect::<Vec<_>>().join(" ");
                let user = process.user_id().map(|uid| uid.to_string()).unwrap_or_else(|| "unknown".to_string());
                
                let (category, is_killable) = detect_process_category(&name, &command, &user);
                
                // Only kill Apps, Browser, Media categories (not System, Kernel, Desktop)
                if is_killable && (category == "Apps" || category == "Browser" || category == "Media") {
                    Some((*pid, process.memory()))
                } else {
                    None
                }
            })
            .collect();
        
        // Kill each process
        for (pid, memory) in killable_pids {
            if let Some(process) = sys.process(pid) {
                if process.kill_with(Signal::Term).is_some() {
                    killed_count += 1;
                    total_memory_freed += memory;
                } else {
                    failed_count += 1;
                }
            }
        }
        
        let freed_mb = total_memory_freed / (1024 * 1024);
        
        ProcessAction {
            pid: 0,
            action: "bulk_terminate".to_string(),
            success: killed_count > 0,
            message: format!(
                "Terminated {} app processes (~{} MB RAM freed). {} failed.",
                killed_count, freed_mb, failed_count
            ),
        }
    }).await.unwrap();

    Ok(result)
}
