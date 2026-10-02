use gpui::{
    Animation, AnimationExt as _, App, Bounds, Context, FocusHandle, KeyDownEvent, Render, Window,
    WindowBounds, WindowOptions, div, prelude::*, px, rgba, size,
};
use gpui_platform::application;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Clone)]
struct DesktopApp {
    name: String,
    exec: String,
    keywords: String,
    icon: Option<PathBuf>,
}

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

fn matches(app: &DesktopApp, query: &str) -> bool {
    let hay = format!("{} {}", app.name, app.keywords).to_lowercase();
    query
        .to_lowercase()
        .split_whitespace()
        .all(|word| hay.contains(word))
}

/// The keyword from this app's desktop entry that best matches the query.
fn best_keyword<'a>(app: &'a DesktopApp, query: &str) -> Option<&'a str> {
    let q = query.to_lowercase();
    app.keywords
        .split(';')
        .map(|k| k.trim())
        .filter(|k| !k.is_empty())
        .find(|k| {
            q.split_whitespace()
                .any(|word| k.to_lowercase().contains(word))
                || q.is_empty()
        })
}

fn launch(app: &DesktopApp) {
    // Strip desktop-entry field codes like %f %U %c ...
    let stripped = app
        .exec
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

struct Fastcast {
    query: String,
    selected: usize,
    apps: Vec<DesktopApp>,
    focus_handle: FocusHandle,
}

impl Fastcast {
    fn results(&self) -> Vec<&DesktopApp> {
        if self.query.is_empty() {
            return self.apps.iter().take(8).collect();
        }
        self.apps
            .iter()
            .filter(|app| matches(app, &self.query))
            .take(10)
            .collect()
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "backspace" => {
                self.query.pop();
                self.selected = 0;
            }
            "escape" => self.query.clear(),
            "up" => self.selected = self.selected.saturating_sub(1),
            "down" => {
                let max = self.results().len().saturating_sub(1);
                self.selected = (self.selected + 1).min(max);
            }
            "enter" => {
                let results = self.results();
                if let Some(app) = results.get(self.selected) {
                    let app = (*app).clone();
                    launch(&app);
                }
            }
            _ => {
                if let Some(ch) = &event.keystroke.key_char {
                    self.query.push_str(ch);
                    self.selected = 0;
                }
            }
        }
        cx.notify();
    }
}

impl Render for Fastcast {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let results = self.results();
        let selected = self.selected;

        div()
            .id("fastcast")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key_down))
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
                    .text_color(rgba(0xffffffd0))
                    .text_lg()
                    .when(self.query.is_empty(), |s| {
                        s.child(
                            div()
                                .w(px(2.0))
                                .h(px(20.0))
                                .mr_2()
                                .bg(rgba(0xffffffd0))
                                .with_animation(
                                    "cursor_blink_empty",
                                    Animation::new(Duration::from_millis(800)).repeat(),
                                    |caret, delta| {
                                        caret.opacity(if delta < 0.5 { 1.0 } else { 0.0 })
                                    },
                                ),
                        )
                        .child(
                            div()
                                .text_color(rgba(0xffffff70))
                                .child("Search for apps and commands..."),
                        )
                    })
                    .when(!self.query.is_empty(), |s| {
                        s.child(div().child(self.query.clone())).child(
                            div()
                                .w(px(2.0))
                                .h(px(20.0))
                                .ml_0p5()
                                .bg(rgba(0xffffffd0))
                                .with_animation(
                                    "cursor_blink",
                                    Animation::new(Duration::from_millis(800)).repeat(),
                                    |caret, delta| {
                                        caret.opacity(if delta < 0.5 { 1.0 } else { 0.0 })
                                    },
                                ),
                        )
                    })
                    .child(div().flex_1()),
            )
            .child(
                div().flex_1().flex().flex_col().gap_1().py_2().children(
                    results
                        .iter()
                        .enumerate()
                        .map(|(i, app)| {
                            let is_selected = i == selected;
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
                                .child(match &app.icon {
                                    Some(path) => div()
                                        .size(px(28.0))
                                        .rounded_md()
                                        .overflow_hidden()
                                        .child(
                                            gpui::img(path.clone())
                                                .size(px(28.0)),
                                        )
                                        .into_any_element(),
                                    None => div()
                                        .size(px(28.0))
                                        .rounded_md()
                                        .bg(rgba(0x3b82f6ff))
                                        .flex()
                                        .justify_center()
                                        .items_center()
                                        .text_color(rgba(0xffffffff))
                                        .child(
                                            app.name
                                                .chars()
                                                .next()
                                                .unwrap_or('?')
                                                .to_uppercase()
                                                .to_string(),
                                        )
                                        .into_any_element(),
                                })
                                .child(div().text_color(rgba(0xffffffff)).child(app.name.clone()))
                                .when_some(best_keyword(app, &self.query).map(|k| k.to_string()), |s, kw| {
                                    s.child(div().text_color(rgba(0xffffff80)).child(kw))
                                })
                                .child(div().flex_1())
                                .child(
                                    div()
                                        .text_color(rgba(0xffffff80))
                                        .child("Application"),
                                )
                        })
                        .collect::<Vec<_>>(),
                ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .px_5()
                    .py_3()
                    .border_t_1()
                    .border_color(rgba(0xffffff1f))
                    .child(div().flex_1())
                    .child(div().text_color(rgba(0xffffffd0)).child("Open"))
                    .child(
                        div()
                            .ml_2()
                            .px_2()
                            .rounded_md()
                            .bg(rgba(0xffffff1f))
                            .text_color(rgba(0xffffffa0))
                            .child("⏎"),
                    )
                    .child(
                        div()
                            .mx_3()
                            .text_color(rgba(0xffffff30))
                            .child("|"),
                    )
                    .child(div().text_color(rgba(0xffffffd0)).child("Actions"))
                    .child(
                        div()
                            .ml_2()
                            .px_2()
                            .rounded_md()
                            .bg(rgba(0xffffff1f))
                            .text_color(rgba(0xffffffa0))
                            .child("⌘ K"),
                    ),
            )
    }
}

fn main() {
    application().run(|cx: &mut App| {
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
                    let focus_handle = cx.focus_handle();
                    focus_handle.focus(window, cx);
                    Fastcast {
                        query: String::new(),
                        selected: 0,
                        apps: load_apps(),
                        focus_handle,
                    }
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
