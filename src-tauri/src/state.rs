//! Shared application state
//! Thread-safe cache for system information and distro context

use std::sync::{Arc, Mutex};
use sysinfo::System;
use crate::utils::{DistroContext, DistroFamily, DesktopEnvironment};

/// Shared system state with cached data and distro context
pub struct AppState {
    /// Cached sysinfo System instance wrapped in Arc for thread-safe cloning
    pub sys: Arc<Mutex<System>>,
    /// Distro-specific runtime context
    pub context: DistroContext,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            sys: Arc::new(Mutex::new(System::new_all())),
            context: DistroContext::new(),
        }
    }
    
    /// Get the detected distro family
    pub fn distro_family(&self) -> DistroFamily {
        self.context.family
    }
    
    /// Get the detected desktop environment
    pub fn desktop_env(&self) -> DesktopEnvironment {
        DesktopEnvironment::detect()
    }
    
    /// Check if a feature is available on this distro
    pub fn has_feature(&self, feature: &str) -> bool {
        self.context.has_feature(feature)
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
