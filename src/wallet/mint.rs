use crate::db::{
    self,
    btcc_wallet::{BtccWalletRecord, BtccWalletSecrets, decrypt_btcc_wallet_secrets},
};
use crate::wallet::{
    BtccBroadcastResult, BtccExplorerClient, BtccSignedTransaction, BtccUtxo, BtccWallet,
    validate_recipient_address, wallet_from_mnemonic, wallet_from_private_key_wif,
};
use anyhow::{Context, anyhow};
use bitcoin::{
    Amount, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Txid, Witness, absolute,
    consensus::encode::serialize_hex,
    ecdsa::Signature,
    hashes::Hash,
    script::PushBytesBuf,
    secp256k1::{Message, Secp256k1},
    sighash::{EcdsaSighashType, SighashCache},
    transaction,
};
use rusqlite::{Connection, params};
use std::str::FromStr;

const VAULT_ADDRESS: &str = "__btcc_wallet_vault__";
const DUST_SATS: u64 = 546;

#[derive(Clone, Debug)]
pub struct BtccMintWalletContext {
    pub record: BtccWalletRecord,
    pub secrets: BtccWalletSecrets,
    pub wallet: BtccWallet,
}

#[derive(Clone, Debug)]
pub struct BtccStampMintRequest {
    pub address_prefix: String,
    pub password: String,
    pub to_address: String,
    pub amount_sats: u64,
    pub fee_rate_sat_vb: u64,
    pub stamp: String,
}

#[derive(Clone, Debug)]
pub struct BtccStampMintResult {
    pub mint_wallet: BtccMintWalletContext,
    pub signed: BtccSignedTransaction,
    pub payload_json: String,
}

pub fn mint_wallet_by_address_prefix_blocking(
    address_prefix: &str,
    password: &str,
) -> anyhow::Result<BtccMintWalletContext> {
    let connection = db::open_default_connection()?;
    mint_wallet_by_address_prefix(&connection, address_prefix, password)
}

pub fn mint_wallet_by_address_prefix(
    connection: &Connection,
    address_prefix: &str,
    password: &str,
) -> anyhow::Result<BtccMintWalletContext> {
    let prefix = address_prefix.trim();
    anyhow::ensure!(!prefix.is_empty(), "address prefix cannot be empty");

    let like = format!("{prefix}%");
    let mut statement = connection
        .prepare(
            r#"
            SELECT
                id,
                name,
                address,
                network,
                derivation_path,
                source_type,
                public_key,
                balance_sats,
                unconfirmed_sats,
                utxo_count,
                last_synced_at,
                note,
                is_active,
                created_at,
                updated_at
            FROM btcc_wallets
            WHERE is_active = 1
              AND address <> ?1
              AND address LIKE ?2
            ORDER BY created_at DESC, id DESC
            LIMIT 2
            "#,
        )
        .context("prepare mint wallet lookup failed")?;

    let rows = statement
        .query_map(params![VAULT_ADDRESS, like], |row| {
            Ok(BtccWalletRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                address: row.get(2)?,
                network: row.get(3)?,
                derivation_path: row.get(4)?,
                source_type: row.get(5)?,
                public_key: row.get(6)?,
                balance_sats: row.get(7)?,
                unconfirmed_sats: row.get(8)?,
                utxo_count: row.get(9)?,
                last_synced_at: row.get(10)?,
                note: row.get(11)?,
                is_active: row.get::<_, i64>(12)? != 0,
                created_at: row.get(13)?,
                updated_at: row.get(14)?,
            })
        })
        .context("query mint wallet lookup failed")?
        .collect::<Result<Vec<_>, _>>()
        .context("read mint wallet lookup result failed")?;

    let record = match rows.as_slice() {
        [] => return Err(anyhow!("no wallet matched address prefix: {prefix}")),
        [record] => record.clone(),
        _ => return Err(anyhow!("multiple wallets matched address prefix: {prefix}")),
    };

    let secrets = decrypt_btcc_wallet_secrets(connection, record.id, password)
        .context("decrypt mint wallet secrets failed")?;

    let wallet = if !secrets.mnemonic.trim().is_empty() {
        wallet_from_mnemonic(&secrets.mnemonic).context("rebuild wallet from mnemonic failed")?
    } else {
        wallet_from_private_key_wif(&secrets.private_key_wif)
            .context("rebuild wallet from WIF failed")?
    };

    anyhow::ensure!(
        wallet.address == record.address,
        "decrypted wallet address does not match database record"
    );

    Ok(BtccMintWalletContext {
        record,
        secrets,
        wallet,
    })
}

pub fn build_cc_stamp_mint_transaction_blocking(
    request: &BtccStampMintRequest,
) -> anyhow::Result<BtccStampMintResult> {
    let mint_wallet =
        mint_wallet_by_address_prefix_blocking(&request.address_prefix, &request.password)?;
    let client = BtccExplorerClient::default();
    let address_info = client.address_info(&mint_wallet.wallet.address)?;
    let payload_json = build_cc_stamp_payload_json(&request.stamp)?;
    let signed = build_cc_stamp_mint_transaction_with_wallet(
        &mint_wallet.wallet,
        &address_info.utxos,
        request,
        &payload_json,
    )?;

    Ok(BtccStampMintResult {
        mint_wallet,
        signed,
        payload_json,
    })
}

pub fn broadcast_cc_stamp_mint_transaction_blocking(
    request: &BtccStampMintRequest,
) -> anyhow::Result<(BtccStampMintResult, BtccBroadcastResult)> {
    let result = build_cc_stamp_mint_transaction_blocking(request)?;
    let broadcast = BtccExplorerClient::default().broadcast_raw_transaction(&result.signed.rawtx)?;
    Ok((result, broadcast))
}

pub fn build_cc_stamp_mint_transaction_with_wallet(
    wallet: &BtccWallet,
    utxos: &[BtccUtxo],
    request: &BtccStampMintRequest,
    payload_json: &str,
) -> anyhow::Result<BtccSignedTransaction> {
    anyhow::ensure!(request.amount_sats > 0, "mint amount must be greater than 0");
    anyhow::ensure!(
        request.fee_rate_sat_vb > 0,
        "fee rate must be greater than 0"
    );

    let recipient_script = btcc_address_script(&request.to_address)?;
    let change_script = p2wpkh_script(wallet)?;
    let op_return_script = op_return_script(payload_json.as_bytes())?;

    let mut selected = Vec::new();
    let mut selected_total = 0u64;
    let mut fee_sats = 0u64;

    for utxo in utxos {
        selected_total = selected_total.saturating_add(utxo.value);
        selected.push(utxo.clone());
        fee_sats = estimate_fee_sats(selected.len(), 3, request.fee_rate_sat_vb);
        if selected_total >= request.amount_sats.saturating_add(fee_sats) {
            break;
        }
    }

    if selected_total < request.amount_sats.saturating_add(fee_sats) {
        return Err(anyhow!("insufficient balance for mint amount plus fee"));
    }

    let mut change_sats = selected_total - request.amount_sats - fee_sats;
    let output_count = if change_sats >= DUST_SATS { 3 } else { 2 };
    fee_sats = estimate_fee_sats(selected.len(), output_count, request.fee_rate_sat_vb);

    if selected_total < request.amount_sats.saturating_add(fee_sats) {
        return Err(anyhow!("insufficient balance after mint fee recalculation"));
    }

    change_sats = selected_total - request.amount_sats - fee_sats;
    if change_sats < DUST_SATS {
        fee_sats = fee_sats.saturating_add(change_sats);
        change_sats = 0;
    }

    let inputs = selected
        .iter()
        .map(|utxo| {
            let txid = Txid::from_str(&utxo.tx_hash).context("invalid UTXO txid")?;
            Ok(TxIn {
                previous_output: OutPoint {
                    txid,
                    vout: utxo.tx_pos,
                },
                script_sig: ScriptBuf::new(),
                sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
                witness: Witness::new(),
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    let mut outputs = vec![
        TxOut {
            value: Amount::from_sat(0),
            script_pubkey: op_return_script,
        },
        TxOut {
            value: Amount::from_sat(request.amount_sats),
            script_pubkey: recipient_script,
        },
    ];

    if change_sats > 0 {
        outputs.push(TxOut {
            value: Amount::from_sat(change_sats),
            script_pubkey: change_script.clone(),
        });
    }

    let mut tx = Transaction {
        version: transaction::Version::TWO,
        lock_time: absolute::LockTime::ZERO,
        input: inputs,
        output: outputs,
    };

    let secp = Secp256k1::new();
    let sighash_type = EcdsaSighashType::All;
    let mut sighasher = SighashCache::new(&mut tx);

    for (input_index, utxo) in selected.iter().enumerate() {
        let sighash = sighasher
            .p2wpkh_signature_hash(
                input_index,
                &change_script,
                Amount::from_sat(utxo.value),
                sighash_type,
            )
            .context("create mint p2wpkh sighash failed")?;
        let message = Message::from(sighash);
        let signature = secp.sign_ecdsa(&message, &wallet.secret_key);
        let signature = Signature {
            signature,
            sighash_type,
        };
        *sighasher
            .witness_mut(input_index)
            .context("missing mint transaction witness")? =
            Witness::p2wpkh(&signature, &wallet.secret_key.public_key(&secp));
    }

    let signed_tx = sighasher.into_transaction();

    Ok(BtccSignedTransaction {
        rawtx: serialize_hex(&signed_tx),
        total_input_sats: selected_total,
        send_sats: request.amount_sats,
        change_sats,
        fee_sats,
        input_count: selected.len(),
    })
}

fn build_cc_stamp_payload_json(stamp: &str) -> anyhow::Result<String> {
    let stamp = stamp.trim();
    anyhow::ensure!(!stamp.is_empty(), "stamp cannot be empty");
    anyhow::ensure!(!stamp.contains('"'), "stamp cannot contain double quotes");

    Ok(format!(
        r#"{{"p":"cc-stamp","op":"gen","s":"{}"}}"#,
        stamp
    ))
}

fn estimate_fee_sats(input_count: usize, output_count: usize, fee_rate_sat_vb: u64) -> u64 {
    let vbytes = 10 + input_count as u64 * 68 + output_count as u64 * 43;
    vbytes.saturating_mul(fee_rate_sat_vb)
}

fn p2wpkh_script(wallet: &BtccWallet) -> anyhow::Result<ScriptBuf> {
    let wpkh = wallet
        .public_key
        .wpubkey_hash()
        .context("compressed public key required")?;
    Ok(ScriptBuf::new_p2wpkh(&wpkh))
}

fn btcc_address_script(address: &str) -> anyhow::Result<ScriptBuf> {
    let address = validate_recipient_address(address)?;
    let (_, _, program) =
        bech32::segwit::decode(&address).context("invalid bech32 recipient address")?;

    let hash = bitcoin::WPubkeyHash::from_slice(&program).context("invalid witness program")?;
    Ok(ScriptBuf::new_p2wpkh(&hash))
}

fn op_return_script(payload: &[u8]) -> anyhow::Result<ScriptBuf> {
    let push = PushBytesBuf::try_from(payload.to_vec()).map_err(|_| anyhow!("mint payload is too large"))?;
    Ok(ScriptBuf::new_op_return(push))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::btcc_wallet::{create_btcc_wallet_password, create_encrypted_btcc_wallet};
    use crate::wallet::generate_btcc_wallet;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_db_path(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("btcc-litedesk-{label}-{nanos}.sqlite"))
    }

    fn bytes_to_hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn mint_wallet_loads_and_decrypts_by_prefix() {
        let db_path = temp_db_path("mint-wallet");
        let connection = db::open_connection(&db_path).unwrap();
        let password = "Mint123";

        create_btcc_wallet_password(&connection, password).unwrap();
        let wallet = generate_btcc_wallet().unwrap();

        create_encrypted_btcc_wallet(
            &connection,
            "Mint Wallet",
            &wallet.address,
            &wallet.derivation_path,
            "generated",
            &wallet.public_key.to_string(),
            "test note",
            &wallet.mnemonic,
            &wallet.private_key_wif,
            password,
        )
        .unwrap();

        let loaded = mint_wallet_by_address_prefix(&connection, &wallet.address[..18], password).unwrap();
        assert_eq!(loaded.record.address, wallet.address);
        assert_eq!(loaded.wallet.private_key_wif, wallet.private_key_wif);
        assert_eq!(loaded.secrets.mnemonic, wallet.mnemonic);

        let _ = connection.close();
        let _ = fs::remove_file(db_path);
    }

    #[test]
    fn builds_cc_stamp_mint_transaction_hex() {
        let mint_wallet = generate_btcc_wallet().unwrap();
        let recipient_wallet = generate_btcc_wallet().unwrap();
        let request = BtccStampMintRequest {
            address_prefix: mint_wallet.address[..18].to_string(),
            password: "Mint123".to_string(),
            to_address: recipient_wallet.address.clone(),
            amount_sats: 100_000,
            fee_rate_sat_vb: 2,
            stamp: "CC-STAMP-16198-0".to_string(),
        };
        let payload_json = build_cc_stamp_payload_json(&request.stamp).unwrap();
        let payload_hex = bytes_to_hex(payload_json.as_bytes());
        let utxo = BtccUtxo {
            tx_hash: "0000000000000000000000000000000000000000000000000000000000000002".to_string(),
            tx_pos: 0,
            height: 1,
            value: 300_000,
        };

        let signed =
            build_cc_stamp_mint_transaction_with_wallet(&mint_wallet, &[utxo], &request, &payload_json)
                .unwrap();

        assert!(signed.rawtx.starts_with("02000000"));
        assert!(signed.rawtx.contains(&payload_hex));
        assert_eq!(signed.input_count, 1);
        assert!(signed.fee_sats > 0);
        assert!(signed.change_sats > 0);
    }

    #[test]
    #[ignore = "live broadcast test; fill constants in this test before running"]
    fn prints_txid_after_live_broadcast() {
        const FROM_ADDRESS: &str = "cc1q6aw5pq56u66l5sjks9hd8z24cqt7qrq6whlprz";
        const PASSWORD: &str = "";
        const TO_ADDRESS: &str = "cc1qnp65ehp0p4ksec4dk7k5wuxwturhjgqx5qunft";
        const AMOUNT_SATS: u64 = 100_000;
        const FEE_RATE_SAT_VB: u64 = 2;
        const STAMP: &str = "CC-STAMP-1688-0";

        if PASSWORD.trim().is_empty() {
            eprintln!("skip live mint test: set PASSWORD in src/wallet/mint.rs");
            return;
        }
        if TO_ADDRESS.trim().is_empty() {
            eprintln!("skip live mint test: set TO_ADDRESS in src/wallet/mint.rs");
            return;
        }

        let request = BtccStampMintRequest {
            address_prefix: FROM_ADDRESS.to_string(),
            password: PASSWORD.to_string(),
            to_address: TO_ADDRESS.to_string(),
            amount_sats: AMOUNT_SATS,
            fee_rate_sat_vb: FEE_RATE_SAT_VB,
            stamp: STAMP.to_string(),
        };

        let (result, broadcast) = broadcast_cc_stamp_mint_transaction_blocking(&request).unwrap();
        println!("from_address={}", result.mint_wallet.record.address);
        println!("rawtx={}", result.signed.rawtx);
        println!("txid={}", broadcast.txid);
    }
}
