use crate::ui::palette;
use anyhow::Context as _;
use gpui::{img, prelude::FluentBuilder, *};
use gpui_component::{
    ActiveTheme, IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    h_flex, v_flex,
};
use qrcode::{QrCode, render::svg};
use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    path::PathBuf,
};

const DONATE_ADDRESS: &str = "cc1quykauu7j98q6xe2af893cf024tdgx3pm4c9g39";

pub struct DonateWindow {
    address: String,
    qr_code_path: Option<PathBuf>,
    qr_error: Option<String>,
}

impl DonateWindow {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        let (qr_code_path, qr_error) = match Self::write_qr_svg(DONATE_ADDRESS) {
            Ok(path) => (Some(path), None),
            Err(err) => (None, Some(err.to_string())),
        };

        Self {
            address: DONATE_ADDRESS.to_string(),
            qr_code_path,
            qr_error,
        }
    }

    fn write_qr_svg(address: &str) -> anyhow::Result<PathBuf> {
        let code = QrCode::new(address.as_bytes()).context("generate QR code failed")?;
        let svg_xml = code
            .render::<svg::Color>()
            .quiet_zone(true)
            .min_dimensions(220, 220)
            .build();

        let mut hasher = DefaultHasher::new();
        address.hash(&mut hasher);

        let output_dir = std::env::temp_dir().join("btcc-litedesk");
        fs::create_dir_all(&output_dir).context("create QR code temp dir failed")?;

        let output_path = output_dir.join(format!("donate-{:016x}.svg", hasher.finish()));
        fs::write(&output_path, svg_xml).context("write QR code svg failed")?;

        Ok(output_path)
    }

    fn copy_address(&self, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(self.address.clone()));
    }
}

impl Render for DonateWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let app_theme = cx.theme().clone();
        let address = self.address.clone();

        v_flex()
            .size_full()
            .p_6()
            .gap_4()
            .bg(app_theme.background)
            .text_size(px(12.))
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(24.))
                            .font_semibold()
                            .text_color(palette::text_strong(&app_theme))
                            .child("打赏支持"),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .line_height(px(18.))
                            .text_color(palette::muted(&app_theme))
                            .child("如果您觉得 BTCC Litedesk 对您有帮助，欢迎打赏支持！"),
                    ),
            )
            .child(
                v_flex()
                    .gap_4()
                    .child(
                        v_flex()
                            .gap_4()
                            .p_5()
                            .rounded(px(10.))
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
                                            .flex_1()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .text_size(px(13.))
                                                    .font_semibold()
                                                    .text_color(palette::text_strong(&app_theme))
                                                    .child("BTCC 打赏地址"),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(12.))
                                                    .line_height(px(18.))
                                                    .text_color(palette::muted(&app_theme))
                                                    .child("请确认转账网络为 BTCC，仅向此地址发送 BTCC 网络资产。"),
                                            ),
                                    )
                                    .child(
                                        Button::new("copy-donate-address")
                                            .ghost()
                                            .small()
                                            .icon(IconName::Copy)
                                            .label("复制地址")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.copy_address(cx);
                                            })),
                                    ),
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
                            .p_5()
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
                                            .child("打赏二维码"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(palette::muted(&app_theme))
                                            .child("扫一扫二维码进行打赏。"),
                                    ),
                            )
                            .child(
                                div()
                                    .p_3()
                                    .rounded(px(8.))
                                    .bg(gpui::white())
                                    .border_1()
                                    .border_color(palette::border_soft(&app_theme))
                                    .when_some(self.qr_code_path.clone(), |el, path| {
                                        el.child(img(path).w(px(240.)).h(px(240.)))
                                    })
                                    .when_some(self.qr_error.clone(), |el, err| {
                                        el.child(
                                            div()
                                                .w(px(240.))
                                                .h(px(240.))
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .text_align(TextAlign::Center)
                                                .text_color(palette::error_text())
                                                .child(format!("二维码生成失败\n{err}")),
                                        )
                                    }),
                            ),
                    ),
            )
    }
}
