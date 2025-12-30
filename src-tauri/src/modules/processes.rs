//! Process management module
//! Uses native sysinfo crate for process listing (async)

use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use sysinfo::{ProcessStatus, ProcessesToUpdate, System, Signal};

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessAction {
    pub pid: u32,
    pub action: String,
    pub success: bool,
    pub message: String,
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
pub async fn get_processes() -> Result<Vec<ProcessInfo>> {
    // Spawn blocking since sysinfo does system calls
    let processes = tokio::task::spawn_blocking(|| {
        let mut sys = System::new();

        // Refresh process list
        sys.refresh_processes(ProcessesToUpdate::All, true);

        let mut processes: Vec<ProcessInfo> = sys
            .processes()
            .iter()
            .map(|(pid, process)| {
                ProcessInfo {
                    pid: pid.as_u32(),
                    name: process.name().to_string_lossy().to_string(),
                    cpu_usage: process.cpu_usage(),
                    memory_bytes: process.memory(),
                    status: status_to_string(process.status()),
                    user: process
                        .user_id()
                        .map(|uid| uid.to_string())
                        .unwrap_or_else(|| "unknown".to_string()),
                    command: process.cmd().iter().map(|s| s.to_string_lossy().to_string()).collect::<Vec<_>>().join(" "),
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
pub async fn get_top_processes(limit: usize) -> Result<Vec<ProcessInfo>> {
    let mut processes = get_processes().await?;
    processes.truncate(limit);
    Ok(processes)
}

/// Search processes by name (async)
#[tauri::command]
pub async fn search_processes(query: String) -> Result<Vec<ProcessInfo>> {
    let all_processes = get_processes().await?;
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
pub async fn kill_process(pid: u32) -> Result<ProcessAction> {
    let result = tokio::task::spawn_blocking(move || {
        let sys = System::new_all();
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
pub async fn force_kill_process(pid: u32) -> Result<ProcessAction> {
    let result = tokio::task::spawn_blocking(move || {
        let sys = System::new_all();
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
pub async fn get_process_count() -> Result<usize> {
    let count = tokio::task::spawn_blocking(|| {
        let sys = System::new_all();
        sys.processes().len()
    }).await.unwrap();

    Ok(count)
}
