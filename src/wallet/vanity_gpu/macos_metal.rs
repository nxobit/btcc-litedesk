use anyhow::{Context, Result, anyhow};
use bech32::Hrp;
use bitcoin::{
    CompressedPublicKey, Network, PrivateKey, PublicKey, WPubkeyHash,
    hashes::Hash,
    secp256k1::{Secp256k1, SecretKey},
};
use bytemuck::{Pod, Zeroable};
use metal::*;
use naga::{
    ShaderStage,
    back::msl,
    front::wgsl,
    valid::{Capabilities, ValidationFlags, Validator},
};
use objc::rc::autoreleasepool;
use rand::{RngCore, SeedableRng, rngs::StdRng};
use std::fs;
use std::mem::size_of;
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use crate::wallet::{
    keys::VanityPattern,
    vanity_gpu::{VanityGpuBackend, VanityGpuMatch, VanityGpuProgressCallback},
};

// ==================== 配置常量 ====================
const BTCC_BECH32_HRP: &str = "cc";

const DEFAULT_BATCH_SIZE: u32 = 2048;
const MIN_BATCH_SIZE: u32 = 512;
const MAX_BATCH_SIZE: u32 = 16384;

const INIT_WORKGROUP_SIZE: u64 = 256;

// ==================== 数据结构 ====================
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct BigInt256 {
    v0: [u32; 4],
    v1: [u32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct Config {
    base_x: BigInt256,
    base_y: BigInt256,
    num_keys: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
    match_mode: u32,
    prefix_len: u32,
    suffix_len: u32,
    _pad3: u32,
    prefix_chars: [[u32; 4]; 11],
    suffix_chars: [[u32; 4]; 11],
}

impl Default for Config {
    fn default() -> Self {
        Self {
            base_x: BigInt256::zeroed(),
            base_y: BigInt256::zeroed(),
            num_keys: 0,
            _pad0: 0,
            _pad1: 0,
            _pad2: 0,
            match_mode: 0,
            prefix_len: 0,
            suffix_len: 0,
            _pad3: 0,
            prefix_chars: [[0; 4]; 11],
            suffix_chars: [[0; 4]; 11],
        }
    }
}

// ==================== Metal Runner（核心优化） ====================
struct MetalVanityGpuRunner {
    device: Device,
    command_queue: CommandQueue,

    compute_jacobian_pipeline: ComputePipelineState,
    batch_normalize_btcc_match_pipeline: ComputePipelineState,

    config_buffer: Buffer,
    table_buffer: Buffer,
    output_buffer: Buffer,
    jacobian_buffer: Buffer,
    p2tr_output_buffer: Buffer,
    match_buffer: Buffer,

    batch_size: u32,
    workgroup_size: u64,
}

impl MetalVanityGpuRunner {
    fn new(mut batch_size: u32) -> Result<Self> {
        autoreleasepool(|| {
            let device = Device::system_default().context("Metal device not found")?;
            let command_queue = device.new_command_queue();

            // ==================== 动态 Batch Size ====================
            batch_size = match device.gpu_family() {
                MTLGPUFamily::Apple8 | MTLGPUFamily::Apple9 => batch_size.clamp(2048, 16384), // M3/M4
                MTLGPUFamily::Apple7 => batch_size.clamp(1024, 8192),                         // M2
                _ => batch_size.clamp(512, 4096), // M1 及更老
            }
            .clamp(MIN_BATCH_SIZE, MAX_BATCH_SIZE);

            // ==================== Shader 编译 ====================
            let init_msl = compile_wgsl_compute_to_msl(
                &join_wgsl_sources(&[FIELD_WGSL, INIT_WGSL]),
                "init_table",
            )?;

            let options = CompileOptions::new();
            let init_lib = device.new_library_with_source(&init_msl, &options)?;
            let jac_lib = device.new_library_with_source(COMPUTE_JACOBIAN_METAL, &options)?;
            let btcc_lib =
                device.new_library_with_source(BATCH_NORMALIZE_BTCC_MATCH_METAL, &options)?;

            let init_pipeline = create_pipeline(&device, &init_lib, "init_table")?;
            let compute_jacobian_pipeline = create_pipeline(&device, &jac_lib, "compute_jacobian")?;
            let batch_normalize_btcc_match_pipeline =
                create_pipeline(&device, &btcc_lib, "batch_normalize_btcc_match")?;

            // ==================== 动态 Workgroup Size ====================
            let max_threads = compute_jacobian_pipeline.max_total_threads_per_threadgroup();
            let workgroup_size = (64..=max_threads).step_by(64).last().unwrap_or(64);

            // ==================== 缓冲区分配 ====================
            let config_buffer = new_shared_buffer_with_value(&device, &Config::default());
            let table_buffer = device.new_buffer(
                u64::from(batch_size) * 64,
                MTLResourceOptions::StorageModeShared,
            );
            let output_buffer = device.new_buffer(
                u64::from(batch_size) * 20,
                MTLResourceOptions::StorageModeShared,
            );
            let jacobian_buffer = device.new_buffer(
                u64::from(batch_size) * 96,
                MTLResourceOptions::StorageModeShared,
            );
            let p2tr_output_buffer = device.new_buffer(
                u64::from(batch_size) * 32,
                MTLResourceOptions::StorageModeShared,
            );
            let match_buffer = device.new_buffer(4, MTLResourceOptions::StorageModeShared);
            write_shared_buffer(&match_buffer, &u32::MAX);

            let runner = Self {
                device,
                command_queue,
                compute_jacobian_pipeline,
                batch_normalize_btcc_match_pipeline,
                config_buffer,
                table_buffer,
                output_buffer,
                jacobian_buffer,
                p2tr_output_buffer,
                match_buffer,
                batch_size,
                workgroup_size,
            };

            runner.initialize_lookup_table(&init_pipeline)?;
            Ok(runner)
        })
    }

    fn initialize_lookup_table(&self, init_pipeline: &ComputePipelineState) -> Result<()> {
        autoreleasepool(|| {
            let command_buffer = self.command_queue.new_command_buffer();
            let encoder = command_buffer.new_compute_command_encoder();
            encoder.set_compute_pipeline_state(init_pipeline);
            self.bind_common_buffers(&encoder);
            dispatch_threads(&encoder, self.batch_size as u64, INIT_WORKGROUP_SIZE);
            encoder.end_encoding();
            command_buffer.commit();
            command_buffer.wait_until_completed();
            ensure_completed(&command_buffer, "init_table")
        })
    }

    fn dispatch_btcc_match(&self, pattern: &VanityPattern, start_key: [u8; 32]) -> Result<u32> {
        autoreleasepool(|| {
            let (x_limbs, y_limbs) = key_to_affine(start_key)?;
            let gpu_pattern = to_gpu_pattern_config(pattern);

            let config = Config {
                base_x: BigInt256 {
                    v0: x_limbs[0..4].try_into()?,
                    v1: x_limbs[4..8].try_into()?,
                },
                base_y: BigInt256 {
                    v0: y_limbs[0..4].try_into()?,
                    v1: y_limbs[4..8].try_into()?,
                },
                num_keys: self.batch_size,
                match_mode: gpu_pattern.match_mode,
                prefix_len: gpu_pattern.prefix_len,
                suffix_len: gpu_pattern.suffix_len,
                ..Default::default()
            };

            write_shared_buffer(&self.config_buffer, &config);
            write_shared_buffer(&self.match_buffer, &u32::MAX);

            let command_buffer = self.command_queue.new_command_buffer();
            let encoder = command_buffer.new_compute_command_encoder();

            // Pipeline 1: Jacobian
            encoder.set_compute_pipeline_state(&self.compute_jacobian_pipeline);
            self.bind_common_buffers(&encoder);
            dispatch_threads(&encoder, self.batch_size as u64, self.workgroup_size);

            // Pipeline 2: Normalize + Match
            encoder.set_compute_pipeline_state(&self.batch_normalize_btcc_match_pipeline);
            self.bind_common_buffers(&encoder);
            dispatch_threads(&encoder, self.batch_size as u64, self.workgroup_size);

            encoder.end_encoding();
            command_buffer.commit();
            command_buffer.wait_until_completed();
            ensure_completed(&command_buffer, "btcc_match")?;

            Ok(read_shared_buffer::<u32>(&self.match_buffer))
        })
    }

    fn bind_common_buffers(&self, encoder: &ComputeCommandEncoderRef) {
        encoder.set_buffer(0, Some(&self.config_buffer), 0);
        encoder.set_buffer(1, Some(&self.table_buffer), 0);
        encoder.set_buffer(2, Some(&self.output_buffer), 0);
        encoder.set_buffer(3, Some(&self.jacobian_buffer), 0);
        encoder.set_buffer(4, Some(&self.p2tr_output_buffer), 0);
        encoder.set_buffer(5, Some(&self.match_buffer), 0);
    }
}

// ==================== 公开接口（保持兼容） ====================
pub fn run_vgen_for_btcc_pattern(
    pattern: &VanityPattern,
    backend: VanityGpuBackend,
    batch_size: u32,
    stop_requested: Arc<AtomicBool>,
    progress_cb: Option<VanityGpuProgressCallback>,
) -> Result<Option<VanityGpuMatch>> {
    if !matches!(backend, VanityGpuBackend::Auto | VanityGpuBackend::Metal) {
        return Err(anyhow!("macOS GPU 模式仅支持 Metal"));
    }

    let runner = MetalVanityGpuRunner::new(batch_size).context("初始化 Metal Vanity GPU 失败")?;

    let secp = Secp256k1::new();
    let hrp = Hrp::parse(BTCC_BECH32_HRP)?;

    let mut total_ops = 0u64;
    let mut batch_start_key = random_valid_secret_key()?;

    while !stop_requested.load(Ordering::Relaxed) {
        let match_index = runner.dispatch_btcc_match(pattern, batch_start_key)?;

        if match_index != u32::MAX {
            // 命中处理逻辑（保持不变）
            if match_index >= runner.batch_size {
                return Err(anyhow!("invalid match index"));
            }

            let matched_key = increment_key(batch_start_key, u64::from(match_index))
                .context("key offset overflow")?;

            let address = address_from_secret_key_with_ctx(&matched_key, &secp, &hrp)?;
            let wif = secret_key_to_wif(&matched_key)?;

            if vanity_match(&address, pattern) {
                return Ok(Some(VanityGpuMatch {
                    address,
                    wif,
                    operations: total_ops + u64::from(match_index) + 1,
                }));
            }

            // False positive 跳过
            let skip = u64::from(match_index) + 1;
            total_ops += skip;
            if let Some(cb) = &progress_cb {
                cb(total_ops);
            }
            batch_start_key = increment_key(batch_start_key, skip).context("skip failed")?;
            continue;
        }

        total_ops += u64::from(runner.batch_size);
        if let Some(cb) = &progress_cb {
            cb(total_ops);
        }

        batch_start_key = increment_key(batch_start_key, u64::from(runner.batch_size))
            .context("key space exhausted")?;
    }

    Ok(None)
}
