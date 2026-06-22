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

<<<<<<< HEAD
const BTCC_BECH32_HRP: &str = "cc";
const INIT_WORKGROUP_SIZE: u64 = 256;
const BTCC_WORKGROUP_SIZE: u64 = 64;
const MAX_MACOS_GPU_BATCH_SIZE: u32 = 2048;

const FIELD_WGSL: &str = include_str!("wgsl/field.wgsl");
const FIELD_JACOBIAN_WGSL: &str = include_str!("wgsl/field_jacobian.wgsl");
const INIT_WGSL: &str = include_str!("wgsl/init.wgsl");
const COMPUTE_JACOBIAN_WGSL: &str = include_str!("wgsl/compute_jacobian.wgsl");
const COMPUTE_JACOBIAN_METAL: &str = include_str!("shaders/compute_jacobian.metal");
const BATCH_NORMALIZE_BTCC_MATCH_METAL: &str =
    include_str!("shaders/batch_normalize_btcc_match.metal");

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct BigInt256 {
    v0: [u32; 4],
    v1: [u32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct PatternConfig {
    match_mode: u32,
    prefix_len: u32,
    suffix_len: u32,
    pad: u32,
    prefix_chars: [[u32; 4]; 11],
    suffix_chars: [[u32; 4]; 11],
}

impl Default for PatternConfig {
    fn default() -> Self {
        Self {
            match_mode: 0,
            prefix_len: 0,
            suffix_len: 0,
            pad: 0,
            prefix_chars: [[0; 4]; 11],
            suffix_chars: [[0; 4]; 11],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct Config {
    base_x: BigInt256,            // offset 0
    base_y: BigInt256,            // offset 32
    num_keys: u32,                // offset 64
    _pad0: u32,                   // offset 68
    _pad1: u32,                   // offset 72
    _pad2: u32,                   // offset 76
    match_mode: u32,              // offset 80  ← 与 WGSL Config 布局一致
    prefix_len: u32,              // offset 84
    suffix_len: u32,              // offset 88
    _pad3: u32,                   // offset 92
    prefix_chars: [[u32; 4]; 11], // offset 96
    suffix_chars: [[u32; 4]; 11], // offset 272
}

struct MetalVanityGpuRunner {
    device: Device,
    command_queue: CommandQueue,
    init_pipeline: ComputePipelineState,
    compute_jacobian_pipeline: ComputePipelineState,
    batch_normalize_btcc_match_pipeline: ComputePipelineState,
    config_buffer: Buffer,
    table_buffer: Buffer,
    output_buffer: Buffer,
    jacobian_buffer: Buffer,
    p2tr_output_buffer: Buffer,
    match_buffer: Buffer,
    batch_size: u32,
}

impl MetalVanityGpuRunner {
    fn new(batch_size: u32) -> Result<Self> {
        autoreleasepool(|| {
            let device = Device::system_default().context("Metal device not found")?;
            let command_queue = device.new_command_queue();

            let init_source = join_wgsl_sources(&[FIELD_WGSL, INIT_WGSL]);
            let jacobian_source = join_wgsl_sources(&[FIELD_JACOBIAN_WGSL, COMPUTE_JACOBIAN_WGSL]);
            let init_msl = compile_wgsl_compute_to_msl(&init_source, "init_table")
                .context("compile init_table WGSL->MSL failed")?;

            write_debug_msl("init_table", &init_msl);
            write_debug_msl("compute_jacobian", COMPUTE_JACOBIAN_METAL);
            write_debug_msl(
                "batch_normalize_btcc_match",
                BATCH_NORMALIZE_BTCC_MATCH_METAL,
            );

            let options = CompileOptions::new();
            let init_library = device
                .new_library_with_source(&init_msl, &options)
                .map_err(|err| anyhow!("failed to compile init_table MSL: {err}"))?;
            let jacobian_library = device
                .new_library_with_source(COMPUTE_JACOBIAN_METAL, &options)
                .map_err(|err| anyhow!("failed to compile compute_jacobian MSL: {err}"))?;
            let btcc_library = device
                .new_library_with_source(BATCH_NORMALIZE_BTCC_MATCH_METAL, &options)
                .map_err(|err| {
                    anyhow!("failed to compile batch_normalize_btcc_match MSL: {err}")
                })?;

            let init_pipeline = create_pipeline(&device, &init_library, "init_table")
                .context("failed to create init_table pipeline")?;
            let compute_jacobian_pipeline =
                create_pipeline(&device, &jacobian_library, "compute_jacobian")
                    .context("failed to create compute_jacobian pipeline")?;
            let batch_normalize_btcc_match_pipeline =
                create_pipeline(&device, &btcc_library, "batch_normalize_btcc_match")
                    .context("failed to create batch_normalize_btcc_match pipeline")?;

            let initial_config = Config {
                base_x: BigInt256::zeroed(),
                base_y: BigInt256::zeroed(),
                num_keys: batch_size,
                _pad0: 0,
                _pad1: 0,
                _pad2: 0,
                match_mode: 0,
                prefix_len: 0,
                suffix_len: 0,
                _pad3: 0,
                prefix_chars: [[0; 4]; 11],
                suffix_chars: [[0; 4]; 11],
            };

            let config_buffer = new_shared_buffer_with_value(&device, &initial_config);
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
                init_pipeline,
                compute_jacobian_pipeline,
                batch_normalize_btcc_match_pipeline,
                config_buffer,
                table_buffer,
                output_buffer,
                jacobian_buffer,
                p2tr_output_buffer,
                match_buffer,
                batch_size,
            };

            runner
                .initialize_lookup_table()
                .context("initialize native Metal lookup table failed")?;

            Ok(runner)
        })
    }

    fn initialize_lookup_table(&self) -> Result<()> {
        autoreleasepool(|| {
            let command_buffer = self.command_queue.new_command_buffer();
            let encoder = command_buffer.new_compute_command_encoder();
            encoder.set_compute_pipeline_state(&self.init_pipeline);
            self.bind_common_buffers(&encoder);
            dispatch_threads(&encoder, self.batch_size.max(1) as u64, INIT_WORKGROUP_SIZE);
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
                    v0: x_limbs[0..4].try_into().expect("x limb split"),
                    v1: x_limbs[4..8].try_into().expect("x limb split"),
                },
                base_y: BigInt256 {
                    v0: y_limbs[0..4].try_into().expect("y limb split"),
                    v1: y_limbs[4..8].try_into().expect("y limb split"),
                },
                num_keys: self.batch_size,
                _pad0: 0,
                _pad1: 0,
                _pad2: 0,
                match_mode: gpu_pattern.match_mode,
                prefix_len: gpu_pattern.prefix_len,
                suffix_len: gpu_pattern.suffix_len,
                _pad3: 0,
                prefix_chars: gpu_pattern.prefix_chars,
                suffix_chars: gpu_pattern.suffix_chars,
            };

            write_shared_buffer(&self.config_buffer, &config);
            write_shared_buffer(&self.match_buffer, &u32::MAX);

            let command_buffer = self.command_queue.new_command_buffer();
            let encoder = command_buffer.new_compute_command_encoder();

            encoder.set_compute_pipeline_state(&self.compute_jacobian_pipeline);
            self.bind_common_buffers(&encoder);
            dispatch_threads(&encoder, self.batch_size.max(1) as u64, BTCC_WORKGROUP_SIZE);

            encoder.set_compute_pipeline_state(&self.batch_normalize_btcc_match_pipeline);
            self.bind_common_buffers(&encoder);
            dispatch_threads(&encoder, self.batch_size.max(1) as u64, BTCC_WORKGROUP_SIZE);

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

pub fn run_vgen_for_btcc_pattern(
    pattern: &VanityPattern,
    backend: VanityGpuBackend,
    batch_size: u32,
    stop_requested: Arc<AtomicBool>,
    progress_cb: Option<VanityGpuProgressCallback>,
) -> Result<Option<VanityGpuMatch>> {
    if !matches!(backend, VanityGpuBackend::Auto | VanityGpuBackend::Metal) {
        return Err(anyhow!("macOS GPU 模式当前仅支持 Auto 或 Metal 后端。"));
    }

    if stop_requested.load(Ordering::Relaxed) {
        return Ok(None);
    }

    let effective_batch_size = batch_size.max(1).min(MAX_MACOS_GPU_BATCH_SIZE);
    let runner = MetalVanityGpuRunner::new(effective_batch_size)
        .context("initialize native Metal vanity backend failed")?;
    let secp = Secp256k1::new();
    let hrp = Hrp::parse(BTCC_BECH32_HRP).context("parse BTCC hrp failed")?;

    let mut total_ops = 0u64;
    let mut batch_start_key = random_valid_secret_key()?;

    loop {
        if stop_requested.load(Ordering::Relaxed) {
            return Ok(None);
        }

        let match_index = runner
            .dispatch_btcc_match(pattern, batch_start_key)
            .context("dispatch native Metal BTCC match failed")?;

        if match_index != u32::MAX {
            if match_index >= runner.batch_size {
                return Err(anyhow!(
                    "native Metal BTCC match returned invalid index {} for batch size {}",
                    match_index,
                    runner.batch_size
                ));
            }

            let matched_key = increment_key(batch_start_key, u64::from(match_index))
                .context("matched key offset overflowed secp256k1 range")?;
            let address = address_from_secret_key_with_ctx(&matched_key, &secp, &hrp)?;
            let wif = secret_key_to_wif(&matched_key)?;

            if !vanity_match(&address, pattern) {
                let skip = u64::from(match_index).saturating_add(1);
                total_ops = total_ops.saturating_add(skip);
                if let Some(cb) = &progress_cb {
                    cb(total_ops);
                }
                batch_start_key = increment_key(batch_start_key, skip).context(
                    "native Metal GPU search exhausted key space after skipping false positive",
                )?;
                continue;
            }

            return Ok(Some(VanityGpuMatch {
                address,
                wif,
                operations: total_ops
                    .saturating_add(u64::from(match_index))
                    .saturating_add(1),
            }));
        }

        total_ops = total_ops.saturating_add(u64::from(runner.batch_size));
        if let Some(cb) = &progress_cb {
            cb(total_ops);
        }

        batch_start_key = increment_key(batch_start_key, u64::from(runner.batch_size))
            .context("native Metal GPU search exhausted key space")?;
    }
}

fn create_pipeline(
    device: &Device,
    library: &Library,
    function_name: &str,
) -> Result<ComputePipelineState> {
    let function = library
        .get_function(function_name, None)
        .map_err(|err| anyhow!("failed to load Metal function {function_name}: {err}"))?;
    let desc = ComputePipelineDescriptor::new();
    desc.set_compute_function(Some(&function));
    device
        .new_compute_pipeline_state(&desc)
        .map_err(|err| anyhow!("failed to create Metal pipeline {function_name}: {err}"))
}

fn join_wgsl_sources(parts: &[&str]) -> String {
    let mut combined = String::new();
    for (index, part) in parts.iter().enumerate() {
        if index > 0 {
            combined.push('\n');
        }
        combined.push_str(part);
    }
    combined
}

fn compile_wgsl_compute_to_msl(source: &str, entry_point: &str) -> Result<String> {
    let module = wgsl::parse_str(source).map_err(|err| anyhow!("WGSL parse failed: {err}"))?;
    let info = Validator::new(ValidationFlags::all(), Capabilities::all())
        .validate(&module)
        .map_err(|err| anyhow!("WGSL validation failed: {err}"))?;

    let options = msl::Options::default();
    let pipeline_options = msl::PipelineOptions {
        entry_point: Some((ShaderStage::Compute, entry_point.to_string())),
        ..Default::default()
    };

    let (msl_source, _) = msl::write_string(&module, &info, &options, &pipeline_options)
        .map_err(|err| anyhow!("WGSL->MSL translation failed: {err}"))?;
    Ok(msl_source)
}

fn write_debug_msl(name: &str, source: &str) {
    let mut path = PathBuf::from("target");
    path.push("metal-debug");
    let _ = fs::create_dir_all(&path);
    path.push(format!("{name}.metal"));
    let _ = fs::write(path, source);
}

fn dispatch_threads(encoder: &ComputeCommandEncoderRef, total_threads: u64, group_size: u64) {
    let group_size = group_size.max(1);
    encoder.dispatch_threads(
        MTLSize {
            width: total_threads,
            height: 1,
            depth: 1,
        },
        MTLSize {
            width: group_size,
            height: 1,
            depth: 1,
        },
    );
}

fn ensure_completed(command_buffer: &CommandBufferRef, label: &str) -> Result<()> {
    let status = command_buffer.status();
    if status == MTLCommandBufferStatus::Completed {
        Ok(())
    } else {
        Err(anyhow!(
            "Metal command buffer {label} failed with status {:?}",
            status
        ))
    }
}

fn new_shared_buffer_with_value<T: Pod>(device: &Device, value: &T) -> Buffer {
    device.new_buffer_with_data(
        bytemuck::bytes_of(value).as_ptr().cast(),
        size_of::<T>() as u64,
        MTLResourceOptions::StorageModeShared,
    )
}

fn write_shared_buffer<T: Pod>(buffer: &Buffer, value: &T) {
    unsafe {
        std::ptr::copy_nonoverlapping(
            bytemuck::bytes_of(value).as_ptr(),
            buffer.contents().cast(),
            size_of::<T>(),
        );
    }
}

fn read_shared_buffer<T: Pod>(buffer: &Buffer) -> T {
    unsafe { *(buffer.contents() as *const T) }
}

fn to_gpu_pattern_config(pattern: &VanityPattern) -> PatternConfig {
    let mut config = PatternConfig::default();
    match pattern {
        VanityPattern::Prefix(prefix) => {
            config.match_mode = 1;
            config.prefix_len = prefix.chars().count() as u32;
            write_pattern_chars(&mut config.prefix_chars, prefix);
        }
        VanityPattern::Suffix(suffix) => {
            config.match_mode = 2;
            config.suffix_len = suffix.chars().count() as u32;
            write_pattern_chars(&mut config.suffix_chars, suffix);
        }
        VanityPattern::PrefixAndSuffix { prefix, suffix } => {
            config.match_mode = 3;
            config.prefix_len = prefix.chars().count() as u32;
            config.suffix_len = suffix.chars().count() as u32;
            write_pattern_chars(&mut config.prefix_chars, prefix);
            write_pattern_chars(&mut config.suffix_chars, suffix);
        }
    }
    config
}

fn write_pattern_chars(target: &mut [[u32; 4]; 11], value: &str) {
    for (index, ch) in value.chars().take(44).enumerate() {
        target[index / 4][index % 4] = ch as u32;
    }
}

fn bytes_be_to_u32_le(bytes: &[u8]) -> [u32; 8] {
    let mut limbs = [0u32; 8];
    for (i, limb) in limbs.iter_mut().enumerate() {
        let start = 28 - i * 4;
        let chunk: [u8; 4] = bytes[start..start + 4].try_into().expect("32-byte limb");
        *limb = u32::from_be_bytes(chunk);
    }
    limbs
}

fn key_to_affine(key: [u8; 32]) -> Result<([u32; 8], [u32; 8])> {
    let secp = Secp256k1::new();
    let sk = SecretKey::from_slice(&key)?;
    let pk = bitcoin::secp256k1::PublicKey::from_secret_key(&secp, &sk);
    let serialized = pk.serialize_uncompressed();

    let x_limbs = bytes_be_to_u32_le(&serialized[1..33]);
    let y_limbs = bytes_be_to_u32_le(&serialized[33..65]);
    Ok((x_limbs, y_limbs))
}

fn secret_key_to_wif(secret: &[u8; 32]) -> Result<String> {
    let secret_key = SecretKey::from_slice(secret).context("invalid secret key from search")?;
    Ok(PrivateKey::new(secret_key, Network::Bitcoin).to_wif())
}

fn address_from_secret_key_with_ctx(
    secret: &[u8; 32],
    secp: &Secp256k1<bitcoin::secp256k1::All>,
    hrp: &Hrp,
) -> Result<String> {
    let secret_key = SecretKey::from_slice(secret).context("invalid secret key from search")?;
    let public_key = PublicKey::new(secret_key.public_key(secp));
    encode_btcc_address_with_hrp(&public_key, hrp)
}

fn encode_btcc_address_with_hrp(public_key: &PublicKey, hrp: &Hrp) -> Result<String> {
    let compressed = CompressedPublicKey::try_from(*public_key)
        .context("BTCC wallet requires compressed public key")?;
    let hash = WPubkeyHash::hash(&compressed.to_bytes());

    bech32::segwit::encode(*hrp, bech32::segwit::VERSION_0, hash.as_byte_array())
        .context("encode BTCC address failed")
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

fn random_valid_secret_key() -> Result<[u8; 32]> {
    let mut rng = StdRng::from_entropy();
    loop {
        let mut key = [0u8; 32];
        rng.fill_bytes(&mut key);
        if SecretKey::from_slice(&key).is_ok() {
            return Ok(key);
        }
    }
}

fn vanity_match(address: &str, pattern: &VanityPattern) -> bool {
    let address = address.to_ascii_lowercase();
    match pattern {
        VanityPattern::Prefix(prefix) => address.starts_with(prefix),
        VanityPattern::Suffix(suffix) => address.ends_with(suffix),
        VanityPattern::PrefixAndSuffix { prefix, suffix } => {
            address.starts_with(prefix) && address.ends_with(suffix)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wallet::keys::VanityPattern;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    /// 测试 GPU 能否生成一个简单的后缀靓号地址，并验证地址正确性
    #[test]
    fn gpu_generates_valid_btcc_address_with_suffix() {
        // 使用一个非常简单的后缀 "q"，GPU 应该很快找到
        let pattern = VanityPattern::Suffix("8888".to_string());
        let batch_size = 2048u32; // 使用较小的 batch size
        let stop = Arc::new(AtomicBool::new(false));

        let result =
            run_vgen_for_btcc_pattern(&pattern, VanityGpuBackend::Metal, batch_size, stop, None);

        match result {
            Ok(Some(found)) => {
                println!("GPU 找到匹配!");
                println!("  地址: {}", found.address);
                println!("  WIF: {}", found.wif);
                println!("  尝试次数: {}", found.operations);

                // 验证地址格式
                assert!(
                    found.address.starts_with("cc1q"),
                    "BTCC 地址应以 'cc1q' 开头，实际: {}",
                    found.address
                );
                assert!(
                    found.address.ends_with("q"),
                    "地址应以 'q' 结尾，实际: {}",
                    found.address
                );

                // 验证地址长度 (bech32m 地址: cc1 + 52 chars = 55)
                assert_eq!(
                    found.address.len(),
                    42,
                    "BTCC Native SegWit (P2WPKH) 地址应为 42 字符，实际: {} ({})",
                    found.address.len(),
                    found.address
                );

                // 验证 WIF 格式
                assert!(
                    found.wif.starts_with('L') || found.wif.starts_with('K'),
                    "WIF 应以 L 或 K 开头，实际: {}",
                    found.wif
                );

                // 用 vanity_match 验证
                assert!(
                    vanity_match(&found.address, &pattern),
                    "vanity_match 应该返回 true"
                );

                // 用 CPU 端验证：从 WIF 恢复地址，确认匹配
                let secp = Secp256k1::new();
                let hrp = Hrp::parse("cc").unwrap();
                let address_from_wif = address_from_secret_key_with_wif(&found.wif, &secp, &hrp);
                assert!(
                    address_from_wif.is_ok(),
                    "从 WIF 恢复地址失败: {:?}",
                    address_from_wif.err()
                );
                let address_from_wif = address_from_wif.unwrap();
                assert_eq!(
                    address_from_wif, found.address,
                    "从 WIF 恢复的地址与 GPU 返回的地址不一致"
                );

                println!("✅ 所有验证通过!");
            }
            Ok(None) => {
                panic!("GPU 未找到匹配，但应该很快找到后缀 'q'");
            }
            Err(e) => {
                panic!("GPU 调用失败: {}", e);
            }
        }
    }

    /// 辅助函数：从 WIF 字符串恢复地址
    fn address_from_secret_key_with_wif(
        wif: &str,
        secp: &bitcoin::secp256k1::Secp256k1<bitcoin::secp256k1::All>,
        hrp: &Hrp,
    ) -> Result<String> {
        use bitcoin::hashes::Hash;
        use bitcoin::{PrivateKey, WPubkeyHash};

        let private_key = PrivateKey::from_wif(wif).context("parse WIF failed")?;
        let public_key = private_key.public_key(secp);
        let compressed_bytes = public_key.to_bytes();
        let hash = WPubkeyHash::hash(&compressed_bytes[..]);
        bech32::segwit::encode(*hrp, bech32::segwit::VERSION_0, hash.as_byte_array())
            .context("encode BTCC address failed")
    }
}
