use anyhow::{Context, Result, anyhow};
use bitcoin::{Network, PrivateKey, secp256k1::SecretKey};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use vgen::{GpuBackend, GpuRunner, gpu::BtccVanityPatternConfig};

use crate::wallet::{
    keys::VanityPattern,
    vanity_gpu::{VanityGpuBackend, VanityGpuMatch, VanityGpuProgressCallback},
};

impl VanityGpuBackend {
    fn to_vgen_backend(self) -> GpuBackend {
        match self {
            Self::Auto => GpuBackend::Auto,
            Self::Vulkan => GpuBackend::Vulkan,
            Self::Metal => GpuBackend::Metal,
            Self::Dx12 => GpuBackend::Dx12,
            Self::Gl => GpuBackend::Gl,
        }
    }
}

pub fn run_vgen_for_btcc_pattern(
    pattern: &VanityPattern,
    backend: VanityGpuBackend,
    batch_size: u32,
    stop_requested: Arc<AtomicBool>,
    progress_cb: Option<VanityGpuProgressCallback>,
) -> Result<Option<VanityGpuMatch>> {
    if matches!(backend, VanityGpuBackend::Dx12) {
        return Err(anyhow!(
            "当前进程内 GPU 版本暂不支持 DX12 后端，请改用 Auto 或 Vulkan。"
        ));
    }

    if stop_requested.load(Ordering::Relaxed) {
        return Ok(None);
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("create tokio runtime for GPU vanity search failed")?;

    let runner = runtime
        .block_on(GpuRunner::new(batch_size.max(1), backend.to_vgen_backend()))
        .context("initialize GPU runner failed")?;

    runtime.block_on(scan_btcc_with_runner(
        pattern,
        batch_size.max(1),
        stop_requested,
        progress_cb,
        Arc::new(runner),
    ))
}

async fn scan_btcc_with_runner(
    pattern: &VanityPattern,
    batch_size: u32,
    stop_requested: Arc<AtomicBool>,
    progress_cb: Option<VanityGpuProgressCallback>,
    runner: Arc<GpuRunner>,
) -> Result<Option<VanityGpuMatch>> {
    let gpu_pattern = to_gpu_pattern(pattern);
    let mut total_ops = 0u64;
    let mut current_key = random_valid_secret_key()?;
    let num_frames = 2usize;
    let mut in_flight = 0usize;

    for frame_index in 0..num_frames {
        if stop_requested.load(Ordering::Relaxed) {
            return Ok(None);
        }
        runner.dispatch_btcc_match(current_key, &gpu_pattern, frame_index)?;
        current_key = match increment_key(current_key, u64::from(batch_size)) {
            Some(next) => next,
            None => break,
        };
        in_flight += 1;
    }

    let mut frame_index = 0usize;
    while in_flight > 0 {
        if stop_requested.load(Ordering::Relaxed) {
            return Ok(None);
        }

        let (match_index, batch_start_key) = runner.await_result_btcc_match(frame_index).await?;
        in_flight -= 1;

        if !stop_requested.load(Ordering::Relaxed) {
            runner.dispatch_btcc_match(current_key, &gpu_pattern, frame_index)?;
            current_key = increment_key(current_key, u64::from(batch_size))
                .context("GPU vanity key space exhausted")?;
            in_flight += 1;
        }

        total_ops = total_ops.saturating_add(u64::from(batch_size));
        if let Some(cb) = &progress_cb {
            cb(total_ops);
        }

        if match_index != u32::MAX {
            let secret = increment_key(batch_start_key, u64::from(match_index))
                .context("invalid GPU match index")?;
            let wif = secret_key_to_wif(&secret)?;
            return Ok(Some(VanityGpuMatch {
                address: String::new(),
                wif,
                operations: total_ops,
            }));
        }

        frame_index = (frame_index + 1) % num_frames;
    }

    Ok(None)
}

fn secret_key_to_wif(secret: &[u8; 32]) -> Result<String> {
    let secret_key = SecretKey::from_slice(secret).context("invalid secret key from GPU batch")?;
    Ok(PrivateKey::new(secret_key, Network::Bitcoin).to_wif())
}

fn random_valid_secret_key() -> Result<[u8; 32]> {
    use rand::{RngCore, SeedableRng, rngs::StdRng};

    let mut rng = StdRng::from_entropy();
    loop {
        let mut key = [0u8; 32];
        rng.fill_bytes(&mut key);
        if SecretKey::from_slice(&key).is_ok() {
            return Ok(key);
        }
    }
}

fn increment_key(key: [u8; 32], amount: u64) -> Option<[u8; 32]> {
    let mut key = key;
    let mut carry = amount;

    for index in (0..32).rev() {
        let sum = u64::from(key[index]) + (carry & 0xff);
        key[index] = sum as u8;
        carry = (carry >> 8) + (sum >> 8);
        if carry == 0 {
            break;
        }
    }

    if carry > 0 || SecretKey::from_slice(&key).is_err() {
        None
    } else {
        Some(key)
    }
}

fn to_gpu_pattern(pattern: &VanityPattern) -> BtccVanityPatternConfig {
    match pattern {
        VanityPattern::Prefix(prefix) => BtccVanityPatternConfig {
            mode: 1,
            prefix: prefix.clone(),
            suffix: String::new(),
        },
        VanityPattern::Suffix(suffix) => BtccVanityPatternConfig {
            mode: 2,
            prefix: String::new(),
            suffix: suffix.clone(),
        },
        VanityPattern::PrefixAndSuffix { prefix, suffix } => BtccVanityPatternConfig {
            mode: 3,
            prefix: prefix.clone(),
            suffix: suffix.clone(),
        },
    }
}
