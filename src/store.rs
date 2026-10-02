//! Local JSON persistence for Fastcast state.
//!
//! Everything lives under one platform-specific data directory:
//! Linux `~/.local/share/fastcast`, macOS `~/Library/Application
//! Support/fastcast`, Windows `%APPDATA%/fastcast`. Files are created with
//! owner-only permissions on Unix.

use std::path::PathBuf;

pub fn data_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Some(dir) = std::env::var_os("APPDATA") {
            return PathBuf::from(dir).join("fastcast");
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home)
                .join("Library/Application Support/fastcast");
        }
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".local/share/fastcast");
    }
    std::env::temp_dir().join("fastcast")
}

pub fn load_json<T: Default + serde::de::DeserializeOwned>(name: &str) -> T {
    let path = data_dir().join(name);
    let Ok(bytes) = std::fs::read(&path) else {
        return T::default();
    };
    serde_json::from_slice(&bytes).unwrap_or_default()
}

pub fn save_json<T: serde::Serialize + ?Sized>(name: &str, value: &T) {
    let dir = data_dir();
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(name);
    let Ok(bytes) = serde_json::to_vec_pretty(value) else {
        return;
    };
    let _ = std::fs::write(&path, bytes);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
}
