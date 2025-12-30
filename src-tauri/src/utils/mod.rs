//! Utility modules

pub mod distro;
pub mod privileged;
pub mod context;
pub mod desktop;

pub use distro::{DistroInfo, DistroFamily};
pub use context::{DistroContext, DistroPaths, FeatureAvailability};
pub use desktop::DesktopEnvironment;
