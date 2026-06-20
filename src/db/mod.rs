use anyhow::Context;
use rusqlite::Connection;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

pub mod btcc_wallet;

pub const DEFAULT_DATABASE_PATH: &str = "db/btcc_litedesk.sqlite";
static RUNTIME_DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

pub fn set_runtime_data_dir(path: PathBuf) {
    let _ = RUNTIME_DATA_DIR.set(path);
}

pub fn default_database_path() -> PathBuf {
    if let Some(base) = RUNTIME_DATA_DIR.get() {
        return base.join(DEFAULT_DATABASE_PATH);
    }
    PathBuf::from(DEFAULT_DATABASE_PATH)
}

pub fn open_default_connection() -> anyhow::Result<Connection> {
    open_connection(default_database_path())
}

pub fn open_connection(path: impl AsRef<Path>) -> anyhow::Result<Connection> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create database directory failed: {}", parent.display()))?;
    }

    let connection = Connection::open(path)
        .with_context(|| format!("open SQLite database failed: {}", path.display()))?;
    run_migrations(&connection)?;
    Ok(connection)
}

fn run_migrations(connection: &Connection) -> anyhow::Result<()> {
    connection
        .execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS btcc_wallets (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                address TEXT NOT NULL UNIQUE,
                network TEXT NOT NULL DEFAULT 'Bitcoin-Classic (BTCC)',
                derivation_path TEXT NOT NULL DEFAULT '',
                source_type TEXT NOT NULL DEFAULT 'watch'
                    CHECK (source_type IN ('generated', 'mnemonic', 'wif', 'watch')),
                public_key TEXT NOT NULL DEFAULT '',
                encrypted_mnemonic BLOB,
                encrypted_wif BLOB,
                balance_sats INTEGER NOT NULL DEFAULT 0,
                unconfirmed_sats INTEGER NOT NULL DEFAULT 0,
                utxo_count INTEGER NOT NULL DEFAULT 0,
                last_synced_at TEXT,
                note TEXT NOT NULL DEFAULT '',
                is_active INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_btcc_wallets_active_updated
                ON btcc_wallets(is_active, updated_at DESC);

            CREATE INDEX IF NOT EXISTS idx_btcc_wallets_address
                ON btcc_wallets(address);
            "#,
        )
        .context("run SQLite migrations failed")?;

    Ok(())
}
