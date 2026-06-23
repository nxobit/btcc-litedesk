use anyhow::{Result, anyhow};
use std::sync::{
    Arc,
    atomic::AtomicBool,
};

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
        "macOS 当前已关闭 GPU 靓号生成，请改用 CPU。"
    ))
}
