//! Favorite folders, persisted across restarts.

use std::path::PathBuf;

pub fn load_favorites() -> Vec<PathBuf> {
    let stored: Vec<PathBuf> = crate::store::load_json("favorites.json");
    stored.into_iter().filter(|p| p.is_dir()).collect()
}

pub fn save_favorites(favorites: &[PathBuf]) {
    crate::store::save_json("favorites.json", favorites);
}

/// Parse `add folder <path>` (`~` supported).
pub fn parse_add_folder(query: &str) -> Option<PathBuf> {
    let rest = query.strip_prefix("add folder")?.trim();
    if rest.is_empty() {
        return None;
    }
    let path = crate::platform::expand_path(rest);
    path.is_dir().then_some(path)
}
