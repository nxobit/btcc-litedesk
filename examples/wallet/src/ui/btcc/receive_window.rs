use crate::ui::palette;
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    ActiveTheme, IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    h_flex, v_flex,
};

pub struct ReceiveWindow {
    address: String,
    wallet_name: Option<String>,
}

impl ReceiveWindow {
    pub fn new(
        address: String,
        wallet_name: Option<String>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Self {
        Self {
            address,
            wallet_name,
        }
    }

    fn copy_address(&self, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(self.address.clone()));
    }
}

impl Render for ReceiveWindow {
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
                            .child("接收 BTCC"),
                    )
                    .when_some(self.wallet_name.clone(), |el, name| {
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
                            .child("请只向这个地址转入 BTCC 资产。转账前请再次核对地址。"),
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
                                                    .child("收款地址"),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(12.))
                                                    .line_height(px(18.))
                                                    .text_color(palette::muted(&app_theme))
                                                    .child("如果要截图，建议先确认当前页面没有暴露其他敏感信息。"),
                                            ),
                                    )
                                    .child(
                                        Button::new("copy-receive-address")
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
                                            .child("二维码"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(palette::muted(&app_theme))
                                            .child("当前版本先关闭二维码预览，避免接收窗口在 Windows 下崩溃。"),
                                    ),
                            )
                            .child(
                                div()
                                    .w(px(240.))
                                    .h(px(240.))
                                    .rounded(px(8.))
                                    .bg(palette::surface(&app_theme))
                                    .border_1()
                                    .border_color(palette::border_soft(&app_theme))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        div()
                                            .w(px(180.))
                                            .text_align(TextAlign::Center)
                                            .text_size(px(12.))
                                            .line_height(px(18.))
                                            .text_color(palette::muted(&app_theme))
                                            .child("请使用上方按钮复制地址后再转账。"),
                                    ),
                            ),
                    ),
            )
    }
}
