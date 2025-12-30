//! Shared application state
//! Thread-safe cache for system information

use std::sync::Mutex;
use sysinfo::System;

/// Shared system state with cached data
pub struct AppState {
    /// Cached sysinfo System instance
    pub sys: Mutex<System>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            sys: Mutex::new(System::new()),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
