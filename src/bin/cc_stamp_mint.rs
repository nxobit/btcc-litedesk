use anyhow::{Context, anyhow};
use btcc_litedesk::wallet::{
    BtccStampMintRequest, broadcast_cc_stamp_mint_transaction_blocking,
};

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let request = parse_args(&args)?;

    let (result, broadcast) = broadcast_cc_stamp_mint_transaction_blocking(&request)?;

    println!("from_address={}", result.mint_wallet.record.address);
    println!("to_address={}", request.to_address);
    println!("rawtx={}", result.signed.rawtx);
    println!("txid={}", broadcast.txid);

    Ok(())
}

fn parse_args(args: &[String]) -> anyhow::Result<BtccStampMintRequest> {
    let mut from_address = None;
    let mut to_address = None;
    let mut password = None;
    let mut amount_sats = None;
    let mut fee_rate_sat_vb = None;
    let mut stamp = None;

    let mut index = 0usize;
    while index < args.len() {
        let key = &args[index];
        let value = args
            .get(index + 1)
            .ok_or_else(|| anyhow!("missing value for argument: {key}"))?;

        match key.as_str() {
            "--from-address" => from_address = Some(value.clone()),
            "--to-address" => to_address = Some(value.clone()),
            "--password" => password = Some(value.clone()),
            "--amount-sats" => {
                amount_sats = Some(
                    value
                        .parse::<u64>()
                        .with_context(|| format!("invalid --amount-sats: {value}"))?,
                )
            }
            "--fee-rate" => {
                fee_rate_sat_vb = Some(
                    value
                        .parse::<u64>()
                        .with_context(|| format!("invalid --fee-rate: {value}"))?,
                )
            }
            "--stamp" => stamp = Some(value.clone()),
            _ => return Err(anyhow!("unknown argument: {key}")),
        }

        index += 2;
    }

    Ok(BtccStampMintRequest {
        address_prefix: from_address.ok_or_else(|| anyhow!("--from-address is required"))?,
        password: password.ok_or_else(|| anyhow!("--password is required"))?,
        to_address: to_address.ok_or_else(|| anyhow!("--to-address is required"))?,
        amount_sats: amount_sats.ok_or_else(|| anyhow!("--amount-sats is required"))?,
        fee_rate_sat_vb: fee_rate_sat_vb.ok_or_else(|| anyhow!("--fee-rate is required"))?,
        stamp: stamp.ok_or_else(|| anyhow!("--stamp is required"))?,
    })
}
