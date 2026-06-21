use crate::{theme, ui::dashboard::Dashboard};
use gpui::*;
use gpui_component::{Root, TitleBar};
use gpui_component_assets::Assets;

fn main_window_options(cx: &mut App) -> WindowOptions {
    let initial_size = if cfg!(target_os = "macos") {
        size(px(1440.0), px(900.0))   
    } else {
        size(px(1290.0), px(840.0))
    };

    let (window_bounds, display_id) = cx
        .primary_display()
        .map(|display| {
            let screen_bounds = display.bounds();
            let bounds = Bounds {
                origin: point(
                    screen_bounds.center().x - initial_size.center().x,
                    screen_bounds.center().y - initial_size.center().y,
                ),
                size: initial_size,
            };
            (WindowBounds::Windowed(bounds), Some(display.id()))
        })
        .unwrap_or_else(|| {
            (
                WindowBounds::Windowed(Bounds::centered(None, initial_size, cx)),
                None,
            )
        });

    WindowOptions {
        window_bounds: Some(window_bounds),
        display_id,
        titlebar: Some(TitleBar::title_bar_options()),
        ..Default::default()
    }
}

fn open_main_window(cx: &mut App) {
    let window_options = main_window_options(cx);
    let handle = cx
        .open_window(window_options, |window, cx| {
        let view = cx.new(|cx| Dashboard::new(window, cx));
        cx.new(|cx| Root::new(view, window, cx))
    })
    .expect("failed to open desktop window");

    cx.defer(move |cx| {
        let _ = handle.update(cx, |_root, window, _cx| {
            window.refresh();
        });
    });
}

pub fn run() {
    let app = Application::new().with_assets(Assets);

    app.on_reopen(|cx| {
        cx.activate(true);
        open_main_window(cx);
    });

    app.run(move |cx| {
        gpui_component::init(cx);
        theme::init(cx);
        cx.activate(true);
        open_main_window(cx);
    });
}
