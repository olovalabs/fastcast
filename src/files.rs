//! File search over a background-built index.
//!
//! The index is built once on a worker thread (home directory, bounded depth,
//! hidden files and heavy dirs skipped) and shipped to the UI thread over a
//! channel, so typing never blocks. Searches are plain in-memory substring
//! matches over names — fast enough per keystroke at this scale.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};

#[derive(Clone, Debug)]
pub struct FileEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
    pub modified: u64,
}

const MAX_DEPTH: usize = 4;
const MAX_ENTRIES: usize = 60_000;

const SKIP_DIRS: &[&str] = &[
    ".cache",
    ".git",
    ".mozilla",
    ".local/share/Trash",
    "node_modules",
    "target",
    ".cargo",
    ".rustup",
    "__pycache__",
    ".venv",
    "Library/Caches",
    "AppData",
];

/// Start background indexing; returns a receiver that yields the index once.
pub fn start_indexing() -> Receiver<Vec<FileEntry>> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let mut entries = Vec::new();
        if let Some(home) = crate::platform::home_dir() {
            walk(&home, 0, &mut entries);
        }
        let _ = tx.send(entries);
    });
    rx
}

fn skipped(path: &std::path::Path) -> bool {
    let s = path.to_string_lossy();
    if path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.starts_with('.'))
        .unwrap_or(false)
    {
        return true;
    }
    SKIP_DIRS.iter().any(|skip| {
        s.contains(&format!("/{skip}/"))
            || s.contains(&format!("\\{skip}\\"))
            || s.ends_with(&format!("/{skip}"))
    })
}

fn walk(dir: &std::path::Path, depth: usize, out: &mut Vec<FileEntry>) {
    if depth > MAX_DEPTH || out.len() >= MAX_ENTRIES {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if out.len() >= MAX_ENTRIES {
            break;
        }
        let path = entry.path();
        if skipped(&path) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let (is_dir, size, modified) = match entry.metadata() {
            Ok(m) => (
                m.is_dir(),
                m.len(),
                m.modified()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).map_err(|_| std::io::Error::other("clock")))
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
            ),
            Err(_) => continue, // unreadable (permissions) -> skip silently
        };
        out.push(FileEntry {
            name,
            path: path.clone(),
            is_dir,
            size,
            modified,
        });
        if is_dir {
            walk(&path, depth + 1, out);
        }
    }
}

/// Ranked substring search: name matches beat path matches.
pub fn search_files(index: &[FileEntry], query: &str) -> Vec<usize> {
    let q = query.to_lowercase();
    let words: Vec<&str> = q.split_whitespace().collect();
    if words.is_empty() {
        return Vec::new();
    }
    let mut scored: Vec<(usize, u8)> = Vec::new();
    for (i, entry) in index.iter().enumerate() {
        let name = entry.name.to_lowercase();
        if words.iter().all(|w| name.contains(w)) {
            scored.push((i, 0));
        } else {
            let path = entry.path.to_string_lossy().to_lowercase();
            if words.iter().all(|w| path.contains(w)) {
                scored.push((i, 1));
            }
        }
        if scored.len() >= 400 {
            break;
        }
    }
    scored.sort_by_key(|&(i, rank)| (rank, index[i].name.len()));
    scored.into_iter().map(|(i, _)| i).take(8).collect()
}

/// Short human metadata: `2.4 MB · Mar 12` or `12 items` for folders.
pub fn describe(entry: &FileEntry) -> String {
    if entry.is_dir {
        let count = std::fs::read_dir(&entry.path)
            .map(|r| r.count())
            .unwrap_or(0);
        return format!("{count} items");
    }
    let size = if entry.size < 1024 {
        format!("{} B", entry.size)
    } else if entry.size < 1024 * 1024 {
        format!("{:.1} KB", entry.size as f64 / 1024.0)
    } else if entry.size < 1024 * 1024 * 1024 {
        format!("{:.1} MB", entry.size as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", entry.size as f64 / (1024.0 * 1024.0 * 1024.0))
    };
    if entry.modified > 0 {
        if let Some(date) = format_date(entry.modified) {
            return format!("{size} · {date}");
        }
    }
    size
}

fn format_date(epoch_secs: u64) -> Option<String> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_secs();
    let days = now.saturating_sub(epoch_secs) / 86400;
    if days == 0 {
        return Some("today".to_string());
    }
    if days == 1 {
        return Some("yesterday".to_string());
    }
    if days < 30 {
        return Some(format!("{days}d ago"));
    }
    if days < 365 {
        return Some(format!("{}mo ago", days / 30));
    }
    Some(format!("{}y ago", days / 365))
}

pub fn is_image(name: &str) -> bool {
    let lower = name.to_lowercase();
    ["png", "jpg", "jpeg", "webp", "gif", "bmp"]
        .iter()
        .any(|ext| lower.ends_with(&format!(".{ext}")))
}

#[cfg(test)]
mod tests {
    use super::{FileEntry, search_files};
    use std::path::PathBuf;

    fn entry(name: &str, path: &str, is_dir: bool) -> FileEntry {
        FileEntry {
            name: name.into(),
            path: PathBuf::from(path),
            is_dir,
            size: 0,
            modified: 0,
        }
    }

    #[test]
    fn ranks_name_over_path() {
        let index = vec![
            entry("notes.txt", "/home/u/docs/notes.txt", false),
            entry("readme.md", "/home/u/notes/readme.md", false),
        ];
        let hits = search_files(&index, "notes");
        assert_eq!(hits, vec![0, 1]);
    }
}
