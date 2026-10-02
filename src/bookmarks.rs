//! Browser bookmarks (read-only).
//!
//! Chromium browsers (Chrome, Edge, Brave, and Chromium itself) store
//! bookmarks as JSON — parsed directly, profiles included. Firefox stores
//! them in `places.sqlite`; that is read through the `sqlite3` CLI when it
//! exists, otherwise Firefox is skipped gracefully. Nothing is ever written
//! back to browser data.

use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Bookmark {
    pub title: String,
    pub url: String,
    pub browser: &'static str,
}

pub fn load_bookmarks() -> Vec<Bookmark> {
    let mut out = Vec::new();
    out.extend(chromium_bookmarks());
    out.extend(firefox_bookmarks());
    out.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    out
}

pub fn search_bookmarks(bookmarks: &[Bookmark], query: &str) -> Vec<usize> {
    let q = query.to_lowercase();
    let words: Vec<&str> = q.split_whitespace().collect();
    bookmarks
        .iter()
        .enumerate()
        .filter(|(_, b)| {
            let hay = format!("{} {} {}", b.title, b.url, b.browser).to_lowercase();
            words.iter().all(|w| hay.contains(w))
        })
        .map(|(i, _)| i)
        .take(6)
        .collect()
}

fn config_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        #[cfg(target_os = "macos")]
        roots.push(home.join("Library/Application Support"));
        #[cfg(target_os = "windows")]
        {}
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        roots.push(home.join(".config"));
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            roots.push(PathBuf::from(local));
        }
    }
    roots
}

fn chromium_dirs() -> Vec<(&'static str, &'static str)> {
    #[cfg(target_os = "macos")]
    {
        vec![
            ("Chrome", "Google/Chrome"),
            ("Edge", "Microsoft Edge"),
            ("Brave", "BraveSoftware/Brave-Browser"),
        ]
    }
    #[cfg(target_os = "windows")]
    {
        vec![
            ("Chrome", "Google\\Chrome\\User Data"),
            ("Edge", "Microsoft\\Edge\\User Data"),
            ("Brave", "BraveSoftware\\Brave-Browser\\User Data"),
        ]
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        vec![
            ("Chrome", "google-chrome"),
            ("Edge", "microsoft-edge"),
            ("Brave", "Brave"),
        ]
    }
}

fn chromium_bookmarks() -> Vec<Bookmark> {
    let mut out = Vec::new();
    for root in config_roots() {
        for (browser, dir) in chromium_dirs() {
            #[cfg(target_os = "windows")]
            let base = root.join(dir.replace("\\\\", "\\"));
            #[cfg(not(target_os = "windows"))]
            let base = root.join(dir);
            if !base.is_dir() {
                continue;
            }
            // Default profile plus any `Profile *` / `Guest Profile` dirs.
            let mut profiles = vec![base.join("Default")];
            if let Ok(entries) = std::fs::read_dir(&base) {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    let name = name.to_string_lossy();
                    if (name.starts_with("Profile ") || name == "Guest Profile")
                        && entry.path().is_dir()
                    {
                        profiles.push(entry.path());
                    }
                }
            }
            for profile in profiles {
                let file = profile.join("Bookmarks");
                if file.is_file() {
                    out.extend(parse_chromium_file(&file, browser));
                }
            }
        }
    }
    out
}

fn parse_chromium_file(path: &PathBuf, browser: &'static str) -> Vec<Bookmark> {
    let Ok(bytes) = std::fs::read(path) else {
        return Vec::new();
    };
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Some(roots) = json.get("roots").and_then(|r| r.as_object()) {
        for root in roots.values() {
            collect_chromium_nodes(root, browser, &mut out);
            if out.len() > 400 {
                break;
            }
        }
    }
    out
}

fn collect_chromium_nodes(node: &serde_json::Value, browser: &'static str, out: &mut Vec<Bookmark>) {
    let node_type = node.get("type").and_then(|t| t.as_str()).unwrap_or("");
    if node_type == "url" {
        if let (Some(name), Some(url)) = (
            node.get("name").and_then(|n| n.as_str()),
            node.get("url").and_then(|u| u.as_str()),
        ) {
            if !url.starts_with("javascript:") && !url.starts_with("chrome://") {
                out.push(Bookmark {
                    title: name.to_string(),
                    url: url.to_string(),
                    browser,
                });
            }
        }
    } else if node_type == "folder" {
        if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
            for child in children {
                collect_chromium_nodes(child, browser, out);
            }
        }
    }
}

fn firefox_bookmarks() -> Vec<Bookmark> {
    // Needs the sqlite3 CLI; absence is a graceful skip, not an error.
    if std::process::Command::new("sqlite3")
        .arg("--version")
        .output()
        .is_err()
    {
        return Vec::new();
    }
    let mut out = Vec::new();
    for root in config_roots() {
        #[cfg(target_os = "macos")]
        let profiles_dir = root.join("Firefox/Profiles");
        #[cfg(not(target_os = "macos"))]
        let profiles_dir = root.join("mozilla/firefox");
        let Ok(entries) = std::fs::read_dir(&profiles_dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let db = entry.path().join("places.sqlite");
            if !db.is_file() {
                continue;
            }
            // Copy first: Firefox locks the live database.
            let tmp = std::env::temp_dir().join("fastcast-places.sqlite");
            if std::fs::copy(&db, &tmp).is_err() {
                continue;
            }
            let query = "SELECT b.title, p.url FROM moz_bookmarks b \
                 JOIN moz_places p ON b.fk = p.id \
                 WHERE b.title IS NOT NULL AND b.title != '' \
                 AND p.url LIKE 'http%' LIMIT 400";
            if let Ok(output) = std::process::Command::new("sqlite3")
                .args(["-separator", "\t", &tmp.to_string_lossy(), query])
                .output()
            {
                let text = String::from_utf8_lossy(&output.stdout);
                for line in text.lines() {
                    if let Some((title, url)) = line.split_once('\t') {
                        out.push(Bookmark {
                            title: title.to_string(),
                            url: url.to_string(),
                            browser: "Firefox",
                        });
                    }
                }
            }
            let _ = std::fs::remove_file(&tmp);
        }
    }
    out
}
