use anyhow::{Context, anyhow};
use bech32::Hrp;
use bip39::{Language, Mnemonic};
use bitcoin::{
    CompressedPublicKey, Network, PrivateKey, PublicKey, WPubkeyHash,
    bip32::{DerivationPath, Xpriv},
    hashes::Hash,
    secp256k1::{Secp256k1, SecretKey},
};
use rand::{RngCore, rngs::OsRng};
use std::str::FromStr;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread;

use crate::wallet::vanity_gpu::{
    DEFAULT_GPU_BATCH_SIZE, VanityGpuBackend, VanityGpuProgressCallback,
    run_vgen_for_btcc_pattern,
};

pub const BTCC_NATIVE_SEGWIT_PATH: &str = "m/84'/0'/0'/0/0";
const BTCC_BECH32_HRP: &str = "cc";
const BTCC_BECH32_CHARSET: &str = "qpzry9x8gf2tvdw0s3jn54khce6mua7l";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BtccWallet {
    pub network: String,
    pub mnemonic: String,
    pub derivation_path: String,
    pub address: String,
    pub private_key_wif: String,
    pub public_key: PublicKey,
    pub secret_key: SecretKey,
}

pub type BitcoinWallet = BtccWallet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VanityPattern {
    Prefix(String),
    Suffix(String),
    PrefixAndSuffix { prefix: String, suffix: String },
}

#[derive(Clone, Debug)]
pub struct VanityGenerationResult {
    pub wallet: BtccWallet,
    pub attempts: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VanityComputeMode {
    Cpu,
    Gpu(VanityGpuConfig),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VanityGpuConfig {
    pub backend: VanityGpuBackend,
    pub batch_size: u32,
}

impl Default for VanityGpuConfig {
    fn default() -> Self {
        Self {
            backend: VanityGpuBackend::Auto,
            batch_size: DEFAULT_GPU_BATCH_SIZE,
        }
    }
}

pub fn generate_btcc_wallet() -> anyhow::Result<BtccWallet> {
    let mut entropy = [0u8; 16];
    OsRng.fill_bytes(&mut entropy);
    wallet_from_entropy(&entropy)
}

pub fn generate_bitcoin_wallet() -> anyhow::Result<BtccWallet> {
    generate_btcc_wallet()
}

pub fn generate_vanity_btcc_wallet(
    pattern: VanityPattern,
    thread_count: usize,
) -> anyhow::Result<BtccWallet> {
    generate_vanity_btcc_wallet_with_stats(pattern, thread_count).map(|result| result.wallet)
}

pub fn generate_vanity_btcc_wallet_with_stats(
    pattern: VanityPattern,
    thread_count: usize,
) -> anyhow::Result<VanityGenerationResult> {
    let pattern = normalize_vanity_pattern(pattern)?;
    if is_empty_vanity_pattern(&pattern) {
        let wallet = generate_btcc_wallet()?;
        return Ok(VanityGenerationResult {
            wallet,
            attempts: 1,
        });
    }
    if thread_count == 0 {
        return Err(anyhow!("Thread count must be greater than 0"));
    }

    let found = Arc::new(AtomicBool::new(false));
    let (sender, receiver) = mpsc::channel();
    let mut handles = Vec::with_capacity(thread_count);

    for _ in 0..thread_count {
        let sender = sender.clone();
        let found = Arc::clone(&found);
        let pattern = pattern.clone();

        handles.push(thread::spawn(move || {
            let mut attempts = 0u64;
            while !found.load(Ordering::Relaxed) {
                let wallet = match generate_btcc_wallet() {
                    Ok(wallet) => wallet,
                    Err(err) => {
                        let _ = sender.send(Err(err));
                        return;
                    }
                };
                attempts = attempts.saturating_add(1);

                if vanity_match(&wallet.address, &pattern) {
                    if !found.swap(true, Ordering::SeqCst) {
                        let _ = sender.send(Ok((wallet, attempts)));
                    }
                    return;
                }
            }
        }));
    }
    drop(sender);

    let channel_result = receiver
        .recv()
        .map_err(|_| anyhow!("Vanity wallet generation workers stopped unexpectedly"))?;
    let (wallet, attempts) = channel_result?;

    found.store(true, Ordering::SeqCst);
    for handle in handles {
        let _ = handle.join();
    }

    Ok(VanityGenerationResult { wallet, attempts })
}

pub fn generate_vanity_btcc_wallet_with_stats_cancellable(
    pattern: VanityPattern,
    thread_count: usize,
    stop_requested: Arc<AtomicBool>,
) -> anyhow::Result<Option<VanityGenerationResult>> {
    generate_vanity_btcc_wallet_with_stats_cancellable_mode(
        pattern,
        thread_count,
        stop_requested,
        VanityComputeMode::Cpu,
    )
}

pub fn generate_vanity_btcc_wallet_with_stats_cancellable_mode(
    pattern: VanityPattern,
    thread_count: usize,
    stop_requested: Arc<AtomicBool>,
    compute_mode: VanityComputeMode,
) -> anyhow::Result<Option<VanityGenerationResult>> {
    generate_vanity_btcc_wallet_with_stats_cancellable_mode_progress(
        pattern,
        thread_count,
        stop_requested,
        compute_mode,
        None,
    )
}

pub fn generate_vanity_btcc_wallet_with_stats_cancellable_mode_progress(
    pattern: VanityPattern,
    thread_count: usize,
    stop_requested: Arc<AtomicBool>,
    compute_mode: VanityComputeMode,
    progress_cb: Option<VanityGpuProgressCallback>,
) -> anyhow::Result<Option<VanityGenerationResult>> {
    match compute_mode {
        VanityComputeMode::Cpu => generate_vanity_btcc_wallet_with_stats_cancellable_cpu(
            pattern,
            thread_count,
            stop_requested,
        ),
        VanityComputeMode::Gpu(config) => generate_vanity_btcc_wallet_with_stats_cancellable_gpu(
            pattern,
            stop_requested,
            config,
            progress_cb,
        ),
    }
}

fn generate_vanity_btcc_wallet_with_stats_cancellable_cpu(
    pattern: VanityPattern,
    thread_count: usize,
    stop_requested: Arc<AtomicBool>,
) -> anyhow::Result<Option<VanityGenerationResult>> {
    let pattern = normalize_vanity_pattern(pattern)?;
    if is_empty_vanity_pattern(&pattern) {
        let wallet = generate_btcc_wallet()?;
        return Ok(Some(VanityGenerationResult {
            wallet,
            attempts: 1,
        }));
    }
    if thread_count == 0 {
        return Err(anyhow!("Thread count must be greater than 0"));
    }

    let found = Arc::new(AtomicBool::new(false));
    let (sender, receiver) = mpsc::channel();
    let mut handles = Vec::with_capacity(thread_count);

    for _ in 0..thread_count {
        let sender = sender.clone();
        let found = Arc::clone(&found);
        let pattern = pattern.clone();
        let stop_requested = Arc::clone(&stop_requested);

        handles.push(thread::spawn(move || {
            let mut attempts = 0u64;
            while !found.load(Ordering::Relaxed) && !stop_requested.load(Ordering::Relaxed) {
                let wallet = match generate_btcc_wallet() {
                    Ok(wallet) => wallet,
                    Err(err) => {
                        let _ = sender.send(Err(err));
                        return;
                    }
                };
                attempts = attempts.saturating_add(1);

                if vanity_match(&wallet.address, &pattern) {
                    if !found.swap(true, Ordering::SeqCst) {
                        let _ = sender.send(Ok(Some((wallet, attempts))));
                    }
                    return;
                }
            }
            if stop_requested.load(Ordering::Relaxed) && !found.load(Ordering::Relaxed) {
                let _ = sender.send(Ok(None));
            }
        }));
    }
    drop(sender);

    let channel_result = receiver
        .recv()
        .map_err(|_| anyhow!("Vanity wallet generation workers stopped unexpectedly"))?;

    found.store(true, Ordering::SeqCst);
    for handle in handles {
        let _ = handle.join();
    }

    channel_result
        .map(|opt| opt.map(|(wallet, attempts)| VanityGenerationResult { wallet, attempts }))
}

fn generate_vanity_btcc_wallet_with_stats_cancellable_gpu(
    pattern: VanityPattern,
    stop_requested: Arc<AtomicBool>,
    gpu_config: VanityGpuConfig,
    progress_cb: Option<VanityGpuProgressCallback>,
) -> anyhow::Result<Option<VanityGenerationResult>> {
    let pattern = normalize_vanity_pattern(pattern)?;
    if is_empty_vanity_pattern(&pattern) {
        let wallet = generate_btcc_wallet()?;
        return Ok(Some(VanityGenerationResult {
            wallet,
            attempts: 1,
        }));
    }

    let Some(found) = run_vgen_for_btcc_pattern(
        &pattern,
        gpu_config.backend,
        gpu_config.batch_size,
        stop_requested,
        progress_cb,
    )?
    else {
        return Ok(None);
    };

    let mut wallet = wallet_from_private_key_wif(&found.wif)
        .context("restore BTCC wallet from GPU-generated WIF failed")?;
    wallet.derivation_path = "gpu-generated".to_string();

    if !vanity_match(&wallet.address, &pattern) {
        return Err(anyhow!(
            "GPU generator returned a WIF that does not match the requested BTCC vanity pattern"
        ));
    }

    Ok(Some(VanityGenerationResult {
        wallet,
        attempts: found.operations,
    }))
}

fn normalize_vanity_pattern(pattern: VanityPattern) -> anyhow::Result<VanityPattern> {
    match pattern {
        VanityPattern::Prefix(prefix) => {
            let prefix = prefix.trim().to_ascii_lowercase();
            validate_vanity_prefix(&prefix)?;
            Ok(VanityPattern::Prefix(prefix))
        }
        VanityPattern::Suffix(suffix) => {
            let suffix = suffix.trim().to_ascii_lowercase();
            validate_vanity_suffix(&suffix)?;
            Ok(VanityPattern::Suffix(suffix))
        }
        VanityPattern::PrefixAndSuffix { prefix, suffix } => {
            let prefix = prefix.trim().to_ascii_lowercase();
            let suffix = suffix.trim().to_ascii_lowercase();
            validate_vanity_prefix(&prefix)?;
            validate_vanity_suffix(&suffix)?;
            Ok(VanityPattern::PrefixAndSuffix { prefix, suffix })
        }
    }
}

fn validate_vanity_prefix(prefix: &str) -> anyhow::Result<()> {
    if prefix.is_empty() {
        return Ok(());
    }
    if !prefix.starts_with(BTCC_BECH32_HRP) {
        return Err(anyhow!(
            "Vanity prefix must start with '{}'",
            BTCC_BECH32_HRP
        ));
    }
    if prefix.len() >= 3 && prefix.as_bytes()[2] != b'1' {
        return Err(anyhow!(
            "Vanity prefix must use '1' as the third character after '{}'",
            BTCC_BECH32_HRP
        ));
    }
    if prefix.len() > 3 {
        validate_bech32_pattern_chars(&prefix[3..], "Vanity prefix")?;
    }
    Ok(())
}

fn validate_vanity_suffix(suffix: &str) -> anyhow::Result<()> {
    if suffix.is_empty() {
        return Ok(());
    }
    validate_bech32_pattern_chars(suffix, "Vanity suffix")
}

fn validate_bech32_pattern_chars(value: &str, label: &str) -> anyhow::Result<()> {
    for ch in value.chars() {
        if !BTCC_BECH32_CHARSET.contains(ch) {
            return Err(anyhow!("{label} contains invalid Bech32 character: '{ch}'"));
        }
    }
    Ok(())
}

fn is_empty_vanity_pattern(pattern: &VanityPattern) -> bool {
    match pattern {
        VanityPattern::Prefix(prefix) => prefix.is_empty(),
        VanityPattern::Suffix(suffix) => suffix.is_empty(),
        VanityPattern::PrefixAndSuffix { prefix, suffix } => prefix.is_empty() && suffix.is_empty(),
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

pub fn wallet_from_mnemonic(mnemonic_str: &str) -> anyhow::Result<BtccWallet> {
    let mnemonic = Mnemonic::from_str(mnemonic_str).context("Invalid mnemonic phrase")?;

    if mnemonic.word_count() != 12 && mnemonic.word_count() != 24 {
        return Err(anyhow!("Mnemonic must be 12 or 24 words"));
    }

    wallet_from_mnemonic_value(&mnemonic)
}

pub fn wallet_from_private_key_wif(wif: &str) -> anyhow::Result<BtccWallet> {
    let private_key = PrivateKey::from_wif(wif).context("Invalid WIF private key")?;
    let secp = Secp256k1::new();
    let public_key = PublicKey::new(private_key.inner.public_key(&secp));
    let address = encode_btcc_address(&public_key)?;

    Ok(BtccWallet {
        network: "Bitcoin-Classic (BTCC)".to_string(),
        mnemonic: String::new(),
        derivation_path: "imported-wif".to_string(),
        address,
        private_key_wif: wif.trim().to_string(),
        public_key,
        secret_key: private_key.inner,
    })
}

fn wallet_from_entropy(entropy: &[u8]) -> anyhow::Result<BtccWallet> {
    let mnemonic =
        Mnemonic::from_entropy_in(Language::English, entropy).context("create mnemonic failed")?;
    wallet_from_mnemonic_value(&mnemonic)
}

fn wallet_from_mnemonic_value(mnemonic: &Mnemonic) -> anyhow::Result<BtccWallet> {
    let seed = mnemonic.to_seed("");
    let secp = Secp256k1::new();
    let master =
        Xpriv::new_master(Network::Bitcoin, &seed).context("create master xpriv failed")?;
    let path = DerivationPath::from_str(BTCC_NATIVE_SEGWIT_PATH)
        .context("parse derivation path failed")?;
    let child = master
        .derive_priv(&secp, &path)
        .context("derive private key failed")?;
    let private_key = PrivateKey::new(child.private_key, Network::Bitcoin);
    let public_key = PublicKey::new(child.private_key.public_key(&secp));
    let address = encode_btcc_address(&public_key)?;

    Ok(BtccWallet {
        network: "Bitcoin-Classic (BTCC)".to_string(),
        mnemonic: mnemonic.to_string(),
        derivation_path: BTCC_NATIVE_SEGWIT_PATH.to_string(),
        address,
        private_key_wif: private_key.to_wif(),
        public_key,
        secret_key: child.private_key,
    })
}

fn encode_btcc_address(public_key: &PublicKey) -> anyhow::Result<String> {
    let compressed = CompressedPublicKey::try_from(*public_key)
        .context("BTCC wallet requires compressed key")?;
    let hash = WPubkeyHash::hash(&compressed.to_bytes());

    encode_btcc_address_from_wpubkey_hash(hash.as_byte_array())
}

fn encode_btcc_address_from_wpubkey_hash(hash: &[u8; 20]) -> anyhow::Result<String> {
    bech32::segwit::encode(
        Hrp::parse(BTCC_BECH32_HRP)?,
        bech32::segwit::VERSION_0,
        hash,
    )
    .context("encode BTCC address failed")
}

#[cfg(test)]
fn increment_secret_key_bytes(key: [u8; 32], amount: u64) -> Option<[u8; 32]> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_repeatable_btcc_wallet_from_mnemonic() {
        let wallet = wallet_from_entropy(&[7u8; 16]).unwrap();
        let imported = wallet_from_mnemonic(&wallet.mnemonic).unwrap();

        assert_eq!(wallet.address, imported.address);
        assert_eq!(wallet.private_key_wif, imported.private_key_wif);
        assert!(wallet.address.starts_with("cc1q"));
    }

    #[test]
    fn imports_wif_wallet() {
        let wallet = wallet_from_entropy(&[9u8; 16]).unwrap();
        let imported = wallet_from_private_key_wif(&wallet.private_key_wif).unwrap();

        assert_eq!(wallet.address, imported.address);
        assert_eq!("imported-wif", imported.derivation_path);
    }

    #[test]
    fn matches_vanity_prefix_suffix_and_both() {
        let address = "cc1qexampleaddressxyz";
        assert!(vanity_match(
            address,
            &VanityPattern::Prefix("cc1qex".to_string())
        ));
        assert!(vanity_match(
            address,
            &VanityPattern::Suffix("xyz".to_string())
        ));
        assert!(vanity_match(
            address,
            &VanityPattern::PrefixAndSuffix {
                prefix: "cc1q".to_string(),
                suffix: "xyz".to_string(),
            }
        ));
        assert!(!vanity_match(
            address,
            &VanityPattern::PrefixAndSuffix {
                prefix: "cc1q".to_string(),
                suffix: "abc".to_string(),
            }
        ));
    }

    #[test]
    fn rejects_invalid_bech32_vanity_characters() {
        assert!(generate_vanity_btcc_wallet(VanityPattern::Suffix("btcc".to_string()), 4).is_err());
        assert!(
            generate_vanity_btcc_wallet(
                VanityPattern::PrefixAndSuffix {
                    prefix: "cc1q".to_string(),
                    suffix: "love".to_string(),
                },
                4,
            )
            .is_err()
        );
    }

    #[test]
    #[ignore = "manual vanity suffix generation test"]
    fn generates_vanity_wallet_with_tccc_suffix() {
        let wallet =
            generate_vanity_btcc_wallet(VanityPattern::Suffix("h".to_string()), 4).unwrap();
        println!("address: {}", wallet.address);
        println!("mnemonic: {}", wallet.mnemonic);
        println!("wif: {}", wallet.private_key_wif);
        assert!(wallet.address.ends_with("t"));
    }

    #[test]
    fn increments_secret_key_bytes_without_overflow() {
        let mut start = [0u8; 32];
        start[31] = 1;
        let incremented = increment_secret_key_bytes(start, 2).unwrap();
        assert_eq!(incremented[31], 3);
    }

    #[test]
    fn rejects_secret_key_increment_overflow() {
        let max = [0xffu8; 32];
        assert!(increment_secret_key_bytes(max, 1).is_none());
    }

    #[test]
    #[ignore = "manual benchmark for local CPU/GPU comparison"]
    fn compares_cpu_gpu_vanity_generation_time() {
        use std::time::Instant;

        let pattern = VanityPattern::PrefixAndSuffix {
            prefix: "cc1q".to_string(),
            suffix: "qq".to_string(),
        };

        let cpu_started = Instant::now();
        let cpu_result = generate_vanity_btcc_wallet_with_stats_cancellable_mode(
            pattern.clone(),
            4,
            Arc::new(AtomicBool::new(false)),
            VanityComputeMode::Cpu,
        )
        .expect("CPU vanity generation should succeed")
        .expect("CPU vanity generation should return a wallet");
        let cpu_elapsed = cpu_started.elapsed();

        let gpu_started = Instant::now();
        let gpu_result = generate_vanity_btcc_wallet_with_stats_cancellable_mode(
            pattern.clone(),
            1,
            Arc::new(AtomicBool::new(false)),
            VanityComputeMode::Gpu(VanityGpuConfig::default()),
        )
        .expect("GPU vanity generation should succeed")
        .expect("GPU vanity generation should return a wallet");
        let gpu_elapsed = gpu_started.elapsed();

        assert!(vanity_match(&cpu_result.wallet.address, &pattern));
        assert!(vanity_match(&gpu_result.wallet.address, &pattern));

        println!(
            "CPU elapsed: {:?}, attempts: {}, address: {}",
            cpu_elapsed, cpu_result.attempts, cpu_result.wallet.address
        );
        println!(
            "GPU elapsed: {:?}, attempts: {}, address: {}",
            gpu_elapsed, gpu_result.attempts, gpu_result.wallet.address
        );
    }

    #[test]
    #[ignore = "manual benchmark for local CPU/GPU suffix comparison"]
    fn compares_cpu_gpu_suffix_lengths_2_to_5() {
        use std::time::Instant;

        for suffix in ["qq", "qqq", "qqqq", "qqqqq"] {
            let pattern = VanityPattern::Suffix(suffix.to_string());

            let cpu_started = Instant::now();
            let cpu_result = generate_vanity_btcc_wallet_with_stats_cancellable_mode(
                pattern.clone(),
                4,
                Arc::new(AtomicBool::new(false)),
                VanityComputeMode::Cpu,
            )
            .expect("CPU vanity generation should succeed")
            .expect("CPU vanity generation should return a wallet");
            let cpu_elapsed = cpu_started.elapsed();

            let gpu_started = Instant::now();
            let gpu_result = generate_vanity_btcc_wallet_with_stats_cancellable_mode(
                pattern.clone(),
                1,
                Arc::new(AtomicBool::new(false)),
                VanityComputeMode::Gpu(VanityGpuConfig::default()),
            )
            .expect("GPU vanity generation should succeed")
            .expect("GPU vanity generation should return a wallet");
            let gpu_elapsed = gpu_started.elapsed();

            assert!(vanity_match(&cpu_result.wallet.address, &pattern));
            assert!(vanity_match(&gpu_result.wallet.address, &pattern));

            println!("suffix: {suffix}");
            println!(
                "  CPU elapsed: {:?}, attempts: {}, address: {}",
                cpu_elapsed, cpu_result.attempts, cpu_result.wallet.address
            );
            println!(
                "  GPU elapsed: {:?}, attempts: {}, address: {}",
                gpu_elapsed, gpu_result.attempts, gpu_result.wallet.address
            );
        }
    }

    #[test]
    #[ignore = "manual GPU smoke test"]
    fn generates_single_gpu_wallet_smoke() {
        let pattern = VanityPattern::Suffix("q".to_string());
        let result = generate_vanity_btcc_wallet_with_stats_cancellable_mode(
            pattern.clone(),
            1,
            Arc::new(AtomicBool::new(false)),
            VanityComputeMode::Gpu(VanityGpuConfig::default()),
        )
        .expect("GPU vanity generation should succeed")
        .expect("GPU vanity generation should return a wallet");

        assert!(vanity_match(&result.wallet.address, &pattern));

        println!("GPU address: {}", result.wallet.address);
        println!("GPU wif: {}", result.wallet.private_key_wif);
        println!("GPU attempts: {}", result.attempts);
    }
}
