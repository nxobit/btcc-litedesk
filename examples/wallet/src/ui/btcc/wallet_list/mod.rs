use crate::theme::{
    load_show_total_balance, load_show_wallet_addresses, load_show_wallet_balances,
    save_show_total_balance, save_show_wallet_addresses, save_show_wallet_balances,
};
use crate::ui::palette;
use bip39::{Language, Mnemonic};
use btcc_litedesk::{
    db::btcc_wallet::{
        BtccWalletRecord, BtccWalletSecrets,
        btcc_wallet_password_exists_blocking, create_btcc_wallet_password_blocking,
        create_encrypted_btcc_wallet_blocking, decrypt_btcc_wallet_secrets_blocking,
        delete_btcc_wallet_blocking, list_btcc_wallets_blocking,
        migrate_btcc_wallet_encryption_blocking, search_btcc_wallets_blocking,
        update_btcc_wallet_balance_blocking, update_btcc_wallet_blocking,
        verify_btcc_wallet_password_blocking,
    },
    wallet::{
        BitcoinWallet, BtccExplorerClient, DEFAULT_BTCC_EXPLORER_API, generate_btcc_wallet,
        wallet_from_mnemonic, wallet_from_private_key_wif,
    },
};
use gpui::{img, prelude::FluentBuilder, *};
use gpui_component::{
    ActiveTheme, IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    clipboard::Clipboard,
    h_flex,
    input::{Input, InputEvent, InputState},
    menu::DropdownMenu,
    scroll::ScrollableElement,
    v_flex,
};
use image::Luma;
use qrcode::QrCode;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

mod render;
mod widgets;

use self::widgets::*;


static GLOBAL_VAULT_PASSWORD: Mutex<Option<String>> = Mutex::new(None);


pub fn get_global_vault_password() -> Option<String> {
    GLOBAL_VAULT_PASSWORD.lock().ok()?.clone()
}

#[derive(Clone, Debug)]
pub enum BtccWalletListEvent {
    OpenTransfer { id: i64, address: String },
    ActiveCountChanged { count: usize },
}

#[derive(Clone, Debug)]
enum WalletAction {
    History { address: String },
    Export { id: i64 },
    Edit { id: i64 },
    Delete { id: i64 },
}

impl gpui::Action for WalletAction {
    fn name(&self) -> &'static str {
        match self {
            WalletAction::History { .. } => "wallet-history",
            WalletAction::Export { .. } => "wallet-export",
            WalletAction::Edit { .. } => "wallet-edit",
            WalletAction::Delete { .. } => "wallet-delete",
        }
    }

    fn name_for_type() -> &'static str {
        "WalletAction"
    }

    fn build(_value: serde_json::Value) -> anyhow::Result<Box<dyn gpui::Action>> {
        Err(anyhow::anyhow!("WalletAction cannot be deserialized"))
    }

    fn boxed_clone(&self) -> Box<dyn gpui::Action> {
        Box::new(self.clone())
    }

    fn partial_eq(&self, action: &dyn gpui::Action) -> bool {
        action
            .as_any()
            .downcast_ref::<Self>()
            .map(|a| std::mem::discriminant(self) == std::mem::discriminant(a))
            .unwrap_or(false)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EditorMode {
    CreateMnemonic,
    VerifyMnemonic,
    ImportMnemonic,
    EditExisting,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ImportMode {
    Mnemonic,
    Wif,
}

const REFRESH_COOLDOWN: Duration = Duration::from_secs(8);
const REFRESH_BATCH_SIZE: usize = 8;
const REFRESH_BATCH_PAUSE: Duration = Duration::from_millis(250);

pub struct BtccWalletListPage {
    wallets: Vec<BtccWalletRecord>,
    display_wallets: Vec<BtccWalletRecord>,
    selected_id: Option<i64>,
    editor_open: bool,
    editor_mode: EditorMode,
    generated_wallet: Option<BitcoinWallet>,
    verify_positions: Vec<usize>,
    vault_initialized: bool,
    vault_unlocked: bool,
    export_wallet_id: Option<i64>,
    delete_confirm_wallet_id: Option<i64>,
    exported_secrets: Option<BtccWalletSecrets>,
    receive_wallet_address: Option<String>,
    receive_wallet_name: Option<String>,
    receive_qr_path: Option<PathBuf>,
    receive_qr_error: Option<String>,
    search_input: Entity<InputState>,
    name_input: Entity<InputState>,
    address_input: Entity<InputState>,
    note_input: Entity<InputState>,
    import_mnemonic_input: Entity<InputState>,
    import_wif_input: Entity<InputState>,
    vault_password_input: Entity<InputState>,
    vault_confirm_input: Entity<InputState>,
    unlock_password_input: Entity<InputState>,
    action_password_input: Entity<InputState>,
    verify_inputs: Vec<Entity<InputState>>,
    import_mode: ImportMode,
    show_total_balance: bool,
    show_wallet_addresses: bool,
    show_wallet_balances: bool,
    loading: bool,
    queued_refresh: bool,
    last_refresh_started_at: Option<Instant>,
    status: Option<String>,
    error: Option<String>,
    _task: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl BtccWalletListPage {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("搜索钱包名称或完整钱包地址")
                .default_value("")
        });
        let name_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("钱包名称，例如主钱包")
                .default_value("")
        });
        let address_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("BTCC 地址，例如 cc1q... 开头")
                .default_value("")
        });
        let note_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("备注，可选")
                .default_value("")
        });
        let import_mnemonic_input = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .rows(4)
                .placeholder("粘贴 12/24 个英文助记词，用空格分隔")
                .default_value("")
        });
        let import_wif_input = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .rows(3)
                .placeholder("粘贴 WIF 私钥，例如 L... 或 K... 开头")
                .default_value("")
        });
        let vault_password_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("至少 6 位，必须包含字母和数字")
                .masked(true)
                .default_value("")
        });
        let vault_confirm_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("再次输入钱包密码")
                .masked(true)
                .default_value("")
        });
        let unlock_password_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("输入钱包密码")
                .masked(true)
                .default_value("")
        });
        let action_password_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("输入钱包初始密码")
                .masked(true)
                .default_value("")
        });
        let verify_inputs = (0..3)
            .map(|_| {
                cx.new(|cx| {
                    InputState::new(window, cx)
                        .placeholder("输入对应单词")
                        .default_value("")
                })
            })
            .collect::<Vec<_>>();

        let mut _subscriptions = vec![
            cx.subscribe_in(&search_input, window, Self::on_input_event),
            cx.subscribe_in(&name_input, window, Self::on_input_event),
            cx.subscribe_in(&address_input, window, Self::on_input_event),
            cx.subscribe_in(&note_input, window, Self::on_input_event),
            cx.subscribe_in(
                &vault_password_input,
                window,
                Self::on_vault_setup_input_event,
            ),
            cx.subscribe_in(
                &vault_confirm_input,
                window,
                Self::on_vault_setup_input_event,
            ),
            cx.subscribe_in(&unlock_password_input, window, Self::on_input_event),
            cx.subscribe_in(&action_password_input, window, Self::on_input_event),
        ];
        for input in &verify_inputs {
            _subscriptions.push(cx.subscribe_in(input, window, Self::on_input_event));
        }
        _subscriptions.push(cx.subscribe_in(&import_mnemonic_input, window, Self::on_input_event));
        _subscriptions.push(cx.subscribe_in(&import_wif_input, window, Self::on_input_event));

        let mut page = Self {
            wallets: Vec::new(),
            display_wallets: Vec::new(),
            selected_id: None,
            editor_open: false,
            editor_mode: EditorMode::CreateMnemonic,
            generated_wallet: None,
            verify_positions: Vec::new(),
            vault_initialized: false,
            vault_unlocked: false,
            export_wallet_id: None,
            delete_confirm_wallet_id: None,
            exported_secrets: None,
            receive_wallet_address: None,
            receive_wallet_name: None,
            receive_qr_path: None,
            receive_qr_error: None,
            search_input,
            name_input,
            address_input,
            note_input,
            import_mnemonic_input,
            import_wif_input,
            vault_password_input,
            vault_confirm_input,
            unlock_password_input,
            action_password_input,
            verify_inputs,
            import_mode: ImportMode::Mnemonic,
            show_total_balance: load_show_total_balance(),
            show_wallet_addresses: load_show_wallet_addresses(),
            show_wallet_balances: load_show_wallet_balances(),
            loading: false,
            queued_refresh: false,
            last_refresh_started_at: None,
            status: None,
            error: None,
            _task: Task::ready(()),
            _subscriptions,
        };
        page.reload_vault_state();
        if page.vault_initialized && page.vault_unlocked {
            page.reload(cx);
        }
        page
    }

    fn on_input_event(
        &mut self,
        input: &Entity<InputState>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            InputEvent::Change => {
                if input == &self.search_input {
                    self.apply_search_query(cx);
                }
                self.status = None;
                self.error = None;
                cx.notify();
            }
            InputEvent::PressEnter { .. } => {
                if input == &self.unlock_password_input {
                    self.unlock_vault_password(window, cx);
                }
            }
            _ => {}
        }
    }

    fn on_vault_setup_input_event(
        &mut self,
        _: &Entity<InputState>,
        event: &InputEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(event, InputEvent::Change) {
            self.validate_vault_password_inputs(cx);
            cx.notify();
        }
    }

    fn validate_vault_password_inputs(&mut self, cx: &mut Context<Self>) {
        let password = self.vault_password_input.read(cx).text().to_string();
        let confirm = self.vault_confirm_input.read(cx).text().to_string();

        self.status = None;
        if !confirm.is_empty() && password != confirm {
            self.error = Some("Passwords do not match".to_string());
        } else {
            self.error = None;
        }
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        match list_btcc_wallets_blocking() {
            Ok(wallets) => {
                self.wallets = wallets;
                self.apply_search_results(cx);
                self.error = None;
            }
            Err(err) => {
                self.error = Some(err.to_string());
            }
        }
    }

    pub fn refresh_from_navigation(&mut self, cx: &mut Context<Self>) {
        if self.vault_initialized && self.vault_unlocked {
            self.reload(cx);
            self.emit_active_count(cx);
        }
        cx.notify();
    }

    fn apply_search_results(&mut self, cx: &mut Context<Self>) {
        let query = self
            .search_input
            .read_with(cx, |input, _| input.value().trim().to_string());

        self.display_wallets = if query.is_empty() {
            self.wallets.clone()
        } else {
            search_btcc_wallets_blocking(query).unwrap_or_default()
        };
    }

    fn apply_search_query(&mut self, cx: &mut Context<Self>) {
        self.apply_search_results(cx);
        cx.notify();
    }

    fn emit_active_count(&self, cx: &mut Context<Self>) {
        let count = self.wallets.iter().filter(|w| w.balance_sats > 0).count();
        cx.emit(BtccWalletListEvent::ActiveCountChanged { count });
    }

    fn toggle_total_balance_visibility(&mut self, cx: &mut Context<Self>) {
        self.show_total_balance = !self.show_total_balance;
        save_show_total_balance(self.show_total_balance);
        cx.notify();
    }

    fn toggle_wallet_addresses_visibility(&mut self, cx: &mut Context<Self>) {
        self.show_wallet_addresses = !self.show_wallet_addresses;
        save_show_wallet_addresses(self.show_wallet_addresses);
        cx.notify();
    }

    fn toggle_wallet_balances_visibility(&mut self, cx: &mut Context<Self>) {
        self.show_wallet_balances = !self.show_wallet_balances;
        save_show_wallet_balances(self.show_wallet_balances);
        cx.notify();
    }

    fn refresh_wallet_balances(&mut self, cx: &mut Context<Self>) {
        self.refresh_wallet_balances_with_policy(false, cx);
    }

    fn refresh_wallet_balances_with_policy(&mut self, force: bool, cx: &mut Context<Self>) {
        if self.wallets.is_empty() {
            self.reload(cx);
            self.emit_active_count(cx);
            cx.notify();
            return;
        }

        if self.loading {
            self.queued_refresh = true;
            self.status = None;
            self.error = None;
            cx.notify();
            return;
        }

        if !force
            && self
                .last_refresh_started_at
                .is_some_and(|started_at| started_at.elapsed() < REFRESH_COOLDOWN)
        {
            self.status = None;
            self.error = None;
            cx.notify();
            return;
        }

        let wallets = self.wallets.clone();
        self.loading = true;
        self.queued_refresh = false;
        self.last_refresh_started_at = Some(Instant::now());
        self.status = None;
        self.error = None;
        self._task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let client = BtccExplorerClient::default();
                    for (index, wallet) in wallets.into_iter().enumerate() {
                        let info = client.address_info(&wallet.address)?;
                        update_btcc_wallet_balance_blocking(
                            wallet.id,
                            info.confirmed_sats.min(i64::MAX as u64) as i64,
                            info.unconfirmed_sats,
                            info.utxo_total.min(i64::MAX as usize) as i64,
                        )?;
                        if (index + 1) % REFRESH_BATCH_SIZE == 0 {
                            thread::sleep(REFRESH_BATCH_PAUSE);
                        }
                    }
                    list_btcc_wallets_blocking()
                })
                .await;

            _ = this.update(cx, |this, cx| {
                this.loading = false;
                match result {
                    Ok(wallets) => {
                        this.wallets = wallets;
                        this.apply_search_results(cx);
                        this.error = None;
                        this.status = None;
                    }
                    Err(err) => {
                        this.error = Some(format!("Failed to refresh balances: {err}"));
                        this.status = None;
                    }
                }
                this.emit_active_count(cx);
                let should_refresh_again = this.queued_refresh && this.error.is_none();
                this.queued_refresh = false;
                if should_refresh_again {
                    this.refresh_wallet_balances_with_policy(true, cx);
                }
                cx.notify();
            });
        });
        cx.notify();
    }

    fn reload_vault_state(&mut self) {
        match btcc_wallet_password_exists_blocking() {
            Ok(exists) => {
                self.vault_initialized = exists;
                self.vault_unlocked = false;
                self.error = None;
            }
            Err(err) => {
                self.vault_initialized = false;
                self.error = Some(err.to_string());
            }
        }
    }

    fn unlock_vault_password(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let password = self.unlock_password_input.read(cx).text().to_string();
        if password.is_empty() {
            self.error = Some("Please enter wallet password".to_string());
            self.status = None;
            cx.notify();
            return;
        }

        match verify_btcc_wallet_password_blocking(password.clone()) {
            Ok(()) => {
                self.vault_unlocked = true;
                self.status = None;
                self.error = None;
                *GLOBAL_VAULT_PASSWORD.lock().unwrap() = Some(password.clone());
                let _ = migrate_btcc_wallet_encryption_blocking(password);
                self.unlock_password_input
                    .update(cx, |input, cx| input.set_value("", window, cx));
                self.reload(cx);
                self.emit_active_count(cx);
            }
            Err(_) => {
                self.vault_unlocked = false;
                self.status = None;
                self.error = Some("钱包密码错误".to_string());
            }
        }
        cx.notify();
    }

    fn create_vault_password(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let password = self.vault_password_input.read(cx).text().to_string();
        let confirm = self.vault_confirm_input.read(cx).text().to_string();
        if !is_strong_vault_password(&password) {
            self.error = None;
            self.status = None;
            cx.notify();
            return;
        }
        if password != confirm {
            self.error = Some("Passwords do not match".to_string());
            self.status = None;
            cx.notify();
            return;
        }

        match create_btcc_wallet_password_blocking(password.clone()) {
            Ok(()) => {
                self.vault_initialized = true;
                self.vault_unlocked = true;
                self.status = Some("Wallet password has been set".to_string());
                self.error = None;
                *GLOBAL_VAULT_PASSWORD.lock().unwrap() = Some(password);
                self.vault_password_input
                    .update(cx, |input, cx| input.set_value("", window, cx));
                self.vault_confirm_input
                    .update(cx, |input, cx| input.set_value("", window, cx));
            }
            Err(err) => {
                let err = err.to_string();
                self.error = if is_password_policy_error(&err) {
                    None
                } else {
                    Some(err)
                };
                self.status = None;
            }
        }
        cx.notify();
    }

    fn open_create_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match generate_btcc_wallet() {
            Ok(wallet) => {
                self.selected_id = None;
                self.editor_open = true;
                self.editor_mode = EditorMode::CreateMnemonic;
                self.verify_positions.clear();
                self.generated_wallet = Some(wallet);
                self.name_input
                    .update(cx, |input, cx| input.set_value("BTCC 钱包", window, cx));
                self.address_input
                    .update(cx, |input, cx| input.set_value("", window, cx));
                self.note_input
                    .update(cx, |input, cx| input.set_value("", window, cx));
                for input in &self.verify_inputs {
                    input.update(cx, |input, cx| input.set_value("", window, cx));
                }
                self.status = Some("已生成新助记词，请先手抄保存".to_string());
                self.error = None;
            }
            Err(err) => {
                self.error = Some(format!("生成钱包失败: {err}"));
                self.status = None;
            }
        }
        cx.notify();
    }

    fn open_import_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_id = None;
        self.editor_open = true;
        self.editor_mode = EditorMode::ImportMnemonic;
        self.import_mode = ImportMode::Mnemonic;
        self.generated_wallet = None;
        self.verify_positions.clear();
        self.name_input
            .update(cx, |input, cx| input.set_value("导入钱包", window, cx));
        self.import_mnemonic_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.import_wif_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.note_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.action_password_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.status = None;
        self.error = None;
        cx.notify();
    }

    fn open_edit_editor(&mut self, id: i64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(wallet) = self.wallets.iter().find(|wallet| wallet.id == id).cloned() else {
            return;
        };
        self.selected_id = Some(id);
        self.editor_open = true;
        self.editor_mode = EditorMode::EditExisting;
        self.generated_wallet = None;
        self.verify_positions.clear();
        self.name_input
            .update(cx, |input, cx| input.set_value(wallet.name, window, cx));
        self.address_input
            .update(cx, |input, cx| input.set_value(wallet.address, window, cx));
        self.note_input
            .update(cx, |input, cx| input.set_value(wallet.note, window, cx));
        self.status = None;
        self.error = None;
        cx.notify();
    }

    fn set_import_mode(
        &mut self,
        mode: ImportMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.import_mode == mode {
            return;
        }

        self.import_mode = mode;
        self.error = None;
        self.status = None;

        match mode {
            ImportMode::Mnemonic => {
                self.import_wif_input
                    .update(cx, |input, cx| input.set_value("", window, cx));
            }
            ImportMode::Wif => {
                self.import_mnemonic_input
                    .update(cx, |input, cx| input.set_value("", window, cx));
            }
        }

        cx.notify();
    }

    fn close_editor(&mut self, cx: &mut Context<Self>) {
        self.editor_open = false;
        self.selected_id = None;
        self.generated_wallet = None;
        self.verify_positions.clear();
        self.error = None;
        cx.notify();
    }

    fn start_verify_generated_wallet(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(wallet) = &self.generated_wallet else {
            self.error = Some("没有可验证的钱包，请重新创建".to_string());
            self.status = None;
            cx.notify();
            return;
        };
        let words = mnemonic_words(wallet);
        if words.len() < 3 {
            self.error = Some("助记词数量不足，无法验证".to_string());
            self.status = None;
            cx.notify();
            return;
        }

        self.verify_positions = choose_verify_positions(words.len());
        for input in &self.verify_inputs {
            input.update(cx, |input, cx| input.set_value("", window, cx));
        }
        self.editor_mode = EditorMode::VerifyMnemonic;
        self.status = Some("请输入随机抽取的 3 个助记词".to_string());
        self.error = None;
        cx.notify();
    }

    fn verify_and_save_generated_wallet(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(wallet) = self.generated_wallet.clone() else {
            self.error = Some("没有可保存的钱包，请重新创建".to_string());
            self.status = None;
            cx.notify();
            return;
        };

        let words = mnemonic_words(&wallet);
        for (index, position) in self.verify_positions.iter().enumerate() {
            let expected = words.get(*position).copied().unwrap_or_default();
            let actual = self.verify_inputs[index].read(cx).text().to_string();
            let actual = actual.trim();
            if actual.is_empty() {
                self.error = Some(format!("Please enter word #{}", position + 1));
                self.status = None;
                cx.notify();
                return;
            }
            if !actual.eq_ignore_ascii_case(expected) {
                self.error = Some(format!("Word #{} is incorrect", position + 1));
                self.status = None;
                cx.notify();
                return;
            }
        }

        let name = self.name_input.read(cx).text().to_string();
        let note = self.note_input.read(cx).text().to_string();

        if name.chars().count() > 7 {
            self.error = Some("Wallet name must be 7 characters or fewer".to_string());
            self.status = None;
            cx.notify();
            return;
        }
        if note.chars().count() > 50 {
            self.error = Some("Note must be 50 characters or fewer".to_string());
            self.status = None;
            cx.notify();
            return;
        }

        let password = self.action_password_input.read(cx).text().to_string();
        if password.is_empty() {
            self.error = Some("Please enter the wallet password".to_string());
            self.status = None;
            cx.notify();
            return;
        }
        if password.chars().count() < 6 {
            self.error = Some("Password must be at least 6 characters".to_string());
            self.status = None;
            cx.notify();
            return;
        }

        let result = create_encrypted_btcc_wallet_blocking(
            name,
            wallet.address.clone(),
            wallet.derivation_path.clone(),
            "generated".to_string(),
            wallet.public_key.to_string(),
            note,
            wallet.mnemonic.clone(),
            wallet.private_key_wif.clone(),
            password,
        );

        match result {
            Ok(_) => {
                self.reload(cx);
                self.editor_open = false;
                self.generated_wallet = None;
                self.verify_positions.clear();
                self.action_password_input
                    .update(cx, |input, cx| input.set_value("", window, cx));
                self.status =
                    Some("Verification passed. Wallet saved in encrypted form.".to_string());
                self.error = None;
            }
            Err(err) => {
                self.error = Some(err.to_string());
                self.status = None;
            }
        }
        cx.notify();
    }

    fn import_and_save_wallet(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.error = None;
        self.status = None;

        let name = self.name_input.read(cx).text().to_string();
        let note = self.note_input.read(cx).text().to_string();

        if name.chars().count() > 7 {
            self.error = Some("Wallet name must be 7 characters or fewer".to_string());
            self.status = None;
            cx.notify();
            return;
        }
        if note.chars().count() > 50 {
            self.error = Some("Note must be 50 characters or fewer".to_string());
            self.status = None;
            cx.notify();
            return;
        }

        let password = self.action_password_input.read(cx).text().to_string();
        if password.is_empty() {
            self.error = Some("Please enter the wallet password".to_string());
            self.status = None;
            cx.notify();
            return;
        }
        if password.chars().count() < 6 {
            self.error = Some("Password must be at least 6 characters".to_string());
            self.status = None;
            cx.notify();
            return;
        }

        let (wallet, source_type) = match self.import_mode {
            ImportMode::Mnemonic => {
                let text = self.import_mnemonic_input.read(cx).text().to_string();
                let words: Vec<&str> = text.split_whitespace().collect();
                if words.len() != 12 && words.len() != 24 {
                    self.error = Some(format!(
                        "Mnemonic must contain 12 or 24 words. Current count: {}",
                        words.len()
                    ));
                    self.status = None;
                    cx.notify();
                    return;
                }

                for (index, word) in words.iter().enumerate() {
                    let normalized = word.trim().to_ascii_lowercase();
                    if normalized.is_empty() || !normalized.chars().all(|ch| ch.is_ascii_alphabetic()) {
                        self.error = Some(format!("Mnemonic word #{} is invalid", index + 1));
                        self.status = None;
                        cx.notify();
                        return;
                    }
                }

                let mnemonic = words.join(" ");
                if Mnemonic::parse_in_normalized(Language::English, &mnemonic).is_err() {
                    self.error = Some("Mnemonic is invalid".to_string());
                    self.status = None;
                    cx.notify();
                    return;
                }

                match wallet_from_mnemonic(&mnemonic) {
                    Ok(wallet) => (wallet, "mnemonic".to_string()),
                    Err(err) => {
                        self.error = Some(format!("助记词导入失败: {err}"));
                        self.status = None;
                        cx.notify();
                        return;
                    }
                }
            }
            ImportMode::Wif => {
                let wif = self.import_wif_input.read(cx).text().to_string();
                let wif = wif.trim().to_string();
                if wif.is_empty() {
                    self.error = Some("Please enter a WIF private key".to_string());
                    self.status = None;
                    cx.notify();
                    return;
                }

                match wallet_from_private_key_wif(&wif) {
                    Ok(wallet) => (wallet, "wif".to_string()),
                    Err(err) => {
                        self.error = Some(format!("WIF 私钥无效: {err}"));
                        self.error = Some(format!("WIF private key is invalid: {err}"));
                        cx.notify();
                        return;
                    }
                }
            }
        };

        let result = create_encrypted_btcc_wallet_blocking(
            name,
            wallet.address.clone(),
            wallet.derivation_path.clone(),
            source_type,
            wallet.public_key.to_string(),
            note,
            wallet.mnemonic.clone(),
            wallet.private_key_wif.clone(),
            password,
        );

        match result {
            Ok(_) => {
                self.reload(cx);
                self.editor_open = false;
                self.action_password_input
                    .update(cx, |input, cx| input.set_value("", window, cx));
                self.import_mnemonic_input
                    .update(cx, |input, cx| input.set_value("", window, cx));
                self.import_wif_input
                    .update(cx, |input, cx| input.set_value("", window, cx));
                self.status = Some("Wallet import succeeded".to_string());
                self.error = None;
            }
            Err(err) => {
                let message = err.to_string();
                self.error = Some(if message.contains("password is incorrect") {
                    "Wallet password is incorrect".to_string()
                } else {
                    message
                });
                self.status = None;
            }
        }
        cx.notify();
    }

    fn save_existing_wallet(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.selected_id else {
            self.error = Some("Please select a wallet first".to_string());
            self.status = None;
            cx.notify();
            return;
        };

        let name = self.name_input.read(cx).text().to_string();
        let note = self.note_input.read(cx).text().to_string();

        if name.chars().count() > 7 {
            self.error = Some("Wallet name must be 7 characters or fewer".to_string());
            self.status = None;
            cx.notify();
            return;
        }
        if note.chars().count() > 50 {
            self.error = Some("Note must be 50 characters or fewer".to_string());
            self.status = None;
            cx.notify();
            return;
        }

        match update_btcc_wallet_blocking(id, name, note) {
            Ok(()) => {
                self.reload(cx);
                self.editor_open = false;
                self.selected_id = None;
                self.status = None;
                self.error = None;
            }
            Err(err) => {
                self.error = Some(err.to_string());
                self.status = None;
            }
        }
        cx.notify();
    }

    fn request_delete_wallet(&mut self, id: i64, cx: &mut Context<Self>) {
        let Some(wallet) = self.wallets.iter().find(|wallet| wallet.id == id) else {
            self.error = Some("????????".to_string());
            self.status = None;
            cx.notify();
            return;
        };

        if wallet.balance_sats > 0 {
            self.delete_confirm_wallet_id = Some(id);
            self.error = None;
            self.status = None;
            cx.notify();
            return;
        }

        self.delete_wallet(id, cx);
    }

    fn confirm_delete_wallet(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.delete_confirm_wallet_id.take() else {
            return;
        };
        self.delete_wallet(id, cx);
    }

    fn cancel_delete_wallet(&mut self, cx: &mut Context<Self>) {
        self.delete_confirm_wallet_id = None;
        cx.notify();
    }

    fn delete_wallet(&mut self, id: i64, cx: &mut Context<Self>) {
        self.error = None;
        self.status = None;
        self.delete_confirm_wallet_id = None;
        self._task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { delete_btcc_wallet_blocking(id) })
                .await;

            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(result) => {
                        this.reload(cx);
                        this.editor_open = false;
                        this.selected_id = None;
                        this.generated_wallet = None;
                        this.verify_positions.clear();
                        this.export_wallet_id = None;
                        this.exported_secrets = None;
                        this.receive_wallet_address = None;
                        this.receive_wallet_name = None;
                        this.receive_qr_path = None;
                        this.receive_qr_error = None;
                        this.emit_active_count(cx);
                        let _ = result;
                        this.status = None;
                        this.error = None;
                    }
                    Err(err) => {
                        this.error = Some(err.to_string());
                        this.status = None;
                    }
                }
                cx.notify();
            });
        });
        cx.notify();
    }

    pub fn open_import_editor_from_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_import_editor(window, cx);
    }

    pub fn open_create_editor_from_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_create_editor(window, cx);
    }

    fn write_receive_qr_png(address: &str) -> anyhow::Result<PathBuf> {
        let code = QrCode::new(address.as_bytes())?;
        let image = code.render::<Luma<u8>>().min_dimensions(240, 240).build();

        let mut hasher = DefaultHasher::new();
        address.hash(&mut hasher);

        let output_dir = std::env::temp_dir().join("btcc-litedesk");
        fs::create_dir_all(&output_dir)?;

        let output_path = output_dir.join(format!("receive-{:016x}.png", hasher.finish()));
        image.save(&output_path)?;
        Ok(output_path)
    }

    fn open_transfer(&mut self, id: i64, address: String, cx: &mut Context<Self>) {
        cx.emit(BtccWalletListEvent::OpenTransfer { id, address });
    }

    fn open_history(&mut self, address: String, cx: &mut Context<Self>) {
        let base_url = DEFAULT_BTCC_EXPLORER_API.trim_end_matches("/api/v1");
        let url = format!("{base_url}/address/{address}");
        cx.open_url(&url);
        self.status = None;
        self.error = None;
        cx.notify();
    }

    fn open_receive(
        &mut self,
        id: i64,
        address: String,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let wallet_name = self
            .wallets
            .iter()
            .find(|w| w.id == id)
            .map(|w| w.name.clone());
        let (receive_qr_path, receive_qr_error) = match Self::write_receive_qr_png(&address) {
            Ok(path) => (Some(path), None),
            Err(err) => (None, Some(format!("Failed to generate QR code: {err}"))),
        };
        self.receive_wallet_address = Some(address);
        self.receive_wallet_name = wallet_name;
        self.receive_qr_path = receive_qr_path;
        self.receive_qr_error = receive_qr_error;
        self.status = None;
        self.error = None;
        cx.notify();
    }

    fn close_receive(&mut self, cx: &mut Context<Self>) {
        self.receive_wallet_address = None;
        self.receive_wallet_name = None;
        self.receive_qr_path = None;
        self.receive_qr_error = None;
        cx.notify();
    }

}

impl EventEmitter<BtccWalletListEvent> for BtccWalletListPage {}

fn render_overview_chip(
    label: &'static str,
    value: String,
    bg: Hsla,
    accent: Hsla,
    wide: bool,
    cx: &mut Context<BtccWalletListPage>,
) -> AnyElement {
    let app_theme = cx.theme().clone();
    v_flex()
        .w(if wide { px(320.) } else { px(168.) })
        .h(px(108.))
        .justify_between()
        .gap_2()
        .p_4()
        .rounded(px(12.))
        .border_1()
        .border_color(accent.opacity(0.18))
        .bg(bg)
        .child(
            div()
                .text_size(px(11.))
                .text_color(palette::muted(&app_theme))
                .child(label),
        )
        .child(
            div()
                .text_size(if wide { px(24.) } else { px(28.) })
                .font_semibold()
                .line_height(if wide { px(28.) } else { px(30.) })
                .font_family("monospace")
                .text_color(accent)
                .child(value),
        )
        .into_any_element()
}

impl Render for BtccWalletListPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .relative()
            .overflow_y_scrollbar()
            .gap_4()
            .on_action(cx.listener(
                |this: &mut Self,
                 action: &WalletAction,
                 window: &mut Window,
                 cx: &mut Context<Self>| {
                    match action {
                        WalletAction::History { address } => {
                            this.open_history(address.clone(), cx);
                        }
                        WalletAction::Export { id } => {
                            this.open_export(*id, window, cx);
                        }
                        WalletAction::Edit { id } => {
                            this.open_edit_editor(*id, window, cx);
                        }
                        WalletAction::Delete { id } => {
                            this.request_delete_wallet(*id, cx);
                        }
                    }
                },
            ))
            .child(self.render_header(cx))
            .when(!self.vault_initialized, |el| {
                el.child(self.render_vault_setup(cx))
            })
            .when(self.vault_initialized && !self.vault_unlocked, |el| {
                el.child(self.render_vault_unlock(cx))
            })
            .when(
                self.vault_initialized && self.vault_unlocked && self.editor_open,
                |el| el.child(self.render_editor(cx)),
            )
            .when(
                self.vault_initialized
                    && self.vault_unlocked
                    && !self.editor_open
                    && self.export_wallet_id.is_some(),
                |el| el.child(self.render_export_panel(cx)),
            )
            .when(
                self.vault_initialized && self.vault_unlocked && !self.editor_open,
                |el| el.child(self.render_table(cx)),
            )
            .when_some(
                self.vault_initialized
                    .then_some(())
                    .and(self.vault_unlocked.then_some(()))
                    .and(self.receive_wallet_address.as_ref().map(|_| ())),
                |el, _| el.child(self.render_receive_panel(cx).unwrap()),
            )
            .when_some(
                self.vault_initialized
                    .then_some(())
                    .and(self.vault_unlocked.then_some(()))
                    .and(self.delete_confirm_wallet_id.map(|_| ())),
                |el, _| el.child(self.render_delete_confirm(cx).unwrap()),
            )
    }
}
