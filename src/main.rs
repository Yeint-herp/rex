#![allow(clippy::needless_return)]

use eframe::{
    Frame, egui,
    egui::{Button, Context, Key, ProgressBar, TopBottomPanel},
};
use egui::{RichText, TextEdit};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    time::{Duration, Instant},
};

mod browser;
mod clipboard;
mod config;
mod fs_ops;
mod history;
mod platform;
mod searcher;

#[derive(Clone)]
struct Toast {
    text: String,
    created: Instant,
    ttl: Duration,
}
struct Toaster {
    items: Vec<Toast>,
}

impl Toaster {
    fn new() -> Self {
        Self { items: vec![] }
    }

    fn info(&mut self, text: impl Into<String>) {
        self.items.push(Toast {
            text: text.into(),
            created: Instant::now(),
            ttl: Duration::from_secs(4),
        });
    }

    fn error(&mut self, text: impl Into<String>) {
        self.items.push(Toast {
            text: format!("❗ {}", text.into()),
            created: Instant::now(),
            ttl: Duration::from_secs(6),
        });
    }

    fn draw(&mut self, ui: &mut egui::Ui) {
        self.items.retain(|t| t.created.elapsed() < t.ttl);
        for t in &self.items {
            ui.label(RichText::new(&t.text));
        }
    }
}

#[derive(Clone, Copy)]
enum CreateKind {
    Folder,
    File,
}

enum ViewMode {
    Browsing,
    Searching {
        results: Vec<PathBuf>,
        rx_results: Receiver<searcher::SearchMsg>,
        rx_prog: Receiver<searcher::ProgressMsg>,
        abort: Arc<AtomicBool>,
        scanned_files: u64,
        scanned_dirs: u64,
        done: bool,
        started_at: Instant,
    },
}

struct AppData {
    current_path: PathBuf,
    path_edit: String,

    pinned: Vec<PathBuf>,

    search_query: String,
    mode: ViewMode,

    nav_hist: history::NavHistory,

    ops_hist: history::OpsHistory,

    autocomplete: Vec<String>,
    scale_factor: f32,
    browser: browser::FileBrowser,

    clipboard: clipboard::Clipboard,

    open_with_buffer: String,
    open_with_target: Option<PathBuf>,

    toasts: Toaster,

    create_dialog: Option<(CreateKind, PathBuf)>,
    create_name_buffer: String,
}

impl Default for AppData {
    fn default() -> Self {
        let current_path = std::env::current_dir().unwrap_or_else(|_| config::os_root());
        Self {
            path_edit: current_path.display().to_string(),
            current_path,
            pinned: config::load_pinned(),
            search_query: String::new(),
            mode: ViewMode::Browsing,
            nav_hist: history::NavHistory::default(),
            ops_hist: history::OpsHistory::new(64),
            autocomplete: vec![],
            scale_factor: config::load_scale(),
            browser: browser::FileBrowser::new(),
            clipboard: clipboard::Clipboard::default(),
            open_with_buffer: String::new(),
            open_with_target: None,
            toasts: Toaster::new(),
            create_dialog: None,
            create_name_buffer: String::new(),
        }
    }
}

impl Drop for AppData {
    fn drop(&mut self) {
        config::save_pinned(&self.pinned);
        config::save_scale(self.scale_factor);
    }
}

impl AppData {
    fn navigate_to(&mut self, new_path: PathBuf) {
        if new_path.exists() && new_path.is_dir() {
            if new_path != self.current_path {
                self.nav_hist.push(self.current_path.clone());
            }
            self.current_path = new_path.clone();
            self.path_edit = new_path.display().to_string();
            self.browser.invalidate();
        } else {
            self.toasts
                .error("Path does not exist or is not a directory.");
            self.path_edit = self.current_path.display().to_string();
        }
    }
    fn back(&mut self) {
        let _ = self.nav_hist.back(&mut self.current_path);
        self.path_edit = self.current_path.display().to_string();
        self.browser.invalidate();
    }
    fn forward(&mut self) {
        let _ = self.nav_hist.forward(&mut self.current_path);
        self.path_edit = self.current_path.display().to_string();
        self.browser.invalidate();
    }

    fn update_autocomplete(&mut self) {
        let input = self.path_edit.clone();
        let parent = PathBuf::from(&input)
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| self.current_path.clone());

        if !parent.exists() || !parent.is_dir() {
            self.autocomplete.clear();
            return;
        }
        let prefix = PathBuf::from(&input)
            .file_name()
            .map(|s| s.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let mut matches = vec![];
        if let Ok(read) = std::fs::read_dir(parent) {
            for entry in read.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = path
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default();
                    if name.to_lowercase().starts_with(&prefix) {
                        matches.push(path.display().to_string());
                    }
                }
            }
        }
        matches.sort();
        matches.truncate(6);
        self.autocomplete = matches;
    }

    fn start_search(&mut self) {
        let (tx_res, rx_res) = mpsc::channel::<searcher::SearchMsg>();
        let (tx_prog, rx_prog) = mpsc::channel::<searcher::ProgressMsg>();
        let abort = Arc::new(AtomicBool::new(false));
        searcher::spawn_search(
            self.current_path.clone(),
            self.search_query.clone(),
            tx_res,
            tx_prog,
            abort.clone(),
        );
        self.mode = ViewMode::Searching {
            results: vec![],
            rx_results: rx_res,
            rx_prog,
            abort,
            scanned_files: 0,
            scanned_dirs: 0,
            done: false,
            started_at: Instant::now(),
        };
    }

    fn cancel_search(&mut self) {
        if let ViewMode::Searching { abort, .. } = &self.mode {
            abort.store(true, Ordering::Relaxed);
        }
        self.mode = ViewMode::Browsing;
    }

    fn paste_into(&mut self, target_dir: &Path) {
        if !self.clipboard.has_items() {
            return;
        }
        let mode = self.clipboard.mode.unwrap();
        let mut any_ok = false;
        for item in self.clipboard.items.clone() {
            let res = match mode {
                clipboard::Mode::Copy => fs_ops::copy(&item, target_dir),
                clipboard::Mode::Cut => fs_ops::mv(&item, target_dir),
            };
            match res {
                Ok(op) => {
                    self.ops_hist.push(op);
                    any_ok = true;
                }
                Err(e) => self
                    .toasts
                    .error(format!("Paste failed for {}: {e}", item.display())),
            }
        }
        if any_ok {
            if mode == clipboard::Mode::Cut {
                self.clipboard.clear();
            }
            self.toasts.info("Paste complete.");
            self.browser.invalidate();
        }
    }

    fn try_undo(&mut self) {
        if let Some(op) = self.ops_hist.pop_undo() {
            match fs_ops::undo(&op) {
                Ok(()) => {
                    self.toasts.info("Undid last operation.");
                    self.browser.invalidate();
                }
                Err(e) => {
                    self.toasts.error(format!("Undo failed: {e}"));
                }
            }
        }
    }
}

impl eframe::App for AppData {
    fn update(&mut self, ctx: &Context, _: &mut Frame) {
        ctx.set_pixels_per_point(self.scale_factor);
        self.scale_factor = ctx.input(|i| {
            let mut s = self.scale_factor;
            if i.modifiers.ctrl {
                if i.key_pressed(Key::Equals) {
                    s = (s + 0.1).clamp(0.5, 3.0);
                }
                if i.key_pressed(Key::Minus) {
                    s = (s - 0.1).clamp(0.5, 3.0);
                }
                if i.raw_scroll_delta.y.abs() > f32::EPSILON {
                    s = (s + i.raw_scroll_delta.y * 0.01).clamp(0.5, 3.0);
                }
                if i.key_pressed(Key::Num0) {
                    s = 1.0;
                }
                if i.key_pressed(Key::N) {
                    self.create_dialog = Some((CreateKind::File, self.current_path.clone()));
                    self.create_name_buffer = "New File.txt".into();
                }
                if i.modifiers.shift && i.key_pressed(Key::N) {
                    self.create_dialog = Some((CreateKind::Folder, self.current_path.clone()));
                    self.create_name_buffer = "New Folder".into();
                }
            }
            s
        });

        TopBottomPanel::top("titlebar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(self.nav_hist.can_back(), Button::new("⮌"))
                    .clicked()
                {
                    self.back();
                }
                if ui
                    .add_enabled(self.nav_hist.can_forward(), Button::new("⮎"))
                    .clicked()
                {
                    self.forward();
                }

                if ui.button("⬆").clicked() {
                    if let Some(parent) = self.current_path.parent() {
                        self.navigate_to(parent.to_path_buf());
                    }
                }

                let resp = ui.add(TextEdit::singleline(&mut self.path_edit).desired_width(400.0));
                if resp.changed() {
                    self.update_autocomplete();
                }
                let enter = ui.input(|i| i.key_pressed(Key::Enter));
                if resp.lost_focus() || enter {
                    self.autocomplete.clear();
                }
                if enter {
                    self.navigate_to(PathBuf::from(self.path_edit.clone()));
                }

                if !self.autocomplete.is_empty() {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        for s in self.autocomplete.clone() {
                            if ui.button(&s).clicked() {
                                self.path_edit = s.clone();
                                self.autocomplete.clear();
                                self.navigate_to(PathBuf::from(s));
                            }
                        }
                    });
                }

                ui.separator();

                ui.add(
                    TextEdit::singleline(&mut self.search_query).hint_text("Search file name..."),
                );
                if ui.button("🔍").clicked() {
                    self.start_search();
                }

                if ui.button("↻").clicked() {
                    self.browser.invalidate();
                }
            });
        });

        let pinned = self.pinned.clone();
        egui::SidePanel::left("sidebar")
            .resizable(false)
            .default_width(170.0)
            .show(ctx, |ui| {
                ui.heading("📌 Pinned");
                let mut to_unpin = None::<PathBuf>;
                for p in pinned {
                    let name = p
                        .file_name()
                        .or_else(|| p.components().last().map(|c| c.as_os_str()))
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    let r = ui.button(name);
                    if r.clicked() {
                        self.navigate_to(p.clone());
                    }
                    r.context_menu(|ui| {
                        if ui.button("❌ Unpin").clicked() {
                            to_unpin = Some(p.clone());
                            ui.close_menu();
                        }
                        if ui.button("📂 Show in parent").clicked() {
                            if let Some(parent) = p.parent() {
                                self.navigate_to(parent.to_path_buf());
                            }
                            ui.close_menu();
                        }
                    });
                }
                if let Some(up) = to_unpin {
                    self.pinned.retain(|x| x != &up);
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            if let ViewMode::Searching {
                results,
                rx_results,
                rx_prog,
                scanned_files,
                scanned_dirs,
                done,
                started_at,
                ..
            } = &mut self.mode
            {
                while let Ok(m) = rx_results.try_recv() {
                    results.push(m.path);
                }
                while let Ok(p) = rx_prog.try_recv() {
                    *scanned_files = p.scanned_files;
                    *scanned_dirs = p.scanned_dirs;
                    if p.done {
                        *done = true;
                    }
                }

                let results_snapshot: Vec<PathBuf> = results.clone();
                let sf = *scanned_files;
                let sd = *scanned_dirs;
                let dn = *done;
                let st = *started_at;

                let mut cancel_requested = false;
                let mut navigate_to: Option<PathBuf> = None;

                ui.horizontal(|ui| {
                    let elapsed = st.elapsed().as_secs_f32();
                    let val = if dn {
                        1.0
                    } else {
                        ((elapsed * 0.4).sin() * 0.5 + 0.5).clamp(0.05, 0.95)
                    };
                    ui.add(ProgressBar::new(val).show_percentage());
                    ui.label(format!(
                        "Scanned: {sf} files in {sd} folders  •  Results: {}",
                        results_snapshot.len()
                    ));
                    if ui.button("❌ Cancel").clicked() {
                        cancel_requested = true;
                    }
                });
                ui.separator();

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for path in &results_snapshot {
                        if ui.button(path.display().to_string()).clicked() {
                            navigate_to = Some(path.clone());
                        }
                    }
                });

                if cancel_requested {
                    self.cancel_search();
                }
                if let Some(p) = navigate_to {
                    if let Some(dir) = p.parent() {
                        self.navigate_to(dir.to_path_buf());
                    }
                    self.mode = ViewMode::Browsing;
                }
            } else {
                let mut on_open = None::<PathBuf>;
                let mut on_pin = None::<PathBuf>;
                let mut on_rename = None::<(PathBuf, String)>;
                let mut on_delete = None::<PathBuf>;
                let mut on_open_with = None::<PathBuf>;
                let mut on_open_term = None::<PathBuf>;

                let mut on_copy_req = None::<PathBuf>;
                let mut on_cut_req = None::<PathBuf>;
                let mut on_paste_here = None::<PathBuf>;
                let mut on_undo_req = false;
                let mut on_new_folder_here = None::<PathBuf>;
                let mut on_new_file_here = None::<PathBuf>;

                self.browser.update(
                    ctx,
                    ui,
                    &self.current_path,
                    &mut on_open,
                    &mut on_pin,
                    &mut on_rename,
                    &mut on_delete,
                    &mut on_open_with,
                    &mut on_open_term,
                    &mut on_copy_req,
                    &mut on_cut_req,
                    &mut on_paste_here,
                    &mut on_undo_req,
                    self.clipboard.has_items(),
                    &mut on_new_folder_here,
                    &mut on_new_file_here,
                );

                if let Some(nav) = on_open {
                    self.navigate_to(nav);
                }
                if let Some(pin) = on_pin {
                    if !self.pinned.contains(&pin) {
                        self.pinned.push(pin);
                        self.pinned.sort();
                        self.pinned.dedup();
                        self.toasts.info("Pinned.");
                    }
                }
                if let Some((from, new_name)) = on_rename {
                    match fs_ops::rename(&from, &new_name) {
                        Ok(op) => {
                            self.ops_hist.push(op);
                            self.browser.invalidate();
                        }
                        Err(e) => self.toasts.error(format!("Rename failed: {e}")),
                    }
                }
                if let Some(p) = on_delete {
                    match fs_ops::delete_to_trash(&p) {
                        Ok(op) => {
                            self.ops_hist.push(op);
                            self.browser.invalidate();
                            self.toasts.info("Moved to trash.");
                        }
                        Err(e) => self.toasts.error(format!("Delete failed: {e}")),
                    }
                }
                if let Some(p) = on_open_with {
                    self.open_with_target = Some(p);
                    self.open_with_buffer.clear();
                }
                if let Some(p) = on_open_term {
                    platform::open_terminal_in(&p);
                }

                if let Some(p) = on_copy_req {
                    self.clipboard.set(vec![p], clipboard::Mode::Copy);
                    self.toasts.info("Copied to buffer");
                }
                if let Some(p) = on_cut_req {
                    self.clipboard.set(vec![p], clipboard::Mode::Cut);
                    self.toasts.info("Cut to buffer");
                }
                if let Some(target_dir) = on_paste_here {
                    self.paste_into(&target_dir);
                }
                if on_undo_req {
                    self.try_undo();
                }
                if let Some(target_dir) = on_new_folder_here {
                    self.create_dialog = Some((CreateKind::Folder, target_dir));
                    self.create_name_buffer = "New Folder".to_string();
                }
                if let Some(target_dir) = on_new_file_here {
                    self.create_dialog = Some((CreateKind::File, target_dir));
                    self.create_name_buffer = "New File.txt".to_string();
                }
            }

            egui::TopBottomPanel::bottom("toasts").show_inside(ui, |ui| {
                self.toasts.draw(ui);
            });
        });

        if let Some(tgt) = self.open_with_target.clone() {
            egui::Window::new("Open with...")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(format!("File: {}", tgt.display()));
                    ui.horizontal(|ui| {
                        if ui.button("System default").clicked() {
                            platform::open_file(&tgt);
                            self.open_with_target = None;
                        }
                        if ui.button("Cancel").clicked() {
                            self.open_with_target = None;
                        }
                    });
                    ui.separator();
                    ui.label("Or enter a program/command:");
                    ui.add(
                        TextEdit::singleline(&mut self.open_with_buffer)
                            .hint_text("eg. code, notepad, vim"),
                    );
                    if ui.button("Open").clicked() {
                        platform::open_with(&tgt, &self.open_with_buffer);
                        self.open_with_target = None;
                    }
                });
        }
        if let Some((kind, target_dir)) = self.create_dialog.clone() {
            let title = match kind {
                CreateKind::Folder => "Create folder",
                CreateKind::File => "Create file",
            };
            egui::Window::new(title)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(format!("Location: {}", target_dir.display()));
                    ui.horizontal(|ui| {
                        ui.label("Name:");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.create_name_buffer)
                                .desired_width(260.0),
                        );
                    });
                    ui.horizontal(|ui| {
                        let do_create = ui.button("Create").clicked()
                            || ui.input(|i| i.key_pressed(Key::Enter));
                        let cancel = ui.button("Cancel").clicked()
                            || ui.input(|i| i.key_pressed(Key::Escape));
                        if do_create {
                            let name = self.create_name_buffer.trim();
                            if name.is_empty() {
                                self.toasts.error("Name cannot be empty.");
                            } else {
                                let res = match kind {
                                    CreateKind::Folder => fs_ops::mkdir(&target_dir, name),
                                    CreateKind::File => fs_ops::touch(&target_dir, name),
                                };
                                match res {
                                    Ok(op) => {
                                        self.ops_hist.push(op);
                                        self.browser.invalidate();
                                        self.toasts.info("Created.");
                                        if let CreateKind::Folder = kind {
                                            let p = target_dir.join(name);
                                            if p.is_dir() {
                                                self.navigate_to(p);
                                            }
                                        }
                                        self.create_dialog = None;
                                    }
                                    Err(e) => self.toasts.error(format!("Create failed: {e}")),
                                }
                            }
                        }
                        if cancel {
                            self.create_dialog = None;
                        }
                    });
                });
        }
    }
}

fn main() {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "REX File Explorer",
        options,
        Box::new(|_cc| Ok(Box::new(AppData::default()))),
    )
    .unwrap();
}
