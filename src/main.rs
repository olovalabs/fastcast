mod bookmarks;
mod calc;
mod clipboard;
mod emoji;
mod favorites;
mod files;
mod input;
mod links;
mod platform;
mod recents;
mod store;

use gpui::{
    App, Bounds, Context, CursorStyle, Entity, KeyBinding, Render, Subscription, Window,
    WindowBounds, WindowOptions, div, prelude::*, px, rgba, size,
};
use gpui_platform::application;
use input::{Clear, Confirm, Editor, MoveDown, MoveUp, RemoveSelected, RevealSelected};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

// ---------------------------------------------------------------------------
// Application discovery (existing) + cross-platform variants
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct DesktopApp {
    name: String,
    exec: String,
    keywords: String,
    icon: Option<PathBuf>,
}

#[cfg(target_os = "linux")]
fn app_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/usr/local/share/applications"),
        PathBuf::from("/var/lib/flatpak/exports/share/applications"),
        PathBuf::from("/var/lib/snapd/desktop/applications"),
    ];
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(&home).join(".local/share/applications"));
        dirs.push(PathBuf::from(home).join(".local/share/flatpak/exports/share/applications"));
    }
    dirs
}

fn resolve_icon(name: &str) -> Option<PathBuf> {
    if name.is_empty() {
        return None;
    }
    let p = PathBuf::from(name);
    if p.is_absolute() && p.exists() {
        return ensure_raster(&p);
    }
    let exts = ["png", "jpg", "jpeg", "webp"];
    let found: Vec<PathBuf> = linicon::lookup_icon(name)
        .with_size(48)
        .with_scale(1)
        .filter_map(|icon| icon.ok().map(|i| i.path))
        .collect();
    // Prefer raster formats that GPUI can decode directly.
    if let Some(pos) = found.iter().position(|path| {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| exts.contains(&e))
            .unwrap_or(false)
    }) {
        return Some(found[pos].clone());
    }
    // SVG from the theme lookup? Rasterize it to a cached PNG.
    if let Some(svg) = found.into_iter().next() {
        return ensure_raster(&svg);
    }
    // linicon misses some locations (pixmaps, ~/.local/share/icons,
    // flatpak/snap exports) — search those by hand, then rasterize if SVG.
    fallback_search(name).and_then(|p| ensure_raster(&p))
}

/// Extra icon roots that linicon doesn't cover on this system.
fn extra_icon_roots() -> Vec<PathBuf> {
    let mut roots = vec![
        PathBuf::from("/usr/share/pixmaps"),
        PathBuf::from("/var/lib/flatpak/exports/share/icons"),
        PathBuf::from("/var/lib/snapd/desktop/icons"),
    ];
    if let Some(home) = std::env::var_os("HOME") {
        roots.push(PathBuf::from(&home).join(".local/share/icons"));
        roots.push(PathBuf::from(&home).join(".icons"));
        roots.push(
            PathBuf::from(&home).join(".local/share/flatpak/exports/share/icons"),
        );
        roots.push(PathBuf::from(home).join(".local/share/icons/hicolor"));
    }
    roots
}

fn fallback_search(name: &str) -> Option<PathBuf> {
    // Fast path: direct hit in pixmaps.
    for ext in ["png", "svg", "jpg", "jpeg", "webp", "xpm"] {
        let candidate = PathBuf::from(format!("/usr/share/pixmaps/{name}.{ext}"));
        if candidate.exists() {
            return Some(candidate);
        }
    }
    // Slow path: match by file stem under the extra roots, raster preferred.
    let mut svg_hit = None;
    let mut stack: Vec<PathBuf> = extra_icon_roots();
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.file_stem().and_then(|s| s.to_str()) != Some(name) {
                continue;
            }
            match path.extension().and_then(|e| e.to_str()) {
                Some("png" | "jpg" | "jpeg" | "webp") => return Some(path),
                Some("svg") if svg_hit.is_none() => svg_hit = Some(path),
                _ => {}
            }
        }
    }
    svg_hit
}

/// GPUI's `img()` cannot decode SVG — rasterize to a cached PNG first.
/// Non-SVG paths pass through untouched.
fn ensure_raster(path: &std::path::Path) -> Option<PathBuf> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if ext != "svg" {
        return Some(path.to_path_buf());
    }
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    use std::hash::{Hash, Hasher};
    path.hash(&mut hasher);
    let dest = std::env::temp_dir()
        .join("fastcast-icons")
        .join(format!("{:x}.png", hasher.finish()));
    let fresh = dest.exists()
        && std::fs::metadata(&dest)
            .and_then(|d| d.modified())
            .ok()
            >= std::fs::metadata(path)
                .and_then(|d| d.modified())
                .ok();
    if fresh {
        return Some(dest);
    }
    rasterize_svg(path, &dest)?;
    Some(dest)
}

fn rasterize_svg(src: &std::path::Path, dest: &std::path::Path) -> Option<()> {
    let data = std::fs::read(src).ok()?;
    let tree = resvg::usvg::Tree::from_data(&data, &resvg::usvg::Options::default()).ok()?;
    let size = tree.size();
    let target = 96.0;
    let scale = target / size.width().max(size.height());
    let w = (size.width() * scale).ceil() as u32;
    let h = (size.height() * scale).ceil() as u32;
    let mut pixmap = tiny_skia::Pixmap::new(w.max(1), h.max(1))?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    std::fs::create_dir_all(dest.parent()?).ok()?;
    pixmap.save_png(dest).ok()
}

#[cfg(target_os = "linux")]
fn load_apps() -> Vec<DesktopApp> {
    let mut apps = Vec::new();
    for dir in app_dirs() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(&path) else { continue };
            let mut name = None;
            let mut exec = None;
            let mut keywords = String::new();
            let mut icon = None;
            let mut skip = false;
            let mut in_entry = false;
            for line in content.lines() {
                if line.starts_with('[') {
                    in_entry = line.trim() == "[Desktop Entry]";
                    continue;
                }
                if !in_entry {
                    continue;
                }
                match line.split_once('=') {
                    Some(("Name", v)) => name = Some(v.to_string()),
                    Some(("Exec", v)) => exec = Some(v.to_string()),
                    Some(("Keywords", v)) => keywords = v.to_string(),
                    Some(("Icon", v)) => icon = Some(v.to_string()),
                    Some(("NoDisplay", "true")) | Some(("Hidden", "true")) => skip = true,
                    _ => {}
                }
            }
            if skip {
                continue;
            }
            if let (Some(name), Some(exec)) = (name, exec) {
                let icon = icon.as_deref().and_then(resolve_icon);
                apps.push(DesktopApp {
                    name,
                    exec,
                    keywords,
                    icon,
                });
            }
        }
    }
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    apps.dedup_by(|a, b| a.name == b.name);
    apps
}

/// macOS: discover `.app` bundles via native /Applications directories.
#[cfg(target_os = "macos")]
fn load_apps() -> Vec<DesktopApp> {
    let mut dirs = vec![
        PathBuf::from("/Applications"),
        PathBuf::from("/System/Applications"),
    ];
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join("Applications"));
    }
    let mut apps = Vec::new();
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("app") {
                continue;
            }
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("?")
                .to_string();
            apps.push(DesktopApp {
                name,
                exec: path.to_string_lossy().to_string(),
                keywords: String::new(),
                icon: None, // .icns not decodable by GPUI; letter avatar fallback
            });
        }
    }
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    apps.dedup_by(|a, b| a.name == b.name);
    apps
}

/// Windows: discover Start Menu shortcuts (native app registry surface).
#[cfg(target_os = "windows")]
fn load_apps() -> Vec<DesktopApp> {
    let mut dirs = Vec::new();
    if let Some(appdata) = std::env::var_os("APPDATA") {
        dirs.push(PathBuf::from(appdata).join("Microsoft/Windows/Start Menu/Programs"));
    }
    if let Some(programdata) = std::env::var_os("PROGRAMDATA") {
        dirs.push(PathBuf::from(programdata).join("Microsoft/Windows/Start Menu/Programs"));
    }
    let mut apps = Vec::new();
    let mut stack: Vec<(PathBuf, usize)> =
        dirs.into_iter().map(|d| (d, 0)).collect();
    while let Some((dir, depth)) = stack.pop() {
        if depth > 2 {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push((path, depth + 1));
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("lnk") {
                continue;
            }
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("?")
                .to_string();
            apps.push(DesktopApp {
                name,
                exec: path.to_string_lossy().to_string(),
                keywords: String::new(),
                icon: None,
            });
        }
    }
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    apps.dedup_by(|a, b| a.name == b.name);
    apps
}

fn matches(app: &DesktopApp, query: &str) -> bool {
    let hay = format!("{} {}", app.name, app.keywords).to_lowercase();
    query
        .to_lowercase()
        .split_whitespace()
        .all(|word| hay.contains(word))
}

/// Run the raw executable/launch target for the current platform.
fn launch_exec(exec: &str) {
    #[cfg(target_os = "macos")]
    {
        if exec.ends_with(".app") {
            let _ = std::process::Command::new("open")
                .arg(exec)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
            return;
        }
    }
    #[cfg(target_os = "windows")]
    {
        // ShellExecute-equivalent: resolves .lnk and opens files/URLs.
        let _ = std::process::Command::new("explorer")
            .arg(exec)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
        return;
    }
    // Strip desktop-entry field codes like %f %U %c ...
    let stripped = exec
        .split_whitespace()
        .filter(|part| !part.starts_with('%'))
        .collect::<Vec<_>>();
    let Some((program, args)) = stripped.split_first() else { return };
    let program = program.trim_matches('"');
    let args: Vec<&str> = args.iter().map(|a| a.trim_matches('"')).collect();
    println!("Launching: {program} {args:?}");
    let _ = std::process::Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

/// Launch an app and record it in recent history.
fn launch_app(name: &str, exec: &str, icon: Option<PathBuf>) {
    launch_exec(exec);
    recents::record_launch(name, exec, icon);
}

// ---------------------------------------------------------------------------
// Unified result rows
// ---------------------------------------------------------------------------

#[derive(Clone)]
enum RowIcon {
    Raster(PathBuf),
    /// Letter avatar with background color (RGBA hex).
    Letter(char, u32),
    /// Raw emoji glyph (emoji picker rows).
    Glyph(String),
}

#[derive(Clone)]
enum ManagementCommand {
    SaveLink { name: String, url: String },
    EditLink { name: String, url: String },
    AddFolder(PathBuf),
    ClearClipboard,
    ClearRecents,
    PauseClipboard(bool),
}

#[derive(Clone)]
enum RowAction {
    App(DesktopApp),
    RecentApp(recents::RecentApp),
    File(files::FileEntry),
    Url(String),
    Copy(String),
    PasteClip(u64),
    Run(ManagementCommand),
}

#[derive(Clone)]
enum Removable {
    Favorite(PathBuf),
    Link(String),
    Clip(u64),
}

#[derive(Clone)]
struct Row {
    title: String,
    subtitle: String,
    tag: &'static str,
    icon: RowIcon,
    action: RowAction,
    removable: Option<Removable>,
    verb: &'static str,
}

impl Row {
    fn app(app: &DesktopApp, recent: bool) -> Self {
        let icon = match &app.icon {
            Some(path) => RowIcon::Raster(path.clone()),
            None => RowIcon::Letter(
                app.name.chars().next().unwrap_or('?').to_uppercase().next().unwrap_or('?'),
                0x3b82f6ff,
            ),
        };
        Self {
            title: app.name.clone(),
            subtitle: String::new(),
            tag: if recent { "Recent" } else { "Application" },
            icon,
            action: RowAction::App(app.clone()),
            removable: None,
            verb: "Open",
        }
    }
}

struct Section {
    header: String,
    rows: Vec<Row>,
}

// ---------------------------------------------------------------------------
// Fastcast view
// ---------------------------------------------------------------------------

struct Fastcast {
    selected: usize,
    apps: Vec<DesktopApp>,
    installed: HashSet<String>,
    recents: Vec<recents::RecentApp>,
    links: Vec<links::QuickLink>,
    favorites: Vec<PathBuf>,
    bookmarks: Vec<bookmarks::Bookmark>,
    file_index: Vec<files::FileEntry>,
    indexing: bool,
    clipboard: Arc<Mutex<clipboard::ClipHistory>>,
    editor: Entity<Editor>,
    _subscriptions: Vec<Subscription>,
}

impl Fastcast {
    fn query(&self, cx: &App) -> String {
        self.editor.read(cx).text(cx)
    }

    fn clip_entries(&self) -> Vec<clipboard::ClipEntry> {
        self.clipboard.lock().map(|h| h.entries().to_vec()).unwrap_or_default()
    }

    /// Build display sections plus a flat selection map.
    fn sections(&self, cx: &App) -> (Vec<Section>, Vec<(usize, usize)>) {
        let query = self.query(cx);
        let mut sections: Vec<Section> = Vec::new();
        if query.trim().is_empty() {
            self.empty_sections(&mut sections);
        } else {
            self.search_sections(&query, &mut sections);
        }
        let mut flat = Vec::new();
        for (si, section) in sections.iter().enumerate() {
            for (ri, _) in section.rows.iter().enumerate() {
                flat.push((si, ri));
            }
        }
        (sections, flat)
    }

    fn empty_sections(&self, sections: &mut Vec<Section>) {
        let recent_rows: Vec<Row> = self
            .recents
            .iter()
            .take(5)
            .map(|r| Row {
                title: r.name.clone(),
                subtitle: String::new(),
                tag: "Recent",
                icon: match &r.icon {
                    Some(path) => RowIcon::Raster(path.clone()),
                    None => RowIcon::Letter(
                        r.name.chars().next().unwrap_or('?').to_uppercase().next().unwrap_or('?'),
                        0x3b82f6ff,
                    ),
                },
                action: RowAction::RecentApp(r.clone()),
                removable: None,
                verb: "Open",
            })
            .collect();
        if !recent_rows.is_empty() {
            sections.push(Section {
                header: "Recent Apps".to_string(),
                rows: recent_rows,
            });
        }

        let mut fav_rows = Vec::new();
        let mut seen = HashSet::new();
        for (name, path) in platform::common_folders()
            .into_iter()
            .chain(self.favorites.iter().map(|p| {
                (
                    p.file_name().and_then(|n| n.to_str()).unwrap_or("?").to_string(),
                    p.clone(),
                )
            }))
        {
            if !seen.insert(path.clone()) {
                continue;
            }
            let removable = self.favorites.contains(&path).then(|| Removable::Favorite(path.clone()));
            fav_rows.push(Row {
                title: name,
                subtitle: path.to_string_lossy().to_string(),
                tag: "Folder",
                icon: RowIcon::Letter('F', 0x22c55eff),
                action: RowAction::File(files::FileEntry {
                    name: String::new(),
                    path: path.clone(),
                    is_dir: true,
                    size: 0,
                    modified: 0,
                }),
                removable,
                verb: "Open",
            });
        }
        if !fav_rows.is_empty() {
            sections.push(Section {
                header: "Folders".to_string(),
                rows: fav_rows,
            });
        }

        let mut rows = Vec::new();
        for entry in self.clip_entries().into_iter().take(5) {
            rows.push(clip_row(&entry));
        }
        if !rows.is_empty() {
            sections.push(Section {
                header: "Clipboard".to_string(),
                rows,
            });
        }

        let app_rows: Vec<Row> = self
            .apps
            .iter()
            .take(6)
            .map(|a| Row::app(a, self.recents.iter().any(|r| r.name == a.name)))
            .collect();
        if !app_rows.is_empty() {
            sections.push(Section {
                header: "Applications".to_string(),
                rows: app_rows,
            });
        }
    }

    fn search_sections(&self, query: &str, sections: &mut Vec<Section>) {
        // Management commands.
        let q = query.trim();
        let mut cmd_rows = Vec::new();
        if let Some((name, url)) = links::parse_add_link(q) {
            cmd_rows.push(Row {
                title: format!("Save link “{name}”"),
                subtitle: url.clone(),
                tag: "Command",
                icon: RowIcon::Letter('+', 0xeab308ff),
                action: RowAction::Run(ManagementCommand::SaveLink { name, url }),
                removable: None,
                verb: "Save",
            });
        }
        if let Some((name, url)) = links::parse_edit_link(q) {
            cmd_rows.push(Row {
                title: format!("Update link “{name}”"),
                subtitle: url.clone(),
                tag: "Command",
                icon: RowIcon::Letter('~', 0xeab308ff),
                action: RowAction::Run(ManagementCommand::EditLink { name, url }),
                removable: None,
                verb: "Save",
            });
        }
        if let Some(path) = favorites::parse_add_folder(q) {
            cmd_rows.push(Row {
                title: format!(
                    "Add “{}” to favorites",
                    path.file_name().and_then(|n| n.to_str()).unwrap_or("?")
                ),
                subtitle: path.to_string_lossy().to_string(),
                tag: "Command",
                icon: RowIcon::Letter('+', 0x22c55eff),
                action: RowAction::Run(ManagementCommand::AddFolder(path)),
                removable: None,
                verb: "Add",
            });
        }
        match q.to_lowercase().as_str() {
            "clear clipboard" => cmd_rows.push(simple_cmd("Clear clipboard history", "Delete all entries", ManagementCommand::ClearClipboard, "Clear")),
            "clear recents" => cmd_rows.push(simple_cmd("Clear recent apps", "Forget launch history", ManagementCommand::ClearRecents, "Clear")),
            "clipboard pause" => cmd_rows.push(simple_cmd("Pause clipboard history", "Stop recording clips", ManagementCommand::PauseClipboard(true), "Pause")),
            "clipboard resume" => cmd_rows.push(simple_cmd("Resume clipboard history", "Record clips again", ManagementCommand::PauseClipboard(false), "Resume")),
            _ => {}
        }
        if !cmd_rows.is_empty() {
            sections.push(Section {
                header: "Commands".to_string(),
                rows: cmd_rows,
            });
        }

        // Calculator.
        if let Some(result) = calc::try_eval(q) {
            sections.push(Section {
                header: "Calculator".to_string(),
                rows: vec![Row {
                    title: format!("= {result}"),
                    subtitle: q.to_string(),
                    tag: "Result",
                    icon: RowIcon::Letter('=', 0x8b5cf6ff),
                    action: RowAction::Copy(result),
                    removable: None,
                    verb: "Copy",
                }],
            });
        }

        // Merged results, priority ordered, capped to fit the window.
        let mut rows: Vec<Row> = Vec::new();
        let space = |len: usize| 12usize.saturating_sub(len.min(12));

        let recent_names: HashSet<&str> =
            self.recents.iter().map(|r| r.name.as_str()).collect();
        let mut app_hits: Vec<&DesktopApp> =
            self.apps.iter().filter(|a| matches(a, q)).collect();
        app_hits.sort_by_key(|a| (!recent_names.contains(a.name.as_str()), a.name.to_lowercase()));
        for app in app_hits.into_iter().take(space(rows.len()).min(5)) {
            rows.push(Row::app(app, recent_names.contains(app.name.as_str())));
        }

        for i in files::search_files(&self.file_index, q).into_iter().take(space(rows.len()).min(4)) {
            let entry = &self.file_index[i];
            let is_image = !entry.is_dir && files::is_image(&entry.name);
            rows.push(Row {
                title: entry.name.clone(),
                subtitle: files::describe(entry),
                tag: if entry.is_dir { "Folder" } else { "File" },
                icon: if is_image {
                    RowIcon::Raster(entry.path.clone())
                } else if entry.is_dir {
                    RowIcon::Letter('F', 0x22c55eff)
                } else {
                    RowIcon::Letter(
                        entry.name.chars().next().unwrap_or('?').to_uppercase().next().unwrap_or('?'),
                        0x6b7280ff,
                    )
                },
                action: RowAction::File(entry.clone()),
                removable: None,
                verb: "Open",
            });
        }

        for path in favorite_hits(&self.favorites, q).into_iter().take(space(rows.len()).min(2)) {
            rows.push(Row {
                title: path.file_name().and_then(|n| n.to_str()).unwrap_or("?").to_string(),
                subtitle: path.to_string_lossy().to_string(),
                tag: "Folder",
                icon: RowIcon::Letter('F', 0x22c55eff),
                action: RowAction::File(files::FileEntry {
                    name: String::new(),
                    path: path.clone(),
                    is_dir: true,
                    size: 0,
                    modified: 0,
                }),
                removable: Some(Removable::Favorite(path.clone())),
                verb: "Open",
            });
        }

        for i in links::search_links(&self.links, q).into_iter().take(space(rows.len()).min(3)) {
            let link = &self.links[i];
            rows.push(Row {
                title: link.name.clone(),
                subtitle: link.url.clone(),
                tag: "Link",
                icon: RowIcon::Letter(
                    link.name.chars().next().unwrap_or('?').to_uppercase().next().unwrap_or('?'),
                    0xeab308ff,
                ),
                action: RowAction::Url(link.url.clone()),
                removable: Some(Removable::Link(link.name.clone())),
                verb: "Open",
            });
        }

        for i in bookmarks::search_bookmarks(&self.bookmarks, q)
            .into_iter()
            .take(space(rows.len()).min(3))
        {
            let bookmark = &self.bookmarks[i];
            rows.push(Row {
                title: if bookmark.title.is_empty() {
                    bookmark.url.clone()
                } else {
                    bookmark.title.clone()
                },
                subtitle: bookmark.url.clone(),
                tag: "Bookmark",
                icon: RowIcon::Letter(
                    bookmark.title.chars().next().unwrap_or('?').to_uppercase().next().unwrap_or('?'),
                    0xef4444ff,
                ),
                action: RowAction::Url(bookmark.url.clone()),
                removable: None,
                verb: "Open",
            });
        }

        for e in emoji::search_emoji(q).into_iter().take(space(rows.len()).min(4)) {
            rows.push(Row {
                title: e.name.to_string(),
                subtitle: e.category.to_string(),
                tag: "Emoji",
                icon: RowIcon::Glyph(e.char.to_string()),
                action: RowAction::Copy(e.char.to_string()),
                removable: None,
                verb: "Copy",
            });
        }

        let clip_ids = self
            .clipboard
            .lock()
            .map(|h| h.search(q))
            .unwrap_or_default();
        let clips = self.clip_entries();
        for id in clip_ids.into_iter().take(space(rows.len()).min(3)) {
            if let Some(entry) = clips.iter().find(|e| e.id == id) {
                rows.push(clip_row(entry));
            }
        }

        if !rows.is_empty() {
            sections.push(Section {
                header: "Results".to_string(),
                rows,
            });
        }
    }

    fn selected_row(&self, cx: &App) -> Option<Row> {
        let (sections, flat) = self.sections(cx);
        if flat.is_empty() {
            return None;
        }
        let idx = self.selected.min(flat.len() - 1);
        let (si, ri) = flat[idx];
        sections.get(si)?.rows.get(ri).cloned()
    }

    fn move_up(&mut self, cx: &mut Context<Self>) {
        self.selected = self.selected.saturating_sub(1);
        cx.notify();
    }

    fn move_down(&mut self, cx: &mut Context<Self>) {
        let (_, flat) = self.sections(cx);
        self.selected = (self.selected + 1).min(flat.len().saturating_sub(1));
        cx.notify();
    }

    fn confirm(&mut self, cx: &mut Context<Self>) {
        let Some(row) = self.selected_row(cx) else { return };
        self.run_row(&row, cx);
    }

    fn run_row(&mut self, row: &Row, cx: &mut Context<Self>) {
        match &row.action {
            RowAction::App(app) => launch_app(&app.name, &app.exec, app.icon.clone()),
            RowAction::RecentApp(r) => launch_app(&r.name, &r.exec, r.icon.clone()),
            RowAction::File(entry) => {
                if entry.path.is_dir() {
                    platform::open_path(&entry.path);
                } else if entry.path.is_file() {
                    platform::open_path(&entry.path);
                }
            }
            RowAction::Url(url) => platform::open_url(url),
            RowAction::Copy(text) => clipboard::ClipHistory::copy_text(text),
            RowAction::PasteClip(id) => {
                if let Ok(h) = self.clipboard.lock() {
                    h.restore(*id);
                }
            }
            RowAction::Run(cmd) => self.run_command(cmd.clone(), cx),
        }
        // Refresh recents after any launch.
        self.recents = recents::prune_missing(recents::load_recents(), &self.installed);
        cx.notify();
    }

    fn run_command(&mut self, cmd: ManagementCommand, cx: &mut Context<Self>) {
        match cmd {
            ManagementCommand::SaveLink { name, url } => {
                if !self.links.iter().any(|l| l.name == name) {
                    self.links.push(links::QuickLink {
                        name,
                        url,
                        keyword: String::new(),
                    });
                    links::save_links(&self.links);
                }
                self.editor.update(cx, |e, cx| e.clear(cx));
            }
            ManagementCommand::EditLink { name, url } => {
                if let Some(link) = self.links.iter_mut().find(|l| l.name == name) {
                    link.url = url;
                    links::save_links(&self.links);
                }
                self.editor.update(cx, |e, cx| e.clear(cx));
            }
            ManagementCommand::AddFolder(path) => {
                if !self.favorites.contains(&path) {
                    self.favorites.push(path);
                    favorites::save_favorites(&self.favorites);
                }
                self.editor.update(cx, |e, cx| e.clear(cx));
            }
            ManagementCommand::ClearClipboard => {
                if let Ok(mut h) = self.clipboard.lock() {
                    h.clear();
                }
                self.editor.update(cx, |e, cx| e.clear(cx));
            }
            ManagementCommand::ClearRecents => {
                recents::clear_recents();
                self.recents.clear();
                self.editor.update(cx, |e, cx| e.clear(cx));
            }
            ManagementCommand::PauseClipboard(paused) => {
                if let Ok(mut h) = self.clipboard.lock() {
                    h.set_paused(paused);
                }
                self.editor.update(cx, |e, cx| e.clear(cx));
            }
        }
    }

    fn alternate(&mut self, cx: &mut Context<Self>) {
        // Ctrl+Enter: reveal files in their folder, open everything else.
        let Some(row) = self.selected_row(cx) else { return };
        match &row.action {
            RowAction::File(entry) if !entry.path.is_dir() => {
                platform::reveal_path(&entry.path)
            }
            _ => self.run_row(&row, cx),
        }
    }

    fn remove_selected(&mut self, cx: &mut Context<Self>) {
        let Some(row) = self.selected_row(cx) else { return };
        match row.removable {
            Some(Removable::Favorite(path)) => {
                self.favorites.retain(|p| p != &path);
                favorites::save_favorites(&self.favorites);
                self.selected = self.selected.saturating_sub(1);
                cx.notify();
            }
            Some(Removable::Link(name)) => {
                self.links.retain(|l| l.name != name);
                links::save_links(&self.links);
                self.selected = self.selected.saturating_sub(1);
                cx.notify();
            }
            Some(Removable::Clip(id)) => {
                if let Ok(mut h) = self.clipboard.lock() {
                    h.remove(id);
                }
                self.selected = self.selected.saturating_sub(1);
                cx.notify();
            }
            None => {}
        }
    }

    fn clear_search(&mut self, cx: &mut Context<Self>) {
        self.selected = 0;
        self.editor.update(cx, |editor, cx| editor.clear(cx));
    }
}

fn simple_cmd(title: &str, subtitle: &str, cmd: ManagementCommand, verb: &'static str) -> Row {
    Row {
        title: title.to_string(),
        subtitle: subtitle.to_string(),
        tag: "Command",
        icon: RowIcon::Letter(
            title.chars().next().unwrap_or('?').to_uppercase().next().unwrap_or('?'),
            0x6b7280ff,
        ),
        action: RowAction::Run(cmd),
        removable: None,
        verb,
    }
}

fn clip_row(entry: &clipboard::ClipEntry) -> Row {
    let icon = match &entry.kind {
        clipboard::ClipKind::Text(t) => RowIcon::Letter(
            t.chars().find(|c| !c.is_whitespace()).unwrap_or('T').to_uppercase().next().unwrap_or('T'),
            0x14b8a6ff,
        ),
        clipboard::ClipKind::Image { .. } => RowIcon::Letter('I', 0x6b7280ff),
    };
    Row {
        title: entry.preview(),
        subtitle: match &entry.kind {
            clipboard::ClipKind::Text(_) => format!("{} chars", entry_char_count(entry)),
            clipboard::ClipKind::Image { .. } => "Screenshot / image".to_string(),
        },
        tag: entry.tag(),
        icon,
        action: RowAction::PasteClip(entry.id),
        removable: Some(Removable::Clip(entry.id)),
        verb: "Paste",
    }
}

fn entry_char_count(entry: &clipboard::ClipEntry) -> usize {
    match &entry.kind {
        clipboard::ClipKind::Text(t) => t.chars().count(),
        clipboard::ClipKind::Image { .. } => 0,
    }
}

fn favorite_hits(favorites: &[PathBuf], query: &str) -> Vec<PathBuf> {
    let q = query.to_lowercase();
    favorites
        .iter()
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.to_lowercase().contains(&q))
                .unwrap_or(false)
        })
        .cloned()
        .collect()
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn render_icon(icon: &RowIcon) -> gpui::AnyElement {
    match icon {
        RowIcon::Raster(path) => div()
            .size(px(28.0))
            .rounded_md()
            .overflow_hidden()
            .child(gpui::img(path.clone()).size(px(28.0)))
            .into_any_element(),
        RowIcon::Letter(ch, bg) => div()
            .size(px(28.0))
            .rounded_md()
            .bg(rgba(*bg))
            .flex()
            .justify_center()
            .items_center()
            .text_color(rgba(0xffffffff))
            .child(ch.to_string())
            .into_any_element(),
        RowIcon::Glyph(g) => div()
            .size(px(28.0))
            .flex()
            .justify_center()
            .items_center()
            .text_size(px(20.0))
            .child(g.clone())
            .into_any_element(),
    }
}

impl Render for Fastcast {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (sections, flat) = self.sections(cx);
        let selected = self.selected.min(flat.len().saturating_sub(1));
        let editor = self.editor.clone();
        let view = cx.entity();
        let verb = flat
            .get(selected)
            .and_then(|(si, ri)| sections.get(*si)?.rows.get(*ri))
            .map(|r| r.verb)
            .unwrap_or("Open");

        div()
            .id("fastcast")
            .key_context("Fastcast")
            .on_action({
                let view = view.clone();
                move |_: &MoveUp, _: &mut Window, cx: &mut App| {
                    view.update(cx, |this, cx| this.move_up(cx))
                }
            })
            .on_action({
                let view = view.clone();
                move |_: &MoveDown, _: &mut Window, cx: &mut App| {
                    view.update(cx, |this, cx| this.move_down(cx))
                }
            })
            .on_action({
                let view = view.clone();
                move |_: &Confirm, _: &mut Window, cx: &mut App| {
                    view.update(cx, |this, cx| this.confirm(cx))
                }
            })
            .on_action({
                let view = view.clone();
                move |_: &Clear, _: &mut Window, cx: &mut App| {
                    view.update(cx, |this, cx| this.clear_search(cx))
                }
            })
            .on_action({
                let view = view.clone();
                move |_: &RemoveSelected, _: &mut Window, cx: &mut App| {
                    view.update(cx, |this, cx| this.remove_selected(cx))
                }
            })
            .on_action({
                let view = view.clone();
                move |_: &RevealSelected, _: &mut Window, cx: &mut App| {
                    view.update(cx, |this, cx| this.alternate(cx))
                }
            })
            .flex()
            .flex_col()
            .size_full()
            .bg(rgba(0x1b1b22b0))
            .backdrop_blur(px(24.0))
            .rounded_xl()
            .border_1()
            .border_color(rgba(0xffffff26))
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .items_center()
                    .px_5()
                    .py_4()
                    .border_b_1()
                    .border_color(rgba(0xffffff1f))
                    .child(
                        div()
                            .id("search")
                            .key_context("SearchInput")
                            .track_focus(&editor.read(cx).focus_handle)
                            .cursor(CursorStyle::IBeam)
                            .map(input::standard_actions(editor.clone()))
                            .flex()
                            .flex_1()
                            .items_center()
                            .text_color(rgba(0xffffffd0))
                            .text_lg()
                            .child(editor.clone()),
                    ),
            )
            .child(
                div()
                    .id("results")
                    .flex_1()
                    .flex()
                    .flex_col()
                    .py_2()
                    .overflow_y_scroll()
                    .children(sections.iter().enumerate().map(|(si, section)| {
                        div().flex().flex_col().gap_1().children(
                            std::iter::once(
                                div()
                                    .px_6()
                                    .pt_2()
                                    .pb_1()
                                    .text_color(rgba(0xffffffa0))
                                    .child(section.header.clone())
                                    .into_any_element(),
                            )
                            .chain(section.rows.iter().enumerate().map(|(ri, row)| {
                                let is_selected =
                                    flat.get(selected) == Some(&(si, ri));
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .px_3()
                                    .py_2()
                                    .mx_3()
                                    .rounded_lg()
                                    .when(is_selected, |s| s.bg(rgba(0xffffff14)))
                                    .hover(|s| s.bg(rgba(0xffffff14)))
                                    .child(render_icon(&row.icon))
                                    .child(
                                        div().text_color(rgba(0xffffffff)).child(row.title.clone()),
                                    )
                                    .when(!row.subtitle.is_empty(), |s| {
                                        s.child(
                                            div()
                                                .text_color(rgba(0xffffff80))
                                                .child(row.subtitle.clone()),
                                        )
                                    })
                                    .child(div().flex_1())
                                    .child(
                                        div().text_color(rgba(0xffffff80)).child(row.tag),
                                    )
                                    .into_any_element()
                            }))
                            .collect::<Vec<_>>(),
                        )
                    })),
            )
            .when(flat.is_empty(), |this| {
                this.child(
                    div()
                        .px_6()
                        .py_4()
                        .text_color(rgba(0xffffff70))
                        .child("No results found"),
                )
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .px_5()
                    .py_3()
                    .border_t_1()
                    .border_color(rgba(0xffffff1f))
                    .child(div().flex_1())
                    .child(div().text_color(rgba(0xffffffd0)).child(verb))
                    .child(
                        div()
                            .ml_2()
                            .px_2()
                            .rounded_md()
                            .bg(rgba(0xffffff1f))
                            .text_color(rgba(0xffffffa0))
                            .child("⏎"),
                    )
                    .child(div().mx_3().text_color(rgba(0xffffff30)).child("|"))
                    .child(div().text_color(rgba(0xffffffd0)).child("Reveal"))
                    .child(
                        div()
                            .ml_2()
                            .px_2()
                            .rounded_md()
                            .bg(rgba(0xffffff1f))
                            .text_color(rgba(0xffffffa0))
                            .child("⌃⏎"),
                    )
                    .child(div().mx_3().text_color(rgba(0xffffff30)).child("|"))
                    .child(div().text_color(rgba(0xffffffd0)).child("Remove"))
                    .child(
                        div()
                            .ml_2()
                            .px_2()
                            .rounded_md()
                            .bg(rgba(0xffffff1f))
                            .text_color(rgba(0xffffffa0))
                            .child("⇧⌫"),
                    ),
            )
    }
}

fn main() {
    application().run(|cx: &mut App| {
        cx.bind_keys([
            KeyBinding::new("backspace", input::Backspace, Some("SearchInput")),
            KeyBinding::new("delete", input::Delete, Some("SearchInput")),
            KeyBinding::new("left", input::Left, Some("SearchInput")),
            KeyBinding::new("right", input::Right, Some("SearchInput")),
            KeyBinding::new("home", input::Home, Some("SearchInput")),
            KeyBinding::new("end", input::End, Some("SearchInput")),
            KeyBinding::new("up", MoveUp, Some("Fastcast")),
            KeyBinding::new("down", MoveDown, Some("Fastcast")),
            KeyBinding::new("enter", Confirm, Some("Fastcast")),
            KeyBinding::new("escape", Clear, Some("Fastcast")),
            KeyBinding::new("shift-delete", RemoveSelected, Some("Fastcast")),
            KeyBinding::new("ctrl-enter", RevealSelected, Some("Fastcast")),
        ]);
        let bounds = Bounds::centered(None, size(px(750.0), px(620.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                is_resizable: false,
                is_minimizable: false,
                titlebar: None,
                window_decorations: Some(gpui::WindowDecorations::Client),
                window_background: gpui::WindowBackgroundAppearance::Blurred,
                window_min_size: Some(size(px(750.0), px(620.0))),
                ..Default::default()
            },
            |window, cx| {
                cx.new(|cx| {
                    let apps = load_apps();
                    let installed: HashSet<String> =
                        apps.iter().map(|a| a.name.clone()).collect();
                    let recents =
                        recents::prune_missing(recents::load_recents(), &installed);
                    let editor = cx.new(|cx| Editor::new("", window, cx));
                    let focus = editor.read(cx).focus_handle.clone();
                    focus.focus(window, cx);
                    let value = editor.read(cx).value();
                    let editor_sub = cx.observe(&editor, |_: &mut Fastcast, _, cx| {
                        cx.notify();
                    });
                    let value_sub = cx.observe(&value, |this: &mut Fastcast, _, cx| {
                        this.selected = 0;
                        cx.notify();
                    });

                    // Background file index + clipboard-dirty polling (300ms).
                    let file_rx = files::start_indexing();
                    let clipboard = Arc::new(Mutex::new(clipboard::ClipHistory::load()));
                    let dirty: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
                    clipboard::start_watcher(clipboard.clone(), {
                        let dirty = dirty.clone();
                        move || dirty.store(true, Ordering::SeqCst)
                    });
                    let mut file_rx = Some(file_rx);
                    let poll = cx.spawn(async move |view, cx| {
                        loop {
                            cx.background_executor()
                                .timer(Duration::from_millis(300))
                                .await;
                            let mut changed = dirty.swap(false, Ordering::SeqCst);
                            if let Some(rx) = file_rx.take() {
                                match rx.try_recv() {
                                    Ok(index) => {
                                        changed = true;
                                        if view
                                            .update(cx, |this, cx| {
                                                this.file_index = index;
                                                this.indexing = false;
                                                this.selected = 0;
                                                cx.notify();
                                            })
                                            .is_err()
                                        {
                                            break;
                                        }
                                    }
                                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                                        file_rx = Some(rx);
                                    }
                                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                                        let _ = view.update(cx, |this, cx| {
                                            this.indexing = false;
                                            cx.notify();
                                        });
                                    }
                                }
                            }
                            if changed
                                && view
                                    .update(cx, |_, cx| cx.notify())
                                    .is_err()
                            {
                                break;
                            }
                        }
                    });
                    poll.detach();

                    Fastcast {
                        selected: 0,
                        apps,
                        installed,
                        recents,
                        links: links::load_links(),
                        favorites: favorites::load_favorites(),
                        bookmarks: bookmarks::load_bookmarks(),
                        file_index: Vec::new(),
                        indexing: true,
                        clipboard,
                        editor,
                        _subscriptions: vec![editor_sub, value_sub],
                    }
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
