use super::*;
use gpui::InteractiveElement;

impl BtccWalletListPage {
    pub(super) fn render_receive_panel(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let address = self.receive_wallet_address.clone()?;
        let app_theme = cx.theme().clone();
        let wallet_name = self.receive_wallet_name.clone();

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
                        .w(px(720.))
                        .gap_4()
                        .p_5()
                        .rounded(px(12.))
                        .border_1()
                        .border_color(palette::border(&app_theme))
                        .bg(app_theme.background)
                        .child(
                            h_flex()
                                .items_start()
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
                                                .child("接收 BTCC"),
                                        )
                                        .when_some(wallet_name, |el, name| {
                                            el.child(
                                                div()
                                                    .text_size(px(13.))
                                                    .text_color(palette::muted(&app_theme))
                                                    .child(format!("钱包: {name}")),
                                            )
                                        })
                                        .child(
                                            div()
                                                .text_size(px(12.))
                                                .line_height(px(18.))
                                                .text_color(palette::muted(&app_theme))
                                                .child("Only send BTCC assets to this address. Double-check the address before transferring."),
                                        ),
                                )
                                .child(
                                    Button::new("close-receive-panel")
                                        .outline()
                                        .xsmall()
                                        .label("关闭")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.close_receive(cx);
                                        })),
                                ),
                        )
                        .child(
                            v_flex()
                                .gap_3()
                                .child(
                                    div()
                                        .text_size(px(13.))
                                        .font_semibold()
                                        .text_color(palette::text_strong(&app_theme))
                                        .child("收款地址"),
                                )
                                .child(
                                    div()
                                        .w_full()
                                        .p_4()
                                        .rounded(px(8.))
                                        .bg(palette::surface(&app_theme))
                                        .border_1()
                                        .border_color(palette::border_soft(&app_theme))
                                        .font_family("monospace")
                                        .text_size(px(15.))
                                        .line_height(px(24.))
                                        .text_color(palette::text_strong(&app_theme))
                                        .child(address.clone()),
                                ),
                        )
                        .child(
                            v_flex()
                                .items_center()
                                .gap_3()
                                .p_4()
                                .rounded(px(10.))
                                .border_1()
                                .border_color(palette::border(&app_theme))
                                .bg(app_theme.background)
                                .child(
                                    v_flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            div()
                                                .text_size(px(13.))
                                                .font_semibold()
                                                .text_color(palette::text_strong(&app_theme))
                                                .child("Receive QR Code"),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(12.))
                                                .text_color(palette::muted(&app_theme))
                                                .child("Use your phone to scan the code for payment."),
                                        ),
                                )
                                .child(
                                    div()
                                        .p_3()
                                        .rounded(px(8.))
                                        .bg(gpui::white())
                                        .border_1()
                                        .border_color(palette::border_soft(&app_theme))
                                        .when_some(self.receive_qr_path.clone(), |el, path| {
                                            el.child(img(path).w(px(240.)).h(px(240.)))
                                        })
                                        .when_some(self.receive_qr_error.clone(), |el, err| {
                                            el.child(
                                                div()
                                                    .w(px(240.))
                                                    .h(px(240.))
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .text_align(TextAlign::Center)
                                                    .text_color(palette::error_text())
                                                    .child(err),
                                            )
                                        }),
                                ),
                        )
                        .child(
                            h_flex()
                                .justify_end()
                                .gap_2()
                                .child(
                                    Clipboard::new("copy-receive-address-inline")
                                        .value(address.clone()),
                                ),
                        ),
                )
                .into_any_element(),
        )
    }

    pub(super) fn render_delete_confirm(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let wallet_id = self.delete_confirm_wallet_id?;
        let wallet = self.wallets.iter().find(|wallet| wallet.id == wallet_id)?;
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
                        .w(px(520.))
                        .gap_4()
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
                                .child("删除钱包确认"),
                        )
                        .child(
                            div()
                                .text_size(px(13.))
                                .line_height(px(22.))
                                .text_color(palette::muted(&app_theme))
                                .child("This wallet still has a balance. Confirm that you have backed up the mnemonic or private key before deleting."),
                        )
                        .child(readonly_field("钱包名称", wallet.name.clone(), cx))
                        .child(readonly_field(
                            "当前余额",
                            format_sats_trunc_2(wallet.balance_sats),
                            cx,
                        ))
                        .child(
                            h_flex()
                                .justify_end()
                                .gap_2()
                                .child(
                                    Button::new("btcc-wallet-delete-cancel")
                                        .ghost()
                                        .small()
                                        .label("取消")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.cancel_delete_wallet(cx);
                                        })),
                                )
                                .child(
                                    Button::new("btcc-wallet-delete-confirm")
                                        .primary()
                                        .small()
                                        .label("确定，已备份")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.confirm_delete_wallet(cx);
                                        })),
                                ),
                        ),
                )
                .into_any_element(),
        )
    }

    pub(super) fn open_export(&mut self, wallet_id: i64, window: &mut Window, cx: &mut Context<Self>) {
        self.export_wallet_id = Some(wallet_id);
        self.exported_secrets = None;
        self.error = None;
        self.status = None;
        self.action_password_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        cx.notify();
    }

    fn close_export(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.export_wallet_id = None;
        self.exported_secrets = None;
        self.error = None;
        self.action_password_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        cx.notify();
    }

    fn decrypt_export(&mut self, cx: &mut Context<Self>) {
        let Some(wallet_id) = self.export_wallet_id else {
            self.error = Some("请先选择要导出的钱包".to_string());
            cx.notify();
            return;
        };
        let password = self.action_password_input.read(cx).text().to_string();
        match decrypt_btcc_wallet_secrets_blocking(wallet_id, password) {
            Ok(secrets) => {
                self.exported_secrets = Some(secrets);
                self.status = None;
                self.error = None;
            }
            Err(err) => {
                self.exported_secrets = None;
                self.error = Some(err.to_string());
                self.status = None;
            }
        }
        cx.notify();
    }

    pub(super) fn render_header(&self, cx: &mut Context<Self>) -> AnyElement {
        let app_theme = cx.theme().clone();
        let total_balance: i64 = self.wallets.iter().map(|wallet| wallet.balance_sats).sum();
        let total_utxo: i64 = self.wallets.iter().map(|wallet| wallet.utxo_count).sum();
        let active_count = self
            .wallets
            .iter()
            .filter(|wallet| wallet.balance_sats > 0)
            .count();

        v_flex()
            .gap_4()
            .p_6()
            .rounded(px(12.))
            .border_1()
            .border_color(palette::border(&app_theme))
            .bg(app_theme.background)
            .child(
                h_flex()
                    .items_start()
                    .justify_between()
                    .gap_6()
                    .child(
                        v_flex()
                            .flex_1()
                            .gap_2()
                            .child(
                                div()
                                    .text_size(px(24.))
                                    .font_semibold()
                                    .text_color(palette::text_strong(&app_theme))
                                    .child("BTCC 钱包列表"),
                            )
                            .child(
                                div()
                                    .max_w(px(560.))
                                    .text_size(px(12.))
                                    .line_height(px(20.))
                                    .text_color(palette::muted(&app_theme))
                                    .child(
                                        "创建钱包后请先离线备份助记词，并完成 3 个随机单词校验后再保存。",
                                    ),
                            ),
                    )
                    .when(self.vault_initialized && self.vault_unlocked, |el| {
                        el.child(
                            v_flex().w(px(360.)).items_end().gap_3().child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        Button::new("btcc-wallet-create")
                                            .label("+ 创建钱包")
                                            .primary()
                                            .small()
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.open_create_editor(window, cx);
                                            })),
                                    )
                                    .child(
                                        Button::new("btcc-wallet-import")
                                            .label("导入钱包")
                                            .outline()
                                            .small()
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.open_import_editor(window, cx);
                                            })),
                                    )
                                    .child(
                                        Button::new("btcc-wallet-list-refresh")
                                            .label(if self.loading {
                                                "刷新中..."
                                            } else {
                                                "刷新"
                                            })
                                            .ghost()
                                            .small()
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.refresh_wallet_balances(cx);
                                            })),
                                    ),
                            ),
                        )
                    }),
            )
            .when(self.vault_initialized && self.vault_unlocked, |el| {
                el.child(
                    h_flex()
                        .gap_3()
                        .child(render_overview_chip(
                            "钱包数量",
                            self.wallets.len().to_string(),
                            app_theme.primary.opacity(0.08),
                            app_theme.primary.opacity(0.92),
                            false,
                            cx,
                        ))
                        .child(render_overview_chip(
                            "Has Balance",
                            active_count.to_string(),
                            app_theme.success.opacity(0.08),
                            app_theme.success.opacity(0.92),
                            false,
                            cx,
                        ))
                        .child(render_overview_chip(
                            "UTXO",
                            total_utxo.to_string(),
                            app_theme.warning.opacity(0.08),
                            app_theme.warning.opacity(0.92),
                            false,
                            cx,
                        ))
                        .child(render_total_balance_chip(
                            if self.show_total_balance {
                                format_sats_trunc_2(total_balance)
                            } else {
                                "******".to_string()
                            },
                            app_theme.info.opacity(0.08),
                            app_theme.info.opacity(0.92),
                            self.show_total_balance,
                            cx,
                        ))
                        .child(
                            v_flex()
                                .w(px(320.))
                                .h(px(108.))
                                .justify_between()
                                .gap_2()
                                .p_4()
                                .rounded(px(12.))
                                .border_1()
                                .border_color(app_theme.primary.opacity(0.18))
                                .bg(app_theme.primary.opacity(0.05))
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .text_color(palette::muted(&app_theme))
                                        .child("搜索"),
                                )
                                .child(
                                    div().w_full().child(Input::new(&self.search_input).small()),
                                ),
                        ),
                )
            })
            .when_some(
                self.error
                    .clone()
                    .filter(|_| self.vault_initialized && self.vault_unlocked && !self.editor_open),
                |el, error| el.child(inline_error(error, cx)),
            )
            .into_any_element()
    }

    pub(super) fn render_vault_setup(&self, cx: &mut Context<Self>) -> AnyElement {
        let app_theme = cx.theme().clone();
        div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .bg(app_theme.background)
            .flex()
            .items_center()
            .justify_center()
            .child(
                v_flex()
                    .w(px(440.))
                    .gap_5()
                    .p_6()
                    .rounded(px(12.))
                    .shadow_lg()
                    .border_1()
                    .border_color(palette::border(&app_theme))
                    .bg(app_theme.background)
                    .child(
                        v_flex()
                            .gap_2()
                            .child(
                                div()
                                    .text_size(px(18.))
                                    .font_semibold()
                                    .text_color(palette::text_strong(&app_theme))
                                    .child("设置 BTCC 钱包密码"),
                            )
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .line_height(px(20.))
                                    .text_color(palette::muted(&app_theme))
                                    .child("首次使用需要设置钱包密码。密码至少 6 位，且必须同时包含字母和数字。后续创建和导入的钱包都会用这个密码加密。"),
                            ),
                    )
                    .child(field("钱包密码", self.vault_password_input.clone()))
                    .child(field("确认密码", self.vault_confirm_input.clone()))
                    .child(
                        h_flex().justify_end().child(
                            Button::new("create-btcc-wallet-vault-password")
                                .label("保存密码")
                                .primary()
                                .small()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.create_vault_password(window, cx);
                                })),
                        ),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn render_vault_unlock(&self, cx: &mut Context<Self>) -> AnyElement {
        let app_theme = cx.theme().clone();
        div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .bg(app_theme.background)
            .flex()
            .items_center()
            .justify_center()
            .child(
                v_flex()
                    .w(px(400.))
                    .gap_5()
                    .p_6()
                    .rounded(px(12.))
                    .shadow_lg()
                    .border_1()
                    .border_color(palette::border(&app_theme))
                    .bg(app_theme.background)
                    .child(
                        v_flex()
                            .gap_2()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_color(app_theme.primary)
                                            .child("🔒"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(18.))
                                            .font_semibold()
                                            .text_color(palette::text_strong(&app_theme))
                                            .child("解锁 BTCC 钱包"),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .line_height(px(20.))
                                    .text_color(palette::muted(&app_theme))
                                    .child("请输入钱包密码以解锁钱包列表、导入、导出和编辑功能。"),
                            ),
                    )
                    .child(field("钱包密码", self.unlock_password_input.clone()))
                    .when_some(self.error.clone(), |el, error| {
                        el.child(inline_error(error, cx))
                    })
                    .child(
                        h_flex().justify_end().child(
                            Button::new("unlock-btcc-wallet-vault")
                                .label("解锁")
                                .primary()
                                .small()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.unlock_vault_password(window, cx);
                                })),
                        ),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn render_editor(&self, cx: &mut Context<Self>) -> AnyElement {
        match self.editor_mode {
            EditorMode::CreateMnemonic => self.render_mnemonic_step(cx),
            EditorMode::VerifyMnemonic => self.render_verify_step(cx),
            EditorMode::ImportMnemonic => self.render_import_mnemonic(cx),
            EditorMode::EditExisting => self.render_edit_existing(cx),
        }
    }

    fn render_import_mnemonic(&self, cx: &mut Context<Self>) -> AnyElement {
        let app_theme = cx.theme().clone();
        let importing_mnemonic = self.import_mode == ImportMode::Mnemonic;
        v_flex()
            .w_full()
            .mt_3()
            .gap_3()
            .p_4()
            .rounded(px(8.))
            .border_1()
            .border_color(app_theme.primary.opacity(0.35))
            .bg(app_theme.background)
            .child(editor_title("导入钱包", cx))
            .child(
                div()
                    .text_size(px(12.))
                    .line_height(px(20.))
                    .text_color(palette::muted(&app_theme))
                    .child("选择助记词或 WIF 私钥导入。地址唯一，已存在的钱包不会重复保存。"),
            )
            .child(field("钱包名称", self.name_input.clone()))
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("btcc-wallet-import-mode-mnemonic")
                            .label("助记词")
                            .small()
                            .when(importing_mnemonic, |this| this.primary())
                            .when(!importing_mnemonic, |this| this.ghost())
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.set_import_mode(ImportMode::Mnemonic, window, cx);
                            })),
                    )
                    .child(
                        Button::new("btcc-wallet-import-mode-wif")
                            .label("WIF 私钥")
                            .small()
                            .when(!importing_mnemonic, |this| this.primary())
                            .when(importing_mnemonic, |this| this.ghost())
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.set_import_mode(ImportMode::Wif, window, cx);
                            })),
                    ),
            )
            .child(if importing_mnemonic {
                mnemonic_field("助记词", self.import_mnemonic_input.clone())
            } else {
                mnemonic_field("WIF 私钥", self.import_wif_input.clone())
            })
            .child(note_single_line_field("备注", self.note_input.clone()))
            .when_some(self.error.clone(), |el, error| {
                el.child(inline_error(error, cx))
            })
            .when_some(self.status.clone(), |el, status| {
                el.child(inline_status(status, cx))
            })
            .child(
                h_flex()
                    .items_end()
                    .gap_3()
                    .child(div().flex_1().child(password_field(
                        "钱包初始密码",
                        self.action_password_input.clone(),
                    )))
                    .child(
                        h_flex()
                            .gap_2()
                            .pb(px(1.))
                            .child(
                                Button::new("btcc-wallet-import-close")
                                    .label("关闭")
                                    .ghost()
                                    .small()
                                    .on_click(cx.listener(|this, _, _, cx| this.close_editor(cx))),
                            )
                            .child(
                                Button::new("btcc-wallet-import-save")
                                    .label("导入并保存")
                                    .primary()
                                    .small()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.import_and_save_wallet(window, cx);
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn render_mnemonic_step(&self, cx: &mut Context<Self>) -> AnyElement {
        let app_theme = cx.theme().clone();
        let wallet = self.generated_wallet.as_ref();

        v_flex()
            .mt_3()
            .gap_3()
            .p_4()
            .rounded(px(8.))
            .border_1()
            .border_color(app_theme.primary.opacity(0.35))
            .bg(app_theme.background)
            .child(editor_title("创建钱包 1/2", cx))
            .child(
                div()
                    .w_full()
                    .p_3()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(app_theme.warning.opacity(0.35))
                    .bg(app_theme.warning.opacity(0.08))
                    .text_size(px(12.))
                    .text_color(app_theme.warning.opacity(0.95))
                    .child("助记词控制资产。请离线备份，不要截图，也不要通过网络发送。"),
            )
            .child(
                h_flex().w_full().child(
                    div()
                        .w(px(360.))
                        .child(field("钱包名称", self.name_input.clone())),
                ),
            )
            .when_some(wallet, |el, wallet| {
                let address = wallet.address.clone();
                el.child(
                    v_flex()
                        .w_full()
                        .gap_3()
                        .child(
                            h_flex()
                                .gap_3()
                                .items_start()
                                .w_full()
                                .child(div().flex_1().child(copyable_readonly_field(
                                    "BTCC 地址",
                                    wallet.address.clone(),
                                    "BTCC 地址",
                                    address,
                                    cx,
                                )))
                                .child(div().w(px(220.)).child(readonly_field(
                                    "派生路径",
                                    wallet.derivation_path.clone(),
                                    cx,
                                ))),
                        )
                        .child(mnemonic_grid(mnemonic_words(wallet), cx)),
                )
            })
            .child(
                h_flex()
                    .w_full()
                    .items_end()
                    .gap_3()
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w(px(420.))
                            .gap_2()
                            .child(div().text_size(px(12.)).child("备注"))
                            .child(Input::new(&self.note_input).w_full()),
                    )
                    .child(div().w(px(320.)).child(password_field(
                        "钱包初始密码",
                        self.action_password_input.clone(),
                    )))
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .pb(px(1.))
                            .child(
                                Button::new("btcc-wallet-create-close")
                                    .label("关闭")
                                    .ghost()
                                    .small()
                                    .on_click(cx.listener(|this, _, _, cx| this.close_editor(cx))),
                            )
                            .child(
                                Button::new("btcc-wallet-create-next")
                                    .label("下一步")
                                    .primary()
                                    .small()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.start_verify_generated_wallet(window, cx);
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn render_verify_step(&self, cx: &mut Context<Self>) -> AnyElement {
        let app_theme = cx.theme().clone();
        v_flex()
            .mt_3()
            .gap_3()
            .p_4()
            .rounded(px(8.))
            .border_1()
            .border_color(app_theme.primary.opacity(0.35))
            .bg(app_theme.background)
            .child(editor_title("创建钱包 2/2", cx))
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(palette::muted(&app_theme))
                    .child("请填写下方要求校验的助记词。校验通过后，钱包才会正式保存。"),
            )
            .child(
                h_flex().w_full().gap_4().items_start().children(
                    self.verify_positions
                        .iter()
                        .enumerate()
                        .map(|(index, pos)| {
                            div()
                                .w(px(240.))
                                .child(verify_word_field(
                                    Box::leak(format!("第 {} 个单词", pos + 1).into_boxed_str()),
                                    self.verify_inputs[index].clone(),
                                    cx,
                                ))
                                .into_any_element()
                        }),
                ),
            )
            .when_some(self.error.clone(), |el, error| {
                el.child(inline_error(error, cx))
            })
            .when_some(self.status.clone(), |el, status| {
                el.child(inline_status(status, cx))
            })
            .child(
                h_flex()
                    .items_end()
                    .gap_3()
                    .child(div().w(px(320.)).child(password_field(
                        "钱包密码",
                        self.action_password_input.clone(),
                    )))
                    .child(div().flex_1())
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .pb(px(1.))
                            .child(
                                Button::new("btcc-wallet-verify-close")
                                    .label("关闭")
                                    .ghost()
                                    .small()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.close_editor(cx);
                                    })),
                            )
                            .child(
                                Button::new("btcc-wallet-verify-back")
                                    .label("返回")
                                    .ghost()
                                    .small()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.editor_mode = EditorMode::CreateMnemonic;
                                        this.error = None;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("btcc-wallet-verify-save")
                                    .label("校验并保存")
                                    .primary()
                                    .small()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.verify_and_save_generated_wallet(window, cx);
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn render_edit_existing(&self, cx: &mut Context<Self>) -> AnyElement {
        let app_theme = cx.theme().clone();
        let wallet_address = self
            .selected_id
            .and_then(|id| self.wallets.iter().find(|wallet| wallet.id == id))
            .map(|wallet| wallet.address.clone())
            .unwrap_or_default();
        v_flex()
            .mt_3()
            .gap_3()
            .p_4()
            .rounded(px(8.))
            .border_1()
            .border_color(app_theme.primary.opacity(0.35))
            .bg(app_theme.background)
            .child(editor_title("修改钱包", cx))
            .child(field("名称", self.name_input.clone()))
            .child(readonly_field(
                "BTCC 地址",
                wallet_address,
                cx,
            ))
            .child(note_single_line_field("备注", self.note_input.clone()))
            .child(
                h_flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("btcc-wallet-edit-close")
                            .label("关闭")
                            .ghost()
                            .small()
                            .on_click(cx.listener(|this, _, _, cx| this.close_editor(cx))),
                    )
                    .child(
                        Button::new("btcc-wallet-edit-save")
                            .label("保存")
                            .primary()
                            .small()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.save_existing_wallet(window, cx);
                            })),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn render_export_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let app_theme = cx.theme().clone();
        let wallet = self
            .export_wallet_id
            .and_then(|id| self.wallets.iter().find(|wallet| wallet.id == id));

        v_flex()
            .mt_3()
            .gap_3()
            .p_4()
            .rounded(px(8.))
            .border_1()
            .border_color(app_theme.warning.opacity(0.35))
            .bg(app_theme.background)
            .child(editor_title("导出钱包", cx))
            .child(
                div()
                    .text_size(px(12.))
                    .line_height(px(20.))
                    .text_color(app_theme.warning.opacity(0.95))
                    .child("输入钱包密码后才会解密显示助记词和 WIF 私钥。导出后请在安全环境中保存，关闭面板会清空本次显示内容。"),
            )
            .when_some(wallet.cloned(), |el, wallet| {
                el.child(readonly_field("钱包名称", wallet.name, cx))
                    .child(readonly_field("BTCC 地址", wallet.address, cx))
            })
            .child(field("钱包密码", self.action_password_input.clone()))
            .child(
                h_flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("btcc-wallet-export-close")
                            .label("关闭")
                            .ghost()
                            .small()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.close_export(window, cx);
                            })),
                    )
                    .child(
                        Button::new("btcc-wallet-export-decrypt")
                            .label("解密导出")
                            .primary()
                            .small()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.decrypt_export(cx);
                            })),
                    ),
            )
            .when_some(self.exported_secrets.clone(), |el, secrets| {
                let mnemonic = secrets.mnemonic.clone();
                let wif = secrets.private_key_wif.clone();
                el.child(copyable_secret_field("助记词", mnemonic, "助记词", cx))
                    .child(copyable_secret_field("WIF 私钥", wif, "WIF 私钥", cx))
            })
            .into_any_element()
    }

    pub(super) fn render_table(&self, cx: &mut Context<Self>) -> AnyElement {
        let app_theme = cx.theme().clone();
        let row_count = self.display_wallets.len();

        v_flex()
            .mt_3()
            .rounded(px(8.))
            .border_1()
            .border_color(palette::border(&app_theme))
            .bg(app_theme.background)
            .overflow_hidden()
            .child(self.render_table_header(cx))
            .when(self.wallets.is_empty(), |el| {
                el.child(
                    div()
                        .p_6()
                        .text_size(px(13.))
                        .text_color(palette::muted(&app_theme))
                        .child("还没有钱包。点击右上角创建钱包，生成并校验助记词后即可保存。"),
                )
            })
            .when(
                !self.wallets.is_empty() && self.display_wallets.is_empty(),
                |el| {
                    el.child(
                        div()
                            .p_6()
                            .text_size(px(13.))
                            .text_color(palette::muted(&app_theme))
                            .child("未找到匹配的钱包。名称支持模糊搜索，地址需要完整精确匹配。"),
                    )
                },
            )
            .when(!self.display_wallets.is_empty(), |el| {
                if row_count <= 10 {
                    el.child(div().children(self.display_wallets.iter().enumerate().map(
                        |(index, wallet)| {
                            self.render_wallet_row(wallet, index + 1 == row_count, cx)
                        },
                    )))
                } else {
                    el.child(
                        div().h(px(440.)).overflow_y_scrollbar().children(
                            self.display_wallets
                                .iter()
                                .enumerate()
                                .map(|(index, wallet)| {
                                    self.render_wallet_row(wallet, index + 1 == row_count, cx)
                                }),
                        ),
                    )
                }
            })
            .into_any_element()
    }

    fn render_table_header(&self, cx: &mut Context<Self>) -> AnyElement {
        let app_theme = cx.theme().clone();
        h_flex()
            .items_center()
            .px_4()
            .py_2()
            .gap_3()
            .bg(app_theme.muted.opacity(0.08))
            .border_b_1()
            .border_color(palette::border(&app_theme))
            .text_size(px(12.))
            .text_color(palette::muted(&app_theme))
            .child(col("钱包名称", 160.))
            .child(header_with_eye(
                "BTCC 地址",
                390.,
                self.show_wallet_addresses,
                "btcc-wallet-toggle-addresses",
                cx.listener(|this, _, _, cx| {
                    this.toggle_wallet_addresses_visibility(cx);
                }),
                cx,
            ))
            .child(header_with_eye(
                "余额",
                140.,
                self.show_wallet_balances,
                "btcc-wallet-toggle-balances",
                cx.listener(|this, _, _, cx| {
                    this.toggle_wallet_balances_visibility(cx);
                }),
                cx,
            ))
            .child(col("Unconfirmed", 100.))
            .child(col("UTXO", 60.))
            .child(col("来源", 86.))
            .child(col("最近同步", 146.))
            .child(div().w(px(166.)).pl(px(8.)).child("操作"))
            .into_any_element()
    }

    fn render_wallet_row(
        &self,
        wallet: &BtccWalletRecord,
        is_last: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let app_theme = cx.theme().clone();
        let address = wallet.address.clone();
        let transfer_address = wallet.address.clone();
        let transfer_id = wallet.id;
        let receive_id = wallet.id;
        let receive_address = wallet.address.clone();
        let history_address = wallet.address.clone();
        let export_id = wallet.id;
        let edit_id = wallet.id;
        let delete_id = wallet.id;
        let address_display = if self.show_wallet_addresses {
            wallet.address.clone()
        } else {
            mask_wallet_address(&wallet.address)
        };
        let balance_display = if self.show_wallet_balances {
            format_sats_plain(wallet.balance_sats)
        } else {
            "******".to_string()
        };
        let unconfirmed_display = if self.show_wallet_balances {
            format_sats_plain(wallet.unconfirmed_sats)
        } else {
            "******".to_string()
        };

        h_flex()
            .items_center()
            .px_4()
            .py_2()
            .gap_3()
            .min_h(px(40.))
            .when(!is_last, |row| {
                row.border_b_1().border_color(palette::border(&app_theme))
            })
            .text_size(px(12.))
            .child(
                h_flex()
                    .w(px(160.))
                    .gap_1()
                    .items_center()
                    .child(
                        div()
                            .size(px(14.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(palette::muted(&app_theme))
                            .child(IconName::Folder),
                    )
                    .child(
                        div()
                            .font_semibold()
                            .text_color(palette::text_strong(&app_theme))
                            .child(wallet.name.clone()),
                    ),
            )
            .child(
                h_flex()
                    .w(px(390.))
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .id(("btcc-wallet-address-tooltip", wallet.id as u64))
                            .flex_1()
                            .overflow_hidden()
                            .font_family("monospace")
                            .text_color(palette::text_strong(&app_theme))
                            .child(address_display)
                            .tooltip({
                                let full_address = address.clone();
                                move |window, cx| {
                                    gpui_component::tooltip::Tooltip::new(full_address.clone())
                                        .build(window, cx)
                                }
                            }),
                    )
                    .child(
                        Clipboard::new(("btcc-wallet-address-copy", wallet.id as u64))
                            .value(address.clone()),
                    ),
            )
            .child(amount_cell(balance_display, 140., cx))
            .child(amount_cell(unconfirmed_display, 100., cx))
            .child(center_cell(wallet.utxo_count.to_string(), 60., cx))
            .child(center_cell(wallet.source_type.clone(), 86., cx))
            .child(cell(
                wallet
                    .last_synced_at
                    .clone()
                    .unwrap_or_else(|| "--".to_string()),
                146.,
                cx,
            ))
            .child(
                h_flex()
                    .w(px(166.))
                    .gap_2()
                    .items_center()
                    .justify_end()
                    .pl(px(8.))
                    .child(
                        Button::new(("btcc-wallet-transfer", wallet.id as u64))
                            .primary()
                            .xsmall()
                            .child(
                                h_flex()
                                    .gap_1()
                                    .items_center()
                                    .child("发送")
                                    .child(IconName::ArrowUp),
                            )
                            .on_click({
                                let transfer_id = transfer_id;
                                let transfer_address = transfer_address.clone();
                                cx.listener(move |this, _, _, cx| {
                                    this.open_transfer(transfer_id, transfer_address.clone(), cx);
                                })
                            }),
                    )
                    .child(
                        Button::new(("btcc-wallet-receive", wallet.id as u64))
                            .primary()
                            .xsmall()
                            .child(
                                h_flex()
                                    .gap_1()
                                    .items_center()
                                    .child("接收")
                                    .child(IconName::ArrowDown),
                            )
                            .on_click({
                                let receive_id = receive_id;
                                let receive_address = receive_address.clone();
                                cx.listener(move |this, _, window, cx| {
                                    this.open_receive(
                                        receive_id,
                                        receive_address.clone(),
                                        window,
                                        cx,
                                    );
                                })
                            }),
                    )
                    .child(
                        Button::new(("btcc-wallet-actions", wallet.id as u64))
                            .ghost()
                            .xsmall()
                            .compact()
                            .icon(IconName::Ellipsis)
                            .dropdown_menu({
                                let history_address = history_address.clone();
                                let export_id = export_id;
                                let edit_id = edit_id;
                                let delete_id = delete_id;
                                move |menu, _window, _cx| {
                                    menu.menu(
                                        "历史",
                                        Box::new(WalletAction::History {
                                            address: history_address.clone(),
                                        }),
                                    )
                                    .menu("导出", Box::new(WalletAction::Export { id: export_id }))
                                    .menu("修改", Box::new(WalletAction::Edit { id: edit_id }))
                                    .separator()
                                    .menu("删除", Box::new(WalletAction::Delete { id: delete_id }))
                                }
                            }),
                    ),
            )
            .into_any_element()
    }
}
