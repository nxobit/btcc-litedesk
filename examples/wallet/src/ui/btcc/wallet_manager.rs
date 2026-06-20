use crate::ui::btcc::wallet_list::get_global_vault_password;
use crate::ui::palette;
use btcc_litedesk::{
    db::btcc_wallet::{decrypt_btcc_wallet_secrets_blocking, update_btcc_wallet_balance_blocking},
    wallet::{
        BTCC_NATIVE_SEGWIT_PATH, BitcoinWallet, BtccAddressInfo, BtccExplorerClient,
        BtccSendRequest, BtccSignedTransaction, btcc_to_sats, build_signed_transaction,
        validate_recipient_address, wallet_from_mnemonic, wallet_from_private_key_wif,
    },
};
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    ActiveTheme, IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputEvent, InputState},
    scroll::ScrollableElement,
    v_flex,
};

pub enum WalletManagerEvent {
    BackToWalletList,
}

impl EventEmitter<WalletManagerEvent> for WalletGeneratorPage {}

pub struct WalletGeneratorPage {
    wallet: Option<BitcoinWallet>,
    transfer_wallet_id: Option<i64>,
    transfer_address: Option<String>,
    balance: Option<BtccAddressInfo>,
    import_input: Entity<InputState>,
    to_input: Entity<InputState>,
    amount_input: Entity<InputState>,
    fee_rate_input: Entity<InputState>,
    password_input: Entity<InputState>,
    vault_password: Option<String>,
    password_dialog_open: bool,
    sign_dialog_open: bool,
    loading: bool,
    import_dialog_open: bool,
    export_dialog_open: bool,
    status: Option<String>,
    error: Option<String>,
    copied: Option<String>,
    last_signed: Option<BtccSignedTransaction>,
    last_txid: Option<String>,
    _task: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl WalletGeneratorPage {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let import_input = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .rows(4)
                .placeholder("粘贴 12/24 个英文助记词，或粘贴 WIF 私钥")
                .default_value("")
        });
        let to_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("收款地址，当前支持 cc1q 开头地址")
                .default_value("")
        });
        let amount_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("发送数量，例如 0.1")
                .default_value("")
        });
        let fee_rate_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("费率 sat/vB")
                .default_value("2")
        });
        let password_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("输入钱包密码")
                .masked(true)
                .default_value("")
        });
        let _subscriptions = vec![
            cx.subscribe_in(&import_input, window, Self::on_input_event),
            cx.subscribe_in(&to_input, window, Self::on_input_event),
            cx.subscribe_in(&amount_input, window, Self::on_input_event),
            cx.subscribe_in(&fee_rate_input, window, Self::on_input_event),
        ];

        Self {
            wallet: None,
            transfer_wallet_id: None,
            transfer_address: None,
            balance: None,
            import_input,
            to_input,
            amount_input,
            fee_rate_input,
            password_input,
            vault_password: None,
            password_dialog_open: false,
            sign_dialog_open: false,
            loading: false,
            import_dialog_open: false,
            export_dialog_open: false,
            status: None,
            error: None,
            copied: None,
            last_signed: None,
            last_txid: None,
            _task: Task::ready(()),
            _subscriptions,
        }
    }

    fn on_input_event(
        &mut self,
        _: &Entity<InputState>,
        event: &InputEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(event, InputEvent::Change) {
            self.status = None;
            self.error = None;
            self.last_signed = None;
            self.last_txid = None;
            self.sign_dialog_open = false;
            cx.notify();
        }
    }

    fn import_wallet(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let value = self.input_value(&self.import_input, cx);
        let wallet = if value.split_whitespace().count() >= 12 {
            wallet_from_mnemonic(&value)
        } else {
            wallet_from_private_key_wif(&value)
        };

        match wallet {
            Ok(wallet) => {
                self.transfer_address = Some(wallet.address.clone());
                self.wallet = Some(wallet);
                self.transfer_wallet_id = None;
                self.balance = None;
                self.status = Some("钱包已导入".to_string());
                self.error = None;
                self.last_signed = None;
                self.last_txid = None;
                self.import_dialog_open = false;
            }
            Err(err) => self.error = Some(format!("导入失败：{err}")),
        }
        cx.notify();
    }

    pub fn open_import_dialog(&mut self, cx: &mut Context<Self>) {
        self.import_dialog_open = true;
        self.status = None;
        self.error = None;
        cx.notify();
    }

    pub fn set_transfer_wallet(
        &mut self,
        id: i64,
        address: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.transfer_wallet_id = Some(id);
        self.transfer_address = Some(address);
        self.balance = None;
        self.import_dialog_open = false;
        self.export_dialog_open = false;
        self.sign_dialog_open = false;
        self.last_signed = None;
        self.last_txid = None;
        self.status = None;
        self.error = None;
        self.copied = None;
        self.to_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.refresh_balance(cx);
    }

    fn clear_transfer_wallet(&mut self, cx: &mut Context<Self>) {
        self.transfer_wallet_id = None;
        self.transfer_address = None;
        self.balance = None;
        self.last_signed = None;
        self.last_txid = None;
        self.sign_dialog_open = false;
        self.status = None;
        self.error = None;
        cx.notify();
    }

    fn close_import_dialog(&mut self, cx: &mut Context<Self>) {
        self.import_dialog_open = false;
        cx.notify();
    }

    fn confirm_password(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let password = self.password_input.read(cx).text().to_string();
        if password.is_empty() {
            self.error = Some("请输入钱包密码".to_string());
            cx.notify();
            return;
        }
        self.vault_password = Some(password);
        self.password_dialog_open = false;
        self.password_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.error = None;
        self.status = Some("密码已就绪，请点击生成签名交易".to_string());
        cx.notify();
    }

    fn cancel_password(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.password_dialog_open = false;
        self.password_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.error = None;
        cx.notify();
    }

    fn open_export_dialog(&mut self, cx: &mut Context<Self>) {
        self.export_dialog_open = true;
        cx.notify();
    }

    fn close_export_dialog(&mut self, cx: &mut Context<Self>) {
        self.export_dialog_open = false;
        cx.notify();
    }

    fn refresh_balance(&mut self, cx: &mut Context<Self>) {
        let Some(address) = self
            .wallet
            .as_ref()
            .map(|wallet| wallet.address.clone())
            .or_else(|| self.transfer_address.clone())
        else {
            self.error = Some("请先导入钱包".to_string());
            cx.notify();
            return;
        };
        let wallet_id = self.transfer_wallet_id;

        self.loading = true;
        self.status = Some("正在查询 BTCC 余额".to_string());
        self.error = None;
        self._task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let info = BtccExplorerClient::default().address_info(&address)?;
                    if let Some(wallet_id) = wallet_id {
                        update_btcc_wallet_balance_blocking(
                            wallet_id,
                            info.confirmed_sats.min(i64::MAX as u64) as i64,
                            info.unconfirmed_sats,
                            info.utxo_total.min(i64::MAX as usize) as i64,
                        )?;
                    }
                    Ok::<_, anyhow::Error>(info)
                })
                .await;

            _ = this.update(cx, |this, cx| {
                this.loading = false;
                match result {
                    Ok(info) => {
                        this.balance = Some(info);
                        this.status = None;
                    }
                    Err(err) => this.error = Some(format!("查询余额失败：{err}")),
                }
                cx.notify();
            });
        });
        cx.notify();
    }

    fn sign_transaction(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let wallet = match self.wallet.clone() {
            Some(w) => Some(w),
            None => match self.transfer_wallet_id {
                Some(id) => {
                    let password = self
                        .vault_password
                        .clone()
                        .or_else(get_global_vault_password);
                    match password {
                        Some(password) => {
                            match decrypt_btcc_wallet_secrets_blocking(id, password.clone()) {
                                Ok(secrets) => {
                                    let w = if !secrets.mnemonic.is_empty() {
                                        wallet_from_mnemonic(&secrets.mnemonic)
                                    } else {
                                        wallet_from_private_key_wif(&secrets.private_key_wif)
                                    };
                                    match w {
                                        Ok(w) => Some(w),
                                        Err(err) => {
                                            self.error = Some(format!("解密后恢复钱包失败：{err}"));
                                            cx.notify();
                                            return;
                                        }
                                    }
                                }
                                Err(err) => {
                                    self.error = Some(format!("解密私钥失败：{err}"));
                                    cx.notify();
                                    return;
                                }
                            }
                        }
                        None => {
                            self.password_dialog_open = true;
                            self.error = Some("请先输入钱包密码".to_string());
                            cx.notify();
                            return;
                        }
                    }
                }
                None => {
                    self.error = Some(
                        "当前钱包还没有解锁签名，请先导入该地址对应的助记词或 WIF 私钥。"
                            .to_string(),
                    );
                    cx.notify();
                    return;
                }
            },
        };

        let Some(wallet) = wallet else {
            return;
        };

        let Some(balance) = self.balance.clone() else {
            self.error = Some("请先查询余额，获取可用 UTXO".to_string());
            cx.notify();
            return;
        };

        match self.build_send_request(cx) {
            Ok(request) => match build_signed_transaction(&wallet, &balance.utxos, &request) {
                Ok(signed) => {
                    self.status = Some(format!(
                        "交易已签名：输入 {} 个，手续费 {} sats，找零 {} sats",
                        signed.input_count, signed.fee_sats, signed.change_sats
                    ));
                    self.last_signed = Some(signed.clone());
                    self.last_txid = None;
                    self.error = None;
                    self.sign_dialog_open = true;
                    self.status = Some(format!(
                        "已签名：向 {} 发送 {} BTCC，手续费 {} sats，找零 {} sats",
                        request.to_address,
                        self.format_btcc_amount(signed.send_sats),
                        signed.fee_sats,
                        signed.change_sats
                    ));
                }
                Err(err) => self.error = Some(format!("签名失败：{err}")),
            },
            Err(err) => self.error = Some(err),
        }
        cx.notify();
    }

    fn broadcast_transaction(&mut self, cx: &mut Context<Self>) {
        let Some(rawtx) = self.last_signed.as_ref().map(|signed| signed.rawtx.clone()) else {
            self.error = Some("请先生成签名交易".to_string());
            cx.notify();
            return;
        };

        self.loading = true;
        self.status = Some("正在广播交易".to_string());
        self.error = None;
        self._task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    BtccExplorerClient::default().broadcast_raw_transaction(&rawtx)
                })
                .await;

            _ = this.update(cx, |this, cx| {
                this.loading = false;
                match result {
                    Ok(result) => {
                        this.last_txid = Some(result.txid.clone());
                        this.status = Some(format!("广播成功：{}", result.txid));
                    }
                    Err(err) => this.error = Some(format!("广播失败：{err}")),
                }
                cx.notify();
            });
        });
        cx.notify();
    }

    fn close_sign_dialog(&mut self, cx: &mut Context<Self>) {
        self.sign_dialog_open = false;
        cx.notify();
    }

    fn confirm_broadcast_from_dialog(&mut self, cx: &mut Context<Self>) {
        self.sign_dialog_open = false;
        self.broadcast_transaction(cx);
    }

    fn build_send_request(&self, cx: &mut Context<Self>) -> Result<BtccSendRequest, String> {
        let to_address = self.input_value(&self.to_input, cx);
        if to_address.trim().is_empty() {
            return Err("请输入收款地址".to_string());
        }
        if !to_address.trim().starts_with("cc1q") {
            return Err("收款地址必须是 cc1q 开头的钱包地址".to_string());
        }

        let to_address = validate_recipient_address(&to_address).map_err(|err| {
            let message = err.to_string();
            if message.contains("start with cc1q") || message.contains("cc bech32 prefix") {
                "收款地址必须是 cc1q 开头的钱包地址".to_string()
            } else {
                "收款地址格式无效，请输入有效的 cc1q 地址".to_string()
            }
        })?;
        let amount_text = self.input_value(&self.amount_input, cx);
        if amount_text.trim().is_empty() {
            return Err("请输入发送金额".to_string());
        }
        let amount_sats = btcc_to_sats(&amount_text).map_err(|err| format!("金额错误：{err}"))?;
        let fee_rate_sat_vb = self
            .input_value(&self.fee_rate_input, cx)
            .trim()
            .parse::<u64>()
            .map_err(|_| "费率必须是整数 sat/vB".to_string())?;

        Ok(BtccSendRequest {
            to_address,
            amount_sats,
            fee_rate_sat_vb,
        })
    }

    fn input_value(&self, input: &Entity<InputState>, cx: &mut Context<Self>) -> String {
        input.read_with(cx, |input, _| input.value()).to_string()
    }

    fn copy_value(&mut self, label: &'static str, value: String, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(value));
        self.copied = Some(format!("已复制 {label}"));
        cx.notify();
    }

    fn render_labeled_input(
        &self,
        label: &'static str,
        input: &Entity<InputState>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let app_theme = cx.theme().clone();
        v_flex()
            .gap_2()
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(palette::muted(&app_theme))
                    .child(label),
            )
            .child(Input::new(input).small())
            .into_any_element()
    }

    fn render_import_textarea(&self, cx: &mut Context<Self>) -> AnyElement {
        let app_theme = cx.theme().clone();
        v_flex()
            .gap_2()
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(palette::muted(&app_theme))
                    .child("助记词 / WIF 私钥"),
            )
            .child(Input::new(&self.import_input))
            .into_any_element()
    }

    fn render_field(
        &self,
        id: &'static str,
        label: &'static str,
        value: String,
        sensitive: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.render_field_with_copy(id, label, value.clone(), value, sensitive, cx)
    }

    fn render_field_with_copy(
        &self,
        id: &'static str,
        label: &'static str,
        display: String,
        copy_value: String,
        sensitive: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let app_theme = cx.theme().clone();

        v_flex()
            .gap_2()
            .p_3()
            .rounded(px(8.))
            .border_1()
            .border_color(palette::border(&app_theme))
            .bg(if sensitive {
                app_theme.danger.opacity(0.06)
            } else {
                app_theme.background
            })
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_semibold()
                            .text_color(if sensitive {
                                app_theme.danger
                            } else {
                                palette::muted(&app_theme)
                            })
                            .child(label),
                    )
                    .child(
                        Button::new(id)
                            .outline()
                            .xsmall()
                            .label("复制")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.copy_value(label, copy_value.clone(), cx);
                            })),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .p_3()
                    .rounded(px(6.))
                    .bg(app_theme.muted.opacity(0.10))
                    .text_size(px(13.))
                    .line_height(px(20.))
                    .font_family(app_theme.mono_font_family.clone())
                    .text_color(palette::text_strong(&app_theme))
                    .overflow_hidden()
                    .child(display),
            )
            .into_any_element()
    }

    fn render_txid_field(&self, txid: String, cx: &mut Context<Self>) -> AnyElement {
        let app_theme = cx.theme().clone();
        let copy_value = txid.clone();
        let open_value = txid.clone();

        v_flex()
            .gap_2()
            .p_3()
            .rounded(px(8.))
            .border_1()
            .border_color(palette::border(&app_theme))
            .bg(app_theme.background)
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_semibold()
                            .text_color(app_theme.success)
                            .child("发送成功 TXID"),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Button::new("copy-btcc-txid")
                                    .outline()
                                    .xsmall()
                                    .label("复制")
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.copy_value("TXID", copy_value.clone(), cx);
                                    })),
                            )
                            .child(
                                Button::new("open-btcc-txid")
                                    .outline()
                                    .xsmall()
                                    .icon(IconName::ExternalLink)
                                    .on_click(move |_, _, cx| {
                                        let base_url = "https://explorer.btc-classic.org";
                                        cx.open_url(&format!("{base_url}/tx/{open_value}"));
                                    }),
                            ),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .p_3()
                    .rounded(px(6.))
                    .bg(app_theme.muted.opacity(0.10))
                    .text_size(px(13.))
                    .line_height(px(20.))
                    .font_family(app_theme.mono_font_family.clone())
                    .text_color(palette::text_strong(&app_theme))
                    .overflow_hidden()
                    .child(txid),
            )
            .into_any_element()
    }

    fn format_btcc_amount(&self, sats: u64) -> String {
        let whole = sats / 100_000_000;
        let frac = sats % 100_000_000;
        format!("{whole}.{frac:08}")
    }

    fn render_summary_item(
        &self,
        label: &'static str,
        value: String,
        accent: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let app_theme = cx.theme().clone();

        v_flex()
            .gap_1()
            .min_w(px(140.))
            .p_3()
            .rounded(px(8.))
            .border_1()
            .border_color(palette::border(&app_theme))
            .bg(app_theme.background)
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(palette::muted(&app_theme))
                    .child(label),
            )
            .child(
                div()
                    .text_size(px(16.))
                    .font_semibold()
                    .text_color(if accent {
                        app_theme.primary
                    } else {
                        palette::text_strong(&app_theme)
                    })
                    .child(value),
            )
            .into_any_element()
    }

    fn render_wallet(&self, wallet: &BitcoinWallet, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_size(px(16.))
                            .font_semibold()
                            .text_color(palette::text_strong(&cx.theme().clone()))
                            .child("账户信息"),
                    )
                    .child(
                        Button::new("export-btcc-wallet")
                            .outline()
                            .small()
                            .label("导出钱包")
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.open_export_dialog(cx);
                            })),
                    ),
            )
            .child(self.render_field(
                "copy-wallet-network",
                "网络",
                wallet.network.clone(),
                false,
                cx,
            ))
            .child(self.render_field(
                "copy-wallet-address",
                "BTCC 地址",
                wallet.address.clone(),
                false,
                cx,
            ))
            .child(self.render_field(
                "copy-wallet-path",
                "派生路径",
                wallet.derivation_path.clone(),
                false,
                cx,
            ))
            .into_any_element()
    }

    fn render_balance(&self, cx: &mut Context<Self>) -> AnyElement {
        let app_theme = cx.theme().clone();
        let balance = self.balance.clone();
        let confirmed = balance
            .as_ref()
            .map(|b| format!("{:.8} BTCC", b.confirmed_btcc))
            .unwrap_or_else(|| "--".to_string());
        let unconfirmed = balance
            .as_ref()
            .map(|b| format!("{:.8} BTCC", b.unconfirmed_btcc))
            .unwrap_or_else(|| "--".to_string());
        let utxos = balance
            .as_ref()
            .map(|b| b.utxo_total.to_string())
            .unwrap_or_else(|| "--".to_string());
        let total = balance
            .as_ref()
            .map(|b| format!("{:.8} BTCC", b.total_btcc))
            .unwrap_or_else(|| "--".to_string());

        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(16.))
                            .font_semibold()
                            .text_color(palette::text_strong(&app_theme))
                            .child("余额概览"),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(palette::muted(&app_theme))
                            .child("explorer.btc-classic.org"),
                    ),
            )
            .child(
                h_flex()
                    .gap_3()
                    .child(
                        div()
                            .flex_1()
                            .child(self.metric_card("确认余额", confirmed, cx)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .child(self.metric_card("未确认", unconfirmed, cx)),
                    ),
            )
            .child(
                h_flex()
                    .gap_3()
                    .child(div().flex_1().child(self.metric_card("UTXO", utxos, cx)))
                    .child(div().flex_1().child(self.metric_card("总额", total, cx))),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .line_height(px(18.))
                    .text_color(palette::muted(&app_theme))
                    .child("查询余额后会刷新确认余额、未确认余额和可用 UTXO。"),
            )
            .into_any_element()
    }

    fn metric_card(
        &self,
        label: &'static str,
        value: String,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let app_theme = cx.theme().clone();
        v_flex()
            .gap_2()
            .h(px(88.))
            .p_4()
            .overflow_hidden()
            .rounded(px(10.))
            .border_1()
            .border_color(palette::border(&app_theme))
            .bg(app_theme.muted.opacity(0.05))
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(palette::muted(&app_theme))
                    .child(label),
            )
            .child(
                div()
                    .text_size(px(16.))
                    .font_semibold()
                    .line_height(px(20.))
                    .overflow_hidden()
                    .text_color(palette::text_strong(&app_theme))
                    .child(value),
            )
            .into_any_element()
    }

    fn render_import_dialog(&self, cx: &mut Context<Self>) -> AnyElement {
        let app_theme = cx.theme().clone();

        div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .bg(gpui::black().opacity(0.30))
            .flex()
            .items_center()
            .justify_center()
            .child(
                v_flex()
                    .w(px(560.))
                    .gap_4()
                    .p_4()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(palette::border(&app_theme))
                    .bg(app_theme.background)
                    .child(
                        h_flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(18.))
                                    .font_semibold()
                                    .text_color(palette::text_strong(&app_theme))
                                    .child("导入 BTCC 钱包"),
                            )
                            .child(
                                Button::new("close-import-wallet-dialog")
                                    .outline()
                                    .xsmall()
                                    .label("关闭")
                                .on_click(cx.listener(|this, _, _window, cx| {
                                        this.close_import_dialog(cx);
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .line_height(px(20.))
                            .text_color(palette::muted(&app_theme))
                            .child("粘贴 12/24 个英文助记词，或粘贴 WIF 私钥。导入后可查询余额并发送交易。"),
                    )
                    .child(self.render_import_textarea(cx))
                    .child(
                        h_flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("cancel-import-wallet")
                                    .outline()
                                    .small()
                                    .label("取消")
                                .on_click(cx.listener(|this, _, _window, cx| {
                                        this.close_import_dialog(cx);
                                    })),
                            )
                            .child(
                                Button::new("confirm-import-wallet")
                                    .primary()
                                    .small()
                                    .label("导入")
                                .on_click(cx.listener(|this, _, window, cx| {
                                        this.import_wallet(window, cx);
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn render_password_dialog(&self, cx: &mut Context<Self>) -> AnyElement {
        let app_theme = cx.theme().clone();

        div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .bg(gpui::black().opacity(0.30))
            .flex()
            .items_center()
            .justify_center()
            .child(
                v_flex()
                    .w(px(420.))
                    .gap_4()
                    .p_4()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(palette::border(&app_theme))
                    .bg(app_theme.background)
                    .child(
                        div()
                            .text_size(px(18.))
                            .font_semibold()
                            .text_color(palette::text_strong(&app_theme))
                            .child("输入钱包密码"),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .line_height(px(20.))
                            .text_color(palette::muted(&app_theme))
                            .child("输入密码以解密数据库中存储的私钥进行签名。"),
                    )
                    .child(Input::new(&self.password_input))
                    .child(
                        h_flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("cancel-password-dialog")
                                    .outline()
                                    .small()
                                    .label("取消")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.cancel_password(window, cx);
                                    })),
                            )
                            .child(
                                Button::new("confirm-password-dialog")
                                    .primary()
                                    .small()
                                    .label("确认")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.confirm_password(window, cx);
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn render_sign_dialog(
        &self,
        signed: &BtccSignedTransaction,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let app_theme = cx.theme().clone();
        let recipient_address = self.input_value(&self.to_input, cx);

        div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .bg(gpui::black().opacity(0.30))
            .flex()
            .items_center()
            .justify_center()
            .child(
                v_flex()
                    .w(px(820.))
                    .max_h(px(760.))
                    .gap_5()
                    .p_5()
                    .rounded(px(12.))
                    .border_1()
                    .border_color(palette::border(&app_theme))
                    .bg(app_theme.background)
                    .child(
                        h_flex()
                            .items_center()
                            .justify_between()
                            .gap_4()
                            .child(
                                v_flex()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_size(px(20.))
                                            .font_semibold()
                                            .text_color(palette::text_strong(&app_theme))
                                            .child("签名交易预览"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(palette::muted(&app_theme))
                                            .child("请先核对转账信息，确认无误后再广播。"),
                                    ),
                            )
                            .child(
                                Button::new("close-sign-dialog")
                                    .outline()
                                    .xsmall()
                                    .label("关闭")
                                .on_click(cx.listener(|this, _, _window, cx| {
                                        this.close_sign_dialog(cx);
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .max_h(px(530.))
                            .overflow_y_scrollbar()
                            .child(
                                v_flex()
                                    .gap_4()
                                    .child(
                                        div()
                                            .p_3()
                                            .rounded(px(10.))
                                            .border_1()
                                            .border_color(app_theme.danger.opacity(0.18))
                                            .bg(app_theme.danger.opacity(0.06))
                                            .child(
                                                div()
                                                    .text_size(px(12.))
                                                    .line_height(px(20.))
                                                    .text_color(app_theme.danger)
                                                    .child("风险提示：交易广播后将写入链上，无法撤回、无法取消。"),
                                            ),
                                    )
                                    .child(self.render_field_with_copy(
                                        "copy-btcc-sign-recipient-address",
                                        "收款地址",
                                        recipient_address.clone(),
                                        recipient_address,
                                        false,
                                        cx,
                                    ))
                                    .child(
                                        h_flex()
                                            .gap_3()
                                            .flex_wrap()
                                            .child(self.render_summary_item(
                                                "发送数量",
                                                format!(
                                                    "{} BTCC",
                                                    self.format_btcc_amount(signed.send_sats)
                                                ),
                                                true,
                                                cx,
                                            ))
                                            .child(self.render_summary_item(
                                                "手续费",
                                                format!("{} sats", signed.fee_sats),
                                                false,
                                                cx,
                                            ))
                                            .child(self.render_summary_item(
                                                "找零",
                                                format!("{} sats", signed.change_sats),
                                                false,
                                                cx,
                                            ))
                                            .child(self.render_summary_item(
                                                "总输入",
                                                format!("{} sats", signed.total_input_sats),
                                                false,
                                                cx,
                                            ))
                                            .child(self.render_summary_item(
                                                "使用 UTXO",
                                                format!("{} 个", signed.input_count),
                                                false,
                                                cx,
                                            )),
                                    )
                                    .child(self.render_field_with_copy(
                                        "copy-btcc-sign-rawtx",
                                        "已签名 Raw Transaction",
                                        signed.rawtx.clone(),
                                        signed.rawtx.clone(),
                                        true,
                                        cx,
                                    )),
                            ),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .justify_end()
                            .gap_3()
                            .child(
                                Button::new("close-sign-preview")
                                    .outline()
                                    .small()
                                    .label("返回修改")
                                .on_click(cx.listener(|this, _, _window, cx| {
                                        this.close_sign_dialog(cx);
                                    })),
                            )
                            .child(
                                Button::new("confirm-broadcast-from-sign-dialog")
                                    .primary()
                                    .small()
                                    .label("确认广播")
                                .on_click(cx.listener(|this, _, _window, cx| {
                                        this.confirm_broadcast_from_dialog(cx);
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn render_export_dialog(&self, wallet: &BitcoinWallet, cx: &mut Context<Self>) -> AnyElement {
        let app_theme = cx.theme().clone();

        div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .bg(gpui::black().opacity(0.30))
            .flex()
            .items_center()
            .justify_center()
            .child(
                v_flex()
                    .w(px(640.))
                    .max_h(px(720.))
                    .gap_4()
                    .p_4()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(palette::border(&app_theme))
                    .bg(app_theme.background)
                    .child(
                        h_flex()
                            .items_center()
                            .justify_between()
                            .child(
                                v_flex()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_size(px(18.))
                                            .font_semibold()
                                            .text_color(palette::text_strong(&app_theme))
                                            .child("导出 BTCC 钱包"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(app_theme.danger)
                                            .child("私钥和助记词泄露后资产无法追回，请只在安全环境中查看。"),
                                    ),
                            )
                            .child(
                                Button::new("close-export-wallet-dialog")
                                    .outline()
                                    .xsmall()
                                    .label("关闭")
                                .on_click(cx.listener(|this, _, _window, cx| {
                                        this.close_export_dialog(cx);
                                    })),
                            ),
                    )
                    .child(self.render_field(
                        "copy-export-wallet-address",
                        "BTCC 地址",
                        wallet.address.clone(),
                        false,
                        cx,
                    ))
                    .child(self.render_field(
                        "copy-export-private-key",
                        "WIF 私钥",
                        wallet.private_key_wif.clone(),
                        true,
                        cx,
                    ))
                    .when(!wallet.mnemonic.is_empty(), |parent| {
                        parent.child(self.render_field(
                            "copy-export-mnemonic",
                            "助记词",
                            wallet.mnemonic.clone(),
                            true,
                            cx,
                        ))
                    })
                    .child(
                        h_flex()
                            .justify_end()
                            .child(
                                Button::new("done-export-wallet")
                                    .primary()
                                    .small()
                                    .label("完成")
                                .on_click(cx.listener(|this, _, _window, cx| {
                                        this.close_export_dialog(cx);
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn render_empty_state(&self, cx: &mut Context<Self>) -> AnyElement {
        let app_theme = cx.theme().clone();

        div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .child(
                v_flex()
                    .w(px(520.))
                    .gap_4()
                    .items_center()
                    .p_6()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(palette::border(&app_theme))
                    .bg(app_theme.background)
                    .child(
                        div()
                            .text_size(px(20.))
                            .font_semibold()
                            .text_color(palette::text_strong(&app_theme))
                            .child("先导入一个 BTCC 钱包"),
                    )
                    .child(
                        div()
                            .text_center()
                            .text_size(px(13.))
                            .line_height(px(22.))
                            .text_color(palette::muted(&app_theme))
                            .child(
                                "导入助记词或 WIF 私钥后，页面会显示余额、UTXO 和发送交易表单。",
                            ),
                    )
                    .child(
                        Button::new("empty-import-wallet")
                            .primary()
                            .small()
                            .label("导入钱包")
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.open_import_dialog(cx);
                            })),
                    ),
            )
            .into_any_element()
    }

    fn render_send_panel(
        &self,
        signed: Option<BtccSignedTransaction>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let app_theme = cx.theme().clone();
        let recipient_address = self.input_value(&self.to_input, cx);

        v_flex()
            .gap_4()
            .min_h(px(320.))
            .p_4()
            .rounded(px(8.))
            .border_1()
            .border_color(palette::border(&app_theme))
            .bg(app_theme.background)
            .child(
                h_flex().items_start().justify_between().gap_3().child(
                    v_flex()
                        .gap_1()
                        .child(
                            div()
                                .text_size(px(16.))
                                .font_semibold()
                                .text_color(palette::text_strong(&app_theme))
                                .child("发送交易"),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(palette::muted(&app_theme))
                                .child("先生成签名交易，核对 raw transaction 后再广播。广播后不可撤销、不可退款！"),
                        ),
                ),
            )
            .child(self.render_labeled_input("收款地址", &self.to_input, cx))
            .child(
                h_flex()
                    .gap_3()
                    .child(div().flex_1().child(self.render_labeled_input(
                        "数量 BTCC",
                        &self.amount_input,
                        cx,
                    )))
                    .child(div().w(px(180.)).child(self.render_labeled_input(
                        "费率 sat/vB",
                        &self.fee_rate_input,
                        cx,
                    ))),
            )
            .when_some(self.error.clone(), |parent, error| {
                parent.child(
                    div()
                        .px_3()
                        .py_2()
                        .rounded(px(6.))
                        .bg(app_theme.danger.opacity(0.10))
                        .text_size(px(12.))
                        .text_color(app_theme.danger)
                        .child(error),
                )
            })
            .when_some(self.status.clone(), |parent, status| {
                parent.child(
                    div()
                        .px_3()
                        .py_2()
                        .rounded(px(6.))
                        .bg(app_theme.success.opacity(0.10))
                        .text_size(px(12.))
                        .text_color(app_theme.success)
                        .child(status),
                )
            })
            .child(
                h_flex()
                    .justify_end()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new("back-transfer-workspace")
                            .outline()
                            .small()
                            .label("返回")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.clear_transfer_wallet(cx);
                                cx.emit(WalletManagerEvent::BackToWalletList);
                            })),
                    )
                    .child(
                        Button::new("sign-btcc-tx")
                            .primary()
                            .small()
                            .label("发送")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.sign_transaction(window, cx);
                            })),
                    ),
            )
            .when_some(signed, |parent, signed| {
                let rawtx_display = if signed.rawtx.len() > 40 {
                    format!("{}...{}", &signed.rawtx[..20], &signed.rawtx[signed.rawtx.len()-20..])
                } else {
                    signed.rawtx.clone()
                };
                parent
                    .child(
                        v_flex()
                            .gap_3()
                            .p_3()
                            .rounded(px(8.))
                            .border_1()
                            .border_color(palette::border(&app_theme))
                            .bg(app_theme.muted.opacity(0.05))
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .font_semibold()
                                    .text_color(palette::text_strong(&app_theme))
                                    .child("签名预览"),
                            )
                            .child(self.render_field_with_copy(
                                "copy-btcc-recipient-address",
                                "收款地址",
                                recipient_address.clone(),
                                recipient_address.clone(),
                                false,
                                cx,
                            ))
                            .child(
                                h_flex()
                                    .gap_3()
                                    .flex_wrap()
                                    .child(self.render_summary_item(
                                        "发送数量",
                                        format!(
                                            "{} BTCC",
                                            self.format_btcc_amount(signed.send_sats)
                                        ),
                                        true,
                                        cx,
                                    ))
                                    .child(self.render_summary_item(
                                        "手续费",
                                        format!("{} sats", signed.fee_sats),
                                        false,
                                        cx,
                                    ))
                                    .child(self.render_summary_item(
                                        "找零",
                                        format!("{} sats", signed.change_sats),
                                        false,
                                        cx,
                                    ))
                                    .child(self.render_summary_item(
                                        "总输入",
                                        format!("{} sats", signed.total_input_sats),
                                        false,
                                        cx,
                                    ))
                                    .child(self.render_summary_item(
                                        "使用 UTXO",
                                        format!("{} 个", signed.input_count),
                                        false,
                                        cx,
                                    )),
                            ),
                    )
                    .child(self.render_field_with_copy(
                        "copy-btcc-rawtx",
                        "已签名 Raw Transaction",
                        rawtx_display,
                        signed.rawtx,
                        true,
                        cx,
                    ))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(palette::muted(&app_theme))
                            .child(format!(
                                "输入 {} sats，发送 {} sats，找零 {} sats，手续费 {} sats",
                                signed.total_input_sats,
                                signed.send_sats,
                                signed.change_sats,
                                signed.fee_sats
                            )),
                    )
            })
            .when_some(self.last_txid.clone(), |parent, txid| {
                parent.child(self.render_txid_field(txid, cx))
            })
            .into_any_element()
    }

    fn render_wallet_workspace(
        &self,
        wallet: &BitcoinWallet,
        _signed: Option<BtccSignedTransaction>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let app_theme = cx.theme().clone();

        h_flex()
            .gap_4()
            .items_start()
            .child(
                div()
                    .w(px(520.))
                    .min_h(px(390.))
                    .p_4()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(palette::border(&app_theme))
                    .bg(app_theme.background)
                    .child(self.render_wallet(wallet, cx)),
            )
            .child(
                v_flex()
                    .flex_1()
                    .gap_4()
                    .child(
                        div()
                            .h_full()
                            .min_h(px(320.))
                            .p_4()
                            .rounded(px(8.))
                            .border_1()
                            .border_color(palette::border(&app_theme))
                            .bg(app_theme.background)
                            .child(self.render_balance(cx)),
                    )
                    .child(div().h_full().child(self.render_send_panel(None, cx))),
            )
            .into_any_element()
    }

    fn render_transfer_workspace(
        &self,
        address: String,
        _signed: Option<BtccSignedTransaction>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let app_theme = cx.theme().clone();
        v_flex()
            .gap_4()
            .child(
                v_flex()
                    .gap_3()
                    .p_4()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(palette::border(&app_theme))
                    .bg(app_theme.background)
                    .child(
                        h_flex()
                            .items_center()
                            .justify_between()
                            .child(
                                v_flex()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_size(px(16.))
                                            .font_semibold()
                                            .text_color(palette::text_strong(&app_theme))
                                            .child("转账钱包"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(palette::muted(&app_theme))
                                            .child("来自钱包列表"),
                                    ),
                            )
                            .child(
                                h_flex().items_center().gap_2().child(
                                    div()
                                        .px_2()
                                        .py_1()
                                        .rounded(px(6.))
                                        .bg(app_theme.primary.opacity(0.10))
                                        .text_size(px(12.))
                                        .text_color(app_theme.primary)
                                        .child("Native SegWit"),
                                ),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_3()
                            .items_start()
                            .child(div().flex_1().child(compact_info("BTCC 地址", address, cx)))
                            .child(div().w(px(190.)).child(compact_info(
                                "派生路径",
                                BTCC_NATIVE_SEGWIT_PATH.to_string(),
                                cx,
                            )))
                            .child(div().w(px(230.)).child(compact_info(
                                "数据 API",
                                "https://api.btc-classic.org".to_string(),
                                cx,
                            ))),
                    ),
            )
            .child(
                h_flex()
                    .gap_4()
                    .items_start()
                    .child(
                        div()
                            .w(px(460.))
                            .h_full()
                            .min_h(px(320.))
                            .p_4()
                            .rounded(px(8.))
                            .border_1()
                            .border_color(palette::border(&app_theme))
                            .bg(app_theme.background)
                            .child(self.render_balance(cx)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .h_full()
                            .child(self.render_send_panel(None, cx)),
                    ),
            )
            .into_any_element()
    }
}

fn compact_info(
    label: &'static str,
    value: String,
    cx: &mut Context<WalletGeneratorPage>,
) -> AnyElement {
    let app_theme = cx.theme().clone();
    v_flex()
        .gap_2()
        .p_2()
        .rounded(px(6.))
        .border_1()
        .border_color(palette::border(&app_theme))
        .bg(app_theme.muted.opacity(0.06))
        .child(
            div()
                .text_size(px(11.))
                .text_color(palette::muted(&app_theme))
                .child(label),
        )
        .child(
            div()
                .font_family("monospace")
                .text_size(px(12.))
                .text_color(palette::text_strong(&app_theme))
                .child(value),
        )
        .into_any_element()
}

impl Render for WalletGeneratorPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let app_theme = cx.theme().clone();
        let wallet = self.wallet.clone();
        let transfer_address = self.transfer_address.clone();
        let export_wallet = self.wallet.clone();
        let signed = self.last_signed.clone();

        v_flex()
            .size_full()
            .p_4()
            .gap_4()
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .gap_3()
                    .p_4()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(palette::border(&app_theme))
                    .bg(app_theme.background)
                    .child(
                        v_flex()
                            .gap_2()
                            .child(
                                div()
                                    .text_size(px(20.))
                                    .font_semibold()
                                    .text_color(palette::text_strong(&app_theme))
                                    .child("BTCC 钱包管理"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(palette::muted(&app_theme))
                                    .child("导入钱包，查询余额，构建签名交易并通过 BTCC Explorer 广播。"),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("refresh-btcc-balance")
                                    .primary()
                                    .small()
                                    .label(if self.loading { "处理中..." } else { "查询余额" })
                                    .on_click(cx.listener(|this, _, _, cx| this.refresh_balance(cx))),
                            ),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .mt_3()
                    .overflow_y_scrollbar()
                    .child(match wallet {
                        Some(wallet) => self.render_wallet_workspace(&wallet, signed.clone(), cx),
                        None => match transfer_address {
                            Some(address) => {
                                self.render_transfer_workspace(address, signed.clone(), cx)
                            }
                            None => self.render_empty_state(cx),
                        },
                    }),
            )
            .when(self.import_dialog_open, |parent| {
                parent.child(self.render_import_dialog(cx))
            })
            .when(self.password_dialog_open, |parent| {
                parent.child(self.render_password_dialog(cx))
            })
            .when_some(
                signed.filter(|_| self.sign_dialog_open),
                |parent, signed| parent.child(self.render_sign_dialog(&signed, cx)),
            )
            .when_some(
                export_wallet.filter(|_| self.export_dialog_open),
                |parent, wallet| parent.child(self.render_export_dialog(&wallet, cx)),
            )
    }
}
