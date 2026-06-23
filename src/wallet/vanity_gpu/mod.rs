use std::sync::Arc;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VanityGpuBackend {
    #[default]
    Auto,
    Vulkan,
}

impl VanityGpuBackend {
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Vulkan => "Vulkan",
        }
    }
}

impl std::fmt::Display for VanityGpuBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

pub const DEFAULT_GPU_BATCH_SIZE: u32 = 512 * 1024;
pub type VanityGpuProgressCallback = Arc<dyn Fn(u64) + Send + Sync>;

#[derive(Debug, Clone)]
pub struct VanityGpuMatch {
    pub address: String,
    pub wif: String,
    pub operations: u64,
}

#[cfg(target_os = "macos")]
mod macos_disabled;
#[cfg(not(target_os = "macos"))]
mod non_macos;

#[cfg(target_os = "macos")]
pub use macos_disabled::run_vgen_for_btcc_pattern;
#[cfg(not(target_os = "macos"))]
pub use non_macos::run_vgen_for_btcc_pattern;
