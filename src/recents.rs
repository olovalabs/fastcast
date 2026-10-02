//! Recently launched applications.
//!
//! Each launch records `{name, exec, icon, at}` (capped at 30, newest first).
//! Entries whose app is no longer installed are filtered out on load.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecentApp {
    pub name: String,
    pub exec: String,
    #[serde(default)]
    pub icon: Option<PathBuf>,
    #[serde(default)]
    pub at: i64,
}

const MAX_RECENTS: usize = 30;

pub fn load_recents() -> Vec<RecentApp> {
    crate::store::load_json("recents.json")
}

fn save_recents(recents: &[RecentApp]) {
    crate::store::save_json("recents.json", recents);
}

pub fn record_launch(name: &str, exec: &str, icon: Option<PathBuf>) {
    let mut recents = load_recents();
    recents.retain(|r| r.name != name);
    recents.insert(
        0,
        RecentApp {
            name: name.to_string(),
            exec: exec.to_string(),
            icon,
            at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0),
        },
    );
    recents.truncate(MAX_RECENTS);
    save_recents(&recents);
}

pub fn clear_recents() {
    save_recents(&[]);
}

/// Keep only recents whose app still exists. `installed` is the set of
/// currently installed app names.
pub fn prune_missing(recents: Vec<RecentApp>, installed: &std::collections::HashSet<String>) -> Vec<RecentApp> {
    recents
        .into_iter()
        .filter(|r| installed.contains(&r.name))
        .collect()
}
