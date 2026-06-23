use crate::theme;
use crate::ui::btcc::wallet_list::get_global_vault_password;
use crate::ui::palette;
use gpui::{actions, prelude::FluentBuilder, *};
use gpui_component::{
    ActiveTheme, IconName, Sizable, TitleBar,
    button::{Button, ButtonVariants},
    menu::{DropdownMenu, PopupMenuItem},
};
use std::{fs, path::Path, path::PathBuf};

actions!(
    btcc_litedesk,
    [
        OpenBtccWalletList,
        OpenBtccWalletCreate,
        OpenBtccWalletImport,
        OpenVanityGenerator,
        OpenWalletGenerator,
        OpenWalletManager,
        OpenBatchSend,
        OpenStampMint,
        OpenDonate,
        OpenNftGallery
    ]
);

pub struct DesktopTitleBar {
    active_wallet_count: usize,
}

pub enum DesktopTitleBarEvent {
    OpenVanityGenerator,
    OpenDonate,
    OpenNftGallery,
}

impl EventEmitter<DesktopTitleBarEvent> for DesktopTitleBar {}

impl DesktopTitleBar {
    pub fn new(_: &mut Window, _: &mut Context<Self>) -> Self {
        Self {
            active_wallet_count: 0,
        }
    }

    pub fn set_active_wallet_count(&mut self, count: usize, cx: &mut Context<Self>) {
        self.active_wallet_count = count;
        cx.notify();
    }

    fn ensure_rarity_help_doc() -> Option<PathBuf> {
        let doc_path = std::env::temp_dir()
            .join("btcc-litedesk")
            .join("help")
            .join("nft_rarity_help.md");
        let content = include_str!("../../docs/nft_rarity_help.md");
        if let Some(parent) = doc_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        fs::write(&doc_path, content).ok()?;
        Some(doc_path)
    }

    fn path_to_file_url(path: &Path) -> String {
        format!("file:///{}", path.to_string_lossy().replace('\\', "/"))
    }
}

impl Render for DesktopTitleBar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let app_theme = cx.theme();
        let vault_unlocked = get_global_vault_password().is_some();

        TitleBar::new()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .px_1()
                    .text_size(px(11.))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        Button::new("github-link-menu")
                            .ghost()
                            .xsmall()
                            .compact()
                            .icon(IconName::GitHub)
                            .on_click(|_, _, cx| {
                                cx.open_url("https://github.com/nxobit/btcc-litedesk");
                            }),
                    )
                    .child(
                        Button::new("theme-menu")
                            .label("主题")
                            .ghost()
                            .xsmall()
                            .dropdown_menu(|mut menu, _, cx| {
                                let current_theme = cx.theme().theme_name().clone();
                                for theme_name in theme::theme_names(cx) {
                                    let checked = theme_name == current_theme;
                                    let item_name = theme_name.clone();
                                    menu = menu.item(
                                        PopupMenuItem::element(move |_, _| {
                                            div().text_size(px(11.)).child(item_name.clone())
                                        })
                                        .checked(checked)
                                        .on_click({
                                            let theme_name = theme_name.clone();
                                            move |_, window, cx| {
                                                theme::apply_and_save_theme(
                                                    &theme_name,
                                                    Some(window),
                                                    cx,
                                                );
                                            }
                                        }),
                                    );
                                }
                                menu.min_w(px(200.)).max_h(px(320.)).scrollable(true)
                            }),
                    )
                    .when(vault_unlocked, |el| {
                        el.child(
                            Button::new("btcc-menu")
                                .label("BTCC")
                                .ghost()
                                .xsmall()
                                .dropdown_menu(|menu, _, _| {
                                    menu.item(
                                        PopupMenuItem::element(|_, _| {
                                            div().text_size(px(11.)).child("钱包列表")
                                        })
                                        .on_click(
                                            |_, window, cx| {
                                                window.dispatch_action(
                                                    Box::new(OpenBtccWalletList),
                                                    cx,
                                                );
                                            },
                                        ),
                                    )
                                    .item(
                                        PopupMenuItem::element(|_, _| {
                                            div().text_size(px(11.)).child("创建钱包")
                                        })
                                        .on_click(
                                            |_, window, cx| {
                                                window.dispatch_action(
                                                    Box::new(OpenBtccWalletCreate),
                                                    cx,
                                                );
                                            },
                                        ),
                                    )
                                    .item(
                                        PopupMenuItem::element(|_, _| {
                                            div().text_size(px(11.)).child("导入钱包")
                                        })
                                        .on_click(
                                            |_, window, cx| {
                                                window.dispatch_action(
                                                    Box::new(OpenBtccWalletImport),
                                                    cx,
                                                );
                                            },
                                        ),
                                    )
                                    .item(
                                        PopupMenuItem::element(|_, _| {
                                            div().text_size(px(11.)).child("钱包生成")
                                        })
                                        .on_click(
                                            |_, window, cx| {
                                                window.dispatch_action(
                                                    Box::new(OpenWalletGenerator),
                                                    cx,
                                                );
                                            },
                                        ),
                                    )
                                    .item(
                                        PopupMenuItem::element(|_, _| {
                                            div().text_size(px(11.)).child("批量转账")
                                        })
                                        .on_click(
                                            |_, window, cx| {
                                                window.dispatch_action(Box::new(OpenBatchSend), cx);
                                            },
                                        ),
                                    )
                                    .min_w(px(120.))
                                }),
                        )
                    })
                    .child(
                        Button::new("vanity-menu")
                            .label("靓号生成")
                            .ghost()
                            .xsmall()
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.emit(DesktopTitleBarEvent::OpenVanityGenerator);
                            })),
                    )
                    .child(
                        Button::new("nft-menu")
                            .label("NFT查询")
                            .ghost()
                            .xsmall()
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.emit(DesktopTitleBarEvent::OpenNftGallery);
                            })),
                    )
                    .child(
                        Button::new("help-menu")
                            .label("帮助")
                            .ghost()
                            .xsmall()
                            .dropdown_menu(|menu, _, _| {
                                menu.item(
                                    PopupMenuItem::element(|_, _| {
                                        div().text_size(px(11.)).child("稀有度说明")
                                    })
                                    .on_click(|_, _, cx| {
                                        if let Some(path) = Self::ensure_rarity_help_doc() {
                                            cx.open_url(&Self::path_to_file_url(&path));
                                        }
                                    }),
                                )
                                .item(
                                    PopupMenuItem::element(|_, _| {
                                        div().text_size(px(11.)).child("GitHub")
                                    })
                                    .on_click(|_, _, cx| {
                                        cx.open_url("https://github.com/nxobit/btcc-litedesk");
                                    }),
                                )
                                .min_w(px(120.))
                            }),
                    )
                    .child(
                        Button::new("donate-menu-entry")
                            .label("打赏")
                            .ghost()
                            .xsmall()
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.emit(DesktopTitleBarEvent::OpenDonate);
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .ml_auto()
                    .mr_2()
                    .gap_3()
                    .text_size(px(11.))
                    .text_color(palette::muted(app_theme))
                    .when(cfg!(target_os = "macos") && window.is_fullscreen(), |el| {
                        el.child(
                            Button::new("exit-fullscreen")
                                .label("退出全屏")
                                .ghost()
                                .xsmall()
                                .on_click(|_, window, _| {
                                    window.toggle_fullscreen();
                                }),
                        )
                    })
                    .when(self.active_wallet_count > 0, |el| {
                        el.child(format!("有余额 {} 个钱包", self.active_wallet_count))
                    })
                    .child("BTCC Litedesk"),
            )
    }
}
