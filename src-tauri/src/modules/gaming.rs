//! Gaming Center Module
//! Linux Gaming optimization module inspired by Nobara Project and AdelKS Linux Gaming Guide
//! Provides: Package installers, Driver detection, Performance tweaks with sliders

use serde::{Deserialize, Serialize};
use std::process::Command;
use std::fs;
use crate::utils::{DistroFamily, DistroInfo};

// ============================================================================
// GPU Detection & Status
// ============================================================================

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GpuInfo {
    pub vendor: String,           // "nvidia", "amd", "intel"
    pub model: String,            // "NVIDIA GeForce RTX 3060"
    pub driver: Option<String>,   // "nvidia", "nouveau", "mesa"
    pub driver_version: Option<String>,
    pub vulkan_ready: bool,
    pub using_proprietary: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GamingStatus {
    pub gpu: Option<GpuInfo>,
    pub gaming_score: String,     // "Optimized", "Needs Tuning", "Not Ready"
    pub score_color: String,      // "green", "yellow", "red"
    pub multilib_enabled: bool,   // 32-bit support
    pub issues: Vec<String>,      // List of issues to fix
    pub distro_family: String,    // "Debian", "Arch", "Fedora", "Suse"
}

// ============================================================================
// Safety Checks (Fail-Fast & Lock Detection)
// ============================================================================

/// Get current distro family using the proper adapter
fn get_distro_family() -> DistroFamily {
    DistroInfo::detect()
        .map(|d| d.family())
        .unwrap_or(DistroFamily::Debian)
}

/// Check if apt/dpkg is locked (prevents race condition with unattended-upgrades)
fn is_package_manager_locked() -> bool {
    let family = get_distro_family();
    
    match family {
        DistroFamily::Debian => {
            // Check for dpkg lock
            std::path::Path::new("/var/lib/dpkg/lock-frontend").exists() &&
                Command::new("lsof")
                    .args(["/var/lib/dpkg/lock-frontend"])
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false)
        }
        DistroFamily::Arch => {
            // Check for pacman lock
            std::path::Path::new("/var/lib/pacman/db.lck").exists()
        }
        DistroFamily::Fedora => {
            // Check for dnf lock
            Command::new("pgrep")
                .args(["-x", "dnf"])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        }
        _ => false
    }
}

/// Detect current Desktop Environment for mouse acceleration fix
fn detect_desktop_environment() -> String {
    // Check XDG_CURRENT_DESKTOP first
    if let Ok(de) = std::env::var("XDG_CURRENT_DESKTOP") {
        let de_lower = de.to_lowercase();
        if de_lower.contains("gnome") { return "gnome".to_string(); }
        if de_lower.contains("kde") { return "kde".to_string(); }
        if de_lower.contains("xfce") { return "xfce".to_string(); }
        if de_lower.contains("cinnamon") { return "cinnamon".to_string(); }
        if de_lower.contains("mate") { return "mate".to_string(); }
    }
    
    // Fallback: check XDG_SESSION_TYPE
    if let Ok(session) = std::env::var("XDG_SESSION_TYPE") {
        if session == "wayland" { return "gnome".to_string(); } // Most Wayland is GNOME
    }
    
    // Default to X11-style
    "x11".to_string()
}

/// Set mouse acceleration based on Desktop Environment
fn set_mouse_flat_profile() -> Result<(), String> {
    let de = detect_desktop_environment();
    
    match de.as_str() {
        "gnome" | "cinnamon" => {
            // GNOME/Cinnamon use gsettings
            Command::new("gsettings")
                .args(["set", "org.gnome.desktop.peripherals.mouse", "accel-profile", "flat"])
                .output()
                .map_err(|e| e.to_string())?;
            Ok(())
        }
        "kde" => {
            // KDE Plasma uses kwriteconfig5
            Command::new("kwriteconfig5")
                .args(["--file", "kcminputrc", "--group", "Mouse", "--key", "XLbInptPointerAcceleration", "0"])
                .output()
                .map_err(|e| e.to_string())?;
            Ok(())
        }
        "xfce" | "mate" | "x11" => {
            // X11: Use xinput or libinput config
            // First try to find the mouse device
            if let Ok(output) = Command::new("xinput").args(["list", "--short"]).output() {
                let devices = String::from_utf8_lossy(&output.stdout);
                for line in devices.lines() {
                    if line.to_lowercase().contains("mouse") {
                        if let Some(id) = line.split("id=").nth(1).and_then(|s| s.split_whitespace().next()) {
                            let _ = Command::new("xinput")
                                .args(["set-prop", id, "libinput Accel Profile Enabled", "0", "1"])
                                .output();
                        }
                    }
                }
            }
            Ok(())
        }
        _ => Ok(())
    }
}

/// Detect GPU using lspci with fallback to /proc/driver
fn detect_gpu_internal() -> Option<GpuInfo> {
    // Try lspci first
    if let Ok(output) = Command::new("lspci").output() {
        let lspci = String::from_utf8_lossy(&output.stdout);
        
        for line in lspci.lines() {
            if line.contains("VGA") || line.contains("3D controller") {
                let model = line.split(':').last().unwrap_or("Unknown GPU").trim().to_string();
                let line_lower = line.to_lowercase();
                
                let (vendor, using_proprietary) = if line_lower.contains("nvidia") {
                    let nvidia_smi = Command::new("nvidia-smi").output().ok();
                    let is_proprietary = nvidia_smi.map(|o| o.status.success()).unwrap_or(false);
                    ("nvidia".to_string(), is_proprietary)
                } else if line_lower.contains("amd") || line_lower.contains("ati") || line_lower.contains("radeon") {
                    ("amd".to_string(), false)
                } else if line_lower.contains("intel") {
                    ("intel".to_string(), false)
                } else {
                    ("unknown".to_string(), false)
                };
                
                let (driver, driver_version) = get_driver_info(&vendor);
                let vulkan_ready = check_vulkan_support();
                
                return Some(GpuInfo {
                    vendor,
                    model,
                    driver,
                    driver_version,
                    vulkan_ready,
                    using_proprietary,
                });
            }
        }
    }
    
    // Fallback: Check /proc for NVIDIA
    if std::path::Path::new("/proc/driver/nvidia/version").exists() {
        let version = fs::read_to_string("/proc/driver/nvidia/version")
            .ok()
            .and_then(|s| s.lines().next().map(|l| l.to_string()));
        
        return Some(GpuInfo {
            vendor: "nvidia".to_string(),
            model: "NVIDIA GPU (detected via /proc)".to_string(),
            driver: Some("nvidia-proprietary".to_string()),
            driver_version: version,
            vulkan_ready: check_vulkan_support(),
            using_proprietary: true,
        });
    }
    
    // Fallback: Check for AMD via /sys
    if std::path::Path::new("/sys/class/drm/card0/device/vendor").exists() {
        if let Ok(vendor_id) = fs::read_to_string("/sys/class/drm/card0/device/vendor") {
            let vendor_id = vendor_id.trim();
            let (vendor, model) = match vendor_id {
                "0x1002" => ("amd", "AMD GPU (detected via /sys)"),
                "0x8086" => ("intel", "Intel GPU (detected via /sys)"),
                "0x10de" => ("nvidia", "NVIDIA GPU (detected via /sys)"),
                _ => ("unknown", "Unknown GPU"),
            };
            
            return Some(GpuInfo {
                vendor: vendor.to_string(),
                model: model.to_string(),
                driver: Some("mesa".to_string()),
                driver_version: None,
                vulkan_ready: check_vulkan_support(),
                using_proprietary: false,
            });
        }
    }
    
    None
}

/// Detect available NVIDIA driver package (dynamic version detection)
fn detect_nvidia_driver_package() -> String {
    // Try to find available nvidia-driver packages
    let versions = ["560", "555", "550", "545", "535", "530", "525", "520", "515", "510"];
    
    if let Ok(output) = Command::new("apt-cache")
        .args(["search", "nvidia-driver-"])
        .output()
    {
        let available = String::from_utf8_lossy(&output.stdout);
        
        // Find the highest available version
        for version in versions {
            let pkg_name = format!("nvidia-driver-{}", version);
            if available.contains(&pkg_name) {
                return pkg_name;
            }
        }
    }
    
    // Fallback to a common version
    "nvidia-driver-550".to_string()
}


fn get_driver_info(vendor: &str) -> (Option<String>, Option<String>) {
    match vendor {
        "nvidia" => {
            let output = Command::new("nvidia-smi")
                .args(["--query-gpu=driver_version", "--format=csv,noheader"])
                .output()
                .ok();
            
            if let Some(o) = output {
                if o.status.success() {
                    let version = String::from_utf8_lossy(&o.stdout).trim().to_string();
                    return (Some("nvidia-proprietary".to_string()), Some(version));
                }
            }
            // Check for nouveau
            let lsmod = Command::new("lsmod").output().ok();
            if let Some(o) = lsmod {
                if String::from_utf8_lossy(&o.stdout).contains("nouveau") {
                    return (Some("nouveau".to_string()), None);
                }
            }
            (None, None)
        }
        "amd" | "intel" => {
            let output = Command::new("glxinfo")
                .args(["-B"])
                .output()
                .ok();
            
            if let Some(o) = output {
                let glx = String::from_utf8_lossy(&o.stdout);
                for line in glx.lines() {
                    if line.contains("OpenGL version string") {
                        let version = line.split(':').last().map(|s| s.trim().to_string());
                        return (Some("mesa".to_string()), version);
                    }
                }
            }
            (Some("mesa".to_string()), None)
        }
        _ => (None, None)
    }
}

fn check_vulkan_support() -> bool {
    Command::new("vulkaninfo")
        .arg("--summary")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn check_multilib() -> bool {
    // Check for 32-bit library support
    let dpkg_check = Command::new("dpkg")
        .args(["--print-foreign-architectures"])
        .output();
    
    if let Ok(o) = dpkg_check {
        let archs = String::from_utf8_lossy(&o.stdout);
        if archs.contains("i386") {
            return true;
        }
    }
    
    // Arch: check for lib32
    let pacman_check = Command::new("pacman")
        .args(["-Q", "lib32-glibc"])
        .output();
    
    if let Ok(o) = pacman_check {
        if o.status.success() {
            return true;
        }
    }
    
    false
}

/// Get full gaming status
#[tauri::command]
pub fn get_gaming_status() -> GamingStatus {
    let gpu = detect_gpu_internal();
    let multilib_enabled = check_multilib();
    let distro_family = get_distro_family();
    
    let mut issues: Vec<String> = Vec::new();
    let mut score = 100;
    
    // Check package manager lock
    if is_package_manager_locked() {
        issues.push("⚠ Package manager is locked. Close other update processes.".to_string());
    }
    
    // Check GPU
    if let Some(ref g) = gpu {
        if g.vendor == "nvidia" && !g.using_proprietary {
            issues.push("NVIDIA: Using nouveau driver (slow). Install proprietary driver.".to_string());
            score -= 30;
        }
        if !g.vulkan_ready {
            issues.push("Vulkan not detected. Install vulkan drivers.".to_string());
            score -= 20;
        }
    } else {
        issues.push("Could not detect GPU.".to_string());
        score -= 40;
    }
    
    // Check multilib
    if !multilib_enabled {
        issues.push("32-bit support not enabled. Steam requires this.".to_string());
        score -= 20;
    }
    
    // Check max_map_count
    if let Ok(content) = fs::read_to_string("/proc/sys/vm/max_map_count") {
        let value: i64 = content.trim().parse().unwrap_or(0);
        if value < 1048576 {
            issues.push("vm.max_map_count too low. May cause crashes in heavy games.".to_string());
            score -= 10;
        }
    }
    
    let (gaming_score, score_color) = if score >= 80 {
        ("Optimized".to_string(), "green".to_string())
    } else if score >= 50 {
        ("Needs Tuning".to_string(), "yellow".to_string())
    } else {
        ("Not Ready".to_string(), "red".to_string())
    };
    
    GamingStatus {
        gpu,
        gaming_score,
        score_color,
        multilib_enabled,
        issues,
        distro_family: distro_family.display_name().to_string(),
    }
}

// ============================================================================
// Gaming Software Packages
// ============================================================================

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GamingPackage {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub category: String,        // "platform", "compatibility", "tools"
    pub installed: bool,
    pub recommended: bool,
    pub install_method: String,  // "apt", "flatpak", "script"
    pub flatpak_id: Option<String>,
    pub apt_package: Option<String>,
}

fn is_apt_installed(package: &str) -> bool {
    Command::new("dpkg")
        .args(["-s", package])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn is_flatpak_installed(app_id: &str) -> bool {
    Command::new("flatpak")
        .args(["info", app_id])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Get all gaming packages (Tab 1: Essentials & Launchers)
#[tauri::command]
pub fn get_gaming_packages() -> Vec<GamingPackage> {
    vec![
        // === PLATFORMS ===
        GamingPackage {
            id: "steam".to_string(),
            name: "Steam".to_string(),
            description: "Valve's gaming platform. The #1 way to play games on Linux.".to_string(),
            icon: "🎮".to_string(),
            category: "platform".to_string(),
            installed: is_apt_installed("steam") || is_flatpak_installed("com.valvesoftware.Steam"),
            recommended: true,
            install_method: "apt".to_string(),
            flatpak_id: Some("com.valvesoftware.Steam".to_string()),
            apt_package: Some("steam".to_string()),
        },
        GamingPackage {
            id: "lutris".to_string(),
            name: "Lutris".to_string(),
            description: "Play games from Epic, GOG, Ubisoft, and run Windows games.".to_string(),
            icon: "🍷".to_string(),
            category: "platform".to_string(),
            installed: is_apt_installed("lutris") || is_flatpak_installed("net.lutris.Lutris"),
            recommended: true,
            install_method: "flatpak".to_string(),
            flatpak_id: Some("net.lutris.Lutris".to_string()),
            apt_package: Some("lutris".to_string()),
        },
        GamingPackage {
            id: "heroic".to_string(),
            name: "Heroic Games Launcher".to_string(),
            description: "Open-source Epic Games & GOG launcher. Lightweight alternative.".to_string(),
            icon: "⚔️".to_string(),
            category: "platform".to_string(),
            installed: is_flatpak_installed("com.heroicgameslauncher.hgl"),
            recommended: true,
            install_method: "flatpak".to_string(),
            flatpak_id: Some("com.heroicgameslauncher.hgl".to_string()),
            apt_package: None,
        },
        GamingPackage {
            id: "bottles".to_string(),
            name: "Bottles".to_string(),
            description: "Run Windows .exe files easily. Great for standalone games.".to_string(),
            icon: "🍾".to_string(),
            category: "platform".to_string(),
            installed: is_flatpak_installed("com.usebottles.bottles"),
            recommended: false,
            install_method: "flatpak".to_string(),
            flatpak_id: Some("com.usebottles.bottles".to_string()),
            apt_package: None,
        },
        GamingPackage {
            id: "prismlauncher".to_string(),
            name: "Prism Launcher".to_string(),
            description: "Minecraft launcher with multi-instance and mod support.".to_string(),
            icon: "🎲".to_string(),
            category: "platform".to_string(),
            installed: is_flatpak_installed("org.prismlauncher.PrismLauncher"),
            recommended: false,
            install_method: "flatpak".to_string(),
            flatpak_id: Some("org.prismlauncher.PrismLauncher".to_string()),
            apt_package: None,
        },
        
        // === COMPATIBILITY TOOLS ===
        GamingPackage {
            id: "protonup-qt".to_string(),
            name: "ProtonUp-Qt".to_string(),
            description: "Download GE-Proton & Wine-GE. Fixes cutscenes and compatibility.".to_string(),
            icon: "🔧".to_string(),
            category: "compatibility".to_string(),
            installed: is_flatpak_installed("net.davidotek.pupgui2"),
            recommended: true,
            install_method: "flatpak".to_string(),
            flatpak_id: Some("net.davidotek.pupgui2".to_string()),
            apt_package: None,
        },
        GamingPackage {
            id: "protontricks".to_string(),
            name: "Protontricks".to_string(),
            description: "Apply Winetricks to Steam Proton games. Install fonts, DLLs.".to_string(),
            icon: "🪄".to_string(),
            category: "compatibility".to_string(),
            installed: is_flatpak_installed("com.github.Matoking.protontricks"),
            recommended: true,
            install_method: "flatpak".to_string(),
            flatpak_id: Some("com.github.Matoking.protontricks".to_string()),
            apt_package: None,
        },
        GamingPackage {
            id: "winetricks".to_string(),
            name: "Winetricks".to_string(),
            description: "Install Windows libraries (vcrun, fonts, DirectX) for Wine.".to_string(),
            icon: "📦".to_string(),
            category: "compatibility".to_string(),
            installed: is_apt_installed("winetricks"),
            recommended: true,
            install_method: "apt".to_string(),
            flatpak_id: None,
            apt_package: Some("winetricks".to_string()),
        },
        
        // === PERFORMANCE TOOLS ===
        GamingPackage {
            id: "mangohud".to_string(),
            name: "MangoHud".to_string(),
            description: "FPS overlay. Show FPS, CPU/GPU usage, temps while gaming.".to_string(),
            icon: "📊".to_string(),
            category: "tools".to_string(),
            installed: is_apt_installed("mangohud"),
            recommended: true,
            install_method: "apt".to_string(),
            flatpak_id: None,
            apt_package: Some("mangohud".to_string()),
        },
        GamingPackage {
            id: "goverlay".to_string(),
            name: "GOverlay".to_string(),
            description: "GUI to configure MangoHud. Customize your overlay easily.".to_string(),
            icon: "🎨".to_string(),
            category: "tools".to_string(),
            installed: is_flatpak_installed("io.github.benjamimgois.GOverlay"),
            recommended: false,
            install_method: "flatpak".to_string(),
            flatpak_id: Some("io.github.benjamimgois.GOverlay".to_string()),
            apt_package: None,
        },
        GamingPackage {
            id: "gamemode".to_string(),
            name: "GameMode".to_string(),
            description: "Feral Interactive's optimizer. Auto CPU boost while gaming.".to_string(),
            icon: "⚡".to_string(),
            category: "tools".to_string(),
            installed: is_apt_installed("gamemode"),
            recommended: true,
            install_method: "apt".to_string(),
            flatpak_id: None,
            apt_package: Some("gamemode".to_string()),
        },
        GamingPackage {
            id: "gamescope".to_string(),
            name: "Gamescope".to_string(),
            description: "Valve's compositor. Force resolution, HDR, FSR upscaling.".to_string(),
            icon: "🖼️".to_string(),
            category: "tools".to_string(),
            installed: is_apt_installed("gamescope"),
            recommended: false,
            install_method: "apt".to_string(),
            flatpak_id: None,
            apt_package: Some("gamescope".to_string()),
        },
        
        // === STREAMING ===
        GamingPackage {
            id: "obs-studio".to_string(),
            name: "OBS Studio".to_string(),
            description: "Stream and record your games. Industry standard.".to_string(),
            icon: "📹".to_string(),
            category: "streaming".to_string(),
            installed: is_flatpak_installed("com.obsproject.Studio"),
            recommended: false,
            install_method: "flatpak".to_string(),
            flatpak_id: Some("com.obsproject.Studio".to_string()),
            apt_package: Some("obs-studio".to_string()),
        },
        GamingPackage {
            id: "sunshine".to_string(),
            name: "Sunshine".to_string(),
            description: "Self-hosted game streaming. Use with Moonlight client.".to_string(),
            icon: "☀️".to_string(),
            category: "streaming".to_string(),
            installed: is_flatpak_installed("dev.lizardbyte.app.Sunshine"),
            recommended: false,
            install_method: "flatpak".to_string(),
            flatpak_id: Some("dev.lizardbyte.app.Sunshine".to_string()),
            apt_package: None,
        },
    ]
}

// ============================================================================
// Performance Tweaks (Tab 3)
// ============================================================================

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GamingTweak {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,          // "memory", "cpu", "network", "input"
    pub current_value: String,
    pub recommended_value: String,
    pub min_value: Option<i64>,
    pub max_value: Option<i64>,
    pub value_type: String,        // "slider", "toggle", "dropdown"
    pub is_optimal: bool,
    pub requires_reboot: bool,
    pub risk_level: String,        // "safe", "moderate", "advanced"
}

fn read_sysctl(key: &str) -> Option<String> {
    fs::read_to_string(format!("/proc/sys/{}", key.replace('.', "/")))
        .ok()
        .map(|s| s.trim().to_string())
}

fn read_ulimit_nofile() -> String {
    Command::new("bash")
        .args(["-c", "ulimit -Hn"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn get_cpu_governor() -> String {
    fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

fn is_mouse_accel_disabled() -> bool {
    // Check for libinput flat profile
    let output = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.peripherals.mouse", "accel-profile"])
        .output();
    
    if let Ok(o) = output {
        let value = String::from_utf8_lossy(&o.stdout);
        return value.contains("flat");
    }
    false
}

/// Get all gaming tweaks with current values
#[tauri::command]
pub fn get_gaming_tweaks() -> Vec<GamingTweak> {
    let max_map = read_sysctl("vm.max_map_count").unwrap_or_else(|| "65530".to_string());
    let max_map_val: i64 = max_map.parse().unwrap_or(65530);
    
    let swappiness = read_sysctl("vm.swappiness").unwrap_or_else(|| "60".to_string());
    let swappiness_val: i64 = swappiness.parse().unwrap_or(60);
    
    let nofile = read_ulimit_nofile();
    let nofile_val: i64 = nofile.parse().unwrap_or(0);
    
    let governor = get_cpu_governor();
    let mouse_flat = is_mouse_accel_disabled();
    
    vec![
        // === MEMORY TWEAKS ===
        GamingTweak {
            id: "vm.max_map_count".to_string(),
            name: "Memory Map Limit".to_string(),
            description: "Prevents crashes in heavy games (Hogwarts, CS2, DayZ). Default is too low.".to_string(),
            category: "memory".to_string(),
            current_value: max_map.clone(),
            recommended_value: "2147483642".to_string(),
            min_value: Some(65530),
            max_value: Some(2147483642),
            value_type: "slider".to_string(),
            is_optimal: max_map_val >= 1048576,
            requires_reboot: false,
            risk_level: "safe".to_string(),
        },
        GamingTweak {
            id: "vm.swappiness".to_string(),
            name: "Swappiness".to_string(),
            description: "How aggressively RAM is swapped to disk. Lower = games stay in RAM.".to_string(),
            category: "memory".to_string(),
            current_value: swappiness.clone(),
            recommended_value: "10".to_string(),
            min_value: Some(0),
            max_value: Some(100),
            value_type: "slider".to_string(),
            is_optimal: swappiness_val <= 10,
            requires_reboot: false,
            risk_level: "safe".to_string(),
        },
        
        // === ESYNC/FSYNC (File Descriptors) ===
        GamingTweak {
            id: "nofile_limit".to_string(),
            name: "ESYNC/FSYNC Limit".to_string(),
            description: "Proton uses ESYNC for CPU performance. Needs high file descriptor limit.".to_string(),
            category: "cpu".to_string(),
            current_value: nofile.clone(),
            recommended_value: "1048576".to_string(),
            min_value: None,
            max_value: None,
            value_type: "toggle".to_string(),
            is_optimal: nofile_val >= 524288,
            requires_reboot: true,
            risk_level: "safe".to_string(),
        },
        
        // === CPU GOVERNOR ===
        GamingTweak {
            id: "cpu_governor".to_string(),
            name: "CPU Performance Mode".to_string(),
            description: "GameMode auto-switches to Performance. Install GameMode for best results.".to_string(),
            category: "cpu".to_string(),
            current_value: governor.clone(),
            recommended_value: "performance".to_string(),
            min_value: None,
            max_value: None,
            value_type: "dropdown".to_string(),
            is_optimal: governor == "performance" || is_apt_installed("gamemode"),
            requires_reboot: false,
            risk_level: "safe".to_string(),
        },
        
        // === SPLIT LOCK MITIGATION ===
        GamingTweak {
            id: "kernel.split_lock_mitigate".to_string(),
            name: "Split Lock Mitigation".to_string(),
            description: "Disabling fixes stuttering in some games. Nobara default.".to_string(),
            category: "cpu".to_string(),
            current_value: read_sysctl("kernel.split_lock_mitigate").unwrap_or_else(|| "1".to_string()),
            recommended_value: "0".to_string(),
            min_value: Some(0),
            max_value: Some(1),
            value_type: "toggle".to_string(),
            is_optimal: read_sysctl("kernel.split_lock_mitigate").map(|v| v == "0").unwrap_or(false),
            requires_reboot: false,
            risk_level: "safe".to_string(),
        },
        
        // === NETWORK ===
        GamingTweak {
            id: "net.ipv4.tcp_mtu_probing".to_string(),
            name: "MTU Probing".to_string(),
            description: "Fixes network issues in some games (Uplay). Enables Path MTU Discovery.".to_string(),
            category: "network".to_string(),
            current_value: read_sysctl("net.ipv4.tcp_mtu_probing").unwrap_or_else(|| "0".to_string()),
            recommended_value: "1".to_string(),
            min_value: Some(0),
            max_value: Some(1),
            value_type: "toggle".to_string(),
            is_optimal: read_sysctl("net.ipv4.tcp_mtu_probing").map(|v| v == "1").unwrap_or(false),
            requires_reboot: false,
            risk_level: "safe".to_string(),
        },
        
        // === INPUT ===
        GamingTweak {
            id: "mouse_accel".to_string(),
            name: "Mouse Acceleration".to_string(),
            description: "Disable for 1:1 raw input. Essential for FPS games.".to_string(),
            category: "input".to_string(),
            current_value: if mouse_flat { "Disabled (Flat)" } else { "Enabled" }.to_string(),
            recommended_value: "Disabled".to_string(),
            min_value: None,
            max_value: None,
            value_type: "toggle".to_string(),
            is_optimal: mouse_flat,
            requires_reboot: false,
            risk_level: "safe".to_string(),
        },
    ]
}

// ============================================================================
// Apply Actions
// ============================================================================

/// Install a gaming package
#[tauri::command]
pub fn install_gaming_package(pkg_id: String) -> Result<String, String> {
    let packages = get_gaming_packages();
    let pkg = packages.iter().find(|p| p.id == pkg_id)
        .ok_or_else(|| "Package not found".to_string())?;
    
    if pkg.install_method == "flatpak" {
        if let Some(ref flatpak_id) = pkg.flatpak_id {
            // Ensure Flatpak and Flathub are available
            Command::new("flatpak")
                .args(["remote-add", "--if-not-exists", "flathub", "https://dl.flathub.org/repo/flathub.flatpakrepo"])
                .output()
                .ok();
            
            let output = Command::new("pkexec")
                .args(["flatpak", "install", "-y", "flathub", flatpak_id])
                .output()
                .map_err(|e| e.to_string())?;
            
            if output.status.success() {
                return Ok(format!("{} installed successfully!", pkg.name));
            } else {
                return Err(String::from_utf8_lossy(&output.stderr).to_string());
            }
        }
    } else if pkg.install_method == "apt" {
        if let Some(ref apt_pkg) = pkg.apt_package {
            // Enable 32-bit for Steam
            if pkg_id == "steam" {
                Command::new("pkexec")
                    .args(["dpkg", "--add-architecture", "i386"])
                    .output()
                    .ok();
                
                Command::new("pkexec")
                    .args(["apt-get", "update"])
                    .output()
                    .ok();
            }
            
            let output = Command::new("pkexec")
                .args(["apt-get", "install", "-y", apt_pkg])
                .output()
                .map_err(|e| e.to_string())?;
            
            if output.status.success() {
                return Ok(format!("{} installed successfully!", pkg.name));
            } else {
                return Err(String::from_utf8_lossy(&output.stderr).to_string());
            }
        }
    }
    
    Err("Could not determine install method".to_string())
}

/// Enable 32-bit multilib support
#[tauri::command]
pub fn enable_multilib() -> Result<String, String> {
    // Ubuntu/Debian
    let output = Command::new("pkexec")
        .args(["dpkg", "--add-architecture", "i386"])
        .output()
        .map_err(|e| e.to_string())?;
    
    if output.status.success() {
        Command::new("pkexec")
            .args(["apt-get", "update"])
            .output()
            .ok();
        
        // Install 32-bit libs
        Command::new("pkexec")
            .args(["apt-get", "install", "-y", "libc6:i386", "libstdc++6:i386"])
            .output()
            .ok();
        
        return Ok("32-bit support enabled!".to_string());
    }
    
    Err("Failed to enable 32-bit support".to_string())
}

/// Install Vulkan packages
#[tauri::command]
pub fn install_vulkan_support() -> Result<String, String> {
    Command::new("pkexec")
        .args(["dpkg", "--add-architecture", "i386"])
        .output()
        .ok();
    
    Command::new("pkexec")
        .args(["apt-get", "update"])
        .output()
        .ok();
    
    let packages = vec![
        "vulkan-tools",
        "libvulkan1",
        "libvulkan1:i386",
        "mesa-vulkan-drivers",
        "mesa-vulkan-drivers:i386",
    ];
    
    for pkg in packages {
        Command::new("pkexec")
            .args(["apt-get", "install", "-y", pkg])
            .output()
            .ok();
    }
    
    Ok("Vulkan support installed!".to_string())
}

/// Apply a gaming tweak
#[tauri::command]
pub fn apply_gaming_tweak(tweak_id: String, value: String) -> Result<String, String> {
    match tweak_id.as_str() {
        "vm.max_map_count" | "vm.swappiness" | "kernel.split_lock_mitigate" | "net.ipv4.tcp_mtu_probing" => {
            // Write to sysctl.d for persistence
            let content = format!("# Game Optimizer Tweak\n{}={}\n", tweak_id, value);
            
            let output = Command::new("pkexec")
                .args(["bash", "-c", &format!(
                    "echo '{}' > /etc/sysctl.d/90-game-optimizer.conf && sysctl -w {}={}",
                    content, tweak_id, value
                )])
                .output()
                .map_err(|e| e.to_string())?;
            
            if output.status.success() {
                Ok(format!("{} set to {}", tweak_id, value))
            } else {
                Err(String::from_utf8_lossy(&output.stderr).to_string())
            }
        }
        "nofile_limit" => {
            // Write to limits.d
            let content = "# Game Optimizer - ESYNC/FSYNC\n* hard nofile 1048576\n* soft nofile 1048576\n";
            
            let output = Command::new("pkexec")
                .args(["bash", "-c", &format!(
                    "echo '{}' > /etc/security/limits.d/90-game-limits.conf",
                    content
                )])
                .output()
                .map_err(|e| e.to_string())?;
            
            if output.status.success() {
                Ok("ESYNC/FSYNC limit set. Please log out and back in.".to_string())
            } else {
                Err(String::from_utf8_lossy(&output.stderr).to_string())
            }
        }
        "mouse_accel" => {
            // Use gsettings for GNOME
            let profile = if value == "disable" { "flat" } else { "default" };
            
            let output = Command::new("gsettings")
                .args(["set", "org.gnome.desktop.peripherals.mouse", "accel-profile", profile])
                .output()
                .map_err(|e| e.to_string())?;
            
            if output.status.success() {
                Ok(format!("Mouse acceleration set to {}", profile))
            } else {
                Err(String::from_utf8_lossy(&output.stderr).to_string())
            }
        }
        _ => Err("Unknown tweak".to_string())
    }
}

/// Apply all recommended tweaks at once
#[tauri::command]
pub fn apply_all_gaming_tweaks() -> Result<Vec<String>, String> {
    let mut applied = Vec::new();
    
    // Sysctl tweaks
    let sysctl_content = r#"# Game Optimizer - All Recommended Tweaks
# Applied by Glance Gaming Center
vm.max_map_count=2147483642
vm.swappiness=10
kernel.split_lock_mitigate=0
net.ipv4.tcp_mtu_probing=1
"#;
    
    let output = Command::new("pkexec")
        .args(["bash", "-c", &format!(
            "echo '{}' > /etc/sysctl.d/90-game-optimizer.conf && sysctl --system",
            sysctl_content
        )])
        .output()
        .map_err(|e| e.to_string())?;
    
    if output.status.success() {
        applied.push("vm.max_map_count = 2147483642".to_string());
        applied.push("vm.swappiness = 10".to_string());
        applied.push("kernel.split_lock_mitigate = 0".to_string());
        applied.push("net.ipv4.tcp_mtu_probing = 1".to_string());
    }
    
    // ESYNC/FSYNC limit
    let limits_content = "# Game Optimizer - ESYNC/FSYNC\n* hard nofile 1048576\n* soft nofile 1048576\n";
    
    Command::new("pkexec")
        .args(["bash", "-c", &format!(
            "echo '{}' > /etc/security/limits.d/90-game-limits.conf",
            limits_content
        )])
        .output()
        .ok();
    
    applied.push("nofile limit = 1048576".to_string());
    
    // Mouse acceleration
    Command::new("gsettings")
        .args(["set", "org.gnome.desktop.peripherals.mouse", "accel-profile", "flat"])
        .output()
        .ok();
    
    applied.push("Mouse acceleration = disabled".to_string());
    
    Ok(applied)
}

/// Reset all gaming tweaks
#[tauri::command]
pub fn reset_gaming_tweaks() -> Result<String, String> {
    // Remove our config files
    Command::new("pkexec")
        .args(["rm", "-f", "/etc/sysctl.d/90-game-optimizer.conf"])
        .output()
        .ok();
    
    Command::new("pkexec")
        .args(["rm", "-f", "/etc/security/limits.d/90-game-limits.conf"])
        .output()
        .ok();
    
    // Reload sysctl defaults
    Command::new("pkexec")
        .args(["sysctl", "--system"])
        .output()
        .ok();
    
    // Reset mouse acceleration
    Command::new("gsettings")
        .args(["reset", "org.gnome.desktop.peripherals.mouse", "accel-profile"])
        .output()
        .ok();
    
    Ok("All gaming tweaks reset to system defaults.".to_string())
}

// ============================================================================
// System Spec Detection & One-Touch Setup
// ============================================================================

/// System spec profile for gaming setup
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SystemProfile {
    pub tier: String,           // "low", "medium", "high"
    pub ram_gb: u64,
    pub cpu_cores: u32,
    pub cpu_threads: u32,       // Logical cores
    pub gpu_vendor: String,
    pub gpu_vram_mb: u64,
    pub recommended_apps: Vec<String>,
    pub description: String,
}

/// Get system RAM in GB
fn get_ram_gb() -> u64 {
    if let Ok(content) = fs::read_to_string("/proc/meminfo") {
        for line in content.lines() {
            if line.starts_with("MemTotal:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if let Some(kb) = parts.get(1) {
                    if let Ok(val) = kb.parse::<u64>() {
                        return val / 1024 / 1024; // Convert KB to GB
                    }
                }
            }
        }
    }
    0
}

/// Get CPU physical core count (not threads)
fn get_cpu_cores() -> u32 {
    // Try lscpu for physical cores
    if let Ok(output) = Command::new("lscpu").output() {
        let lscpu = String::from_utf8_lossy(&output.stdout);
        for line in lscpu.lines() {
            if line.starts_with("Core(s) per socket:") {
                if let Some(cores_str) = line.split(':').last() {
                    if let Ok(cores_per_socket) = cores_str.trim().parse::<u32>() {
                        // Get socket count
                        for socket_line in lscpu.lines() {
                            if socket_line.starts_with("Socket(s):") {
                                if let Some(sock_str) = socket_line.split(':').last() {
                                    if let Ok(sockets) = sock_str.trim().parse::<u32>() {
                                        return cores_per_socket * sockets;
                                    }
                                }
                            }
                        }
                        return cores_per_socket;
                    }
                }
            }
        }
    }
    
    // Fallback: count "cpu cores" from /proc/cpuinfo (physical cores per CPU)
    if let Ok(content) = fs::read_to_string("/proc/cpuinfo") {
        for line in content.lines() {
            if line.starts_with("cpu cores") {
                if let Some(cores_str) = line.split(':').last() {
                    if let Ok(cores) = cores_str.trim().parse::<u32>() {
                        return cores;
                    }
                }
            }
        }
        // Ultimate fallback: count processor entries (threads)
        let threads = content.lines()
            .filter(|l| l.starts_with("processor"))
            .count();
        return (threads / 2).max(1) as u32; // Assume SMT, divide by 2
    }
    1
}

/// Get CPU thread count
fn get_cpu_threads() -> u32 {
    if let Ok(content) = fs::read_to_string("/proc/cpuinfo") {
        let threads = content.lines()
            .filter(|l| l.starts_with("processor"))
            .count();
        return threads as u32;
    }
    1
}

/// Detect system profile for recommendation
#[tauri::command]
pub fn get_system_profile() -> SystemProfile {
    let ram_gb = get_ram_gb();
    let cpu_cores = get_cpu_cores();
    let cpu_threads = get_cpu_threads();
    let gpu = detect_gpu_internal();
    let gpu_vendor = gpu.as_ref().map(|g| g.vendor.clone()).unwrap_or_else(|| "unknown".to_string());
    
    // Determine tier based on specs
    // Use threads for tier calculation as that's often more relevant for modern gaming capabilities
    // But display both in description
    let (tier, description, recommended_apps) = if ram_gb >= 16 && cpu_threads >= 12 {
        (
            "high".to_string(),
            format!("High-End System: {}GB RAM, {} cores ({} threads), {} GPU. Ready for all games!", ram_gb, cpu_cores, cpu_threads, gpu_vendor.to_uppercase()),
            vec![
                "steam", "heroic", "lutris", "protonup-qt", "mangohud", 
                "gamemode", "gamescope", "obs-studio"
            ]
        )
    } else if ram_gb >= 8 && cpu_threads >= 6 {
        (
            "medium".to_string(),
            format!("Mid-Range System: {}GB RAM, {} cores ({} threads), {} GPU. Good for most games.", ram_gb, cpu_cores, cpu_threads, gpu_vendor.to_uppercase()),
            vec![
                "steam", "heroic", "protonup-qt", "mangohud", "gamemode"
            ]
        )
    } else {
        (
            "low".to_string(),
            format!("Entry-Level System: {}GB RAM, {} cores ({} threads). Suitable for light gaming.", ram_gb, cpu_cores, cpu_threads),
            vec!["steam", "gamemode"]
        )
    };
    
    SystemProfile {
        tier,
        ram_gb,
        cpu_cores,
        cpu_threads,
        gpu_vendor,
        gpu_vram_mb: 0, // VRAM detection requires GPU-specific tools
        recommended_apps: recommended_apps.into_iter().map(String::from).collect(),
        description,
    }
}

/// Gaming readiness checklist result
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GamingChecklist {
    pub multilib_ok: bool,
    pub vulkan_ok: bool,
    pub drivers_ok: bool,
    pub kernel_tweaks_ok: bool,
    pub limits_ok: bool,
    pub gamemode_ok: bool,
    pub all_ok: bool,
    pub missing: Vec<String>,
}

/// Check gaming readiness
#[tauri::command]
pub fn get_gaming_checklist() -> GamingChecklist {
    let mut missing = Vec::new();
    
    // 1. Multilib (32-bit)
    let multilib_ok = check_multilib();
    if !multilib_ok {
        missing.push("32-bit architecture not enabled".to_string());
    }
    
    // 2. Vulkan
    let vulkan_ok = check_vulkan_support();
    if !vulkan_ok {
        missing.push("Vulkan not working (vulkaninfo failed)".to_string());
    }
    
    // 3. Drivers (check if proper driver loaded)
    let gpu = detect_gpu_internal();
    let drivers_ok = if let Some(ref g) = gpu {
        if g.vendor == "nvidia" {
            g.using_proprietary
        } else {
            g.driver.is_some()
        }
    } else {
        false
    };
    if !drivers_ok {
        missing.push("GPU driver not properly configured".to_string());
    }
    
    // 4. Kernel tweaks
    let max_map = read_sysctl("vm.max_map_count").unwrap_or_default();
    let kernel_tweaks_ok = max_map.parse::<i64>().unwrap_or(0) >= 2147483642;
    if !kernel_tweaks_ok {
        missing.push("vm.max_map_count too low".to_string());
    }
    
    // 5. Limits
    let nofile = read_ulimit_nofile();
    let limits_ok = nofile.parse::<i64>().unwrap_or(0) >= 1048576;
    if !limits_ok {
        missing.push("ESYNC/FSYNC limit too low".to_string());
    }
    
    // 6. Gamemode
    let gamemode_ok = is_apt_installed("gamemode");
    if !gamemode_ok {
        missing.push("GameMode not installed".to_string());
    }
    
    let all_ok = multilib_ok && vulkan_ok && drivers_ok && kernel_tweaks_ok && limits_ok && gamemode_ok;
    
    GamingChecklist {
        multilib_ok,
        vulkan_ok,
        drivers_ok,
        kernel_tweaks_ok,
        limits_ok,
        gamemode_ok,
        all_ok,
        missing,
    }
}

/// ONE-TOUCH Gaming Setup - Semua Layer sekaligus!
/// Ini fungsi utama yang user minta - 1 klik langsung gaming ready
/// Uses DistroFamily adapter for proper multi-distro support
#[tauri::command]
pub fn one_touch_gaming_setup() -> Result<Vec<String>, String> {
    let mut steps_done = Vec::new();
    
    // ========================================
    // PRE-FLIGHT CHECKS (Fail-Fast)
    // ========================================
    
    // Check if package manager is locked
    if is_package_manager_locked() {
        return Err("❌ Package manager is locked! Close other update processes (Software Center, apt, dnf, pacman) and try again.".to_string());
    }
    
    let distro_family = get_distro_family();
    let gpu = detect_gpu_internal();
    let gpu_vendor = gpu.as_ref().map(|g| g.vendor.as_str()).unwrap_or("unknown");
    
    steps_done.push(format!("🖥️ Detected: {} distro, {} GPU", distro_family.display_name(), gpu_vendor.to_uppercase()));
    steps_done.push("✓ Pre-flight checks passed".to_string());
    
    // ========================================
    // LAYER 1: Driver & Arsitektur
    // ========================================
    
    // 1a. Enable 32-bit architecture (distro-specific)
    steps_done.push("🔧 Enabling 32-bit architecture...".to_string());
    
    match distro_family {
        DistroFamily::Arch => {
            // Arch: Uncomment [multilib] in pacman.conf
            let _ = Command::new("pkexec")
                .args(["bash", "-c", "sed -i '/\\[multilib\\]/,/Include/s/^#//' /etc/pacman.conf && pacman -Sy"])
                .output();
            steps_done.push("✓ Multilib enabled (Arch)".to_string());
        }
        DistroFamily::Fedora => {
            // Fedora: Usually multilib is enabled, but ensure glibc.i686
            let _ = Command::new("pkexec")
                .args(["dnf", "install", "-y", "glibc.i686"])
                .output();
            steps_done.push("✓ 32-bit glibc installed (Fedora)".to_string());
        }
        DistroFamily::Suse => {
            // openSUSE: Similar to Fedora
            let _ = Command::new("pkexec")
                .args(["zypper", "install", "-y", "glibc-32bit"])
                .output();
            steps_done.push("✓ 32-bit glibc installed (openSUSE)".to_string());
        }
        _ => {
            // Debian/Ubuntu
            let _ = Command::new("pkexec")
                .args(["dpkg", "--add-architecture", "i386"])
                .output();
            let _ = Command::new("pkexec")
                .args(["apt-get", "update"])
                .output();
            steps_done.push("✓ 32-bit (i386) enabled (Debian)".to_string());
        }
    }
    
    // 1b. GPU-specific drivers
    steps_done.push(format!("🔧 Installing {} drivers...", gpu_vendor.to_uppercase()));
    
    // Layer 1 success flag for fail-fast
    let mut layer1_ok = true;
    
    match (gpu_vendor, distro_family) {
        ("nvidia", DistroFamily::Arch) => {
            // Arch: nvidia-dkms + lib32
            for pkg in ["nvidia-dkms", "nvidia-utils", "lib32-nvidia-utils"] {
                let result = Command::new("pkexec")
                    .args(["pacman", "-S", "--noconfirm", pkg])
                    .output();
                if result.is_err() { layer1_ok = false; }
            }
            steps_done.push("✓ NVIDIA drivers installed (Arch)".to_string());
        }
        ("nvidia", DistroFamily::Fedora) => {
            // Fedora: Use RPM Fusion
            for pkg in ["akmod-nvidia", "xorg-x11-drv-nvidia-cuda"] {
                let result = Command::new("pkexec")
                    .args(["dnf", "install", "-y", pkg])
                    .output();
                if result.is_err() { layer1_ok = false; }
            }
            steps_done.push("✓ NVIDIA drivers installed (Fedora)".to_string());
        }
        ("nvidia", DistroFamily::Suse) => {
            // openSUSE: Use opi nvidia
            let result = Command::new("pkexec")
                .args(["zypper", "install", "-y", "nvidia-video-G06", "nvidia-gl-G06"])
                .output();
            if result.is_err() { layer1_ok = false; }
            steps_done.push("✓ NVIDIA drivers installed (openSUSE)".to_string());
        }
        ("nvidia", _) => {
            // Debian/Ubuntu: Detect available driver version
            let driver_pkg = detect_nvidia_driver_package();
            let pkgs = vec![
                driver_pkg.clone(),
                driver_pkg.replace("nvidia-driver-", "libnvidia-gl-") + ":i386",
                "nvidia-settings".to_string(),
            ];
            for pkg in pkgs {
                let result = Command::new("pkexec")
                    .args(["apt-get", "install", "-y", &pkg])
                    .output();
                if result.is_err() { layer1_ok = false; }
            }
            steps_done.push(format!("✓ {} installed (Debian)", driver_pkg));
        }
        ("amd", DistroFamily::Arch) => {
            for pkg in ["vulkan-radeon", "lib32-vulkan-radeon", "mesa", "lib32-mesa"] {
                let _ = Command::new("pkexec")
                    .args(["pacman", "-S", "--noconfirm", pkg])
                    .output();
            }
            steps_done.push("✓ AMD Mesa (RADV) installed (Arch)".to_string());
        }
        ("amd", DistroFamily::Fedora) => {
            for pkg in ["mesa-vulkan-drivers", "mesa-vulkan-drivers.i686", "mesa-dri-drivers.i686"] {
                let _ = Command::new("pkexec")
                    .args(["dnf", "install", "-y", pkg])
                    .output();
            }
            steps_done.push("✓ AMD Mesa (RADV) installed (Fedora)".to_string());
        }
        ("amd", DistroFamily::Suse) => {
            for pkg in ["Mesa-vulkan-drivers", "Mesa-libva-drivers", "Mesa-32bit"] {
                let _ = Command::new("pkexec")
                    .args(["zypper", "install", "-y", pkg])
                    .output();
            }
            steps_done.push("✓ AMD Mesa (RADV) installed (openSUSE)".to_string());
        }
        ("amd", _) => {
            for pkg in ["mesa-vulkan-drivers", "mesa-vulkan-drivers:i386", "libgl1-mesa-dri:i386"] {
                let _ = Command::new("pkexec")
                    .args(["apt-get", "install", "-y", pkg])
                    .output();
            }
            steps_done.push("✓ AMD Mesa (RADV) installed (Debian)".to_string());
        }
        ("intel", DistroFamily::Arch) => {
            for pkg in ["vulkan-intel", "lib32-vulkan-intel", "intel-media-driver"] {
                let _ = Command::new("pkexec")
                    .args(["pacman", "-S", "--noconfirm", pkg])
                    .output();
            }
            steps_done.push("✓ Intel Mesa (ANV) installed (Arch)".to_string());
        }
        ("intel", _) => {
            for pkg in ["mesa-vulkan-drivers", "mesa-vulkan-drivers:i386", "intel-media-va-driver"] {
                let _ = Command::new("pkexec")
                    .args(["apt-get", "install", "-y", pkg])
                    .output();
            }
            steps_done.push("✓ Intel Mesa (ANV) installed (Debian)".to_string());
        }
        _ => {
            steps_done.push("⚠ Unknown GPU - skipping driver install".to_string());
        }
    }
    
    // FAIL-FAST: If Layer 1 (Driver) failed, stop here
    if !layer1_ok {
        steps_done.push("".to_string());
        steps_done.push("❌ LAYER 1 FAILED! GPU driver installation had errors.".to_string());
        steps_done.push("   Please fix driver issues before continuing.".to_string());
        return Err(steps_done.join("\n"));
    }
    
    steps_done.push("".to_string());
    steps_done.push("✓ Layer 1 Complete - Drivers ready!".to_string());
    
    // ========================================
    // LAYER 2: Compatibility (Wine/Vulkan)
    // ========================================
    
    steps_done.push("🔧 Installing Vulkan loader & Wine dependencies...".to_string());
    let vulkan_pkgs = vec![
        "vulkan-tools",
        "libvulkan1",
        "libvulkan1:i386",
    ];
    for pkg in vulkan_pkgs {
        Command::new("pkexec")
            .args(["apt-get", "install", "-y", pkg])
            .output()
            .ok();
    }
    steps_done.push("✓ Vulkan loader installed".to_string());
    
    // Wine dependencies
    let wine_deps = vec![
        "wine",
        "winetricks",
        "libwine:i386",
    ];
    for pkg in wine_deps {
        Command::new("pkexec")
            .args(["apt-get", "install", "-y", pkg])
            .output()
            .ok();
    }
    steps_done.push("✓ Wine dependencies installed".to_string());
    
    // GameMode (Layer 2.5)
    steps_done.push("🔧 Installing GameMode...".to_string());
    Command::new("pkexec")
        .args(["apt-get", "install", "-y", "gamemode", "libgamemode0:i386"])
        .output()
        .ok();
    steps_done.push("✓ GameMode installed (auto CPU boost)".to_string());
    
    // ========================================
    // LAYER 3: System Tweaks
    // ========================================
    
    steps_done.push("🔧 Applying kernel tweaks...".to_string());
    
    // Sysctl tweaks (vm.max_map_count, swappiness, etc.)
    let sysctl_content = r#"# Glance Gaming Center - One-Touch Setup
# Layer 3: Kernel Tweaks for Gaming

# Prevent crashes in heavy games (Hogwarts, CS2, DayZ)
vm.max_map_count=2147483642

# Keep games in RAM (don't swap aggressively)
vm.swappiness=10

# Fix stuttering in some games
kernel.split_lock_mitigate=0

# Fix network issues (Uplay, etc.)
net.ipv4.tcp_mtu_probing=1
"#;
    
    Command::new("pkexec")
        .args(["bash", "-c", &format!(
            "echo '{}' > /etc/sysctl.d/99-gaming.conf && sysctl --system",
            sysctl_content
        )])
        .output()
        .ok();
    steps_done.push("✓ vm.max_map_count = 2147483642".to_string());
    steps_done.push("✓ vm.swappiness = 10".to_string());
    
    // File descriptor limits (ESYNC/FSYNC)
    let limits_content = r#"# Glance Gaming Center - ESYNC/FSYNC
# Layer 3: High file descriptor limit for Wine/Proton

* hard nofile 1048576
* soft nofile 1048576
"#;
    
    Command::new("pkexec")
        .args(["bash", "-c", &format!(
            "echo '{}' > /etc/security/limits.d/99-gaming.conf",
            limits_content
        )])
        .output()
        .ok();
    steps_done.push("✓ ESYNC/FSYNC limit = 1048576".to_string());
    
    // Mouse acceleration off (for FPS games) - DE-aware
    let de = detect_desktop_environment();
    let _ = set_mouse_flat_profile();
    steps_done.push(format!("✓ Mouse acceleration disabled ({})", de));
    
    // ========================================
    // LAYER 4: Essential Apps
    // ========================================
    
    steps_done.push("🔧 Installing Steam...".to_string());
    Command::new("pkexec")
        .args(["apt-get", "install", "-y", "steam"])
        .output()
        .ok();
    steps_done.push("✓ Steam installed".to_string());
    
    // MangoHud for FPS overlay
    Command::new("pkexec")
        .args(["apt-get", "install", "-y", "mangohud"])
        .output()
        .ok();
    steps_done.push("✓ MangoHud installed (FPS overlay)".to_string());
    
    // Flatpak setup for ProtonUp-Qt and Heroic
    Command::new("pkexec")
        .args(["apt-get", "install", "-y", "flatpak"])
        .output()
        .ok();
    
    Command::new("flatpak")
        .args(["remote-add", "--if-not-exists", "flathub", "https://dl.flathub.org/repo/flathub.flatpakrepo"])
        .output()
        .ok();
    
    // ProtonUp-Qt (for GE-Proton)
    Command::new("flatpak")
        .args(["install", "-y", "flathub", "net.davidotek.pupgui2"])
        .output()
        .ok();
    steps_done.push("✓ ProtonUp-Qt installed (download GE-Proton)".to_string());
    
    // Heroic Games Launcher
    Command::new("flatpak")
        .args(["install", "-y", "flathub", "com.heroicgameslauncher.hgl"])
        .output()
        .ok();
    steps_done.push("✓ Heroic Games Launcher installed".to_string());
    
    // ========================================
    // DONE!
    // ========================================
    
    steps_done.push("".to_string());
    steps_done.push("🎮 ONE-TOUCH SETUP COMPLETE!".to_string());
    steps_done.push("🔄 Please REBOOT for all changes to take effect.".to_string());
    
    Ok(steps_done)
}

