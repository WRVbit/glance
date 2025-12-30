//! Package management module
//! Lists and uninstalls packages with categorization (async)
//! Now uses distro-agnostic PackageManager trait

use crate::adapters::{PackageInfo, PackageAction};
use crate::error::{AppError, Result};
use crate::state::AppState;
use tauri::State;

// Re-export types for frontend
pub use crate::adapters::PackageInfo as PackageInfoExport;
pub use crate::adapters::PackageAction as PackageActionExport;

// ============================================================================
// Tauri Commands (All async, use PackageManager adapter)
// ============================================================================

/// Get all installed packages (async, distro-agnostic)
#[tauri::command]
pub async fn get_packages(state: State<'_, AppState>) -> Result<Vec<PackageInfo>> {
    state.context.package_manager.get_installed_packages().await
}

/// Search packages by name (async)
#[tauri::command]
pub async fn search_packages(query: String, state: State<'_, AppState>) -> Result<Vec<PackageInfo>> {
    state.context.package_manager.search_packages(&query).await
}

/// Uninstall a package (requires auth, async with timeout)
#[tauri::command]
pub async fn uninstall_package(name: String, state: State<'_, AppState>) -> Result<PackageAction> {
    // Validate package name (prevent injection)
    if !name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '.' || c == '+' || c == ':' || c == '_') {
        return Err(AppError::PermissionDenied(
            "Invalid package name".to_string(),
        ));
    }

    state.context.package_manager.uninstall_package(&name).await
}

/// Purge a package (remove with config files, async with timeout)
#[tauri::command]
pub async fn purge_package(name: String, state: State<'_, AppState>) -> Result<PackageAction> {
    // Validate package name
    if !name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '.' || c == '+' || c == ':' || c == '_') {
        return Err(AppError::PermissionDenied(
            "Invalid package name".to_string(),
        ));
    }

    state.context.package_manager.purge_package(&name).await
}

/// Remove unused dependencies (async with timeout)
#[tauri::command]
pub async fn autoremove_packages(state: State<'_, AppState>) -> Result<PackageAction> {
    state.context.package_manager.autoremove().await
}

/// Get package count statistics (async)
#[tauri::command]
pub async fn get_package_stats(state: State<'_, AppState>) -> Result<(usize, usize, u64)> {
    state.context.package_manager.get_stats().await
}

/// Get package manager name for this distro
#[tauri::command]
pub fn get_package_manager_name(state: State<'_, AppState>) -> String {
    state.context.package_manager.name().to_string()
}
