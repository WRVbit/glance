//! Desktop Environment detection
//! Detects GNOME, KDE, Xfce, and other DEs for theme integration

use serde::{Deserialize, Serialize};
use std::process::Command;

// ============================================================================
// Desktop Environment Enum
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DesktopEnvironment {
    Gnome,
    Kde,
    Xfce,
    Cinnamon,
    Mate,
    Lxde,
    Lxqt,
    Budgie,
    Pantheon,
    Deepin,
    TilingWM, // i3, Sway, Hyprland, bspwm, etc.
    Unknown,
}

impl DesktopEnvironment {
    /// Detect the current desktop environment
    pub fn detect() -> Self {
        let xdg_desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
        let desktop_session = std::env::var("DESKTOP_SESSION").unwrap_or_default();
        let xdg_session = std::env::var("XDG_SESSION_DESKTOP").unwrap_or_default();
        
        let combined = format!("{} {} {}", xdg_desktop, desktop_session, xdg_session)
            .to_lowercase();
        
        // Check in order of popularity
        if combined.contains("gnome") || combined.contains("ubuntu") {
            Self::Gnome
        } else if combined.contains("kde") || combined.contains("plasma") {
            Self::Kde
        } else if combined.contains("xfce") {
            Self::Xfce
        } else if combined.contains("cinnamon") {
            Self::Cinnamon
        } else if combined.contains("mate") {
            Self::Mate
        } else if combined.contains("lxde") {
            Self::Lxde
        } else if combined.contains("lxqt") {
            Self::Lxqt
        } else if combined.contains("budgie") {
            Self::Budgie
        } else if combined.contains("pantheon") {
            Self::Pantheon
        } else if combined.contains("deepin") {
            Self::Deepin
        } else if combined.contains("i3") 
            || combined.contains("sway") 
            || combined.contains("hyprland")
            || combined.contains("bspwm")
            || combined.contains("awesome")
            || combined.contains("dwm")
            || combined.contains("qtile")
        {
            Self::TilingWM
        } else {
            Self::Unknown
        }
    }
    
    /// Get display name
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Gnome => "GNOME",
            Self::Kde => "KDE Plasma",
            Self::Xfce => "Xfce",
            Self::Cinnamon => "Cinnamon",
            Self::Mate => "MATE",
            Self::Lxde => "LXDE",
            Self::Lxqt => "LXQt",
            Self::Budgie => "Budgie",
            Self::Pantheon => "Pantheon",
            Self::Deepin => "Deepin",
            Self::TilingWM => "Tiling WM",
            Self::Unknown => "Unknown",
        }
    }
    
    /// Check if the system is in dark mode
    pub fn is_dark_mode(&self) -> bool {
        match self {
            Self::Gnome | Self::Budgie | Self::Pantheon => {
                // gsettings get org.gnome.desktop.interface color-scheme
                Command::new("gsettings")
                    .args(["get", "org.gnome.desktop.interface", "color-scheme"])
                    .output()
                    .map(|o| {
                        let output = String::from_utf8_lossy(&o.stdout).to_lowercase();
                        output.contains("dark") || output.contains("prefer-dark")
                    })
                    .unwrap_or(true) // Default to dark
            }
            Self::Kde => {
                // kreadconfig5 --group General --key ColorScheme
                Command::new("kreadconfig5")
                    .args(["--group", "General", "--key", "ColorScheme"])
                    .output()
                    .map(|o| {
                        String::from_utf8_lossy(&o.stdout).to_lowercase().contains("dark")
                    })
                    .unwrap_or(true)
            }
            Self::Xfce => {
                // xfconf-query -c xsettings -p /Net/ThemeName
                Command::new("xfconf-query")
                    .args(["-c", "xsettings", "-p", "/Net/ThemeName"])
                    .output()
                    .map(|o| {
                        String::from_utf8_lossy(&o.stdout).to_lowercase().contains("dark")
                    })
                    .unwrap_or(true)
            }
            Self::Cinnamon => {
                // gsettings get org.cinnamon.desktop.interface gtk-theme
                Command::new("gsettings")
                    .args(["get", "org.cinnamon.desktop.interface", "gtk-theme"])
                    .output()
                    .map(|o| {
                        String::from_utf8_lossy(&o.stdout).to_lowercase().contains("dark")
                    })
                    .unwrap_or(true)
            }
            Self::Mate => {
                // gsettings get org.mate.interface gtk-theme
                Command::new("gsettings")
                    .args(["get", "org.mate.interface", "gtk-theme"])
                    .output()
                    .map(|o| {
                        String::from_utf8_lossy(&o.stdout).to_lowercase().contains("dark")
                    })
                    .unwrap_or(true)
            }
            // Tiling WMs and others: default to dark (common preference)
            _ => true,
        }
    }
    
    /// Check if this DE supports CSD (Client-Side Decorations)
    pub fn supports_csd(&self) -> bool {
        matches!(self, Self::Gnome | Self::Budgie | Self::Pantheon | Self::Deepin)
    }
    
    /// Check if this DE uses traditional window decorations
    pub fn has_server_decorations(&self) -> bool {
        matches!(self, Self::Kde | Self::Xfce | Self::Cinnamon | Self::Mate | Self::Lxde | Self::Lxqt)
    }
}

impl Default for DesktopEnvironment {
    fn default() -> Self {
        Self::detect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_detect_de() {
        let de = DesktopEnvironment::detect();
        println!("Detected DE: {:?} ({})", de, de.display_name());
        println!("Dark mode: {}", de.is_dark_mode());
    }
}
