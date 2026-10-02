//! Searchable clipboard history (text + images).
//!
//! A lightweight 1s poller watches the system clipboard and records new
//! content (deduplicated, capped). Texts persist to disk with owner-only
//! permissions; images are session-only. Polling can be paused, history can
//! be cleared, and single entries removed. Oversized or blank content is
//! never recorded.

use arboard::{Clipboard, ImageData};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub struct ClipEntry {
    pub id: u64,
    pub kind: ClipKind,
}

#[derive(Clone, Debug)]
pub enum ClipKind {
    Text(String),
    Image { width: usize, height: usize, bytes: Vec<u8> },
}

impl ClipEntry {
    pub fn preview(&self) -> String {
        match &self.kind {
            ClipKind::Text(t) => {
                let one_line: String = t.split_whitespace().collect::<Vec<_>>().join(" ");
                let mut chars = one_line.chars();
                let short: String = chars.by_ref().take(80).collect();
                if chars.next().is_some() {
                    format!("{short}…")
                } else {
                    short
                }
            }
            ClipKind::Image { width, height, .. } => format!("{width}×{height} image"),
        }
    }

    pub fn tag(&self) -> &'static str {
        match &self.kind {
            ClipKind::Text(_) => "Clipboard",
            ClipKind::Image { .. } => "Image",
        }
    }
}

#[derive(Default, Serialize, Deserialize)]
struct Persisted {
    #[serde(default)]
    paused: bool,
    #[serde(default)]
    texts: Vec<String>,
}

const MAX_ENTRIES: usize = 50;
const MAX_TEXT_LEN: usize = 8_000;

pub struct ClipHistory {
    entries: Vec<ClipEntry>,
    next_id: u64,
    paused: bool,
    last_hash: u64,
}

impl ClipHistory {
    pub fn load() -> Self {
        let persisted: Persisted = crate::store::load_json("clipboard.json");
        let mut history = Self {
            entries: Vec::new(),
            next_id: 1,
            paused: persisted.paused,
            last_hash: 0,
        };
        for text in persisted.texts.into_iter().take(MAX_ENTRIES) {
            history.push_text(text, true);
        }
        history.entries.reverse();
        history
    }

    fn persist(&self) {
        let texts: Vec<String> = self
            .entries
            .iter()
            .filter_map(|e| match &e.kind {
                ClipKind::Text(t) => Some(t.clone()),
                _ => None,
            })
            .take(MAX_ENTRIES)
            .collect();
        crate::store::save_json(
            "clipboard.json",
            &Persisted {
                paused: self.paused,
                texts,
            },
        );
    }

    fn fingerprint_text(t: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        t.hash(&mut h);
        h.finish()
    }

    fn fingerprint_image(img: &ImageData) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        img.width.hash(&mut h);
        img.height.hash(&mut h);
        img.bytes.as_ref().len().hash(&mut h);
        img.bytes.as_ref().iter().step_by(997).for_each(|b| b.hash(&mut h));
        h.finish()
    }

    fn push_text(&mut self, text: String, quiet: bool) {
        let id = self.next_id;
        self.next_id += 1;
        self.entries.insert(
            0,
            ClipEntry {
                id,
                kind: ClipKind::Text(text),
            },
        );
        self.entries.truncate(MAX_ENTRIES);
        if !quiet {
            self.persist();
        }
    }

    fn push_image(&mut self, width: usize, height: usize, bytes: Vec<u8>) {
        let id = self.next_id;
        self.next_id += 1;
        self.entries.insert(
            0,
            ClipEntry {
                id,
                kind: ClipKind::Image {
                    width,
                    height,
                    bytes,
                },
            },
        );
        self.entries.truncate(MAX_ENTRIES);
    }

    pub fn entries(&self) -> &[ClipEntry] {
        &self.entries
    }

    #[allow(dead_code)]
    pub fn paused(&self) -> bool {
        self.paused
    }

    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
        self.persist();
    }

    pub fn remove(&mut self, id: u64) {
        self.entries.retain(|e| e.id != id);
        self.persist();
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.persist();
    }

    pub fn restore(&self, id: u64) {
        let Some(entry) = self.entries.iter().find(|e| e.id == id) else {
            return;
        };
        let Ok(mut cb) = Clipboard::new() else {
            return;
        };
        match &entry.kind {
            ClipKind::Text(t) => {
                let _ = cb.set_text(t.clone());
            }
            ClipKind::Image { width, height, bytes } => {
                let _ = cb.set_image(ImageData {
                    width: *width,
                    height: *height,
                    bytes: bytes.as_slice().into(),
                });
            }
        }
    }

    pub fn search(&self, query: &str) -> Vec<u64> {
        if query.is_empty() {
            return self.entries.iter().take(6).map(|e| e.id).collect();
        }
        let q = query.to_lowercase();
        let words: Vec<&str> = q.split_whitespace().collect();
        self.entries
            .iter()
            .filter(|e| match &e.kind {
                ClipKind::Text(t) => {
                    let hay = t.to_lowercase();
                    words.iter().all(|w| hay.contains(w))
                }
                ClipKind::Image { .. } => "image screenshot photo picture"
                    .split_whitespace()
                    .any(|k| words.iter().any(|w| k.contains(w))),
            })
            .take(5)
            .map(|e| e.id)
            .collect()
    }

    pub fn copy_text(text: &str) {
        if let Ok(mut cb) = Clipboard::new() {
            let _ = cb.set_text(text.to_string());
        }
    }
}

/// Spawn the 1s clipboard watcher. `notify` is called on the UI thread
/// whenever new content lands in history.
pub fn start_watcher(history: Arc<Mutex<ClipHistory>>, notify: impl Fn() + Send + 'static) {
    std::thread::spawn(move || {
        let mut cb = match Clipboard::new() {
            Ok(cb) => cb,
            Err(_) => return, // no clipboard access (e.g. headless) -> disabled
        };
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            let changed = (|| {
                let mut history = history.lock().ok()?;
                if history.paused {
                    return Some(false);
                }
                // Prefer images: a screenshot copy usually also exposes stale text.
                if let Ok(img) = cb.get_image() {
                    if img.width > 0 && img.height > 0 && !img.bytes.is_empty() {
                        let hash = ClipHistory::fingerprint_image(&img);
                        if hash != history.last_hash {
                            history.last_hash = hash;
                            history.push_image(img.width, img.height, img.bytes.into_owned());
                            return Some(true);
                        }
                        return Some(false);
                    }
                }
                let text = cb.get_text().ok()?;
                let text = text.trim().to_string();
                if text.is_empty() || text.len() > MAX_TEXT_LEN {
                    return Some(false);
                }
                let hash = ClipHistory::fingerprint_text(&text);
                if hash == history.last_hash {
                    return Some(false);
                }
                // Skip if already the newest entry (e.g. we just restored it).
                if matches!(history.entries.first(), Some(first) if matches!(&first.kind, ClipKind::Text(t) if t == &text))
                {
                    history.last_hash = hash;
                    return Some(false);
                }
                history.last_hash = hash;
                history.push_text(text, false);
                Some(true)
            })();
            if changed == Some(true) {
                notify();
            }
        }
    });
}
