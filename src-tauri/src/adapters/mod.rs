//! Package Manager abstraction layer
//! Provides a unified interface for different Linux package managers

pub mod package_manager;
pub mod debian;
pub mod arch;
pub mod fedora;
pub mod suse;

pub use package_manager::*;
pub use debian::DebianAdapter;
pub use arch::ArchAdapter;
pub use fedora::FedoraAdapter;
pub use suse::SuseAdapter;
