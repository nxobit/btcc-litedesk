use anyhow::{Result, anyhow};
use std::sync::{Arc, atomic::AtomicBool};

use crate::wallet::{
    keys::VanityPattern,
    vanity_gpu::{VanityGpuBackend, VanityGpuMatch, VanityGpuProgressCallback},
};

pub fn run_vgen_for_btcc_pattern(
    _pattern: &VanityPattern,
    _backend: VanityGpuBackend,
    _batch_size: u32,
    _stop_requested: Arc<AtomicBool>,
    _progress_cb: Option<VanityGpuProgressCallback>,
) -> Result<Option<VanityGpuMatch>> {
    Err(anyhow!(
        "macOS GPU 后端正在切换为原生 Metal 实现，当前版本暂不可用。"
    ))
}
