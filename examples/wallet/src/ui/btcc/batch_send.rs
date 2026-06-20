use crate::ui::btcc::wallet_list::get_global_vault_password;
use crate::ui::palette;
use btcc_litedesk::{
    db::btcc_wallet::{
        decrypt_btcc_wallet_secrets_blocking, list_btcc_wallets_blocking,
        update_btcc_wallet_balance_blocking,
    },
    wallet::{
        BtccAddressInfo, BtccBatchRecipient, BtccBatchSendRequest, BtccExplorerClient,
        BtccSignedTransaction, BtccWallet, btcc_to_sats, build_batch_signed_transaction,
        validate_recipient_address, wallet_from_mnemonic, wallet_from_private_key_wif,
    },
};
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    ActiveTheme, Disableable, IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputEvent, InputState},
    scroll::ScrollableElement,
    select::{Select, SelectDelegate, SelectEvent, SelectItem, SelectState},
    v_flex,
};
use std::collections::HashSet;

#[derive(Clone, Copy, PartialEq, Eq)]
enum BatchMode {
    Simple,
    Expert,
}

#[derive(Clone)]
struct WalletOption {
    id: i64,
    name: String,
    address: String,
}

impl SelectItem for WalletOption {
    type Value = i64;

    fn title(&self) -> SharedString {
        SharedString::from(format!("{} | {}", self.name, self.address))
    }

    fn display_title(&self) -> Option<AnyElement> {
        Some(self.address.clone().into_any_element())
    }

    fn value(&self) -> &Self::Value {
        &self.id
    }
}

#[derive(Clone)]
struct WalletSelectItems {
    items: Vec<WalletOption>,
    matched_items: Vec<WalletOption>,
}

impl WalletSelectItems {
    fn new(items: Vec<WalletOption>) -> Self {
        Self {
            matched_items: items.clone(),
            items,
        }
    }
}

impl SelectDelegate for WalletSelectItems {
    type Item = WalletOption;

    fn items_count(&self, _: usize) -> usize {
        self.matched_items.len()
    }

    fn item(&self, ix: gpui_component::IndexPath) -> Option<&Self::Item> {
        self.matched_items.get(ix.row)
    }

    fn position<V>(&self, value: &V) -> Option<gpui_component::IndexPath>
    where
        Self::Item: SelectItem<Value = V>,
        V: PartialEq,
    {
        self.matched_items
            .iter()
            .position(|item| item.value() == value)
            .map(|row| gpui_component::IndexPath::default().row(row))
    }

    fn perform_search(
        &mut self,
        query: &str,
        _: &mut Window,
        _: &mut Context<SelectState<Self>>,
    ) -> Task<()> {
        let query = query.trim().to_lowercase();
        self.matched_items = if query.is_empty() {
            self.items.clone()
        } else {
            self.items
                .iter()
                .filter(|item| {
                    item.name.to_lowercase().contains(&query)
                        || item.address.to_lowercase().contains(&query)
                })
                .cloned()
                .collect()
        };
        Task::ready(())
    }
}

pub struct BatchSendPage {
    wallets: Vec<(i64, String, String)>,
    wallet_select: Entity<SelectState<WalletSelectItems>>,
    selected_wallet_id: Option<i64>,
    balance: Option<BtccAddressInfo>,
    mode: BatchMode,
    recipients_input: Entity<InputState>,
    amount_input: Entity<InputState>,
    fee_rate_input: Entity<InputState>,
    password_input: Entity<InputState>,
    vault_password: Option<String>,
    password_prompt_open: bool,
    send_confirm_open: bool,
    loading: bool,
    status: Option<String>,
    error: Option<String>,
    last_signed: Option<BtccSignedTransaction>,
    last_txid: Option<String>,
    _task: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl BatchSendPage {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let recipients_input = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .rows(6)
                .placeholder("每行一个钱包地址")
                .default_value("")
        });
        let amount_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("每个地址发送金额")
                .default_value("")
        });
        let fee_rate_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("费率 sat/vB")
                .default_value("5")
        });
        let password_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("输入钱包密码")
                .masked(true)
                .default_value("")
        });

        let wallets = list_btcc_wallets_blocking()
            .unwrap_or_default()
            .into_iter()
            .filter(|wallet| wallet.balance_sats > 0)
            .map(|wallet| (wallet.id, wallet.name, wallet.address))
            .collect::<Vec<_>>();

        let wallet_options = wallets
            .iter()
            .map(|(id, name, address)| WalletOption {
                id: *id,
                name: name.clone(),
                address: address.clone(),
            })
            .collect::<Vec<_>>();

        let wallet_select = cx.new(|cx| {
            SelectState::new(WalletSelectItems::new(wallet_options), None, window, cx)
                .searchable(true)
        });

        let subscriptions = vec![
            cx.subscribe_in(&wallet_select, window, Self::on_wallet_select_event),
            cx.subscribe_in(&recipients_input, window, Self::on_input_event),
            cx.subscribe_in(&amount_input, window, Self::on_input_event),
            cx.subscribe_in(&fee_rate_input, window, Self::on_input_event),
            cx.subscribe_in(&password_input, window, Self::on_input_event),
        ];

        Self {
            wallets,
            wallet_select,
            selected_wallet_id: None,
            balance: None,
            mode: BatchMode::Simple,
            recipients_input,
            amount_input,
            fee_rate_input,
            password_input,
            vault_password: None,
            password_prompt_open: false,
            send_confirm_open: false,
            loading: false,
            status: None,
            error: None,
            last_signed: None,
            last_txid: None,
            _task: Task::ready(()),
            _subscriptions: subscriptions,
        }
    }

    fn on_wallet_select_event(
        &mut self,
        _: &Entity<SelectState<WalletSelectItems>>,
        event: &SelectEvent<WalletSelectItems>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            SelectEvent::Confirm(Some(wallet_id)) => self.select_wallet(*wallet_id, window, cx),
            SelectEvent::Confirm(None) => {
                self.selected_wallet_id = None;
                self.balance = None;
                self.clear_runtime_messages(cx);
            }
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
            self.clear_runtime_messages(cx);
        }
    }

    fn input_value(&self, input: &Entity<InputState>, cx: &mut Context<Self>) -> String {
        input.read_with(cx, |input, _| input.value()).to_string()
    }

    fn clear_runtime_messages(&mut self, cx: &mut Context<Self>) {
        self.error = None;
        self.status = None;
        self.last_signed = None;
        self.last_txid = None;
        self.send_confirm_open = false;
        cx.notify();
    }

    fn recipient_lines(&self, cx: &mut Context<Self>) -> Vec<String> {
        self.input_value(&self.recipients_input, cx)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(ToString::to_string)
            .collect()
    }

    fn selected_wallet(&self) -> Option<(i64, String, String)> {
        let selected_id = self.selected_wallet_id?;
        self.wallets
            .iter()
            .find(|(id, _, _)| *id == selected_id)
            .cloned()
    }

    fn set_mode(&mut self, mode: BatchMode, window: &mut Window, cx: &mut Context<Self>) {
        self.mode = mode;
        let placeholder = match mode {
            BatchMode::Simple => "每行一个钱包地址",
            BatchMode::Expert => "钱包地址,0.1",
        };
        self.recipients_input.update(cx, |input, cx| {
            input.set_placeholder(placeholder, window, cx)
        });
        self.clear_runtime_messages(cx);
    }

    fn select_wallet(&mut self, wallet_id: i64, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_wallet_id = Some(wallet_id);
        self.balance = None;
        self.error = None;
        self.status = Some("已选择发送钱包，正在查询余额".to_string());
        self.last_signed = None;
        self.last_txid = None;
        self.refresh_balance(window, cx);
    }

    fn refresh_balance(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some((wallet_id, _, address)) = self.selected_wallet() else {
            self.error = Some("请先选择发送钱包".to_string());
            cx.notify();
            return;
        };

        self.loading = true;
        self.error = None;
        self.status = Some("正在查询余额".to_string());
        self._task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let info = BtccExplorerClient::default().address_info(&address)?;
                    update_btcc_wallet_balance_blocking(
                        wallet_id,
                        info.confirmed_sats.min(i64::MAX as u64) as i64,
                        info.unconfirmed_sats,
                        info.utxo_total.min(i64::MAX as usize) as i64,
                    )?;
                    Ok::<_, anyhow::Error>(info)
                })
                .await;

            let _ = this.update(cx, |this, cx| {
                this.loading = false;
                match result {
                    Ok(info) => {
                        this.balance = Some(info);
                        this.status = None;
                        this.error = None;
                    }
                    Err(err) => {
                        this.balance = None;
                        this.status = None;
                        this.error = Some(format!("查询余额失败: {err}"));
                    }
                }
                cx.notify();
            });
        });
    }

    fn parse_recipients(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Result<Vec<BtccBatchRecipient>, String> {
        let lines = self.recipient_lines(cx);
        if lines.is_empty() {
            return Err("请输入批量地址".to_string());
        }

        if lines.len() > 50 {
            return Err("单次最多发送 50 个钱包地址，请拆分后再发送".to_string());
        }

        match self.mode {
            BatchMode::Simple => {
                let amount_text = self.input_value(&self.amount_input, cx);
                if amount_text.trim().is_empty() {
                    return Err("请输入每个地址发送金额".to_string());
                }

                let amount_sats = btcc_to_sats(amount_text.trim())
                    .map_err(|err| format!("发送金额格式错误: {err}"))?;
                if amount_sats == 0 {
                    return Err("发送金额必须大于 0".to_string());
                }

                let mut seen = HashSet::new();
                let mut recipients = Vec::new();
                for (index, line) in lines.into_iter().enumerate() {
                    let address = validate_recipient_address(&line)
                        .map_err(|_| format!("第 {} 个钱包地址不正确", index + 1))?;
                    if seen.insert(address.clone()) {
                        recipients.push(BtccBatchRecipient {
                            address,
                            amount_sats,
                        });
                    }
                }
                Ok(recipients)
            }
            BatchMode::Expert => {
                let mut seen = HashSet::new();
                let mut recipients = Vec::new();
                for (index, line) in lines.into_iter().enumerate() {
                    let (address_text, amount_text) = line.split_once(',').ok_or_else(|| {
                        format!("第 {} 行格式不正确，应为 钱包地址,0.1", index + 1)
                    })?;
                    let address = validate_recipient_address(address_text.trim())
                        .map_err(|_| format!("第 {} 个钱包地址不正确", index + 1))?;
                    let amount_sats = btcc_to_sats(amount_text.trim())
                        .map_err(|err| format!("第 {} 行金额格式错误: {err}", index + 1))?;
                    if amount_sats == 0 {
                        return Err(format!("第 {} 行金额必须大于 0", index + 1));
                    }
                    if seen.insert(address.clone()) {
                        recipients.push(BtccBatchRecipient {
                            address,
                            amount_sats,
                        });
                    }
                }
                Ok(recipients)
            }
        }
    }

    fn resolve_wallet(&mut self) -> Result<BtccWallet, String> {
        let wallet_id = self
            .selected_wallet_id
            .ok_or_else(|| "请先选择发送钱包".to_string())?;

        let password = self
            .vault_password
            .clone()
            .or_else(get_global_vault_password)
            .ok_or_else(|| {
                self.password_prompt_open = true;
                "请输入钱包密码".to_string()
            })?;

        let secrets = decrypt_btcc_wallet_secrets_blocking(wallet_id, password)
            .map_err(|err| format!("解密钱包失败: {err}"))?;

        if !secrets.mnemonic.trim().is_empty() {
            wallet_from_mnemonic(&secrets.mnemonic).map_err(|err| format!("恢复钱包失败: {err}"))
        } else {
            wallet_from_private_key_wif(&secrets.private_key_wif)
                .map_err(|err| format!("恢复钱包失败: {err}"))
        }
    }

    fn sign_batch_transaction(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.error = None;
        self.status = None;
        self.last_signed = None;
        self.last_txid = None;
        self.send_confirm_open = false;

        if self.selected_wallet_id.is_none() {
            self.error = Some("请先选择发送钱包".to_string());
            cx.notify();
            return;
        }

        let balance = match self.balance.clone() {
            Some(balance) => balance,
            None => {
                self.error = Some("请先查询发送钱包余额".to_string());
                cx.notify();
                return;
            }
        };

        let recipients = match self.parse_recipients(cx) {
            Ok(recipients) => recipients,
            Err(err) => {
                self.error = Some(err);
                cx.notify();
                return;
            }
        };

        let fee_rate = match self
            .input_value(&self.fee_rate_input, cx)
            .trim()
            .parse::<u64>()
        {
            Ok(value) if value > 0 => value,
            _ => {
                self.error = Some("璐圭巼蹇呴』鏄ぇ浜?0 鐨勬暣鏁?sat/vB".to_string());
                cx.notify();
                return;
            }
        };

        let wallet = match self.resolve_wallet() {
            Ok(wallet) => wallet,
            Err(err) => {
                self.error = Some(err);
                cx.notify();
                return;
            }
        };

        self.loading = true;
        self.status = Some("正在生成签名交易".to_string());
        cx.notify();

        let request = BtccBatchSendRequest {
            recipients,
            fee_rate_sat_vb: fee_rate,
        };

        self._task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    build_batch_signed_transaction(&wallet, &balance.utxos, &request)
                })
                .await;

            let _ = this.update(cx, |this, cx| {
                this.loading = false;
                match result {
                    Ok(signed) => {
                        this.password_prompt_open = false;
                        this.last_signed = Some(signed);
                        this.send_confirm_open = true;
                        this.error = None;
                        this.status = None;
                    }
                    Err(err) => {
                        this.error = Some(format!("生成签名交易失败: {err}"));
                        this.status = None;
                    }
                }
                cx.notify();
            });
        });
    }

    fn broadcast_transaction(&mut self, cx: &mut Context<Self>) {
        let Some(signed) = self.last_signed.clone() else {
            self.error = Some("请先生成签名交易".to_string());
            cx.notify();
            return;
        };

        self.loading = true;
        self.error = None;
        self.status = Some("正在广播交易".to_string());
        self._task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    BtccExplorerClient::default().broadcast_raw_transaction(&signed.rawtx)
                })
                .await;

            let _ = this.update(cx, |this, cx| {
                this.loading = false;
                match result {
                    Ok(result) => {
                        this.last_txid = Some(result.txid.clone());
                        this.status = None;
                        this.error = None;
                        this.send_confirm_open = false;
                    }
                    Err(err) => {
                        this.status = None;
                        this.error = Some(format!("广播交易失败: {err}"));
                    }
                }
                cx.notify();
            });
        });
    }

    fn confirm_password(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let password = self.password_input.read(cx).text().to_string();
        if password.trim().is_empty() {
            self.error = Some("请输入钱包密码".to_string());
            cx.notify();
            return;
        }

        self.vault_password = Some(password);
        self.password_prompt_open = false;
        self.password_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.error = None;
        self.status = Some("密码已缓存，请重新点击发送".to_string());
        cx.notify();
    }

    fn cancel_password(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.password_prompt_open = false;
        self.password_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.error = None;
        cx.notify();
    }

    fn copy_rawtx(&mut self, rawtx: String, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(rawtx));
        self.error = None;
        self.status = Some("RawTx 已复制".to_string());
        cx.notify();
    }

    fn copy_txid(&mut self, txid: String, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(txid));
        self.error = None;
        self.status = Some("TXID 已复制".to_string());
        cx.notify();
    }

    fn render_wallet_section(&self, cx: &mut Context<Self>) -> AnyElement {
        let app_theme = cx.theme().clone();

        v_flex()
            .gap_4()
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_size(px(18.))
                            .font_semibold()
                            .text_color(palette::text_strong(&app_theme))
                            .child("发送钱包"),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .line_height(px(20.))
                            .text_color(palette::muted(&app_theme))
                            .child("仅显示有余额的钱包，可按钱包名称或钱包地址搜索。"),
                    ),
            )
            .child(
                Select::new(&self.wallet_select)
                    .placeholder("选择发送钱包")
                    .search_placeholder("按名称或钱包地址搜索")
                    .cleanable(true)
                    .small()
                    .w_full(),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(palette::muted(&app_theme))
                    .child(match self.selected_wallet() {
                        Some((_, name, address)) => format!("已选: {name} | {address}"),
                        None => "尚未选择发送钱包".to_string(),
                    }),
            )
            .when_some(self.balance.as_ref(), |parent, balance| {
                parent.child(
                    h_flex()
                        .gap_3()
                        .child(Self::render_info_chip(
                            "确认余额",
                            format!("{:.8} BTCC", balance.confirmed_btcc),
                            false,
                            cx,
                        ))
                        .child(Self::render_info_chip(
                            "未确认余额",
                            format!("{:.8} BTCC", balance.unconfirmed_btcc),
                            false,
                            cx,
                        ))
                        .child(Self::render_info_chip(
                            "UTXO",
                            balance.utxos.len().to_string(),
                            true,
                            cx,
                        )),
                )
            })
            .into_any_element()
    }

    fn render_info_chip(
        label: &'static str,
        value: String,
        accent: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let app_theme = cx.theme().clone();
        v_flex()
            .gap_1()
            .min_w(px(150.))
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
                    .text_size(px(15.))
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

    fn render_mode_switcher(&self, cx: &mut Context<Self>) -> AnyElement {
        let is_simple = self.mode == BatchMode::Simple;
        h_flex()
            .gap_2()
            .items_center()
            .child(
                Button::new("batch-mode-simple")
                    .ghost()
                    .small()
                    .label("简单模式")
                    .when(is_simple, |button| button.primary())
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.set_mode(BatchMode::Simple, window, cx);
                    })),
            )
            .child(
                Button::new("batch-mode-expert")
                    .ghost()
                    .small()
                    .label("专家模式")
                    .when(!is_simple, |button| button.primary())
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.set_mode(BatchMode::Expert, window, cx);
                    })),
            )
            .into_any_element()
    }

    fn render_recipients_input(&self, cx: &mut Context<Self>) -> AnyElement {
        let app_theme = cx.theme().clone();
        let line_count = self
            .input_value(&self.recipients_input, cx)
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count();
        v_flex()
            .gap_3()
            .child(
                div()
                    .text_size(px(14.))
                    .font_semibold()
                    .text_color(palette::text_strong(&app_theme))
                    .child("批量地址"),
            )
            .child(Input::new(&self.recipients_input).h(px(168.)))
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(palette::muted(&app_theme))
                    .child(format!("当前 {} 行", line_count)),
            )
            .into_any_element()
    }

    fn render_password_prompt(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.password_prompt_open {
            return None;
        }

        let app_theme = cx.theme().clone();
        Some(
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
                                .child("输入密码后继续生成批量转账签名。"),
                        )
                        .child(Input::new(&self.password_input))
                        .child(
                            h_flex()
                                .justify_end()
                                .gap_2()
                                .child(
                                    Button::new("batch-password-cancel")
                                        .outline()
                                        .small()
                                        .label("取消")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.cancel_password(window, cx);
                                        })),
                                )
                                .child(
                                    Button::new("batch-password-confirm")
                                        .primary()
                                        .small()
                                        .label("确认")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.confirm_password(window, cx);
                                        })),
                                ),
                        ),
                )
                .into_any_element(),
        )
    }

    fn render_status(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let app_theme = cx.theme().clone();
        if let Some(error) = &self.error {
            Some(
                div()
                    .p_3()
                    .rounded(px(8.))
                    .bg(app_theme.danger.opacity(0.10))
                    .text_size(px(12.))
                    .text_color(app_theme.danger)
                    .child(error.clone())
                    .into_any_element(),
            )
        } else {
            self.status.as_ref().map(|status| {
                div()
                    .p_3()
                    .rounded(px(8.))
                    .bg(app_theme.info.opacity(0.10))
                    .text_size(px(12.))
                    .text_color(app_theme.info)
                    .child(status.clone())
                    .into_any_element()
            })
        }
    }

    fn render_send_confirm(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.send_confirm_open {
            return None;
        }

        let signed = self.last_signed.as_ref()?;
        let app_theme = cx.theme().clone();
        let recipient_count = self.recipient_lines(cx).len();
        let sender = self
            .selected_wallet()
            .map(|(_, _, address)| address)
            .unwrap_or_else(|| "未选择".to_string());

        Some(
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
                                                .child("请先核对批量转账信息，确认无误后再广播。"),
                                        ),
                                )
                                .child(
                                    Button::new("close-batch-sign-dialog")
                                        .outline()
                                        .xsmall()
                                        .label("关闭")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.send_confirm_open = false;
                                            cx.notify();
                                        })),
                                ),
                        )
                        .child(
                            v_flex()
                                .gap_3()
                                .child(format!("发送钱包: {sender}"))
                                .child(format!("收款地址数量: {recipient_count}"))
                                .child(format!(
                                    "发送总额: {:.8} BTCC",
                                    signed.send_sats as f64 / 100_000_000.0
                                ))
                                .child(format!("手续费: {} sats", signed.fee_sats))
                                .child(format!("找零: {} sats", signed.change_sats))
                                .child(format!("输入 UTXO: {}", signed.input_count))
                                .child(
                                    v_flex()
                                        .gap_2()
                                        .child(
                                            h_flex()
                                                .items_center()
                                                .justify_between()
                                                .gap_2()
                                                .child(
                                                    div()
                                                        .text_size(px(12.))
                                                        .font_semibold()
                                                        .text_color(palette::text_strong(
                                                            &app_theme,
                                                        ))
                                                        .child("RawTx"),
                                                )
                                                .child(
                                                    Button::new("copy-batch-rawtx")
                                                        .ghost()
                                                        .xsmall()
                                                        .label("复制")
                                                        .on_click(cx.listener({
                                                            let rawtx = signed.rawtx.clone();
                                                            move |this, _, _, cx| {
                                                                this.copy_rawtx(rawtx.clone(), cx);
                                                            }
                                                        })),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .max_h(px(140.))
                                                .overflow_y_scrollbar()
                                                .rounded(px(8.))
                                                .bg(app_theme.muted.opacity(0.06))
                                                .p_2()
                                                .text_size(px(12.))
                                                .font_family("monospace")
                                                .text_color(palette::muted(&app_theme))
                                                .child(signed.rawtx.clone()),
                                        ),
                                )
                                .child(
                                    h_flex()
                                        .justify_end()
                                        .gap_2()
                                        .child(
                                            Button::new("cancel-send-confirm")
                                                .outline()
                                                .small()
                                                .label("返回")
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.send_confirm_open = false;
                                                    cx.notify();
                                                })),
                                        )
                                        .child(
                                            Button::new("confirm-broadcast")
                                                .primary()
                                                .small()
                                                .label("广播交易")
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.send_confirm_open = false;
                                                    this.broadcast_transaction(cx);
                                                })),
                                        ),
                                ),
                        ),
                )
                .into_any_element(),
        )
    }
}

impl Render for BatchSendPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let app_theme = cx.theme().clone();
        let is_simple = self.mode == BatchMode::Simple;

        div()
            .size_full()
            .p_5()
            .child(
                v_flex()
                    .relative()
                    .size_full()
                    .gap_4()
                    .bg(app_theme.background)
                    .child(
                        div()
                            .p_6()
                            .rounded(px(12.))
                            .border_1()
                            .border_color(palette::border(&app_theme))
                            .bg(app_theme.background)
                            .child(self.render_wallet_section(cx)),
                    )
                    .child(
                        div()
                            .p_6()
                            .rounded(px(12.))
                            .border_1()
                            .border_color(palette::border(&app_theme))
                            .bg(app_theme.background)
                            .child(
                                v_flex()
                                    .gap_5()
                                    .child(self.render_mode_switcher(cx))
                                    .child(self.render_recipients_input(cx))
                                    .child(
                                        h_flex()
                                            .w_full()
                                            .gap_4()
                                            .items_start()
                                            .child(if is_simple {
                                                v_flex()
                                                    .w(px(420.))
                                                    .gap_0()
                                                    .child(
                                                        div()
                                                            .h(px(56.))
                                                            .child(
                                                                Input::new(&self.amount_input)
                                                                    .h_full(),
                                                            ),
                                                    )
                                                    .into_any_element()
                                            } else {
                                                div().w(px(420.)).into_any_element()
                                            })
                                            .child(
                                                h_flex()
                                                    .w(px(320.))
                                                    .h(px(56.))
                                                    .items_center()
                                                    .gap_2()
                                                    .child(
                                                        div()
                                                            .flex_1()
                                                            .h(px(56.))
                                                            .child(
                                                                Input::new(&self.fee_rate_input)
                                                                    .h_full(),
                                                            ),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_size(px(12.))
                                                            .text_color(palette::muted(&app_theme))
                                                            .child("sat/vB"),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .w(px(96.))
                                                    .h(px(56.))
                                                    .flex()
                                                    .items_start()
                                                    .justify_end()
                                                    .child(
                                                        Button::new("batch-send")
                                                            .primary()
                                                            .small()
                                                            .label(if self.loading {
                                                                "处理中"
                                                            } else {
                                                                "发送"
                                                            })
                                                            .disabled(self.loading)
                                                            .on_click(cx.listener(
                                                                |this, _, window, cx| {
                                                                    this.sign_batch_transaction(window, cx);
                                                                },
                                                            )),
                                                    ),
                                            ),
                                    )
                                    .when_some(self.render_status(cx), |parent, status| {
                                        parent.child(status)
                                    })
                                    .when_some(self.last_txid.clone(), |parent, txid| {
                                        parent.child(
                                            h_flex()
                                                .justify_end()
                                                .child(
                                                    div()
                                                        .max_w(px(520.))
                                                        .p_3()
                                                        .rounded(px(8.))
                                                        .bg(app_theme.info.opacity(0.08))
                                                        .child(
                                                            v_flex()
                                                                .gap_2()
                                                                .child(
                                                                    h_flex()
                                                                        .items_center()
                                                                        .justify_between()
                                                                        .gap_2()
                                                                        .child(
                                                                            div()
                                                                                .text_size(px(12.))
                                                                                .font_semibold()
                                                                                .text_color(app_theme.success)
                                                                                .child("广播成功 TXID"),
                                                                        )
                                                                        .child(
                                                                            h_flex()
                                                                                .items_center()
                                                                                .gap_2()
                                                                                .child(
                                                                                    Button::new("copy-batch-txid")
                                                                                        .outline()
                                                                                        .xsmall()
                                                                                        .icon(IconName::Copy)
                                                                                        .on_click(cx.listener({
                                                                                            let txid = txid.clone();
                                                                                            move |this, _, _, cx| {
                                                                                                this.copy_txid(txid.clone(), cx);
                                                                                            }
                                                                                        })),
                                                                                )
                                                                                .child(
                                                                                    Button::new("open-batch-txid")
                                                                                        .outline()
                                                                                        .xsmall()
                                                                                        .icon(IconName::ExternalLink)
                                                                                        .on_click({
                                                                                            let txid = txid.clone();
                                                                                            move |_, _, cx| {
                                                                                                cx.open_url(&format!("https://explorer.btc-classic.org/tx/{txid}"));
                                                                                            }
                                                                                        }),
                                                                                ),
                                                                        ),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .text_size(px(12.))
                                                                        .font_family("monospace")
                                                                        .child(txid),
                                                                ),
                                                        ),
                                                )
                                        )
                                    }),
                            ),
                    )
                    .when_some(self.render_password_prompt(cx), |parent, overlay| {
                        parent.child(overlay)
                    })
                    .when_some(self.render_send_confirm(cx), |parent, overlay| {
                        parent.child(overlay)
                    }),
            )
    }
}
