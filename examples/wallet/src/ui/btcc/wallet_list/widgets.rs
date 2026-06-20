use super::*;

pub(super) fn field(label: &'static str, input: Entity<InputState>) -> AnyElement {
    v_flex()
        .w(px(360.))
        .gap_2()
        .child(div().text_size(px(12.)).child(label))
        .child(Input::new(&input).w(px(360.)))
        .into_any_element()
}

pub(super) fn render_total_balance_chip(
    value: String,
    bg: Hsla,
    accent: Hsla,
    visible: bool,
    cx: &mut Context<BtccWalletListPage>,
) -> AnyElement {
    let app_theme = cx.theme().clone();
    v_flex()
        .w(px(320.))
        .h(px(108.))
        .justify_between()
        .gap_2()
        .p_4()
        .rounded(px(12.))
        .border_1()
        .border_color(accent.opacity(0.18))
        .bg(bg)
        .child(
            h_flex()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(palette::muted(&app_theme))
                        .child("总余额"),
                )
                .child(
                    Button::new("btcc-wallet-toggle-total-balance")
                        .ghost()
                        .xsmall()
                        .compact()
                        .icon(if visible {
                            IconName::Eye
                        } else {
                            IconName::EyeOff
                        })
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.toggle_total_balance_visibility(cx);
                        })),
                ),
        )
        .child(
            div()
                .text_size(px(24.))
                .font_semibold()
                .line_height(px(28.))
                .font_family("monospace")
                .text_color(accent)
                .child(value),
        )
        .into_any_element()
}

pub(super) fn password_field(label: &'static str, input: Entity<InputState>) -> AnyElement {
    v_flex()
        .w(px(320.))
        .gap_2()
        .child(div().text_size(px(12.)).child(label))
        .child(Input::new(&input).w(px(320.)))
        .into_any_element()
}

pub(super) fn readonly_field(
    label: &'static str,
    value: String,
    cx: &mut Context<BtccWalletListPage>,
) -> AnyElement {
    let app_theme = cx.theme().clone();
    v_flex()
        .gap_2()
        .child(div().text_size(px(12.)).child(label))
        .child(
            div()
                .p_2()
                .rounded(px(6.))
                .border_1()
                .border_color(palette::border(&app_theme))
                .font_family("monospace")
                .text_size(px(12.))
                .text_color(palette::text_strong(&app_theme))
                .child(value),
        )
        .into_any_element()
}

pub(super) fn copyable_readonly_field(
    label: &'static str,
    value: String,
    _copy_label: &'static str,
    copy_value: String,
    cx: &mut Context<BtccWalletListPage>,
) -> AnyElement {
    let app_theme = cx.theme().clone();
    let mut copy_hasher = DefaultHasher::new();
    label.hash(&mut copy_hasher);
    copy_value.hash(&mut copy_hasher);
    let clipboard_id = copy_hasher.finish();
    v_flex()
        .gap_2()
        .child(div().text_size(px(12.)).child(label))
        .child(
            h_flex()
                .items_center()
                .justify_between()
                .gap_2()
                .p_2()
                .rounded(px(6.))
                .border_1()
                .border_color(palette::border(&app_theme))
                .child(
                    div()
                        .flex_1()
                        .font_family("monospace")
                        .text_size(px(12.))
                        .text_color(palette::text_strong(&app_theme))
                        .child(value),
                )
                .child(
                    Clipboard::new(("btcc-wallet-readonly-copy", clipboard_id))
                        .value(copy_value.clone()),
                ),
        )
        .into_any_element()
}

pub(super) fn copyable_secret_field(
    label: &'static str,
    value: String,
    copy_label: &'static str,
    cx: &mut Context<BtccWalletListPage>,
) -> AnyElement {
    let app_theme = cx.theme().clone();
    let copy_value = value.clone();
    let copy_id = if copy_label == "助记词" {
        1_u64
    } else {
        2_u64
    };
    v_flex()
        .gap_2()
        .child(
            h_flex()
                .justify_between()
                .child(div().text_size(px(12.)).child(label))
                .child(
                    Clipboard::new(("btcc-wallet-secret-copy", copy_id))
                        .value(copy_value.clone()),
                ),
        )
        .child(
            div()
                .p_2()
                .rounded(px(6.))
                .border_1()
                .border_color(palette::border(&app_theme))
                .font_family("monospace")
                .text_size(px(12.))
                .line_height(px(20.))
                .text_color(palette::text_strong(&app_theme))
                .child(value),
        )
        .into_any_element()
}

pub(super) fn editor_title(title: &'static str, cx: &mut Context<BtccWalletListPage>) -> AnyElement {
    let app_theme = cx.theme();
    div()
        .text_size(px(16.))
        .font_semibold()
        .text_color(palette::text_strong(app_theme))
        .child(title)
        .into_any_element()
}

pub(super) fn mnemonic_grid(words: Vec<&str>, cx: &mut Context<BtccWalletListPage>) -> AnyElement {
    let app_theme = cx.theme().clone();
    v_flex()
        .gap_3()
        .children(words.chunks(6).enumerate().map(|(row_index, row)| {
            h_flex()
                .w_full()
                .gap_3()
                .children(row.iter().enumerate().map(|(col_index, word)| {
                    let index = row_index * 6 + col_index;
                    h_flex()
                        .gap_2()
                        .flex_1()
                        .min_w(px(0.))
                        .px_3()
                        .py_2()
                        .rounded(px(6.))
                        .border_1()
                        .border_color(palette::border(&app_theme))
                        .bg(app_theme.muted.opacity(0.08))
                        .child(
                            div()
                                .w(px(22.))
                                .text_size(px(11.))
                                .text_color(palette::muted(&app_theme))
                                .child(format!("{}", index + 1)),
                        )
                        .child(
                            div()
                                .font_family("monospace")
                                .text_size(px(15.))
                                .text_color(palette::text_strong(&app_theme))
                                .child((*word).to_string()),
                        )
                        .into_any_element()
                }))
                .into_any_element()
        }))
        .into_any_element()
}

pub(super) fn mnemonic_field(label: &'static str, input: Entity<InputState>) -> AnyElement {
    v_flex()
        .w(px(720.))
        .gap_2()
        .child(div().text_size(px(12.)).child(label))
        .child(Input::new(&input).w(px(720.)).h(px(96.)))
        .into_any_element()
}

pub(super) fn header_with_eye<F>(
    label: &'static str,
    width: f32,
    visible: bool,
    id: &'static str,
    on_click: F,
    cx: &mut Context<BtccWalletListPage>,
) -> AnyElement
where
    F: Fn(&ClickEvent, &mut Window, &mut App) + 'static,
{
    let app_theme = cx.theme().clone();
    h_flex()
        .w(px(width))
        .items_center()
        .justify_between()
        .gap_2()
        .child(label)
        .child(
            Button::new(id)
                .ghost()
                .xsmall()
                .compact()
                .icon(if visible {
                    IconName::Eye
                } else {
                    IconName::EyeOff
                })
                .text_color(palette::muted(&app_theme))
                .on_click(on_click),
        )
        .into_any_element()
}

pub(super) fn note_single_line_field(label: &'static str, input: Entity<InputState>) -> AnyElement {
    v_flex()
        .w(px(900.))
        .gap_2()
        .child(div().text_size(px(12.)).child(label))
        .child(Input::new(&input).w(px(900.)))
        .into_any_element()
}

pub(super) fn verify_word_field(
    label: &'static str,
    input: Entity<InputState>,
    cx: &mut Context<BtccWalletListPage>,
) -> AnyElement {
    let app_theme = cx.theme().clone();
    v_flex()
        .w_full()
        .gap_2()
        .child(
            div()
                .text_size(px(12.))
                .font_semibold()
                .text_color(palette::text_strong(&app_theme))
                .child(label),
        )
        .child(
            div()
                .w_full()
                .h(px(44.))
                .child(Input::new(&input).w_full().h_full()),
        )
        .into_any_element()
}

pub(super) fn col(label: &'static str, width: f32) -> AnyElement {
    h_flex()
        .w(px(width))
        .items_center()
        .child(label)
        .into_any_element()
}

pub(super) fn cell(value: String, width: f32, cx: &mut Context<BtccWalletListPage>) -> AnyElement {
    h_flex()
        .w(px(width))
        .items_center()
        .child(div().text_color(palette::muted(cx.theme())).child(value))
        .into_any_element()
}

pub(super) fn center_cell(value: String, width: f32, cx: &mut Context<BtccWalletListPage>) -> AnyElement {
    h_flex()
        .w(px(width))
        .items_center()
        .justify_center()
        .child(div().text_color(palette::muted(cx.theme())).child(value))
        .into_any_element()
}

pub(super) fn amount_cell(value: String, width: f32, cx: &mut Context<BtccWalletListPage>) -> AnyElement {
    h_flex()
        .w(px(width))
        .items_center()
        .justify_end()
        .child(
            div()
                .font_family("monospace")
                .text_color(palette::muted(cx.theme()))
                .child(value),
        )
        .into_any_element()
}

pub(super) fn inline_error(text: String, cx: &mut Context<BtccWalletListPage>) -> AnyElement {
    div()
        .text_size(px(12.))
        .text_color(cx.theme().danger)
        .child(text)
        .into_any_element()
}

pub(super) fn inline_status(text: String, cx: &mut Context<BtccWalletListPage>) -> AnyElement {
    div()
        .text_size(px(12.))
        .text_color(cx.theme().primary)
        .child(text)
        .into_any_element()
}

pub(super) fn floating_status(text: String, cx: &mut Context<BtccWalletListPage>) -> AnyElement {
    let app_theme = cx.theme().clone();

    div()
        .absolute()
        .top(px(20.))
        .right(px(20.))
        .max_w(px(360.))
        .px_3()
        .py_2()
        .rounded(px(8.))
        .border_1()
        .border_color(app_theme.primary.opacity(0.22))
        .bg(app_theme.primary.opacity(0.10))
        .text_size(px(12.))
        .text_color(app_theme.primary)
        .shadow_md()
        .child(text)
        .into_any_element()
}

pub(super) fn format_sats_plain(sats: i64) -> String {
    if sats == 0 {
        "--".to_string()
    } else {
        format!("{:.8}", sats as f64 / 100_000_000.0)
    }
}

pub(super) fn mask_wallet_address(address: &str) -> String {
    let chars: Vec<char> = address.chars().collect();
    if chars.len() <= 10 {
        address.to_string()
    } else {
        let prefix: String = chars.iter().take(5).collect();
        let suffix: String = chars
            .iter()
            .rev()
            .take(5)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        format!("{prefix}...{suffix}")
    }
}

pub(super) fn format_sats_trunc_2(sats: i64) -> String {
    if sats == 0 {
        "0.00 BTCC".to_string()
    } else {
        let sign = if sats < 0 { "-" } else { "" };
        let abs = sats.abs();
        let whole = abs / 100_000_000;
        let decimals = (abs % 100_000_000) / 1_000_000;
        format!("{sign}{whole}.{decimals:02} BTCC")
    }
}

pub(super) fn mnemonic_words(wallet: &BitcoinWallet) -> Vec<&str> {
    wallet.mnemonic.split_whitespace().collect()
}

pub(super) fn choose_verify_positions(word_count: usize) -> Vec<usize> {
    let mut seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0xC0FFEE);
    let mut positions = Vec::new();
    while positions.len() < 3 {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let position = (seed as usize) % word_count;
        if !positions.contains(&position) {
            positions.push(position);
        }
    }
    positions.sort_unstable();
    positions
}

pub(super) fn is_strong_vault_password(value: &str) -> bool {
    value.chars().count() >= 6
        && value.chars().any(|ch| ch.is_ascii_alphabetic())
        && value.chars().any(|ch| ch.is_ascii_digit())
}

pub(super) fn is_password_policy_error(value: &str) -> bool {
    value.contains("must be at least 6 characters and include letters and numbers")
}
