//! Shared application state
//! Thread-safe cache for system information

use std::sync::{Arc, Mutex};
use sysinfo::System;

/// Shared system state with cached data
pub struct AppState {
    /// Cached sysinfo System instance wrapped in Arc for thread-safe cloning
    pub sys: Arc<Mutex<System>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            sys: Arc::new(Mutex::new(System::new_all())),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
