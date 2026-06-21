use crate::ui::{
    btcc::{
        batch_send::BatchSendPage,
        donate_window::DonateWindow,
        stamp_mint::StampMintPage,
        vanity_generator::VanityGeneratorPage,
        wallet_generator::WalletGeneratorPage,
        wallet_list::{BtccWalletListEvent, BtccWalletListPage},
        wallet_manager::{WalletGeneratorPage as WalletManagerPage, WalletManagerEvent},
    },
    palette,
    title_bar::{
        DesktopTitleBar, DesktopTitleBarEvent, OpenBatchSend, OpenBtccWalletCreate,
        OpenBtccWalletImport, OpenBtccWalletList, OpenDonate, OpenStampMint, OpenVanityGenerator,
        OpenWalletGenerator, OpenWalletManager,
    },
};
use gpui::*;
use gpui_component::{ActiveTheme, h_flex, v_flex};

pub struct Dashboard {
    title_bar: Entity<DesktopTitleBar>,
    btcc_wallet_list_page: Entity<BtccWalletListPage>,
    vanity_generator_page: Option<Entity<VanityGeneratorPage>>,
    wallet_generator_page: Option<Entity<WalletGeneratorPage>>,
    wallet_manager_page: Option<Entity<WalletManagerPage>>,
    batch_send_page: Option<Entity<BatchSendPage>>,
    stamp_mint_page: Option<Entity<StampMintPage>>,
    donate_page: Option<Entity<DonateWindow>>,
    active_page: ActivePage,
    _subscriptions: Vec<Subscription>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ActivePage {
    BtccWalletList,
    VanityGenerator,
    WalletGenerator,
    WalletManager,
    BatchSend,
    StampMint,
    Donate,
}

impl Dashboard {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let title_bar = cx.new(|cx| DesktopTitleBar::new(window, cx));
        let btcc_wallet_list_page = cx.new(|cx| BtccWalletListPage::new(window, cx));
        let mut _subscriptions = vec![
            cx.subscribe_in(&title_bar, window, Self::on_title_bar_event),
            cx.subscribe_in(
                &btcc_wallet_list_page,
                window,
                Self::on_btcc_wallet_list_event,
            ),
        ];

        let wallet_manager_page = cx.new(|cx| WalletManagerPage::new(window, cx));
        _subscriptions.push(cx.subscribe_in(
            &wallet_manager_page,
            window,
            Self::on_wallet_manager_event,
        ));

        Self {
            title_bar,
            btcc_wallet_list_page,
            vanity_generator_page: None,
            wallet_generator_page: None,
            wallet_manager_page: Some(wallet_manager_page),
            batch_send_page: None,
            stamp_mint_page: None,
            donate_page: None,
            active_page: ActivePage::BtccWalletList,
            _subscriptions,
        }
    }

    fn on_open_btcc_wallet_list(
        &mut self,
        _: &OpenBtccWalletList,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.active_page = ActivePage::BtccWalletList;
        self.btcc_wallet_list_page.update(cx, |page, cx| {
            page.refresh_from_navigation(cx);
        });
        cx.notify();
    }

    fn on_title_bar_event(
        &mut self,
        _: &Entity<DesktopTitleBar>,
        event: &DesktopTitleBarEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            DesktopTitleBarEvent::OpenVanityGenerator => {
                self.open_vanity_generator_page(window, cx)
            }
            DesktopTitleBarEvent::OpenDonate => self.open_donate_page(window, cx),
        }
    }

    fn on_open_btcc_wallet_import(
        &mut self,
        _: &OpenBtccWalletImport,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.active_page = ActivePage::BtccWalletList;
        self.btcc_wallet_list_page.update(cx, |page, cx| {
            page.open_import_editor_from_menu(window, cx);
        });
        cx.notify();
    }

    fn on_open_btcc_wallet_create(
        &mut self,
        _: &OpenBtccWalletCreate,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.active_page = ActivePage::BtccWalletList;
        self.btcc_wallet_list_page.update(cx, |page, cx| {
            page.open_create_editor_from_menu(window, cx);
        });
        cx.notify();
    }

    fn on_open_wallet_generator(
        &mut self,
        _: &OpenWalletGenerator,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_wallet_generator_page(window, cx);
    }

    fn on_open_vanity_generator(
        &mut self,
        _: &OpenVanityGenerator,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_vanity_generator_page(window, cx);
    }

    fn on_open_wallet_manager(
        &mut self,
        _: &OpenWalletManager,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_wallet_manager_page(window, cx);
    }

    fn on_wallet_manager_event(
        &mut self,
        _: &Entity<WalletManagerPage>,
        event: &WalletManagerEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            WalletManagerEvent::BackToWalletList => {
                self.active_page = ActivePage::BtccWalletList;
                cx.notify();
            }
        }
    }

    fn on_open_donate(&mut self, _: &OpenDonate, window: &mut Window, cx: &mut Context<Self>) {
        self.open_donate_page(window, cx);
    }

    fn on_open_batch_send(
        &mut self,
        _: &OpenBatchSend,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_batch_send_page(window, cx);
    }

    fn on_open_stamp_mint(
        &mut self,
        _: &OpenStampMint,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_stamp_mint_page(window, cx);
    }

    fn on_btcc_wallet_list_event(
        &mut self,
        _: &Entity<BtccWalletListPage>,
        event: &BtccWalletListEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            BtccWalletListEvent::OpenTransfer { id, address } => {
                self.open_wallet_manager_page(window, cx);
                if let Some(page) = &self.wallet_manager_page {
                    page.update(cx, |page, cx| {
                        page.set_transfer_wallet(*id, address.clone(), window, cx)
                    });
                }
            }
            BtccWalletListEvent::ActiveCountChanged { count } => {
                self.title_bar.update(cx, |title_bar, cx| {
                    title_bar.set_active_wallet_count(*count, cx)
                });
            }
        }
    }

    fn open_wallet_generator_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.wallet_generator_page.is_none() {
            self.wallet_generator_page = Some(cx.new(|cx| WalletGeneratorPage::new(window, cx)));
        }
        self.active_page = ActivePage::WalletGenerator;
        cx.notify();
    }

    fn open_vanity_generator_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.vanity_generator_page.is_none() {
            self.vanity_generator_page = Some(cx.new(|cx| VanityGeneratorPage::new(window, cx)));
        }
        self.active_page = ActivePage::VanityGenerator;
        cx.notify();
    }

    fn open_wallet_manager_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.wallet_manager_page.is_none() {
            self.wallet_manager_page = Some(cx.new(|cx| WalletManagerPage::new(window, cx)));
        }
        self.active_page = ActivePage::WalletManager;
        cx.notify();
    }

    fn open_donate_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.donate_page.is_none() {
            self.donate_page = Some(cx.new(|cx| DonateWindow::new(window, cx)));
        }
        self.active_page = ActivePage::Donate;
        cx.notify();
    }

    fn open_batch_send_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.batch_send_page.is_none() {
            self.batch_send_page = Some(cx.new(|cx| BatchSendPage::new(window, cx)));
        }
        self.active_page = ActivePage::BatchSend;
        cx.notify();
    }

    fn open_stamp_mint_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.stamp_mint_page.is_none() {
            self.stamp_mint_page = Some(cx.new(|cx| StampMintPage::new(window, cx)));
        }
        self.active_page = ActivePage::StampMint;
        cx.notify();
    }

    fn render_content(&self) -> AnyElement {
        match self.active_page {
            ActivePage::BtccWalletList => div()
                .flex_1()
                .size_full()
                .p_6()
                .child(self.btcc_wallet_list_page.clone())
                .into_any_element(),
            ActivePage::VanityGenerator => self
                .vanity_generator_page
                .as_ref()
                .map(|page| {
                    div()
                        .flex_1()
                        .size_full()
                        .p_6()
                        .child(page.clone())
                        .into_any_element()
                })
                .unwrap_or_else(|| div().into_any_element()),
            ActivePage::WalletGenerator => self
                .wallet_generator_page
                .as_ref()
                .map(|page| {
                    div()
                        .flex_1()
                        .size_full()
                        .p_6()
                        .child(page.clone())
                        .into_any_element()
                })
                .unwrap_or_else(|| div().into_any_element()),
            ActivePage::WalletManager => self
                .wallet_manager_page
                .as_ref()
                .map(|page| {
                    div()
                        .flex_1()
                        .size_full()
                        .p_6()
                        .child(page.clone())
                        .into_any_element()
                })
                .unwrap_or_else(|| div().into_any_element()),
            ActivePage::Donate => self
                .donate_page
                .as_ref()
                .map(|page| {
                    div()
                        .flex_1()
                        .size_full()
                        .child(page.clone())
                        .into_any_element()
                })
                .unwrap_or_else(|| div().into_any_element()),
            ActivePage::BatchSend => self
                .batch_send_page
                .as_ref()
                .map(|page| {
                    div()
                        .flex_1()
                        .size_full()
                        .child(page.clone())
                        .into_any_element()
                })
                .unwrap_or_else(|| div().into_any_element()),
            ActivePage::StampMint => self
                .stamp_mint_page
                .as_ref()
                .map(|page| {
                    div()
                        .flex_1()
                        .size_full()
                        .p_6()
                        .child(page.clone())
                        .into_any_element()
                })
                .unwrap_or_else(|| div().into_any_element()),
        }
    }
}

impl Render for Dashboard {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let app_theme = cx.theme();

        v_flex()
            .size_full()
            .relative()
            .text_size(px(12.))
            .bg(app_theme.background)
            .text_color(palette::text(app_theme))
            .on_action(cx.listener(Self::on_open_btcc_wallet_list))
            .on_action(cx.listener(Self::on_open_btcc_wallet_create))
            .on_action(cx.listener(Self::on_open_btcc_wallet_import))
            .on_action(cx.listener(Self::on_open_vanity_generator))
            .on_action(cx.listener(Self::on_open_wallet_generator))
            .on_action(cx.listener(Self::on_open_wallet_manager))
            .on_action(cx.listener(Self::on_open_donate))
            .on_action(cx.listener(Self::on_open_batch_send))
            .on_action(cx.listener(Self::on_open_stamp_mint))
            .child(self.title_bar.clone())
            .child(
                h_flex()
                    .flex_1()
                    .overflow_hidden()
                    .child(self.render_content()),
            )
    }
}
