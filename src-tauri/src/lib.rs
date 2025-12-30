//! Linux Optimizer - Main library
//! A modern Linux system optimizer inspired by Stacer
//! Supports: Debian/Ubuntu, Arch, Fedora, OpenSUSE

pub mod adapters;
mod error;
mod modules;
mod state;
mod utils;

use modules::{cleaner, dns, hosts, packages, processes, repositories, resources, services, startup, system_stats, tweaks};
use state::AppState;
use utils::distro::DistroInfo;
use utils::{DistroFamily, DesktopEnvironment};
use tauri::State;

/// Get distribution information
#[tauri::command]
fn get_distro_info() -> Result<DistroInfo, error::AppError> {
    DistroInfo::detect()
}

/// Get distro family name for UI display
#[tauri::command]
fn get_distro_family(state: State<'_, AppState>) -> String {
    state.context.family.display_name().to_string()
}

/// Get package manager name for UI display
#[tauri::command]
fn get_pm_name(state: State<'_, AppState>) -> String {
    state.context.package_manager.name().to_string()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Plugins
        .plugin(tauri_plugin_shell::init())
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .build(),
        )
        // Shared state
        .manage(AppState::new())
        .manage(resources::ResourceHistoryState::new())
        // Register all commands
        .invoke_handler(tauri::generate_handler![
            // Distro
            get_distro_info,
            get_distro_family,
            get_pm_name,
            packages::get_package_manager_name,
            // System Stats
            system_stats::get_system_info,
            system_stats::get_cpu_stats,
            system_stats::get_memory_stats,
            system_stats::get_disk_stats,
            system_stats::get_network_stats,
            // Cleaner
            cleaner::get_cleanup_categories,
            cleaner::preview_cleanup,
            cleaner::clean_category,
            cleaner::get_total_reclaimable,
            cleaner::get_autoclean_schedule,
            cleaner::set_autoclean_schedule,
            cleaner::get_autoclean_status,
            cleaner::run_autoclean_now,
            // Tweaks
            tweaks::get_tweaks,
            tweaks::apply_tweak,
            tweaks::apply_all_recommended,
            tweaks::get_device_info,
            // Services
            services::get_services,
            services::start_service,
            services::stop_service,
            services::restart_service,
            services::enable_service,
            services::disable_service,
            services::search_services,
            // Startup
            startup::get_startup_apps,
            startup::enable_startup_app,
            startup::disable_startup_app,
            startup::remove_startup_app,
            startup::add_startup_app,
            // Packages
            packages::get_packages,
            packages::search_packages,
            packages::uninstall_package,
            packages::purge_package,
            packages::autoremove_packages,
            packages::get_package_stats,
            // Processes
            processes::get_processes,
            processes::get_top_processes,
            processes::search_processes,
            processes::kill_process,
            processes::force_kill_process,
            processes::get_process_count,
            processes::bulk_terminate_apps,
            // Repositories (Enhanced)
            repositories::is_repositories_available,
            repositories::get_repositories,
            repositories::toggle_repository,
            repositories::delete_repository,
            repositories::add_ppa,
            repositories::remove_ppa,
            repositories::get_region_info,
            repositories::get_mirrors,
            repositories::test_mirror_speed,
            repositories::test_all_mirrors,
            repositories::set_mirror,
            repositories::apt_update,
            // apt-fast
            repositories::check_apt_fast,
            repositories::install_apt_fast,
            repositories::configure_apt_fast,
            // Resources (Enhanced)
            resources::get_resource_snapshot,
            resources::get_resource_history,
            resources::add_resource_snapshot,
            resources::clear_resource_history,
            resources::get_per_core_usage,
            resources::get_gpu_info,
            resources::get_disk_io_stats,
            // Ad-Block Manager (formerly Hosts)
            hosts::get_blocklist_sources,
            hosts::get_adblock_stats,
            hosts::apply_blocklists,
            hosts::clear_blocklists,
            hosts::backup_hosts,
            hosts::list_hosts_backups,
            hosts::restore_hosts,
            // DNS Manager
            dns::get_dns_providers,
            dns::get_current_dns,
            dns::set_dns_provider,
            dns::set_custom_dns,
            dns::reset_dns,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

