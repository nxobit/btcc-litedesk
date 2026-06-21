use crate::ui::btcc::wallet_list::get_global_vault_password;
use crate::ui::palette;
use btcc_litedesk::{
    db::btcc_wallet::list_btcc_wallets_blocking,
    wallet::{
        broadcast_cc_stamp_mint_transaction_blocking, build_cc_stamp_mint_transaction_blocking,
        BtccStampMintRequest,
    },
};
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    ActiveTheme, Disableable, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    clipboard::Clipboard,
    h_flex,
    input::{Input, InputEvent, InputState},
    scroll::ScrollableElement,
    select::{Select, SelectDelegate, SelectEvent, SelectItem, SelectState},
    v_flex,
};

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

pub struct StampMintPage {
    wallets: Vec<(i64, String, String)>,
    wallet_select: Entity<SelectState<WalletSelectItems>>,
    selected_wallet_id: Option<i64>,
    to_address_input: Entity<InputState>,
    amount_input: Entity<InputState>,
    fee_rate_input: Entity<InputState>,
    stamp_input: Entity<InputState>,
    loading: bool,
    status: Option<String>,
    error: Option<String>,
    preview_rawtx: Option<String>,
    last_txid: Option<String>,
    _task: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl StampMintPage {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let wallets = list_btcc_wallets_blocking()
            .unwrap_or_default()
            .into_iter()
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

        let to_address_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("输入接收地址，必须为合法 cc1q... 地址")
                .default_value("")
        });
        let amount_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("发送金额，单位 sats")
                .default_value("100000")
        });
        let fee_rate_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("费率 sat/vB")
                .default_value("2")
        });
        let stamp_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("铭文标识，例如 CC-STAMP-08553-0")
                .default_value("CC-STAMP-08553-0")
        });

        let subscriptions = vec![
            cx.subscribe_in(&wallet_select, window, Self::on_wallet_select_event),
            cx.subscribe_in(&to_address_input, window, Self::on_input_event),
            cx.subscribe_in(&amount_input, window, Self::on_input_event),
            cx.subscribe_in(&fee_rate_input, window, Self::on_input_event),
            cx.subscribe_in(&stamp_input, window, Self::on_input_event),
        ];

        Self {
            wallets,
            wallet_select,
            selected_wallet_id: None,
            to_address_input,
            amount_input,
            fee_rate_input,
            stamp_input,
            loading: false,
            status: None,
            error: None,
            preview_rawtx: None,
            last_txid: None,
            _task: Task::ready(()),
            _subscriptions: subscriptions,
        }
    }

    fn on_wallet_select_event(
        &mut self,
        _: &Entity<SelectState<WalletSelectItems>>,
        event: &SelectEvent<WalletSelectItems>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            SelectEvent::Confirm(Some(wallet_id)) => {
                self.selected_wallet_id = Some(*wallet_id);
                self.clear_runtime_messages(cx);
            }
            SelectEvent::Confirm(None) => {
                self.selected_wallet_id = None;
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

    fn clear_runtime_messages(&mut self, cx: &mut Context<Self>) {
        self.status = None;
        self.error = None;
        self.preview_rawtx = None;
        self.last_txid = None;
        cx.notify();
    }

    fn selected_wallet(&self) -> Option<(i64, String, String)> {
        let wallet_id = self.selected_wallet_id?;
        self.wallets
            .iter()
            .find(|(id, _, _)| *id == wallet_id)
            .cloned()
    }

    fn build_request(&self, cx: &mut Context<Self>) -> Result<BtccStampMintRequest, String> {
        let (_, _, wallet_address) = self
            .selected_wallet()
            .ok_or_else(|| "请先选择发送钱包".to_string())?;
        let password = get_global_vault_password()
            .ok_or_else(|| "请先在钱包列表页解锁钱包，再回来执行 Mint".to_string())?;
        let to_address = self.to_address_input.read(cx).text().to_string();
        let amount_sats_text = self.amount_input.read(cx).text().to_string();
        let fee_rate_sat_vb_text = self.fee_rate_input.read(cx).text().to_string();
        let stamp = self.stamp_input.read(cx).text().to_string();

        if to_address.trim().is_empty() {
            return Err("请输入接收地址".to_string());
        }
        if stamp.trim().is_empty() {
            return Err("请输入铭文标识".to_string());
        }

        let amount_sats = amount_sats_text
            .trim()
            .parse::<u64>()
            .map_err(|_| "发送金额必须是整数 sats".to_string())?;
        if amount_sats == 0 {
            return Err("发送金额必须大于 0".to_string());
        }

        let fee_rate_sat_vb = fee_rate_sat_vb_text
            .trim()
            .parse::<u64>()
            .map_err(|_| "费率必须是整数 sat/vB".to_string())?;
        if fee_rate_sat_vb == 0 {
            return Err("费率必须大于 0".to_string());
        }

        Ok(BtccStampMintRequest {
            address_prefix: wallet_address,
            password,
            to_address: to_address.trim().to_string(),
            amount_sats,
            fee_rate_sat_vb,
            stamp: stamp.trim().to_string(),
        })
    }

    fn build_rawtx(&mut self, cx: &mut Context<Self>) {
        let request = match self.build_request(cx) {
            Ok(request) => request,
            Err(err) => {
                self.error = Some(err);
                self.status = None;
                cx.notify();
                return;
            }
        };

        self.loading = true;
        self.error = None;
        self.status = Some("正在生成 RawTx".to_string());
        self.preview_rawtx = None;
        self.last_txid = None;

        self._task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { build_cc_stamp_mint_transaction_blocking(&request) })
                .await;

            let _ = this.update(cx, |this, cx| {
                this.loading = false;
                match result {
                    Ok(result) => {
                        this.preview_rawtx = Some(result.signed.rawtx);
                        this.status = Some("RawTx 已生成".to_string());
                        this.error = None;
                    }
                    Err(err) => {
                        this.preview_rawtx = None;
                        this.status = None;
                        this.error = Some(err.to_string());
                    }
                }
                cx.notify();
            });
        });

        cx.notify();
    }

    fn broadcast(&mut self, cx: &mut Context<Self>) {
        let request = match self.build_request(cx) {
            Ok(request) => request,
            Err(err) => {
                self.error = Some(err);
                self.status = None;
                cx.notify();
                return;
            }
        };

        self.loading = true;
        self.error = None;
        self.status = Some("正在广播交易".to_string());
        self.last_txid = None;

        self._task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    broadcast_cc_stamp_mint_transaction_blocking(&request)
                })
                .await;

            let _ = this.update(cx, |this, cx| {
                this.loading = false;
                match result {
                    Ok((result, broadcast)) => {
                        this.preview_rawtx = Some(result.signed.rawtx);
                        this.last_txid = Some(broadcast.txid);
                        this.status = Some("Mint 交易已广播".to_string());
                        this.error = None;
                    }
                    Err(err) => {
                        this.status = None;
                        this.error = Some(err.to_string());
                    }
                }
                cx.notify();
            });
        });

        cx.notify();
    }
}

impl Render for StampMintPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let app_theme = cx.theme().clone();
        let selected_wallet = self.selected_wallet();

        v_flex()
            .size_full()
            .gap_4()
            .child(
                v_flex()
                    .w_full()
                    .gap_4()
                    .p_5()
                    .rounded(px(12.))
                    .border_1()
                    .border_color(palette::border(&app_theme))
                    .bg(app_theme.background)
                    .child(
                        v_flex()
                            .gap_2()
                            .child(
                                div()
                                    .text_size(px(24.))
                                    .font_semibold()
                                    .text_color(palette::text_strong(&app_theme))
                                    .child("铭文 Mint"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .line_height(px(20.))
                                    .text_color(palette::muted(&app_theme))
                                    .child("单个 Mint。先选择发送钱包，再填写接收地址、金额、费率和铭文标识。密码复用钱包列表的解锁密码。"),
                            ),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(palette::text_strong(&app_theme))
                                    .child("发送钱包"),
                            )
                            .child(Select::new(&self.wallet_select))
                            .when_some(selected_wallet.clone(), |el, (_, name, address)| {
                                el.child(
                                    div()
                                        .text_size(px(12.))
                                        .line_height(px(18.))
                                        .text_color(palette::muted(&app_theme))
                                        .child(format!("{name} | {address}")),
                                )
                            })
                            .when(selected_wallet.is_none(), |el| {
                                el.child(
                                    div()
                                        .text_size(px(12.))
                                        .line_height(px(18.))
                                        .text_color(palette::muted(&app_theme))
                                        .child("请从下拉列表中选择一条钱包记录"),
                                )
                            }),
                    )
                    .child(form_field("接收地址", self.to_address_input.clone()))
                    .child(
                        h_flex()
                            .gap_3()
                            .child(
                                div()
                                    .flex_1()
                                    .child(form_field("发送金额（sats）", self.amount_input.clone())),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .child(form_field("费率（sat/vB）", self.fee_rate_input.clone())),
                            ),
                    )
                    .child(form_field("铭文标识", self.stamp_input.clone()))
                    .when_some(self.error.clone(), |el, error| {
                        el.child(inline_error(error, cx))
                    })
                    .when_some(self.status.clone(), |el, status| {
                        el.child(inline_status(status, cx))
                    })
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("stamp-mint-build")
                                    .label("生成 RawTx")
                                    .primary()
                                    .small()
                                    .disabled(self.loading)
                                    .on_click(cx.listener(|this, _, _, cx| this.build_rawtx(cx))),
                            )
                            .child(
                                Button::new("stamp-mint-broadcast")
                                    .label("广播")
                                    .small()
                                    .disabled(self.loading)
                                    .on_click(cx.listener(|this, _, _, cx| this.broadcast(cx))),
                            ),
                    )
                    .when_some(self.preview_rawtx.clone(), |el, rawtx| {
                        el.child(
                            v_flex()
                                .gap_2()
                                .child(
                                    h_flex()
                                        .justify_between()
                                        .items_center()
                                        .child(
                                            div()
                                                .text_size(px(12.))
                                                .font_semibold()
                                                .text_color(palette::text_strong(&app_theme))
                                                .child("RawTx"),
                                        )
                                        .child(Clipboard::new("stamp-mint-rawtx").value(rawtx.clone())),
                                )
                                .child(
                                    div()
                                        .w_full()
                                        .max_h(px(220.))
                                        .overflow_y_scrollbar()
                                        .p_3()
                                        .rounded(px(8.))
                                        .border_1()
                                        .border_color(palette::border(&app_theme))
                                        .font_family("monospace")
                                        .text_size(px(12.))
                                        .line_height(px(20.))
                                        .child(rawtx),
                                ),
                        )
                    }),
            )
            .child(
                v_flex()
                    .w_full()
                    .gap_3()
                    .p_5()
                    .rounded(px(12.))
                    .border_1()
                    .border_color(palette::border(&app_theme))
                    .bg(app_theme.background)
                    .child(
                        div()
                            .text_size(px(18.))
                            .font_semibold()
                            .text_color(palette::text_strong(&app_theme))
                            .child("广播结果"),
                    )
                    .when_some(self.last_txid.clone(), |el, txid| {
                        el.child(
                            v_flex()
                                .gap_2()
                                .child(
                                    h_flex()
                                        .justify_between()
                                        .items_center()
                                        .child(
                                            div()
                                                .text_size(px(12.))
                                                .font_semibold()
                                                .text_color(palette::text_strong(&app_theme))
                                                .child("TxID"),
                                        )
                                        .child(Clipboard::new("stamp-mint-txid").value(txid.clone())),
                                )
                                .child(
                                    div()
                                        .w_full()
                                        .p_3()
                                        .rounded(px(8.))
                                        .border_1()
                                        .border_color(palette::border(&app_theme))
                                        .font_family("monospace")
                                        .text_size(px(12.))
                                        .line_height(px(20.))
                                        .child(txid),
                                ),
                        )
                    })
                    .when(self.last_txid.is_none(), |el| {
                        el.child(
                            div()
                                .text_size(px(12.))
                                .text_color(palette::muted(&app_theme))
                                .child("广播成功后，这里显示单笔 TxID。"),
                        )
                    }),
            )
    }
}

fn form_field(label: &str, input: Entity<InputState>) -> AnyElement {
    let label = label.to_string();
    v_flex()
        .gap_1()
        .child(div().text_size(px(12.)).child(label))
        .child(Input::new(&input))
        .into_any_element()
}

fn inline_error(text: String, cx: &mut Context<StampMintPage>) -> AnyElement {
    div()
        .text_size(px(12.))
        .text_color(cx.theme().danger)
        .child(text)
        .into_any_element()
}

fn inline_status(text: String, cx: &mut Context<StampMintPage>) -> AnyElement {
    div()
        .text_size(px(12.))
        .text_color(cx.theme().primary)
        .child(text)
        .into_any_element()
}
