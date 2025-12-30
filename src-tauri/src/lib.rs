//! Linux Optimizer - Main library
//! A modern Linux system optimizer inspired by Stacer

mod error;
mod modules;
mod state;
mod utils;

use modules::{cleaner, hosts, packages, processes, repositories, resources, services, startup, system_stats, tweaks};
use state::AppState;
use utils::distro::DistroInfo;

/// Get distribution information
#[tauri::command]
fn get_distro_info() -> Result<DistroInfo, error::AppError> {
    DistroInfo::detect()
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
            // System Stats
            system_stats::get_system_info,
            system_stats::get_cpu_stats,
            system_stats::get_memory_stats,
            system_stats::get_disk_stats,
            system_stats::get_network_stats,
            // Cleaner
            cleaner::get_cleanup_categories,
            cleaner::clean_category,
            cleaner::get_total_reclaimable,
            // Tweaks
            tweaks::get_tweaks,
            tweaks::apply_tweak,
            tweaks::apply_all_recommended,
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
            // Repositories (NEW)
            repositories::get_repositories,
            repositories::toggle_repository,
            repositories::add_ppa,
            repositories::remove_ppa,
            repositories::get_mirrors,
            repositories::test_mirror_speed,
            repositories::test_all_mirrors,
            repositories::set_mirror,
            repositories::apt_update,
            // Resources (NEW)
            resources::get_resource_snapshot,
            resources::get_resource_history,
            resources::add_resource_snapshot,
            resources::clear_resource_history,
            resources::get_per_core_usage,
            // Hosts (NEW)
            hosts::get_hosts,
            hosts::get_hosts_stats,
            hosts::add_host,
            hosts::remove_host,
            hosts::toggle_host,
            hosts::get_blocklists,
            hosts::import_blocklist,
            hosts::backup_hosts,
            hosts::list_hosts_backups,
            hosts::restore_hosts,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

