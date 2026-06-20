#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod runtime_paths;
mod theme;
mod ui;

fn main() {
    configure_runtime_paths();
    install_panic_log_hook();
    app::run();
}

fn configure_runtime_paths() {
    let exe_dir = runtime_paths::app_install_dir();
    let workspace_dir = runtime_paths::workspace_dir_from_exe(&exe_dir);
    let working_dir = workspace_dir.clone().unwrap_or_else(|| exe_dir.clone());

    if let Err(err) = std::env::set_current_dir(&working_dir) {
        eprintln!(
            "failed to set working directory to {}: {err}",
            working_dir.display()
        );
    }

    if workspace_dir.is_none() {
        btcc_litedesk::db::set_runtime_data_dir(runtime_paths::app_data_dir());
    }
}

fn install_panic_log_hook() {
    use std::backtrace::Backtrace;
    use std::fs::OpenOptions;
    use std::io::Write;

    std::panic::set_hook(Box::new(|panic_info| {
        let log_path = runtime_paths::panic_log_path();
        if let Some(parent) = log_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&log_path) {
            let location = panic_info
                .location()
                .map(|loc| format!("{}:{}", loc.file(), loc.line()))
                .unwrap_or_else(|| "<unknown>".to_string());

            let payload = if let Some(message) = panic_info.payload().downcast_ref::<&str>() {
                (*message).to_string()
            } else if let Some(message) = panic_info.payload().downcast_ref::<String>() {
                message.clone()
            } else {
                "<non-string panic payload>".to_string()
            };

            let _ = writeln!(
                file,
                "\n==== PANIC ====\nlocation: {location}\nmessage: {payload}\nbacktrace:\n{:?}\n",
                Backtrace::force_capture()
            );
        }
    }));
}
