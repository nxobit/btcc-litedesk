use std::path::{Path, PathBuf};

const APP_NAME: &str = "BTCC Litedesk";

pub fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
}

pub fn workspace_dir_from_exe(exe_dir: &Path) -> Option<PathBuf> {
    let exe_dir_text = exe_dir.to_string_lossy().replace('\\', "/");
    if !exe_dir_text.contains("/target/debug") && !exe_dir_text.contains("/target/release") {
        return None;
    }

    let workspace_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    workspace_dir
        .canonicalize()
        .ok()
        .filter(|path| path.join("Cargo.toml").exists())
}

pub fn app_install_dir() -> PathBuf {
    exe_dir().unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

pub fn app_data_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            return home
                .join("Library")
                .join("Application Support")
                .join(APP_NAME);
        }
    }

    let base = std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("APPDATA"))
        .map(PathBuf::from)
        .unwrap_or_else(|| app_install_dir());

    base.join(APP_NAME)
}

pub fn themes_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        if let Some(exe_dir) = exe_dir() {
            let resources_dir = exe_dir
                .parent()
                .map(|path| path.join("Resources"))
                .unwrap_or_else(|| exe_dir.clone())
                .join("themes");
            if resources_dir.exists() {
                return resources_dir;
            }
        }
    }

    if let Some(workspace_dir) = exe_dir()
        .as_deref()
        .and_then(workspace_dir_from_exe)
        .filter(|path| path.join("examples").join("wallet").join("themes").exists())
    {
        return workspace_dir.join("examples").join("wallet").join("themes");
    }

    app_install_dir().join("themes")
}

pub fn settings_path() -> PathBuf {
    app_data_dir().join("config").join("setings.json")
}

pub fn panic_log_path() -> PathBuf {
    app_data_dir().join("logs").join("wallet-panic.log")
}
