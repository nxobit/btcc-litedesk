use crate::ui::nft::render::*;
use crate::ui::palette;
use gpui::{
    px, relative, App, Bounds, ContentMask, Context, Element, ElementId, Entity,
    GlobalElementId, Hitbox, Hsla, InspectorElementId, IntoElement, IsZero, LayoutId,
    ParentElement as _, Pixels, Position, Render, ScrollWheelEvent, Style, Window, div, img,
    prelude::FluentBuilder, *,
};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable, StyledExt,
    button::{Button, ButtonCustomVariant, ButtonVariants},
    h_flex,
    input::{Input, InputEvent, InputState},
    v_flex,
};
use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

const NFT_COLUMNS: usize = 6;
const NFT_PAGE_SIZE: u32 = 18;
const NFT_CARD_WIDTH: f32 = 196.0;
const NFT_CARD_HEIGHT: f32 = 186.0;
const NFT_WHEEL_COOLDOWN: Duration = Duration::from_millis(180);
const NFT_SCROLLBAR_TRACK_WIDTH: f32 = 6.0;
const NFT_SCROLLBAR_THUMB_WIDTH: f32 = 6.0;
const NFT_SCROLLBAR_MIN_THUMB_HEIGHT: f32 = 56.0;
const NFT_DONATE_ADDRESS: &str = "cc1qwgc0w2llyk20e3hcwp0lyq8l4t8xdc4wthanks";

pub struct NftGalleryPage {
    current_page: u32,
    nfts: Vec<NftInfo>,
    filtered_nfts: Option<Vec<NftInfo>>,
    selected_rarity: Option<&'static str>,
    sort_mode: NftSortMode,
    query_input: Entity<InputState>,
    query_error: Option<String>,
    last_wheel_flip: Option<Instant>,
    _subscriptions: Vec<Subscription>,
}

impl NftGalleryPage {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let query_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("输入 NFT 编号, 1-21000")
                .default_value("")
        });
        let subscriptions = vec![cx.subscribe_in(&query_input, window, Self::on_query_input_event)];

        Self {
            current_page: 0,
            nfts: Self::page_items(NftSortMode::NumberAsc, 0),
            filtered_nfts: None,
            selected_rarity: None,
            sort_mode: NftSortMode::NumberAsc,
            query_input,
            query_error: None,
            last_wheel_flip: None,
            _subscriptions: subscriptions,
        }
    }

    fn page_items(sort_mode: NftSortMode, page: u32) -> Vec<NftInfo> {
        let start = page * NFT_PAGE_SIZE + 1;
        get_nft_range_sorted(sort_mode, None, start, NFT_PAGE_SIZE)
    }

    fn copy_donate_address(&self, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(NFT_DONATE_ADDRESS.to_string()));
    }

    fn active_total_supply(&self) -> u32 {
        self.selected_rarity.map(total_supply_by_rarity).unwrap_or_else(total_supply)
    }

    fn active_total_pages(&self) -> u32 {
        self.active_total_supply().div_ceil(NFT_PAGE_SIZE)
    }

    fn active_page_items(&self, page: u32) -> Vec<NftInfo> {
        let start = page * NFT_PAGE_SIZE + 1;
        get_nft_range_sorted(self.sort_mode, self.selected_rarity, start, NFT_PAGE_SIZE)
    }

    fn reload_current_page(&mut self) {
        self.nfts = self.active_page_items(self.current_page);
    }

    fn on_query_input_event(
        &mut self,
        _: &Entity<InputState>,
        event: &InputEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            InputEvent::Change => self.go_to_nft(cx),
            InputEvent::PressEnter { .. } => self.go_to_nft(cx),
            _ => {}
        }
    }

    fn go_to_nft(&mut self, cx: &mut Context<Self>) {
        let value = self.query_input.read(cx).text().to_string();
        let value = value.trim().to_string();
        if value.is_empty() {
            self.filtered_nfts = None;
            self.query_error = None;
            self.reload_current_page();
            cx.notify();
            return;
        }

        let Ok(index) = value.parse::<u32>() else {
            self.query_error = Some("NFT 编号必须是整数".to_string());
            cx.notify();
            return;
        };

        let total = total_supply();
        if !(1..=total).contains(&index) {
            self.query_error = Some(format!("NFT 编号必须在 1 到 {total} 之间"));
            cx.notify();
            return;
        }

        let nft = traits_from_seed(&create_nft_id(index, 0));
        if let Some(rarity) = self.selected_rarity {
            if nft.rarity != rarity {
                self.query_error = Some("当前稀有度筛选下无此 NFT".to_string());
                self.filtered_nfts = None;
                cx.notify();
                return;
            }
        }

        self.current_page = match self.selected_rarity {
            _ => find_sorted_position(index, self.sort_mode, self.selected_rarity).unwrap_or(0)
                / NFT_PAGE_SIZE,
        };
        self.filtered_nfts = Some(vec![nft]);
        self.query_error = None;
        cx.notify();
    }

    fn next_page(&mut self, cx: &mut Context<Self>) {
        let total = self.active_total_pages();
        if self.filtered_nfts.is_some() || self.current_page + 1 >= total {
            return;
        }
        self.current_page += 1;
        self.reload_current_page();
        self.query_error = None;
        cx.notify();
    }

    fn prev_page(&mut self, cx: &mut Context<Self>) {
        if self.filtered_nfts.is_some() || self.current_page == 0 {
            return;
        }
        self.current_page -= 1;
        self.reload_current_page();
        self.query_error = None;
        cx.notify();
    }

    fn set_rarity_filter(
        &mut self,
        rarity: Option<&'static str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selected_rarity = rarity;
        self.current_page = 0;
        self.filtered_nfts = None;
        self.query_error = None;
        self.query_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.reload_current_page();
        cx.notify();
    }

    fn set_sort_mode(&mut self, sort_mode: NftSortMode, cx: &mut Context<Self>) {
        if self.sort_mode == sort_mode {
            return;
        }
        self.sort_mode = sort_mode;
        self.current_page = 0;
        if self.filtered_nfts.is_none() {
            self.reload_current_page();
        }
        self.query_error = None;
        cx.notify();
    }

    fn first_page(&mut self, cx: &mut Context<Self>) {
        if self.filtered_nfts.is_some() || self.current_page == 0 {
            return;
        }
        self.current_page = 0;
        self.reload_current_page();
        self.query_error = None;
        cx.notify();
    }

    fn last_page(&mut self, cx: &mut Context<Self>) {
        if self.filtered_nfts.is_some() {
            return;
        }
        let last_page = self.active_total_pages().saturating_sub(1);
        if self.current_page == last_page {
            return;
        }
        self.current_page = last_page;
        self.reload_current_page();
        self.query_error = None;
        cx.notify();
    }

    fn handle_wheel(&mut self, delta_y: Pixels, cx: &mut Context<Self>) {
        if self.filtered_nfts.is_some() || delta_y.is_zero() {
            return;
        }
        let now = Instant::now();
        if self
            .last_wheel_flip
            .is_some_and(|last| now.duration_since(last) < NFT_WHEEL_COOLDOWN)
        {
            return;
        }
        self.last_wheel_flip = Some(now);
        if delta_y > px(0.) {
            self.prev_page(cx);
        } else {
            self.next_page(cx);
        }
    }
}

impl Render for NftGalleryPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let app_theme = cx.theme().clone();
        let viewport_width: f32 = window.viewport_size().width.into();
        let scale = (viewport_width / 1440.0).clamp(0.85, 1.45);
        let card_width = NFT_CARD_WIDTH * scale;
        let card_height = NFT_CARD_HEIGHT * scale;
        let image_size = 82.0 * scale;
        let id_font = 10.0 * scale;
        let rarity_font = 9.0 * scale;
        let detail_font = 8.5 * scale;
        let meta_font = 7.5 * scale;
        let total = self.active_total_supply();
        let total_pages = self.active_total_pages();
        let display_nfts = self.filtered_nfts.as_ref().unwrap_or(&self.nfts);
        let nft_rows = display_nfts.chunks(NFT_COLUMNS).collect::<Vec<_>>();
        let page_label = if self.filtered_nfts.is_some() {
            "筛选结果".to_string()
        } else {
            format!("第 {} / {} 页", self.current_page + 1, total_pages)
        };
        let count_label = if self.filtered_nfts.is_some() {
            format!("1 / {total} 个 NFT")
        } else {
            format!("{} / {total} 个 NFT", display_nfts.len())
        };
        let scrollbar_track_height = 420.0f32;
        let total_pages_f = total_pages.max(1) as f32;
        let scrollbar_thumb_height =
            (scrollbar_track_height / total_pages_f).max(NFT_SCROLLBAR_MIN_THUMB_HEIGHT).min(scrollbar_track_height);
        let scrollbar_travel = (scrollbar_track_height - scrollbar_thumb_height).max(0.0);
        let scrollbar_progress = if self.filtered_nfts.is_some() || total_pages <= 1 {
            0.0
        } else {
            self.current_page as f32 / (total_pages - 1) as f32
        };
        let scrollbar_top = scrollbar_travel * scrollbar_progress;

        v_flex()
            .size_full()
            .relative()
            .bg(app_theme.background)
            .overflow_hidden()
            .gap_3()
            .px_5()
            .child(
                v_flex()
                    .w_full()
                    .gap_3()
                    .p_4()
                    .rounded(px(12.0))
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
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_size(px(20.0))
                                            .font_semibold()
                                            .text_color(palette::text_strong(&app_theme))
                                            .child("CC-STAMP NFT Gallery"),
                                    )
                                    .child(
                                        h_flex()
                                            .items_center()
                                            .gap_4()
                                            .child(
                                                div()
                                                    .text_size(px(11.0))
                                                    .text_color(palette::muted(&app_theme))
                                                    .child("在下方列表区域滚动鼠标滚轮即可翻页，只做 NFT 查询不做归属查询，稀有度算法在帮助菜单里有说明，仅供娱乐参考不做市价价值评估。"),
                                            ),
                                    )
                                    .child(
                                        h_flex()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .text_size(px(11.0))
                                                    .text_color(palette::muted(&app_theme))
                                                    .child(format!("捐赠 NFT 地址：{NFT_DONATE_ADDRESS}")),
                                            )
                                            .child(
                                                Button::new("copy-nft-donate-address")
                                                    .ghost()
                                                    .xsmall()
                                                    .icon(IconName::Copy)
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.copy_donate_address(cx);
                                                    })),
                                            ),
                                    ),
                            ),
                    )
                    .child(
                        h_flex()
                            .items_start()
                            .justify_between()
                            .gap_3()
                            .child(
                                h_flex()
                                    .gap_3()
                                    .child(render_page_chip(
                                        page_label,
                                        self.filtered_nfts.is_none(),
                                        app_theme.primary.opacity(0.08),
                                        app_theme.primary.opacity(0.92),
                                        cx,
                                    ))
                                    .child(render_overview_chip(
                                        "显示数量",
                                        count_label,
                                        app_theme.success.opacity(0.08),
                                        app_theme.success.opacity(0.92),
                                    ))
                                    .child(render_rarity_filter_card_v2(
                                        self.selected_rarity,
                                        &app_theme,
                                        cx,
                                    ))
                                    .child(render_sort_card(self.sort_mode, &app_theme, cx)),
                            )
                            .child(
                                v_flex()
                                    .w(px(320.0))
                                    .h(px(108.0))
                                    .justify_between()
                                    .gap_1()
                                    .p_3()
                                    .rounded(px(12.0))
                                    .border_1()
                                    .border_color(app_theme.primary.opacity(0.18))
                                    .bg(app_theme.primary.opacity(0.05))
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .text_color(palette::muted(&app_theme))
                                            .child("NFT 编号"),
                                    )
                                    .child(
                                        h_flex()
                                            .w_full()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .w_full()
                                                    .child(
                                                        Input::new(&self.query_input)
                                                            .small()
                                                            .prefix(Icon::new(IconName::Search).small()),
                                                    ),
                                            ),
                                    )
                                    .when_some(self.query_error.clone(), |el, error| {
                                        el.child(
                                            div()
                                                .text_size(px(11.0))
                                                .text_color(app_theme.danger)
                                                .child(error),
                                        )
                                    }),
                            ),
                    ),
            )
            .child(
                v_flex()
                    .flex_1()
                    .w_full()
                    .overflow_hidden()
                    .gap_2()
                    .p_4()
                    .rounded(px(12.0))
                    .border_1()
                    .border_color(palette::border(&app_theme))
                    .bg(app_theme.background)
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(palette::muted(&app_theme))
                            .child("合集预览"),
                    )
                    .child(
                        div()
                            .relative()
                            .flex_1()
                            .w_full()
                            .child(
                                v_flex()
                                    .w_full()
                                    .items_start()
                                    .pr_6()
                                    .gap_3()
                                    .children(nft_rows.into_iter().map(|row| {
                                        h_flex()
                                            .w_full()
                                            .gap_3()
                                            .justify_between()
                                            .children(row.iter().map(|nft| {
                                                div()
                                                    .flex_1()
                                                    .flex()
                                                    .justify_center()
                                                    .child(nft_item_card(
                                                        nft,
                                                        &app_theme,
                                                        card_width,
                                                        card_height,
                                                        image_size,
                                                        id_font,
                                                        rarity_font,
                                                        detail_font,
                                                        meta_font,
                                                    ))
                                            }))
                                    })),
                            )
                            .when(self.filtered_nfts.is_none(), |el| {
                                el.child(
                                    div()
                                        .absolute()
                                        .top_2()
                                        .right_0()
                                        .bottom_2()
                                        .w(px(12.0))
                                        .flex()
                                        .justify_center()
                                        .child(
                                            div()
                                                .relative()
                                                .w(px(NFT_SCROLLBAR_TRACK_WIDTH))
                                                .h(px(scrollbar_track_height))
                                                .rounded(px(NFT_SCROLLBAR_TRACK_WIDTH / 2.0))
                                                .bg(rgb(0xe5e7eb))
                                                .child(
                                                    div()
                                                        .absolute()
                                                        .top(px(scrollbar_top))
                                                        .left_0()
                                                        .w(px(NFT_SCROLLBAR_THUMB_WIDTH))
                                                        .h(px(scrollbar_thumb_height))
                                                        .rounded(px(NFT_SCROLLBAR_THUMB_WIDTH / 2.0))
                                                        .bg(rgb(0x9ca3af)),
                                                ),
                                        ),
                                )
                            })
                            .when(self.filtered_nfts.is_none(), |el| {
                                el.child(WheelPageMask::new(cx.entity().clone()))
                            }),
                    ),
            )
    }
}

fn render_overview_chip(
    label: impl Into<SharedString>,
    value: impl Into<SharedString>,
    background: Hsla,
    accent: Hsla,
) -> impl IntoElement {
    let label = label.into();
    let value = value.into();

    v_flex()
        .w(px(168.0))
        .h(px(108.0))
        .justify_start()
        .gap_1()
        .p_3()
        .rounded(px(12.0))
        .border_1()
        .border_color(accent.opacity(0.22))
        .bg(background)
        .child(
            div()
                .text_size(px(11.0))
                .text_color(accent.opacity(0.72))
                .child(label),
        )
        .child(
            div()
                .text_size(px(14.0))
                .font_semibold()
                .text_color(accent)
                .child(value),
        )
        .child(div().h(px(24.0)))
}

fn render_page_chip(
    value: impl Into<SharedString>,
    allow_navigation: bool,
    background: Hsla,
    accent: Hsla,
    cx: &mut Context<NftGalleryPage>,
) -> impl IntoElement {
    let value = value.into();

    v_flex()
        .w(px(188.0))
        .h(px(108.0))
        .justify_between()
        .gap_1()
        .p_3()
        .rounded(px(12.0))
        .border_1()
        .border_color(accent.opacity(0.22))
        .bg(background)
        .child(
            div()
                .text_size(px(11.0))
                .text_color(accent.opacity(0.72))
                .child("页码"),
        )
        .child(
            div()
                .text_size(px(14.0))
                .font_semibold()
                .text_color(accent)
                .child(value),
        )
        .child(
            h_flex()
                .w_full()
                .justify_between()
                .gap_2()
                .child(
                    page_jump_button(0, "首页", IconName::ArrowLeft, accent, allow_navigation, cx)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.first_page(cx);
                        })),
                )
                .child(
                    page_jump_button(1, "尾页", IconName::ArrowRight, accent, allow_navigation, cx)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.last_page(cx);
                        })),
                ),
        )
}

fn page_jump_button(
    index: usize,
    label: &'static str,
    icon: IconName,
    accent: Hsla,
    enabled: bool,
    cx: &mut Context<NftGalleryPage>,
) -> Button {
    let background = if enabled { accent.opacity(0.12) } else { accent.opacity(0.05) };
    let foreground = if enabled { accent } else { accent.opacity(0.45) };
    let variant = ButtonCustomVariant::new(cx)
        .color(background)
        .foreground(foreground)
        .border(accent.opacity(if enabled { 0.28 } else { 0.12 }))
        .hover(background)
        .active(background);

    Button::new(("nft-page-jump", index))
        .xsmall()
        .custom(variant)
        .child(
            h_flex()
                .gap_1()
                .items_center()
                .child(Icon::new(icon).small())
                .child(label),
        )
}

fn render_sort_card(
    selected_sort: NftSortMode,
    app_theme: &gpui_component::Theme,
    cx: &mut Context<NftGalleryPage>,
) -> impl IntoElement {
    v_flex()
        .w(px(280.0))
        .h(px(108.0))
        .justify_between()
        .gap_2()
        .p_3()
        .rounded(px(12.0))
        .border_1()
        .border_color(app_theme.primary.opacity(0.18))
        .bg(app_theme.primary.opacity(0.05))
        .child(
            h_flex()
                .items_center()
                .gap_1()
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(palette::muted(app_theme))
                        .child("排序"),
                ),
        )
        .child(
            v_flex()
                .w_full()
                .gap_1()
                .child(
                    h_flex()
                        .w_full()
                        .gap_2()
                        .items_center()
                        .child(
                            div()
                                .w(px(40.0))
                                .text_size(px(10.0))
                                .text_color(palette::muted_soft(app_theme))
                                .font_semibold()
                                .child("编号"),
                        )
                        .child(
                            div().flex_1().child(
                                sort_action_button(
                                    0,
                                    "小到大",
                                    selected_sort == NftSortMode::NumberAsc,
                                    NftSortMode::NumberAsc,
                                    cx,
                                )
                                .w_full(),
                            ),
                        )
                        .child(
                            div().flex_1().child(
                                sort_action_button(
                                    1,
                                    "大到小",
                                    selected_sort == NftSortMode::NumberDesc,
                                    NftSortMode::NumberDesc,
                                    cx,
                                )
                                .w_full(),
                            ),
                        ),
                )
                .child(
                    h_flex()
                        .w_full()
                        .gap_2()
                        .items_center()
                        .child(
                            div()
                                .w(px(40.0))
                                .text_size(px(10.0))
                                .text_color(palette::muted_soft(app_theme))
                                .font_semibold()
                                .child("稀有"),
                        )
                        .child(
                            div().flex_1().child(
                                sort_action_button(
                                    2,
                                    "最常见",
                                    selected_sort == NftSortMode::RarityAsc,
                                    NftSortMode::RarityAsc,
                                    cx,
                                )
                                .w_full(),
                            ),
                        )
                        .child(
                            div().flex_1().child(
                                sort_action_button(
                                    3,
                                    "最稀有",
                                    selected_sort == NftSortMode::RarityDesc,
                                    NftSortMode::RarityDesc,
                                    cx,
                                )
                                .w_full(),
                            ),
                        ),
                )
        )
}

fn sort_action_button(
    index: usize,
    label: impl Into<SharedString>,
    active: bool,
    sort_mode: NftSortMode,
    cx: &mut Context<NftGalleryPage>,
) -> Button {
    let app_theme = cx.theme().clone();
    let variant = if active {
        ButtonCustomVariant::new(cx)
            .color(app_theme.primary)
            .foreground(gpui::white())
            .border(app_theme.primary)
            .hover(app_theme.primary.opacity(0.92))
            .active(app_theme.primary.opacity(0.84))
    } else {
        ButtonCustomVariant::new(cx)
            .color(app_theme.primary.opacity(0.08))
            .foreground(palette::text_strong(&app_theme))
            .border(app_theme.primary.opacity(0.18))
            .hover(app_theme.primary.opacity(0.12))
            .active(app_theme.primary.opacity(0.18))
    };

    Button::new(("nft-sort-action", index))
        .xsmall()
        .custom(variant)
        .h(px(24.0))
        .label(label)
        .on_click(cx.listener(move |this, _, _, cx| {
            this.set_sort_mode(sort_mode, cx);
        }))
}


fn render_rarity_filter_card_v2(
    selected_rarity: Option<&'static str>,
    app_theme: &gpui_component::Theme,
    cx: &mut Context<NftGalleryPage>,
) -> impl IntoElement {
    let gold_count = total_supply_by_rarity("gold");
    let red_count = total_supply_by_rarity("red");
    let cyan_count = total_supply_by_rarity("cyan");

    let chart = rarity_pie_chart(gold_count, red_count, cyan_count).ok();

    v_flex()
        .w(px(360.0))
        .h(px(108.0))
        .justify_between()
        .gap_1()
        .p_3()
        .rounded(px(12.0))
        .border_1()
        .border_color(app_theme.warning.opacity(0.18))
        .bg(app_theme.warning.opacity(0.05))
        .child(
            div()
                .text_size(px(11.0))
                .text_color(palette::muted(app_theme))
                .child("稀有度"),
        )
        .child(
            h_flex()
                .w_full()
                .items_center()
                .gap_3()
                .when_some(chart, |el, path| {
                    el.child(
                        div()
                            .w(px(58.0))
                            .h(px(58.0))
                            .rounded(px(8.0))
                            .overflow_hidden()
                            .child(img(path).w_full().h_full()),
                    )
                })
                .child(
                    v_flex()
                        .flex_1()
                        .gap_2()
                        .child(
                            h_flex()
                                .w_full()
                                .gap_2()
                                .child(
                                    div()
                                        .flex_1()
                                        .child(
                                            rarity_filter_action_button(
                                                0,
                                                format!("金 {gold_count}"),
                                                selected_rarity == Some("gold"),
                                                Some("gold"),
                                                cx,
                                            )
                                                .w_full(),
                                        )
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .child(
                                            rarity_filter_action_button(
                                                1,
                                                format!("红 {red_count}"),
                                                selected_rarity == Some("red"),
                                                Some("red"),
                                                cx,
                                            )
                                                .w_full(),
                                        )
                                ),
                        )
                        .child(
                            h_flex()
                                .w_full()
                                .gap_2()
                                .child(
                                    div()
                                        .flex_1()
                                        .child(
                                            rarity_filter_action_button(
                                                2,
                                                format!("青 {cyan_count}"),
                                                selected_rarity == Some("cyan"),
                                                Some("cyan"),
                                                cx,
                                            )
                                                .w_full(),
                                        )
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .child(
                                            rarity_filter_action_button(
                                                3,
                                                format!("全部 {}", TOTAL_SUPPLY_U32),
                                                selected_rarity.is_none(),
                                                None,
                                                cx,
                                            )
                                                .w_full(),
                                        )
                                ),
                        ),
                ),
        )
}

fn rarity_filter_action_button(
    index: usize,
    label: impl Into<SharedString>,
    active: bool,
    rarity: Option<&'static str>,
    cx: &mut Context<NftGalleryPage>,
) -> Button {
    let app_theme = cx.theme().clone();
    let (base, strong, foreground): (Hsla, Hsla, Hsla) = match rarity {
        Some("gold") => (rgb(0xfff4c2).into(), rgb(0xe0a81a).into(), rgb(0x5a3e08).into()),
        Some("red") => (rgb(0xffd5df).into(), rgb(0xe0344f).into(), rgb(0x5a1020).into()),
        Some("cyan") => (rgb(0xd8fffb).into(), rgb(0x2bb3a0).into(), rgb(0x0a3a34).into()),
        _ => (
            palette::surface(&app_theme),
            palette::border(&app_theme),
            palette::text_strong(&app_theme),
        ),
    };

    let variant = if active {
        ButtonCustomVariant::new(cx)
            .color(strong)
            .foreground(gpui::white())
            .border(strong)
            .hover(strong.opacity(0.92))
            .active(strong.opacity(0.84))
    } else {
        ButtonCustomVariant::new(cx)
            .color(base)
            .foreground(foreground)
            .border(strong.opacity(0.35))
            .hover(base.opacity(0.92))
            .active(base.opacity(0.84))
    };

    Button::new(("nft-rarity-action", index))
        .label(label)
        .small()
        .custom(variant)
        .on_click(cx.listener(move |this, _, window, cx| {
            this.set_rarity_filter(rarity, window, cx);
        }))
}

fn rarity_pie_chart(gold_count: u32, red_count: u32, cyan_count: u32) -> anyhow::Result<PathBuf> {
    let total = (gold_count + red_count + cyan_count).max(1) as f32;
    let gold_angle = 360.0 * gold_count as f32 / total;
    let red_angle = 360.0 * red_count as f32 / total;
    let cyan_angle = 360.0 * cyan_count as f32 / total;
    let gold_pct = gold_count as f32 * 100.0 / total;
    let red_pct = red_count as f32 * 100.0 / total;
    let cyan_pct = cyan_count as f32 * 100.0 / total;
    let path = std::env::temp_dir().join("btcc-litedesk").join(format!(
        "nft-rarity-pie-{}-{}-{}.svg",
        gold_count, red_count, cyan_count
    ));
    if path.exists() {
        return Ok(path);
    }

    let svg = format!(
        concat!(
            "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 72 72'>",
            "<circle cx='36' cy='36' r='35' fill='#f8fafc'/>",
            "<path d='{gold}' fill='#ffd700'/>",
            "<path d='{red}' fill='#ff5a7a'/>",
            "<path d='{cyan}' fill='#7affe8'/>",
            "<circle cx='36' cy='36' r='15' fill='white'/>",
            "<text x='36' y='27' text-anchor='middle' font-size='5' font-family='Arial' fill='#5a3e08'>金 {gold_pct:.0}%</text>",
            "<text x='36' y='36' text-anchor='middle' font-size='5' font-family='Arial' fill='#5a1020'>红 {red_pct:.0}%</text>",
            "<text x='36' y='45' text-anchor='middle' font-size='5' font-family='Arial' fill='#0a3a34'>青 {cyan_pct:.0}%</text>",
            "</svg>"
        ),
        gold = pie_slice_path(36.0, 36.0, 34.0, -90.0, -90.0 + gold_angle),
        red = pie_slice_path(36.0, 36.0, 34.0, -90.0 + gold_angle, -90.0 + gold_angle + red_angle),
        cyan = pie_slice_path(
            36.0,
            36.0,
            34.0,
            -90.0 + gold_angle + red_angle,
            -90.0 + gold_angle + red_angle + cyan_angle
        ),
        gold_pct = gold_pct,
        red_pct = red_pct,
        cyan_pct = cyan_pct,
    );

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, svg)?;
    Ok(path)
}

fn pie_slice_path(cx: f32, cy: f32, radius: f32, start_deg: f32, end_deg: f32) -> String {
    let start = polar_point(cx, cy, radius, start_deg);
    let end = polar_point(cx, cy, radius, end_deg);
    let large_arc = if (end_deg - start_deg).abs() > 180.0 { 1 } else { 0 };
    format!(
        "M {cx:.2} {cy:.2} L {sx:.2} {sy:.2} A {r:.2} {r:.2} 0 {large_arc} 1 {ex:.2} {ey:.2} Z",
        cx = cx,
        cy = cy,
        sx = start.0,
        sy = start.1,
        r = radius,
        large_arc = large_arc,
        ex = end.0,
        ey = end.1
    )
}

fn polar_point(cx: f32, cy: f32, radius: f32, deg: f32) -> (f32, f32) {
    let rad = deg.to_radians();
    (cx + radius * rad.cos(), cy + radius * rad.sin())
}

fn rarity_label(rarity: &str) -> SharedString {
    match rarity {
        "gold" => "金".into(),
        "red" => "红".into(),
        "cyan" => "青".into(),
        _ => rarity.to_string().into(),
    }
}

fn rarity_display_color(rarity: &str) -> Rgba {
    match rarity {
        "gold" => rgb(0xc9a227),
        "red" => rgb(0xd46a7f),
        _ => rgb(0x64cfc8),
    }
}

fn rarity_rank_badge(nft: &NftInfo, rank_font: f32) -> Option<impl IntoElement> {
    let rank = rarity_rank(nft.index)?;
    Some(
        div()
            .w_full()
            .px_2()
            .text_left()
            .child(
                div()
                    .text_size(px((rank_font - 0.5).max(10.0)))
                    .font_semibold()
                    .text_color(rarity_display_color(&nft.rarity))
                    .child(format!("#{rank}")),
            ),
    )
}

fn nft_item_card(
    nft: &NftInfo,
    app_theme: &gpui_component::Theme,
    card_width: f32,
    card_height: f32,
    image_size: f32,
    id_font: f32,
    rarity_font: f32,
    detail_font: f32,
    meta_font: f32,
) -> impl IntoElement {
    let rarity_color = rarity_display_color(&nft.rarity);
    let rank_font = (id_font + 0.5).max(10.0);

    let preview = match render_svg_path(&nft.id) {
        Ok(path) => img(path).w_full().h_full().into_any_element(),
        Err(_) => div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(10.0))
            .text_color(rgb(0x666666))
            .child(format!("#{}", nft.index))
            .into_any_element(),
    };

    div()
        .flex()
        .flex_col()
        .items_center()
        .w_full()
        .max_w(px(card_width))
        .h(px(card_height))
        .p_2()
        .gap_1()
        .rounded(px(10.0))
        .overflow_hidden()
        .border_1()
        .border_color(palette::border_soft(app_theme))
        .bg(palette::surface(app_theme))
        .when_some(rarity_rank_badge(nft, rank_font), |el, badge| el.child(badge))
        .child(
            h_flex()
                .w_full()
                .justify_center()
                .child(
                    div()
                        .relative()
                        .w(px(image_size))
                        .h(px(image_size))
                        .rounded(px(8.0))
                        .overflow_hidden()
                        .border_1()
                        .border_color(palette::border_soft(app_theme))
                        .bg(palette::surface_strong(app_theme))
                        .child(preview),
                )
        )
        .child(
            div()
                .w_full()
                .text_center()
                .text_size(px(id_font))
                .text_color(palette::text_strong(app_theme))
                .child(nft.id.clone()),
        )
        .child(
            div()
                .w_full()
                .text_center()
                .text_size(px(rarity_font))
                .text_color(rarity_color)
                .child(rarity_label(&nft.rarity)),
        )
        .child(
            div()
                .w_full()
                .text_center()
                .text_size(px(detail_font))
                .text_color(palette::muted(app_theme))
                .child(format!("{} / {}", nft.head_name, nft.eye)),
        )
        .child(
            div()
                .w_full()
                .text_center()
                .text_size(px(meta_font))
                .text_color(palette::muted_soft(app_theme))
                .child(format!("身体 {}  色相 {}", nft.body_color, nft.head_hue)),
        )
}

struct WheelPageMask {
    page: Entity<NftGalleryPage>,
    debug: Option<Hsla>,
}

impl WheelPageMask {
    fn new(page: Entity<NftGalleryPage>) -> Self {
        Self { page, debug: None }
    }
}

impl IntoElement for WheelPageMask {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for WheelPageMask {
    type RequestLayoutState = ();
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.position = Position::Absolute;
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        style.flex_grow = 1.0;
        style.flex_shrink = 1.0;
        (window.request_layout(style, None, cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        _: &mut App,
    ) -> Self::PrepaintState {
        let cover_bounds = Bounds {
            origin: gpui::Point {
                x: bounds.origin.x,
                y: bounds.origin.y - bounds.size.height,
            },
            size: bounds.size,
        };

        window.insert_hitbox(cover_bounds, gpui::HitboxBehavior::Normal)
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        hitbox: &mut Self::PrepaintState,
        window: &mut Window,
        _: &mut App,
    ) {
        let bounds = hitbox.bounds;
        let page = self.page.clone();
        let line_height = window.line_height();
        let _ = self.debug;

        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            window.on_mouse_event({
                move |event: &ScrollWheelEvent, phase, _, cx| {
                    if !(bounds.contains(&event.position) && phase.bubble()) {
                        return;
                    }
                    let delta = event.delta.pixel_delta(line_height).y;
                    let _ = page.update(cx, |this, cx| this.handle_wheel(delta, cx));
                    if !delta.is_zero() {
                        cx.stop_propagation();
                    }
                }
            });
        });
    }
}
