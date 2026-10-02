//! Cross-platform open/reveal helpers and well-known folders.
//!
//! Linux uses `xdg-open`, macOS uses `open`, Windows uses `explorer` / `cmd`.

use std::path::{Path, PathBuf};

pub fn home_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

pub fn expand_path(raw: &str) -> PathBuf {
    let trimmed = raw.trim().trim_matches('"');
    if trimmed == "~" {
        return home_dir().unwrap_or_else(|| PathBuf::from(trimmed));
    }
    if let Some(rest) = trimmed.strip_prefix("~/") {
        if let Some(home) = home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(trimmed)
}

fn run_detached(program: &str, args: &[&str]) {
    let _ = std::process::Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

/// Open a URL in the default browser.
pub fn open_url(url: &str) {
    let url = normalize_url(url);
    #[cfg(target_os = "macos")]
    run_detached("open", &[&url]);
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", &url])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    run_detached("xdg-open", &[&url]);
}

fn normalize_url(raw: &str) -> String {
    let raw = raw.trim();
    if raw.contains("://") {
        raw.to_string()
    } else {
        format!("https://{raw}")
    }
}

/// Open a file with its default app, or a folder in the file manager.
pub fn open_path(path: &Path) {
    let Some(s) = path.to_str() else { return };
    #[cfg(target_os = "macos")]
    run_detached("open", &[s]);
    #[cfg(target_os = "windows")]
    run_detached("explorer", &[s]);
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    run_detached("xdg-open", &[s]);
}

/// Reveal a file in its containing folder (selecting it where supported).
pub fn reveal_path(path: &Path) {
    #[cfg(target_os = "macos")]
    {
        if let Some(s) = path.to_str() {
            run_detached("open", &["-R", s]);
            return;
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(s) = path.to_str() {
            run_detached("explorer", &[&format!("/select,{s}")]);
            return;
        }
    }
    // Linux file managers have no portable select API: open the parent folder.
    if let Some(parent) = path.parent() {
        open_path(parent);
    }
}

/// Common user locations for the folder section.
pub fn common_folders() -> Vec<(String, PathBuf)> {
    let Some(home) = home_dir() else {
        return Vec::new();
    };
    let mut out = vec![("Home".to_string(), home.clone())];
    #[cfg(target_os = "windows")]
    let names = ["Desktop", "Downloads", "Documents", "Pictures", "Music"];
    #[cfg(target_os = "macos")]
    let names = ["Desktop", "Downloads", "Documents", "Pictures", "Projects"];
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let names = ["Desktop", "Downloads", "Documents", "Pictures", "Projects"];
    for name in names {
        let path = home.join(name);
        if path.is_dir() {
            out.push((name.to_string(), path));
        }
    }
    out
}
